// backend/src/files.rs
//! File system access module for GeminiHydra v15.
//!
//! Detects file paths in user prompts, reads their contents, and builds
//! a context block that gets prepended to the Gemini API request.

use regex::Regex;
use std::path::Path;
use std::sync::OnceLock;
use tokio::fs::File;
use tokio::io::AsyncReadExt;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Max bytes per single file (100 KB).
const MAX_FILE_SIZE: u64 = 100 * 1024;

/// Max total context bytes across all files (500 KB).
const MAX_TOTAL_SIZE: usize = 500 * 1024;

/// Max number of files to include in context.
const MAX_FILES: usize = 10;

/// Path prefixes that are blocked for reading (sensitive system directories).
const BLOCKED_READ_PREFIXES: &[&str] = &[
    "/etc/shadow",
    "/etc/passwd",
    "/proc",
    "/sys",
    "C:\\Windows\\System32\\config",
];

/// Text file extensions we allow reading.
const TEXT_EXTENSIONS: &[&str] = &[
    // Code
    "rs", "ts", "tsx", "js", "jsx", "py", "go", "java", "kt", "c", "cpp",
    "h", "hpp", "cs", "rb", "php", "swift", "scala", "zig", "lua", "r",
    "sql", "sh", "bash", "zsh", "ps1", "bat", "cmd",
    // Config / Data
    "json", "yaml", "yml", "toml", "xml", "csv", "env", "ini", "cfg",
    "properties", "lock",
    // Web
    "html", "htm", "css", "scss", "sass", "less", "svg",
    // Docs
    "md", "txt", "rst", "log", "gitignore", "dockerignore", "editorconfig",
];

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Represents a successfully read file.
#[derive(Debug, Clone)]
pub struct FileContext {
    pub path: String,
    pub content: String,
    pub size_bytes: u64,
    pub truncated: bool,
    pub extension: String,
}

/// Errors that can occur when reading a file.
#[derive(Debug, Clone)]
pub struct FileError {
    pub path: String,
    pub reason: String,
}

// ---------------------------------------------------------------------------
// Path extraction
// ---------------------------------------------------------------------------

static QUOTED_RE: OnceLock<Regex> = OnceLock::new();
static WIN_FILE_RE: OnceLock<Regex> = OnceLock::new();
static WIN_DIR_RE: OnceLock<Regex> = OnceLock::new();
static UNIX_FILE_RE: OnceLock<Regex> = OnceLock::new();
static UNIX_DIR_RE: OnceLock<Regex> = OnceLock::new();

/// Extract file/directory paths from user prompt text.
///
/// Matches:
/// - Windows paths: `C:\Users\...\file.ext` and `C:\Users\...\directory`
/// - Unix paths: `/home/user/.../file.ext` and `/home/user/.../directory`
/// - Paths in quotes: `"C:\path\file.ext"` or `'/path/dir'`
/// - Paths in backticks: `` `C:\path\file.ext` ``
pub fn extract_file_paths(prompt: &str) -> Vec<String> {
    let mut paths = Vec::new();

    let patterns = [
        // Pattern 1: Quoted/backtick paths (highest priority — captures exact path)
        (
            QUOTED_RE.get_or_init(|| {
                Regex::new(r#"(?:["`'])((?:[A-Za-z]:\\|/)(?:[^\s"'`]*[^\s"'`.,;:!?]))["`']"#).unwrap()
            }),
            1,
        ),
        // Pattern 2: Windows file paths (unquoted) — C:\...\file.ext
        (
            WIN_FILE_RE.get_or_init(|| {
                Regex::new(r"(?:^|\s)([A-Za-z]:\\(?:[^\s\\]+\\)*[^\s\\]+\.[A-Za-z0-9]+)").unwrap()
            }),
            1,
        ),
        // Pattern 3: Windows directory paths (unquoted) — C:\dir1\dir2 (at least 2 segments, no extension)
        (
            WIN_DIR_RE.get_or_init(|| {
                Regex::new(r"(?:^|\s)([A-Za-z]:\\[^\s\\]+(?:\\[^\s\\]+)+)").unwrap()
            }),
            1,
        ),
        // Pattern 4: Unix file paths (unquoted) — /path/to/file.ext
        (
            UNIX_FILE_RE.get_or_init(|| {
                Regex::new(r"(?:^|\s)(/(?:[^\s/]+/)+[^\s/]+\.[A-Za-z0-9]+)").unwrap()
            }),
            1,
        ),
        // Pattern 5: Unix directory paths (unquoted) — /dir1/dir2 (at least 2 segments)
        (
            UNIX_DIR_RE.get_or_init(|| Regex::new(r"(?:^|\s)(/[^\s/]+(?:/[^\s/]+)+)").unwrap()),
            1,
        ),
    ];

    for (re, group_idx) in patterns {
        for cap in re.captures_iter(prompt) {
            if let Some(m) = cap.get(group_idx) {
                let p = m.as_str().to_string();
                if !paths.contains(&p) {
                    paths.push(p);
                }
            }
        }
    }

    paths
}

// ---------------------------------------------------------------------------
// File reading
// ---------------------------------------------------------------------------

fn is_text_extension(ext: &str) -> bool {
    TEXT_EXTENSIONS.contains(&ext.to_lowercase().as_str())
}

/// Canonicalize a path and check it against a blocklist.
///
/// For existing paths, `std::fs::canonicalize` resolves all `..`, symlinks, etc.
/// For new files (write path), the parent directory is canonicalized and the
/// filename is re-joined, preventing traversal via `..` segments.
///
/// This is modeled after ClaudeHydra's `ToolExecutor::validate_path()`.
fn validate_and_canonicalize(
    raw_path: &str,
    blocked_prefixes: &[&str],
) -> Result<std::path::PathBuf, FileError> {
    let p = Path::new(raw_path);

    // Canonicalize: resolve .., symlinks, etc.
    let canonical = if p.exists() {
        std::fs::canonicalize(p).map_err(|e| FileError {
            path: raw_path.to_string(),
            reason: format!("Cannot resolve path: {}", e),
        })?
    } else {
        // File doesn't exist yet (write case) — canonicalize parent + rejoin filename
        let parent = p.parent().ok_or_else(|| FileError {
            path: raw_path.to_string(),
            reason: "Invalid path: no parent directory".to_string(),
        })?;
        let canonical_parent = std::fs::canonicalize(parent).map_err(|e| FileError {
            path: raw_path.to_string(),
            reason: format!("Cannot resolve parent directory: {}", e),
        })?;
        let file_name = p.file_name().ok_or_else(|| FileError {
            path: raw_path.to_string(),
            reason: "Invalid path: no filename".to_string(),
        })?;
        canonical_parent.join(file_name)
    };

    // Check canonical path against blocked prefixes (case-insensitive)
    let canonical_str = canonical.to_string_lossy();
    // Normalize to backslash for Windows comparison + keep original for Unix
    let canonical_win = canonical_str.replace('/', "\\");
    let canonical_lower = canonical_str.to_lowercase();
    let canonical_win_lower = canonical_win.to_lowercase();

    for prefix in blocked_prefixes {
        let prefix_lower = prefix.to_lowercase();
        if canonical_lower.starts_with(&prefix_lower)
            || canonical_win_lower.starts_with(&prefix_lower)
        {
            return Err(FileError {
                path: raw_path.to_string(),
                reason: format!(
                    "Access denied: path '{}' resolves to blocked location '{}'",
                    raw_path,
                    canonical.display()
                ),
            });
        }
    }

    Ok(canonical)
}

/// Read a single file for context injection.
pub async fn read_file_for_context(path: &str) -> Result<FileContext, FileError> {
    // Canonicalize path BEFORE any checks to prevent traversal attacks
    let canonical = validate_and_canonicalize(path, BLOCKED_READ_PREFIXES)?;

    // Check extension whitelist on canonical path
    let ext = canonical
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    if !is_text_extension(&ext) {
        return Err(FileError {
            path: path.to_string(),
            reason: format!("Extension '.{}' is not in the text-file whitelist", ext),
        });
    }

    // Check file exists and size
    let metadata = tokio::fs::metadata(&canonical).await.map_err(|e| FileError {
        path: path.to_string(),
        reason: format!("Cannot access file: {}", e),
    })?;

    if !metadata.is_file() {
        return Err(FileError {
            path: path.to_string(),
            reason: "Path is not a file".to_string(),
        });
    }

    let file_size = metadata.len();
    let file = File::open(&canonical).await.map_err(|e| FileError {
        path: path.to_string(),
        reason: format!("Cannot open file: {}", e),
    })?;

    // Read up to MAX_FILE_SIZE + 1 to detect truncation
    let limit = MAX_FILE_SIZE as usize;
    let mut buffer = Vec::with_capacity(limit + 1);
    file.take((limit + 1) as u64)
        .read_to_end(&mut buffer)
        .await
        .map_err(|e| FileError {
            path: path.to_string(),
            reason: format!("Cannot read file: {}", e),
        })?;

    let truncated = buffer.len() > limit;
    
    // Convert to string (lossy to avoid UTF-8 errors on cut boundaries)
    let raw = String::from_utf8_lossy(&buffer).to_string();
    
    let content = if truncated {
        // Find safe char boundary for truncation
        let end = raw
            .char_indices()
            .take_while(|(i, _)| *i < limit)
            .last()
            .map(|(i, c)| i + c.len_utf8())
            .unwrap_or(limit.min(raw.len()));
            
        format!(
            "{}\n\n... [TRUNCATED — file is {} bytes, showing first {} bytes]",
            &raw[..end],
            file_size,
            end
        )
    } else {
        raw
    };

    Ok(FileContext {
        path: path.to_string(),
        content,
        size_bytes: file_size,
        truncated,
        extension: ext,
    })
}

/// Read a file and return its full content plus metadata (for the /api/files/read endpoint).
pub async fn read_file_raw(path: &str) -> Result<FileContext, FileError> {
    read_file_for_context(path).await
}

// ---------------------------------------------------------------------------
// Context builder
// ---------------------------------------------------------------------------

/// Key project files to auto-read when a directory path is detected.
const KEY_PROJECT_FILES: &[&str] = &[
    "package.json",
    "Cargo.toml",
    "README.md",
    "CLAUDE.md",
    "pyproject.toml",
    "go.mod",
    "tsconfig.json",
    "vite.config.ts",
    "docker-compose.yml",
    "Makefile",
];

/// Build a directory context: listing + auto-read key project files.
async fn build_directory_context(
    dir_path: &str,
    total_size: &mut usize,
) -> Result<(String, Vec<FileContext>), FileError> {
    let entries = list_directory(dir_path, false).await?;

    // Build directory listing
    let mut listing = format!("### Directory: {}\n", dir_path);
    listing.push_str(&format!("_{} entries:_\n```\n", entries.len()));
    for entry in &entries {
        let suffix = if entry.is_dir { "/" } else { "" };
        listing.push_str(&format!("  {}{}\n", entry.name, suffix));
    }
    listing.push_str("```\n\n");

    *total_size += listing.len();

    // Auto-read key project files found in this directory
    let mut key_files: Vec<FileContext> = Vec::new();
    for key_name in KEY_PROJECT_FILES {
        let full_path = format!("{}\\{}", dir_path.trim_end_matches('\\'), key_name);
        // Removed redundant p.is_file() check as read_file_for_context handles it
        if let Some(fc) = read_file_for_context(&full_path)
            .await
            .ok()
            .filter(|fc| *total_size + fc.content.len() <= MAX_TOTAL_SIZE)
        {
            *total_size += fc.content.len();
            key_files.push(fc);
        }
    }

    Ok((listing, key_files))
}

/// Build a combined context block from detected file/directory paths.
///
/// Returns `(context_string, errors)` where `context_string` is the formatted
/// block ready to prepend to the user prompt, and `errors` lists any paths
/// that could not be read.
pub async fn build_file_context(paths: &[String]) -> (String, Vec<FileError>) {
    let mut files: Vec<FileContext> = Vec::new();
    let mut dir_listings: Vec<String> = Vec::new();
    let mut errors: Vec<FileError> = Vec::new();
    let mut total_size: usize = 0;
    let mut item_count: usize = 0;

    for path in paths.iter() {
        if item_count >= MAX_FILES {
            errors.push(FileError {
                path: path.clone(),
                reason: format!("Skipped — max {} items per request", MAX_FILES),
            });
            continue;
        }

        let p = Path::new(path);

        if p.is_dir() {
            // Handle directory: listing + key files
            match build_directory_context(path, &mut total_size).await {
                Ok((listing, key_files)) => {
                    dir_listings.push(listing);
                    item_count += 1;
                    for fc in key_files {
                        files.push(fc);
                        item_count += 1;
                    }
                }
                Err(e) => errors.push(e),
            }
        } else {
            // Handle file
            match read_file_for_context(path).await {
                Ok(fc) => {
                    let content_len = fc.content.len();
                    if total_size + content_len > MAX_TOTAL_SIZE {
                        errors.push(FileError {
                            path: path.clone(),
                            reason: format!(
                                "Skipped — would exceed total context limit of {}KB",
                                MAX_TOTAL_SIZE / 1024
                            ),
                        });
                        continue;
                    }
                    total_size += content_len;
                    files.push(fc);
                    item_count += 1;
                }
                Err(e) => errors.push(e),
            }
        }
    }

    if files.is_empty() && dir_listings.is_empty() {
        return (String::new(), errors);
    }

    let total_items = dir_listings.len() + files.len();
    let mut ctx = String::from("--- FILE CONTEXT ---\n");
    ctx.push_str(&format!(
        "The following {} item(s) were automatically loaded from the user's local filesystem:\n\n",
        total_items
    ));

    // Append directory listings first
    for listing in &dir_listings {
        ctx.push_str(listing);
    }

    // Append file contents
    for fc in &files {
        let lang_hint = match fc.extension.as_str() {
            "rs" => "rust",
            "ts" | "tsx" => "typescript",
            "js" | "jsx" => "javascript",
            "py" => "python",
            "go" => "go",
            "java" => "java",
            "json" => "json",
            "yaml" | "yml" => "yaml",
            "toml" => "toml",
            "xml" => "xml",
            "html" | "htm" => "html",
            "css" | "scss" => "css",
            "sql" => "sql",
            "sh" | "bash" => "bash",
            "md" => "markdown",
            _ => "",
        };

        ctx.push_str(&format!("### {}\n", fc.path));
        if fc.truncated {
            ctx.push_str(&format!(
                "_Truncated: showing first ~{}KB of {}KB_\n",
                MAX_FILE_SIZE / 1024,
                fc.size_bytes / 1024
            ));
        }
        ctx.push_str(&format!("```{}\n{}\n```\n\n", lang_hint, fc.content));
    }

    ctx.push_str("--- END FILE CONTEXT ---\n\n");

    (ctx, errors)
}

// ---------------------------------------------------------------------------
// Directory listing (for /api/files/list)
// ---------------------------------------------------------------------------

/// A single entry in a directory listing.
#[derive(Debug, Clone, serde::Serialize)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size_bytes: u64,
    pub extension: Option<String>,
}

/// List contents of a directory, sorted: directories first, then files alphabetically.
pub async fn list_directory(path: &str, show_hidden: bool) -> Result<Vec<FileEntry>, FileError> {
    // Canonicalize path to prevent traversal attacks
    let dir = validate_and_canonicalize(path, BLOCKED_READ_PREFIXES)?;

    if !dir.is_dir() {
        return Err(FileError {
            path: path.to_string(),
            reason: "Path is not a directory".to_string(),
        });
    }

    let mut entries = Vec::new();
    let mut read_dir = tokio::fs::read_dir(&dir).await.map_err(|e| FileError {
        path: path.to_string(),
        reason: format!("Cannot read directory: {}", e),
    })?;

    while let Some(entry) = read_dir.next_entry().await.map_err(|e| FileError {
        path: path.to_string(),
        reason: format!("Error reading entry: {}", e),
    })? {
        let name = entry.file_name().to_string_lossy().to_string();

        // Skip hidden files unless requested
        if !show_hidden && name.starts_with('.') {
            continue;
        }

        let metadata = entry.metadata().await.map_err(|e| FileError {
            path: path.to_string(),
            reason: format!("Cannot read metadata for {}: {}", name, e),
        })?;

        let ext = Path::new(&name)
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_lowercase());

        entries.push(FileEntry {
            name,
            path: entry.path().to_string_lossy().to_string(),
            is_dir: metadata.is_dir(),
            size_bytes: metadata.len(),
            extension: ext,
        });
    }

    // Sort: directories first, then alphabetically by name
    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    Ok(entries)
}

// ---------------------------------------------------------------------------
// File writing (for tool calling)
// ---------------------------------------------------------------------------

/// Max content size for write_file (1 MB).
const MAX_WRITE_SIZE: usize = 1024 * 1024;

/// Path prefixes that are blocked for writing.
const BLOCKED_WRITE_PREFIXES: &[&str] = &[
    "/etc",
    "/sys",
    "/proc",
    "/boot",
    "C:\\Windows",
    "C:\\Program Files",
    "C:\\Program Files (x86)",
];

/// Write content to a file with safety checks.
pub async fn write_file(path: &str, content: &str) -> Result<String, FileError> {
    if content.len() > MAX_WRITE_SIZE {
        return Err(FileError {
            path: path.to_string(),
            reason: format!(
                "Content too large: {} bytes (max {} bytes)",
                content.len(),
                MAX_WRITE_SIZE
            ),
        });
    }

    // Ensure parent directory exists BEFORE canonicalization (so parent can be resolved)
    if let Some(parent) = Path::new(path).parent().filter(|p| !p.as_os_str().is_empty() && !p.exists()) {
        tokio::fs::create_dir_all(parent).await.map_err(|e| FileError {
            path: path.to_string(),
            reason: format!("Cannot create parent directory: {}", e),
        })?;
    }

    // Canonicalize BEFORE blocklist check — prevents ../ traversal bypass
    // For new files: canonicalize parent + rejoin filename
    let canonical = validate_and_canonicalize(path, BLOCKED_WRITE_PREFIXES)?;

    tokio::fs::write(&canonical, content).await.map_err(|e| FileError {
        path: path.to_string(),
        reason: format!("Cannot write file: {}", e),
    })?;

    Ok(format!(
        "Successfully wrote {} bytes to {}",
        content.len(),
        canonical.display()
    ))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_windows_path() {
        let prompt = r"Odczytaj plik C:\Users\BIURODOM\Desktop\GeminiHydra-v15\package.json";
        let paths = extract_file_paths(prompt);
        assert!(paths.contains(&r"C:\Users\BIURODOM\Desktop\GeminiHydra-v15\package.json".to_string()));
    }

    #[test]
    fn test_extract_windows_directory() {
        let prompt = r"C:\Users\BIURODOM\Desktop\GeminiHydra-v15";
        let paths = extract_file_paths(prompt);
        assert!(paths.contains(&r"C:\Users\BIURODOM\Desktop\GeminiHydra-v15".to_string()));
    }

    #[test]
    fn test_extract_quoted_path() {
        let prompt = r#"Pokaż mi zawartość "C:\Users\test\file.rs" proszę"#;
        let paths = extract_file_paths(prompt);
        assert!(paths.contains(&r"C:\Users\test\file.rs".to_string()));
    }

    #[test]
    fn test_extract_unix_path() {
        let prompt = "Read /home/user/project/src/main.rs please";
        let paths = extract_file_paths(prompt);
        assert!(paths.contains(&"/home/user/project/src/main.rs".to_string()));
    }

    #[test]
    fn test_extract_unix_directory() {
        let prompt = "Show me /home/user/project contents";
        let paths = extract_file_paths(prompt);
        assert!(paths.contains(&"/home/user/project".to_string()));
    }

    #[test]
    fn test_extract_multiple_paths() {
        let prompt = r"Compare C:\a\b.rs and C:\c\d.ts";
        let paths = extract_file_paths(prompt);
        assert_eq!(paths.len(), 2);
    }

    #[test]
    fn test_no_false_positives() {
        let prompt = "Tell me about the API and how it works";
        let paths = extract_file_paths(prompt);
        assert!(paths.is_empty());
    }

    #[test]
    fn test_text_extension_check() {
        assert!(is_text_extension("rs"));
        assert!(is_text_extension("json"));
        assert!(is_text_extension("toml"));
        assert!(!is_text_extension("exe"));
        assert!(!is_text_extension("dll"));
        assert!(!is_text_extension("png"));
    }
}
