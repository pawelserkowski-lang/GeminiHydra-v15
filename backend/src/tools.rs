// backend/src/tools.rs
//! Tool execution module for Gemini function calling.
//!
//! Provides 12 local tools that Gemini agents can invoke:
//! - `execute_command` — run shell commands with timeout + safety filters
//! - `read_file` — read file contents (reuses files::read_file_for_context)
//! - `read_file_section` — read specific line range from a file (1-indexed)
//! - `write_file` — create/overwrite files with size + path restrictions
//! - `edit_file` — targeted text replacement in existing files (safer than write_file)
//! - `list_directory` — list directory contents with line counts
//! - `search_files` — search for text/regex patterns across files (pagination + multiline)
//! - `get_code_structure` — analyze code AST without full read
//! - `find_file` — find files by glob pattern (recursive)
//! - `diff_files` — line-by-line diff between two files
//! - `read_pdf` — extract text from PDF with OCR fallback via Gemini Vision
//! - `analyze_image` — Gemini Vision API image analysis + OCR text extraction

use base64::Engine;
use regex::Regex;
use scraper::{Html, Selector};
use serde_json::Value;
use std::collections::{HashSet, VecDeque};
use std::time::Duration;
use tokio::process::Command;
use url::Url;
use crate::state::AppState;

/// Max output bytes from a single command (50 KB).
const MAX_COMMAND_OUTPUT: usize = 50 * 1024;

/// Command execution timeout.
const COMMAND_TIMEOUT: Duration = Duration::from_secs(30);

// ---------------------------------------------------------------------------
// Web Scraping Constants
// ---------------------------------------------------------------------------
const MAX_PAGE_SIZE: usize = 5 * 1024 * 1024; // 5 MB
const FETCH_TIMEOUT: Duration = Duration::from_secs(30);
const CRAWL_DELAY: Duration = Duration::from_millis(500);
const MAX_CRAWL_DEPTH: u32 = 3;
const MAX_CRAWL_PAGES: usize = 20;
const WEB_USER_AGENT: &str = "Jaskier-Bot/1.0 (AI Agent Tool)";

// ---------------------------------------------------------------------------
// Gemini 3 — Multimodal Tool Output
// ---------------------------------------------------------------------------

/// Output from a tool execution, supporting text and optional binary data (Gemini 3 multimodal function responses).
#[derive(Debug, Clone)]
pub struct ToolOutput {
    /// Primary text result
    pub text: String,
    /// Optional binary data (e.g., image) with MIME type for Gemini multimodal function responses
    pub inline_data: Option<InlineData>,
}

/// Binary data attachment for multimodal function responses.
#[derive(Debug, Clone)]
pub struct InlineData {
    pub mime_type: String,
    pub data: String, // base64-encoded
}

impl ToolOutput {
    /// Create a text-only output (most common case)
    pub fn text(s: String) -> Self {
        Self { text: s, inline_data: None }
    }
}

// ---------------------------------------------------------------------------

/// Dangerous command patterns that are always blocked (even in sandbox, for now, or maybe relax in sandbox?)
/// For now, keep them blocked to prevent resource exhaustion (fork bombs) even in container.
const BLOCKED_PATTERNS: &[&str] = &[
    // Original patterns
    "rm -rf /",       // Still dangerous if volume mounted
    "format c:",
    ":(){:|:&};:",    // Fork bomb
    "dd if=/dev/zero",
    // Curl/wget piping to shell — remote code execution
    "curl|sh",
    "curl|bash",
    "curl |sh",
    "curl |bash",
    "wget|sh",
    "wget|bash",
    "wget |sh",
    "wget |bash",
    "curl|zsh",
    "curl |zsh",
    "wget|zsh",
    "wget |zsh",
    // Docker volume escape — mount host root into container
    "docker run -v /:/",
    "docker run --volume /:/",
    "docker run -v c:\\:/",
    "docker run -v c:/:/",
    // Sudo — privilege escalation
    "sudo ",
    // Destructive disk commands
    "rm -rf /",
    "fdisk ",
    "mkfs.",
    // Windows registry editing
    "reg add ",
    "reg delete ",
    "regedit ",
    // User account manipulation
    "net user ",
    "net localgroup ",
];

/// Resolve a path against the working directory.
/// If the path is absolute or working_directory is empty, return it as-is.
/// If relative and working_directory is non-empty, join them.
fn resolve_path(raw: &str, working_directory: &str) -> String {
    let p = std::path::Path::new(raw);
    if p.is_absolute() || working_directory.is_empty() {
        raw.to_string()
    } else {
        std::path::Path::new(working_directory)
            .join(raw)
            .to_string_lossy()
            .to_string()
    }
}

/// Central dispatcher — routes tool call to the appropriate handler.
/// Returns `ToolOutput` supporting text + optional multimodal data (Gemini 3).
pub async fn execute_tool(name: &str, args: &Value, state: &AppState, working_directory: &str) -> Result<ToolOutput, String> {
    match name {
        "execute_command" => {
            let command = args["command"]
                .as_str()
                .ok_or("Missing required argument: command")?;
            // Per-call working_directory takes priority, fallback to global setting
            let per_call_wd = args["working_directory"].as_str();
            let effective_wd = per_call_wd.or_else(|| {
                if working_directory.is_empty() { None } else { Some(working_directory) }
            });
            tool_execute_command(command, effective_wd, state).await.map(ToolOutput::text)
        }
        "read_file" => {
            let path = args["path"]
                .as_str()
                .ok_or("Missing required argument: path")?;
            let resolved = resolve_path(path, working_directory);
            tool_read_file(&resolved).await.map(ToolOutput::text)
        }
        "write_file" => {
            let path = args["path"]
                .as_str()
                .ok_or("Missing required argument: path")?;
            let resolved = resolve_path(path, working_directory);
            let content = args["content"]
                .as_str()
                .ok_or("Missing required argument: content")?;
            tool_write_file(&resolved, content).await.map(ToolOutput::text)
        }
        "edit_file" => {
            let path = args["path"]
                .as_str()
                .ok_or("Missing required argument: path")?;
            let resolved = resolve_path(path, working_directory);
            let old_text = args["old_text"]
                .as_str()
                .ok_or("Missing required argument: old_text")?;
            let new_text = args["new_text"]
                .as_str()
                .ok_or("Missing required argument: new_text")?;
            tool_edit_file(&resolved, old_text, new_text).await.map(ToolOutput::text)
        }
        "list_directory" => {
            let path = args["path"]
                .as_str()
                .ok_or("Missing required argument: path")?;
            let resolved = resolve_path(path, working_directory);
            let show_hidden = args["show_hidden"].as_bool().unwrap_or(false);
            tool_list_directory(&resolved, show_hidden).await.map(ToolOutput::text)
        }
        "search_files" => {
            let path = args["path"]
                .as_str()
                .ok_or("Missing required argument: path")?;
            let resolved = resolve_path(path, working_directory);
            let pattern = args["pattern"]
                .as_str()
                .ok_or("Missing required argument: pattern")?;
            let extensions = args["file_extensions"].as_str();
            let offset = args["offset"].as_u64().unwrap_or(0) as usize;
            let limit = args["limit"].as_u64().unwrap_or(80) as usize;
            let multiline = args["multiline"].as_bool().unwrap_or(false);
            tool_search_files(&resolved, pattern, extensions, offset, limit, multiline).await.map(ToolOutput::text)
        }
        "get_code_structure" => {
            let path = args["path"]
                .as_str()
                .ok_or("Missing required argument: path")?;
            let resolved = resolve_path(path, working_directory);
            tool_get_code_structure(&resolved).await.map(ToolOutput::text)
        }
        "read_file_section" => {
            let path = args["path"]
                .as_str()
                .ok_or("Missing required argument: path")?;
            let resolved = resolve_path(path, working_directory);
            let start_line = args["start_line"]
                .as_u64()
                .ok_or("Missing required argument: start_line")? as usize;
            let end_line = args["end_line"]
                .as_u64()
                .ok_or("Missing required argument: end_line")? as usize;
            tool_read_file_section(&resolved, start_line, end_line).await.map(ToolOutput::text)
        }
        "find_file" => {
            let path = args["path"]
                .as_str()
                .ok_or("Missing required argument: path")?;
            let resolved = resolve_path(path, working_directory);
            let pattern = args["pattern"]
                .as_str()
                .ok_or("Missing required argument: pattern")?;
            tool_find_file(&resolved, pattern).await.map(ToolOutput::text)
        }
        "diff_files" => {
            let path_a = args["path_a"]
                .as_str()
                .ok_or("Missing required argument: path_a")?;
            let resolved_a = resolve_path(path_a, working_directory);
            let path_b = args["path_b"]
                .as_str()
                .ok_or("Missing required argument: path_b")?;
            let resolved_b = resolve_path(path_b, working_directory);
            tool_diff_files(&resolved_a, &resolved_b).await.map(ToolOutput::text)
        }
        "read_pdf" => {
            let path = args["path"]
                .as_str()
                .ok_or("Missing required argument: path")?;
            let resolved = resolve_path(path, working_directory);
            let page_range = args["page_range"].as_str();
            tool_read_pdf(&resolved, page_range, state).await.map(ToolOutput::text)
        }
        "analyze_image" => {
            let path = args["path"]
                .as_str()
                .ok_or("Missing required argument: path")?;
            let resolved = resolve_path(path, working_directory);
            let prompt = args["prompt"].as_str();
            let extract_text = args["extract_text"].as_bool();
            tool_analyze_image(&resolved, prompt, extract_text, state).await
        }
        "ocr_document" => {
            let path = args["path"]
                .as_str()
                .ok_or("Missing required argument: path")?;
            let resolved = resolve_path(path, working_directory);
            let prompt = args["prompt"].as_str();
            tool_ocr_document(&resolved, prompt, state).await.map(ToolOutput::text)
        }
        "fetch_webpage" => {
            let url = args["url"]
                .as_str()
                .ok_or("Missing required argument: url")?;
            let extract_links = args["extract_links"].as_bool().unwrap_or(true);
            tool_fetch_webpage(url, extract_links, &state.client).await
        }
        "crawl_website" => {
            let url = args["url"]
                .as_str()
                .ok_or("Missing required argument: url")?;
            let max_depth = args["max_depth"].as_u64().unwrap_or(1) as u32;
            let max_pages = args["max_pages"].as_u64().unwrap_or(10) as usize;
            let same_domain = args["same_domain_only"].as_bool().unwrap_or(true);
            tool_crawl_website(
                url,
                max_depth.min(MAX_CRAWL_DEPTH),
                max_pages.min(MAX_CRAWL_PAGES),
                same_domain,
                &state.client,
            ).await
        }
        _ => Err(format!("Unknown tool: {}", name)),
    }
}

// ---------------------------------------------------------------------------
// execute_command
// ---------------------------------------------------------------------------

async fn tool_execute_command(command: &str, working_directory: Option<&str>, state: &AppState) -> Result<String, String> {
    let lower = command.to_lowercase();
    for pattern in BLOCKED_PATTERNS {
        if lower.contains(pattern) {
            return Err(format!("Blocked dangerous command pattern: {}", pattern));
        }
    }

    // Validate and resolve working directory
    let cwd = if let Some(dir) = working_directory {
        let p = std::path::Path::new(dir);
        if !p.is_dir() {
            return Err(format!("working_directory '{}' does not exist or is not a directory", dir));
        }
        Some(p.to_path_buf())
    } else {
        None
    };

    // Check sandbox setting
    let use_sandbox = sqlx::query_scalar::<_, bool>("SELECT use_docker_sandbox FROM gh_settings WHERE id = 1")
        .fetch_one(&state.db)
        .await
        .unwrap_or(false);

    let output_res = if use_sandbox {
        // Run in Docker — mount working dir (or cwd) to /app
        let mount_dir = cwd.as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .or_else(|| std::env::current_dir().ok().map(|p| p.to_string_lossy().to_string()))
            .ok_or_else(|| "Cannot determine working directory".to_string())?;

        let docker_args = [
            "run",
            "--rm",
            "-v", &format!("{}:/app", mount_dir),
            "-w", "/app",
            "alpine:latest",
            "sh", "-c", command
        ];

        tokio::time::timeout(COMMAND_TIMEOUT, Command::new("docker").args(docker_args).output()).await
    } else {
        // Run locally with optional working directory
        tokio::time::timeout(COMMAND_TIMEOUT, run_command(command, cwd.as_deref())).await
    };

    let output = output_res
        .map_err(|_| format!("Command timed out after {}s", COMMAND_TIMEOUT.as_secs()))?
        .map_err(|e| format!("Failed to execute command: {}", e))?;

    let mut result = String::new();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !stdout.is_empty() {
        result.push_str(&stdout);
    }
    if !stderr.is_empty() {
        if !result.is_empty() {
            result.push_str("\n--- stderr ---\n");
        }
        result.push_str(&stderr);
    }

    if result.is_empty() {
        result = format!("Command completed with exit code: {}", output.status.code().unwrap_or(-1));
    }

    // Truncate if too large
    if result.len() > MAX_COMMAND_OUTPUT {
        result.truncate(MAX_COMMAND_OUTPUT);
        result.push_str("\n... [output truncated at 50KB]");
    }

    if output.status.success() {
        Ok(result)
    } else {
        let code = output.status.code().unwrap_or(-1);
        Ok(format!("[exit code: {}]\n{}", code, result))
    }
}

async fn run_command(command: &str, cwd: Option<&std::path::Path>) -> std::io::Result<std::process::Output> {
    let mut cmd = if cfg!(target_os = "windows") {
        let mut c = Command::new("cmd");
        c.args(["/C", command]);
        c
    } else {
        let mut c = Command::new("sh");
        c.args(["-c", command]);
        c
    };
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    cmd.output().await
}

// ---------------------------------------------------------------------------
// read_file
// ---------------------------------------------------------------------------

async fn tool_read_file(path: &str) -> Result<String, String> {
    let ctx = crate::files::read_file_for_context(path)
        .await
        .map_err(|e| format!("Cannot read file '{}': {}", e.path, e.reason))?;

    let mut result = ctx.content;
    if ctx.truncated {
        result.push_str("\n... [file truncated]");
    }
    Ok(result)
}

// ---------------------------------------------------------------------------
// write_file
// ---------------------------------------------------------------------------

async fn tool_write_file(path: &str, content: &str) -> Result<String, String> {
    crate::files::write_file(path, content)
        .await
        .map_err(|e| format!("Cannot write file '{}': {}", e.path, e.reason))
}

// ---------------------------------------------------------------------------
// edit_file
// ---------------------------------------------------------------------------

/// Edit a file by replacing a specific text section.
/// Safer than write_file for modifications — only changes the targeted section.
async fn tool_edit_file(path: &str, old_text: &str, new_text: &str) -> Result<String, String> {
    let p = std::path::Path::new(path);

    if let Err(e) = crate::files::validate_write_path(path) {
        return Err(format!("Path rejected: {}", e.reason));
    }

    if !p.exists() {
        return Err(format!("File not found: {}", path));
    }

    let content = tokio::fs::read_to_string(p).await
        .map_err(|e| format!("Failed to read file: {}", e))?;

    let count = content.matches(old_text).count();
    if count == 0 {
        // Fuzzy fallback: try matching with normalized whitespace
        // This helps when Gemini generates slightly different indentation
        let normalize = |s: &str| -> String {
            s.lines()
                .map(|line| {
                    let trimmed = line.trim_start();
                    let indent = line.len() - trimmed.len();
                    // Normalize: collapse runs of spaces/tabs to single spaces, but keep indent level
                    format!("{}{}", " ".repeat(indent), trimmed)
                })
                .collect::<Vec<_>>()
                .join("\n")
        };
        let norm_content = normalize(&content);
        let norm_old = normalize(old_text);
        let fuzzy_count = norm_content.matches(&norm_old).count();
        if fuzzy_count == 1 {
            // Find the original text by locating it in normalized form, then map back
            if let Some(norm_pos) = norm_content.find(&norm_old) {
                // Count newlines before the match to find the line range
                let start_line = norm_content[..norm_pos].matches('\n').count();
                let old_lines: Vec<&str> = old_text.lines().collect();
                let content_lines: Vec<&str> = content.lines().collect();
                let end_line = start_line + old_lines.len();
                if end_line <= content_lines.len() {
                    let original_section = content_lines[start_line..end_line].join("\n");
                    let new_content = content.replacen(&original_section, new_text, 1);
                    tokio::fs::write(p, &new_content).await
                        .map_err(|e| format!("Failed to write file: {}", e))?;
                    return Ok(format!(
                        "Successfully edited {} (fuzzy match on lines {}-{}, {} bytes -> {} bytes)",
                        path, start_line + 1, end_line, content.len(), new_content.len()
                    ));
                }
            }
        }
        return Err(format!(
            "old_text not found in {}. Copy text VERBATIM from read_file output — every space, tab, newline must match exactly. Use read_file_section to get exact text first.",
            path
        ));
    }
    if count > 1 {
        return Err(format!(
            "old_text found {} times in {}. Provide more context to make the match unique.",
            count, path
        ));
    }

    let new_content = content.replacen(old_text, new_text, 1);
    tokio::fs::write(p, &new_content).await
        .map_err(|e| format!("Failed to write file: {}", e))?;

    Ok(format!(
        "Successfully edited {} ({} bytes -> {} bytes)",
        path, content.len(), new_content.len()
    ))
}

// ---------------------------------------------------------------------------
// list_directory
// ---------------------------------------------------------------------------

async fn tool_list_directory(path: &str, show_hidden: bool) -> Result<String, String> {
    let entries = crate::files::list_directory(path, show_hidden)
        .await
        .map_err(|e| format!("Cannot list '{}': {}", e.path, e.reason))?;

    if entries.is_empty() {
        return Ok("(empty directory)".to_string());
    }

    let mut lines = Vec::with_capacity(entries.len());
    for entry in &entries {
        if entry.is_dir {
            lines.push(format!("  [DIR]  {}/", entry.name));
        } else {
            let size = format_size(entry.size_bytes);
            // Count lines for text files under 1MB (#16)
            let line_count = if entry.size_bytes < 1024 * 1024
                && crate::files::is_text_file(std::path::Path::new(&entry.path))
            {
                count_lines(&entry.path).await
            } else {
                None
            };
            if let Some(count) = line_count {
                lines.push(format!("  {:>8} ({:>4} lines)  {}", size, count, entry.name));
            } else {
                lines.push(format!("  {:>8}  {}", size, entry.name));
            }
        }
    }

    Ok(format!("Directory: {}\n{}", path, lines.join("\n")))
}

/// Count lines in a file. Returns None on any error.
async fn count_lines(path: &str) -> Option<usize> {
    use tokio::io::{AsyncBufReadExt, BufReader};
    let file = tokio::fs::File::open(path).await.ok()?;
    let reader = BufReader::new(file);
    let mut lines = reader.lines();
    let mut count = 0usize;
    while lines.next_line().await.ok()?.is_some() {
        count += 1;
    }
    Some(count)
}

fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

// ---------------------------------------------------------------------------
// search_files
// ---------------------------------------------------------------------------

/// Max search results to return.
const MAX_SEARCH_RESULTS: usize = 150;

/// Max directory depth for recursive search.
const MAX_SEARCH_DEPTH: usize = 12;

/// Directories to skip during search.
const SKIP_DIRS: &[&str] = &[
    "node_modules", "target", "dist", "build", ".git", "__pycache__",
    ".next", ".nuxt", "vendor", ".cache", "coverage", ".turbo",
];

async fn tool_search_files(
    path: &str,
    pattern: &str,
    extensions: Option<&str>,
    offset: usize,
    limit: usize,
    multiline: bool,
) -> Result<String, String> {
    let dir = std::path::Path::new(path);
    if !dir.is_dir() {
        return Err(format!("'{}' is not a directory", path));
    }

    // Build regex: try as regex first, fall back to literal escape
    let flags = if multiline { "(?is)" } else { "(?i)" };
    let re = Regex::new(&format!("{}{}", flags, pattern))
        .or_else(|_| Regex::new(&format!("{}{}", flags, regex::escape(pattern))))
        .map_err(|e| format!("Invalid pattern '{}': {}", pattern, e))?;

    // Parse extension filter
    let ext_filter: Option<Vec<String>> = extensions.map(|e| {
        e.split(',')
            .map(|s| s.trim().trim_start_matches('.').to_lowercase())
            .filter(|s| !s.is_empty())
            .collect()
    });

    let mut all_results = Vec::new();
    let mut files_searched: usize = 0;
    let mut stack: Vec<(std::path::PathBuf, usize)> = vec![(dir.to_path_buf(), 0)];

    while let Some((current_dir, depth)) = stack.pop() {
        if depth > MAX_SEARCH_DEPTH || all_results.len() >= MAX_SEARCH_RESULTS {
            break;
        }

        let mut entries = match tokio::fs::read_dir(&current_dir).await {
            Ok(e) => e,
            Err(_) => continue,
        };

        while let Ok(Some(entry)) = entries.next_entry().await {
            if all_results.len() >= MAX_SEARCH_RESULTS {
                break;
            }

            let entry_path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();

            // Skip hidden and ignored directories
            if name.starts_with('.') && entry_path.is_dir() {
                continue;
            }
            if SKIP_DIRS.contains(&name.as_str()) {
                continue;
            }

            if entry_path.is_dir() {
                stack.push((entry_path, depth + 1));
            } else if entry_path.is_file() {
                let ext = entry_path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_lowercase();

                // Filter by extension if specified
                if let Some(ref filter) = ext_filter {
                    if !filter.contains(&ext) {
                        continue;
                    }
                }

                // Only search text files
                if !crate::files::is_text_file(&entry_path) {
                    continue;
                }

                // Read and search
                if let Ok(content) = tokio::fs::read_to_string(&entry_path).await {
                    files_searched += 1;

                    if multiline {
                        // Multiline mode: search entire file content, return match with ±2 lines context
                        let lines: Vec<&str> = content.lines().collect();
                        for mat in re.find_iter(&content) {
                            if all_results.len() >= MAX_SEARCH_RESULTS {
                                break;
                            }
                            // Find the line number of the match start
                            let match_line = content[..mat.start()].lines().count();
                            let ctx_start = match_line.saturating_sub(2);
                            let ctx_end = (match_line + 3).min(lines.len());
                            let mut snippet = String::new();
                            for i in ctx_start..ctx_end {
                                snippet.push_str(&format!("  {:>4} | {}\n", i + 1, lines[i]));
                            }
                            all_results.push(format!(
                                "{}:{}-{}:\n{}",
                                entry_path.display(),
                                ctx_start + 1,
                                ctx_end,
                                snippet.trim_end()
                            ));
                        }
                    } else {
                        // Line-by-line mode (default, faster)
                        for (line_num, line) in content.lines().enumerate() {
                            if all_results.len() >= MAX_SEARCH_RESULTS {
                                break;
                            }
                            if re.is_match(line) {
                                let trimmed = line.trim();
                                // Truncate very long lines (#15: 500 chars)
                                let display = if trimmed.len() > 500 {
                                    let end = trimmed
                                        .char_indices()
                                        .take_while(|(i, _)| *i < 500)
                                        .last()
                                        .map(|(i, c)| i + c.len_utf8())
                                        .unwrap_or(500.min(trimmed.len()));
                                    format!("{}...", &trimmed[..end])
                                } else {
                                    trimmed.to_string()
                                };
                                all_results.push(format!(
                                    "{}:{}:  {}",
                                    entry_path.display(),
                                    line_num + 1,
                                    display
                                ));
                            }
                        }
                    }
                }
            }
        }
    }

    let total = all_results.len();

    if total == 0 {
        Ok(format!(
            "No matches found for '{}' in '{}' ({} files searched)",
            pattern, path, files_searched
        ))
    } else {
        // Apply pagination
        let page: Vec<&String> = all_results.iter().skip(offset).take(limit).collect();
        let shown_start = offset + 1;
        let shown_end = (offset + page.len()).min(total);
        let page_str: String = page.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("\n");

        let truncated = if total >= MAX_SEARCH_RESULTS {
            format!(" (capped at {} total results)", MAX_SEARCH_RESULTS)
        } else {
            String::new()
        };

        Ok(format!(
            "Showing matches {}-{} of {} total ({} files searched){}:\n\n{}",
            shown_start,
            shown_end,
            total,
            files_searched,
            truncated,
            page_str
        ))
    }
}

// ---------------------------------------------------------------------------
// get_code_structure
// ---------------------------------------------------------------------------

async fn tool_get_code_structure(path: &str) -> Result<String, String> {
    // Read file (reuse context reader for safety/limits)
    let ctx = crate::files::read_file_for_context(path)
        .await
        .map_err(|e| format!("Cannot read file '{}': {}", e.path, e.reason))?;

    // Analyze (tree-sitter first, regex fallback)
    if let Some(structure) = crate::analysis::analyze_file(&ctx.path, &ctx.content) {
        let mut out = format!("### Code Structure: {}\n", ctx.path);
        if structure.symbols.is_empty() {
            out.push_str("(No symbols detected or language not supported)");
        } else {
            for sym in &structure.symbols {
                out.push_str(&format!("- [L{}] {} `{}`\n", sym.line, sym.kind, sym.name));
                out.push_str(&format!("  `{}`\n", sym.signature));
            }
            out.push_str(&format!("\n{} symbols total", structure.symbols.len()));
        }
        Ok(out)
    } else {
        Ok(format!("Could not analyze structure for '{}' (unsupported language?)", path))
    }
}

// ---------------------------------------------------------------------------
// read_file_section (#13)
// ---------------------------------------------------------------------------

/// Read a specific line range from a file (1-indexed, inclusive).
async fn tool_read_file_section(path: &str, start_line: usize, end_line: usize) -> Result<String, String> {
    if start_line == 0 {
        return Err("start_line must be >= 1 (1-indexed)".to_string());
    }
    if end_line < start_line {
        return Err(format!("end_line ({}) must be >= start_line ({})", end_line, start_line));
    }
    if end_line - start_line + 1 > 500 {
        return Err(format!(
            "Requested {} lines (max 500). Narrow the range.",
            end_line - start_line + 1
        ));
    }

    let p = std::path::Path::new(path);
    if !p.is_file() {
        return Err(format!("File not found: {}", path));
    }

    let content = tokio::fs::read_to_string(p)
        .await
        .map_err(|e| format!("Cannot read '{}': {}", path, e))?;

    let lines: Vec<&str> = content.lines().collect();
    let total_lines = lines.len();

    if start_line > total_lines {
        return Err(format!(
            "start_line {} exceeds file length ({} lines)",
            start_line, total_lines
        ));
    }

    let actual_end = end_line.min(total_lines);
    let mut out = String::new();
    for i in (start_line - 1)..actual_end {
        out.push_str(&format!("{:>5} | {}\n", i + 1, lines[i]));
    }

    Ok(format!(
        "### {} (lines {}-{} of {})\n{}",
        path, start_line, actual_end, total_lines, out
    ))
}

// ---------------------------------------------------------------------------
// find_file (#18)
// ---------------------------------------------------------------------------

/// Max results for find_file.
const MAX_FIND_RESULTS: usize = 50;

/// Find files by glob pattern (simple wildcard matching).
async fn tool_find_file(path: &str, pattern: &str) -> Result<String, String> {
    let dir = std::path::Path::new(path);
    if !dir.is_dir() {
        return Err(format!("'{}' is not a directory", path));
    }

    // Convert glob pattern to regex: * -> .*, ? -> ., escape the rest
    let mut regex_str = String::from("(?i)^");
    for ch in pattern.chars() {
        match ch {
            '*' => regex_str.push_str(".*"),
            '?' => regex_str.push('.'),
            '.' | '+' | '(' | ')' | '[' | ']' | '{' | '}' | '^' | '$' | '|' | '\\' => {
                regex_str.push('\\');
                regex_str.push(ch);
            }
            _ => regex_str.push(ch),
        }
    }
    regex_str.push('$');

    let re = Regex::new(&regex_str)
        .map_err(|e| format!("Invalid glob pattern '{}': {}", pattern, e))?;

    let mut results: Vec<(String, u64)> = Vec::new();
    let mut stack: Vec<(std::path::PathBuf, usize)> = vec![(dir.to_path_buf(), 0)];

    while let Some((current_dir, depth)) = stack.pop() {
        if depth > MAX_SEARCH_DEPTH || results.len() >= MAX_FIND_RESULTS {
            break;
        }

        let mut entries = match tokio::fs::read_dir(&current_dir).await {
            Ok(e) => e,
            Err(_) => continue,
        };

        while let Ok(Some(entry)) = entries.next_entry().await {
            if results.len() >= MAX_FIND_RESULTS {
                break;
            }

            let entry_path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();

            // Skip hidden and ignored directories
            if name.starts_with('.') && entry_path.is_dir() {
                continue;
            }
            if SKIP_DIRS.contains(&name.as_str()) {
                continue;
            }

            if entry_path.is_dir() {
                stack.push((entry_path, depth + 1));
            } else if entry_path.is_file() && re.is_match(&name) {
                let size = entry.metadata().await.map(|m| m.len()).unwrap_or(0);
                results.push((entry_path.to_string_lossy().to_string(), size));
            }
        }
    }

    if results.is_empty() {
        Ok(format!(
            "No files matching '{}' found in '{}'",
            pattern, path
        ))
    } else {
        let mut lines = Vec::with_capacity(results.len());
        for (file_path, size) in &results {
            lines.push(format!("  {} ({})", file_path, format_size(*size)));
        }
        Ok(format!(
            "Found {} file(s) matching '{}' in {}:\n{}",
            results.len(),
            pattern,
            path,
            lines.join("\n")
        ))
    }
}

// ---------------------------------------------------------------------------
// diff_files (#20)
// ---------------------------------------------------------------------------

/// Max diff output lines.
const MAX_DIFF_LINES: usize = 200;

/// Simple line-by-line diff between two files (unified-style output).
async fn tool_diff_files(path_a: &str, path_b: &str) -> Result<String, String> {
    let p_a = std::path::Path::new(path_a);
    let p_b = std::path::Path::new(path_b);

    if !p_a.is_file() {
        return Err(format!("File not found: {}", path_a));
    }
    if !p_b.is_file() {
        return Err(format!("File not found: {}", path_b));
    }

    let content_a = tokio::fs::read_to_string(p_a)
        .await
        .map_err(|e| format!("Cannot read '{}': {}", path_a, e))?;
    let content_b = tokio::fs::read_to_string(p_b)
        .await
        .map_err(|e| format!("Cannot read '{}': {}", path_b, e))?;

    let lines_a: Vec<&str> = content_a.lines().collect();
    let lines_b: Vec<&str> = content_b.lines().collect();

    // Simple LCS-based diff
    let mut diff_output = Vec::new();
    diff_output.push(format!("--- {}", path_a));
    diff_output.push(format!("+++ {}", path_b));

    let (mut i, mut j) = (0usize, 0usize);
    while i < lines_a.len() || j < lines_b.len() {
        if diff_output.len() > MAX_DIFF_LINES + 2 {
            diff_output.push(format!("... [truncated at {} diff lines]", MAX_DIFF_LINES));
            break;
        }

        if i < lines_a.len() && j < lines_b.len() && lines_a[i] == lines_b[j] {
            // Context line (identical)
            diff_output.push(format!(" {}", lines_a[i]));
            i += 1;
            j += 1;
        } else {
            // Look ahead in B for current A line (detect deletion vs replacement)
            let b_ahead = lines_b.iter().skip(j).take(5).position(|l| i < lines_a.len() && *l == lines_a[i]);
            let a_ahead = lines_a.iter().skip(i).take(5).position(|l| j < lines_b.len() && *l == lines_b[j]);

            match (a_ahead, b_ahead) {
                (Some(a_off), Some(b_off)) if a_off <= b_off => {
                    // Lines added in B before the match point
                    for k in 0..a_off {
                        if i + k < lines_a.len() {
                            diff_output.push(format!("-{}", lines_a[i + k]));
                        }
                    }
                    i += a_off;
                }
                (Some(_), Some(b_off)) => {
                    // Lines added in B
                    for k in 0..b_off {
                        if j + k < lines_b.len() {
                            diff_output.push(format!("+{}", lines_b[j + k]));
                        }
                    }
                    j += b_off;
                }
                (None, Some(b_off)) => {
                    for k in 0..b_off {
                        if j + k < lines_b.len() {
                            diff_output.push(format!("+{}", lines_b[j + k]));
                        }
                    }
                    j += b_off;
                }
                (Some(a_off), None) => {
                    for k in 0..a_off {
                        if i + k < lines_a.len() {
                            diff_output.push(format!("-{}", lines_a[i + k]));
                        }
                    }
                    i += a_off;
                }
                (None, None) => {
                    // No match found — both lines differ
                    if i < lines_a.len() {
                        diff_output.push(format!("-{}", lines_a[i]));
                        i += 1;
                    }
                    if j < lines_b.len() {
                        diff_output.push(format!("+{}", lines_b[j]));
                        j += 1;
                    }
                }
            }
        }
    }

    let changed = diff_output.iter().filter(|l| l.starts_with('+') || l.starts_with('-')).count() - 2; // minus header lines
    Ok(format!(
        "{}\n\n{} changed line(s)",
        diff_output.join("\n"),
        changed.max(0)
    ))
}

// ---------------------------------------------------------------------------
// read_pdf — PDF text extraction with OCR fallback
// ---------------------------------------------------------------------------

/// Maximum PDF file size (50 MB).
const MAX_PDF_SIZE: u64 = 50 * 1024 * 1024;

/// Maximum output text length for PDF/image tools.
const MAX_TOOL_OUTPUT_CHARS: usize = 6000;

/// Minimum alphanumeric characters to consider extraction successful.
const MIN_ALPHA_THRESHOLD: usize = 20;

/// Read and extract text from a PDF file.
/// Falls back to Gemini Vision OCR when pdf-extract yields empty/garbage text.
async fn tool_read_pdf(
    path: &str,
    page_range: Option<&str>,
    state: &AppState,
) -> Result<String, String> {
    let file_path = std::path::Path::new(path);

    if !file_path.exists() {
        return Err(format!("File not found: {}", path));
    }

    match file_path.extension().and_then(|e| e.to_str()) {
        Some(ext) if ext.eq_ignore_ascii_case("pdf") => {}
        _ => return Err(format!("Not a PDF file: {}", path)),
    }

    let metadata = tokio::fs::metadata(file_path)
        .await
        .map_err(|e| format!("Cannot read file metadata: {}", e))?;
    if metadata.len() > MAX_PDF_SIZE {
        return Err(format!(
            "PDF too large: {} bytes (max {} MB)",
            metadata.len(),
            MAX_PDF_SIZE / (1024 * 1024)
        ));
    }

    let bytes = tokio::fs::read(file_path)
        .await
        .map_err(|e| format!("Cannot read file: {}", e))?;

    let filename = file_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown.pdf");

    // Extract text (blocking — pdf-extract is synchronous)
    let bytes_clone = bytes.clone();
    let text = tokio::task::spawn_blocking(move || {
        pdf_extract::extract_text_from_mem(&bytes_clone)
            .map_err(|e| format!("PDF extraction failed: {}", e))
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))??;

    // Check if extraction yielded meaningful text
    let alpha_count = text.chars().filter(|c| c.is_alphanumeric()).count();
    let is_scanned = text.trim().len() < 50 || alpha_count < MIN_ALPHA_THRESHOLD;

    if is_scanned {
        tracing::info!(
            "read_pdf: text extraction yielded {} alphanumeric chars, falling back to Vision OCR",
            alpha_count
        );
        let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
        let ocr_text = crate::ocr::ocr_pdf_text(state, &b64, page_range).await?;

        let mut output = format!("### PDF (OCR): {} (Vision API)\n\n", filename);
        if ocr_text.len() + output.len() > MAX_TOOL_OUTPUT_CHARS {
            let available = MAX_TOOL_OUTPUT_CHARS.saturating_sub(output.len() + 40);
            let truncated: String = ocr_text.chars().take(available).collect();
            output.push_str(&truncated);
            output.push_str("\n\n[... truncated ...]");
        } else {
            output.push_str(&ocr_text);
        }
        return Ok(output);
    }

    // Split into pages (form feed character)
    let pages: Vec<&str> = text.split('\x0c').collect();
    let total_pages = pages.len();

    let (selected_text, range_label) = if let Some(range) = page_range {
        let (start, end) = parse_pdf_page_range(range, total_pages)?;
        let selected: String = pages[start - 1..end]
            .iter()
            .enumerate()
            .map(|(i, p)| format!("--- Page {} ---\n{}", start + i, p.trim()))
            .collect::<Vec<_>>()
            .join("\n\n");
        (selected, format!("pages {}-{} of {}", start, end, total_pages))
    } else {
        (text.clone(), format!("{} pages", total_pages))
    };

    let header = format!("### PDF: {} ({})\n\n", filename, range_label);
    let mut output = header;

    if selected_text.len() + output.len() > MAX_TOOL_OUTPUT_CHARS {
        let available = MAX_TOOL_OUTPUT_CHARS.saturating_sub(output.len() + 40);
        let truncated: String = selected_text.chars().take(available).collect();
        output.push_str(&truncated);
        output.push_str("\n\n[... truncated ...]");
    } else {
        output.push_str(&selected_text);
    }

    Ok(output)
}

/// Parse a page range string like "1-5" or "3" into (start, end) 1-indexed.
fn parse_pdf_page_range(range: &str, total: usize) -> Result<(usize, usize), String> {
    let range = range.trim();
    if let Some((start_s, end_s)) = range.split_once('-') {
        let start: usize = start_s.trim().parse().map_err(|_| "Invalid page range start")?;
        let end: usize = end_s.trim().parse().map_err(|_| "Invalid page range end")?;
        if start < 1 || end < start || end > total {
            return Err(format!(
                "Page range {}-{} out of bounds (1-{})",
                start, end, total
            ));
        }
        Ok((start, end))
    } else {
        let page: usize = range.parse().map_err(|_| "Invalid page number")?;
        if page < 1 || page > total {
            return Err(format!("Page {} out of bounds (1-{})", page, total));
        }
        Ok((page, page))
    }
}

// ---------------------------------------------------------------------------
// analyze_image — Gemini Vision API with OCR mode
// ---------------------------------------------------------------------------

/// Allowed image extensions.
const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "webp", "gif"];

/// Maximum image file size (10 MB — Gemini limit).
const MAX_IMAGE_SIZE: u64 = 10 * 1024 * 1024;

/// OCR prompt for text extraction mode.
const OCR_PROMPT: &str = "\
Extract ALL text from this image exactly as written. Preserve:\n\
- Line breaks and paragraph structure\n\
- Formatting (headers, lists, tables)\n\
- Special characters and numbers\n\
- Reading order (left-to-right, top-to-bottom)\n\
\n\
If the text is handwritten, transcribe it as accurately as possible.\n\
If there are tables, format them using markdown table syntax.\n\
Return ONLY the extracted text, no descriptions or commentary.";

/// Analyze an image using Gemini Vision API.
/// When `extract_text` is true, performs OCR instead of description.
/// Returns ToolOutput with text + optional inline_data for Gemini multimodal responses.
async fn tool_analyze_image(
    path: &str,
    prompt: Option<&str>,
    extract_text: Option<bool>,
    state: &AppState,
) -> Result<ToolOutput, String> {
    let file_path = std::path::Path::new(path);

    if !file_path.exists() {
        return Err(format!("Image file not found: {}", path));
    }

    let ext = file_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();
    if !IMAGE_EXTENSIONS.contains(&ext.as_str()) {
        return Err(format!(
            "Not a supported image format: {}. Supported: {:?}",
            path, IMAGE_EXTENSIONS
        ));
    }

    let metadata = tokio::fs::metadata(file_path)
        .await
        .map_err(|e| format!("Cannot read metadata: {}", e))?;
    if metadata.len() > MAX_IMAGE_SIZE {
        return Err(format!(
            "Image too large: {} bytes (max {} MB)",
            metadata.len(),
            MAX_IMAGE_SIZE / (1024 * 1024)
        ));
    }

    let bytes = tokio::fs::read(file_path)
        .await
        .map_err(|e| format!("Cannot read image: {}", e))?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);

    let mime_type = match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        _ => "application/octet-stream",
    };

    let analysis_prompt = if extract_text.unwrap_or(false) {
        prompt.unwrap_or(OCR_PROMPT)
    } else {
        prompt.unwrap_or(
            "Describe this image in detail. Include any text, objects, people, colors, layout, and notable features.",
        )
    };

    // Get credential via oauth module
    let (credential, is_oauth) = crate::oauth::get_google_credential(state)
        .await
        .ok_or_else(|| "No Google API credential configured".to_string())?;

    let url = "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:generateContent";

    let request_body = serde_json::json!({
        "contents": [{
            "parts": [
                {
                    "inlineData": {
                        "mimeType": mime_type,
                        "data": b64
                    }
                },
                {
                    "text": analysis_prompt
                }
            ]
        }],
        "generationConfig": {
            "temperature": 1.0,  // Gemini 3: ALWAYS 1.0
            "maxOutputTokens": 4096
        }
    });

    let builder = state.client.post(url).json(&request_body);
    let builder = crate::oauth::apply_google_auth(builder, &credential, is_oauth);

    let response = builder
        .send()
        .await
        .map_err(|e| format!("Gemini API request failed: {}", e))?;

    let status = response.status();
    let body: Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse Gemini response: {}", e))?;

    if !status.is_success() {
        let msg = body["error"]["message"]
            .as_str()
            .unwrap_or("Unknown Gemini API error");
        return Err(format!("Gemini API error ({}): {}", status, msg));
    }

    let text = body["candidates"][0]["content"]["parts"]
        .as_array()
        .and_then(|parts| {
            parts
                .iter()
                .filter_map(|p| p["text"].as_str())
                .collect::<Vec<_>>()
                .first()
                .map(|s| s.to_string())
        })
        .unwrap_or_default();

    if text.is_empty() {
        return Err("Gemini returned empty result".to_string());
    }

    let filename = file_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("image");
    let label = if extract_text.unwrap_or(false) { "OCR" } else { "Image Analysis" };
    let output_text = format!(
        "### {}: {} ({}, {} bytes)\n\n{}",
        label, filename, mime_type, metadata.len(), text
    );

    // Return text + inline_data for Gemini multimodal function responses
    Ok(ToolOutput {
        text: output_text,
        inline_data: Some(InlineData {
            mime_type: mime_type.to_string(),
            data: b64,
        }),
    })
}

// ---------------------------------------------------------------------------
// ocr_document — dedicated OCR tool with markdown table preservation
// ---------------------------------------------------------------------------

const OCR_DOCUMENT_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "webp", "gif", "pdf"];

async fn tool_ocr_document(
    path: &str,
    custom_prompt: Option<&str>,
    state: &AppState,
) -> Result<String, String> {
    let file_path = std::path::Path::new(path);

    if !file_path.exists() {
        return Err(format!("File not found: {}", path));
    }

    let ext = file_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    if !OCR_DOCUMENT_EXTENSIONS.contains(&ext.as_str()) {
        return Err(format!(
            "Unsupported file type: .{}. Supported: {:?}",
            ext, OCR_DOCUMENT_EXTENSIONS
        ));
    }

    let metadata = tokio::fs::metadata(file_path)
        .await
        .map_err(|e| format!("Cannot read metadata: {}", e))?;
    if metadata.len() > 30_000_000 {
        return Err(format!(
            "File too large: {} bytes (max 22 MB decoded)",
            metadata.len()
        ));
    }

    let bytes = tokio::fs::read(file_path)
        .await
        .map_err(|e| format!("Cannot read file: {}", e))?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);

    let mime_type = match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "pdf" => "application/pdf",
        _ => "application/octet-stream",
    };

    // OCR functions use the default OCR_PROMPT which already preserves tables as markdown
    let _ = custom_prompt; // reserved for future custom prompt support
    let text = if ext == "pdf" {
        crate::ocr::ocr_pdf_text(state, &b64, None).await?
    } else {
        crate::ocr::ocr_image_text(state, &b64, mime_type).await?
    };

    let filename = file_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("document");
    Ok(format!(
        "### OCR: {} ({}, {} bytes)\n\n{}",
        filename, mime_type, metadata.len(), text
    ))
}

// ---------------------------------------------------------------------------
// Web Scraping Tools
// ---------------------------------------------------------------------------

/// Validate URL — only http/https allowed
fn validate_web_url(raw: &str) -> Result<Url, String> {
    let parsed = Url::parse(raw).map_err(|e| format!("Invalid URL '{}': {}", raw, e))?;
    match parsed.scheme() {
        "http" | "https" => Ok(parsed),
        other => Err(format!("Unsupported URL scheme '{}' — only http/https allowed", other)),
    }
}

/// Extract readable text from HTML, preserving structure
fn extract_text_from_html(html: &str) -> String {
    let doc = Html::parse_document(html);

    // Extract title
    let title = Selector::parse("title").ok()
        .and_then(|sel| doc.select(&sel).next())
        .map(|el| el.text().collect::<String>());

    // Walk body for text
    let mut raw_text = String::new();
    if let Ok(body_sel) = Selector::parse("body") {
        if let Some(body) = doc.select(&body_sel).next() {
            collect_element_text(body, &mut raw_text);
        }
    }

    // Clean up excessive whitespace
    let lines: Vec<&str> = raw_text.lines().map(|l| l.trim()).collect();
    let mut output = String::new();
    let mut last_was_blank = false;
    for line in lines {
        if line.is_empty() {
            if !last_was_blank {
                output.push('\n');
                last_was_blank = true;
            }
        } else {
            output.push_str(line);
            output.push('\n');
            last_was_blank = false;
        }
    }

    if let Some(t) = title {
        let t = t.trim();
        if !t.is_empty() {
            return format!("# {}\n\n{}", t, output.trim());
        }
    }

    output.trim().to_string()
}

/// Recursively collect text from an ElementRef, skipping noise tags
fn collect_element_text(element: scraper::ElementRef, output: &mut String) {
    let tag = element.value().name();

    // Skip noise elements entirely
    if matches!(tag, "script" | "style" | "nav" | "footer" | "noscript" | "svg" | "iframe") {
        return;
    }

    let is_block = matches!(
        tag,
        "p" | "div" | "section" | "article" | "main" | "blockquote"
        | "pre" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6"
        | "ul" | "ol" | "table" | "tr" | "br" | "hr"
    );
    let is_list_item = tag == "li";
    let is_heading = matches!(tag, "h1" | "h2" | "h3" | "h4" | "h5" | "h6");

    if is_block {
        output.push('\n');
    }
    if is_list_item {
        output.push_str("\n- ");
    }
    if is_heading {
        let level: usize = tag[1..].parse().unwrap_or(1);
        let prefix = "#".repeat(level);
        output.push_str(&format!("\n{} ", prefix));
    }

    for child in element.children() {
        match child.value() {
            scraper::node::Node::Text(text) => {
                let t = text.text.trim();
                if !t.is_empty() {
                    output.push_str(t);
                    output.push(' ');
                }
            }
            scraper::node::Node::Element(_) => {
                if let Some(child_el) = scraper::ElementRef::wrap(child) {
                    collect_element_text(child_el, output);
                }
            }
            _ => {}
        }
    }

    if is_block {
        output.push('\n');
    }
}

/// Extract all links from HTML, resolving relative URLs
fn extract_links_from_html(html: &str, base_url: &Url) -> Vec<(String, String)> {
    let doc = Html::parse_document(html);
    let mut links = Vec::new();
    let mut seen = HashSet::new();

    if let Ok(sel) = Selector::parse("a[href]") {
        for el in doc.select(&sel) {
            if let Some(href) = el.value().attr("href") {
                let href = href.trim();
                // Skip anchors, javascript, mailto
                if href.is_empty()
                    || href.starts_with('#')
                    || href.starts_with("javascript:")
                    || href.starts_with("mailto:")
                    || href.starts_with("tel:")
                {
                    continue;
                }
                // Resolve relative URL
                let resolved = match base_url.join(href) {
                    Ok(u) => u.to_string(),
                    Err(_) => continue,
                };
                if seen.contains(&resolved) {
                    continue;
                }
                seen.insert(resolved.clone());

                let anchor: String = el.text().collect::<Vec<_>>().join(" ");
                let anchor = anchor.trim().to_string();
                links.push((resolved, anchor));
            }
        }
    }

    links
}

/// Fetch a URL and return (html_body, final_url)
async fn fetch_url(client: &reqwest::Client, url: &str) -> Result<(String, Url), String> {
    let parsed = validate_web_url(url)?;

    let resp = client
        .get(parsed.as_str())
        .header("User-Agent", WEB_USER_AGENT)
        .timeout(FETCH_TIMEOUT)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch '{}': {}", url, e))?;

    let status = resp.status();
    if !status.is_success() {
        return Err(format!("HTTP {} for '{}'", status, url));
    }

    // Check content length
    if let Some(len) = resp.content_length() {
        if len as usize > MAX_PAGE_SIZE {
            return Err(format!("Response too large: {} bytes (max {})", len, MAX_PAGE_SIZE));
        }
    }

    let final_url = Url::parse(resp.url().as_str())
        .unwrap_or(parsed);

    let bytes = resp.bytes().await
        .map_err(|e| format!("Failed to read body from '{}': {}", url, e))?;

    if bytes.len() > MAX_PAGE_SIZE {
        return Err(format!("Response too large: {} bytes (max {})", bytes.len(), MAX_PAGE_SIZE));
    }

    let body = String::from_utf8_lossy(&bytes).to_string();
    Ok((body, final_url))
}

/// Fetch a single web page and extract text + links
async fn tool_fetch_webpage(
    url: &str,
    extract_links: bool,
    client: &reqwest::Client,
) -> Result<ToolOutput, String> {
    let (html, final_url) = fetch_url(client, url).await?;

    let text = extract_text_from_html(&html);
    let mut output = format!("### Web Page: {}\n\n{}", final_url, text);

    if extract_links {
        let links = extract_links_from_html(&html, &final_url);
        if !links.is_empty() {
            output.push_str("\n\n---\n### Links Found\n\n");
            for (i, (href, anchor)) in links.iter().enumerate() {
                let label = if anchor.is_empty() { href.as_str() } else { anchor.as_str() };
                output.push_str(&format!("{}. [{}]({})\n", i + 1, label, href));
            }
            output.push_str(&format!("\nTotal: {} links", links.len()));
        }
    }

    Ok(ToolOutput::text(output))
}

/// Crawl a website starting from a URL, following links to subpages
async fn tool_crawl_website(
    start_url: &str,
    max_depth: u32,
    max_pages: usize,
    same_domain_only: bool,
    client: &reqwest::Client,
) -> Result<ToolOutput, String> {
    let start_parsed = validate_web_url(start_url)?;
    let start_domain = start_parsed.domain().unwrap_or("").to_string();

    let mut visited: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<(String, u32)> = VecDeque::new();
    let mut results: Vec<(String, String)> = Vec::new(); // (url, text_excerpt)
    let mut all_links: Vec<(String, String, String)> = Vec::new(); // (source, href, anchor)
    let mut errors: Vec<String> = Vec::new();

    queue.push_back((start_parsed.to_string(), 0));

    while let Some((url, depth)) = queue.pop_front() {
        if visited.contains(&url) || visited.len() >= max_pages {
            continue;
        }
        visited.insert(url.clone());

        // Rate limiting
        if visited.len() > 1 {
            tokio::time::sleep(CRAWL_DELAY).await;
        }

        match fetch_url(client, &url).await {
            Ok((html, final_url)) => {
                let text = extract_text_from_html(&html);
                // Excerpt — first 2000 chars
                let excerpt: String = text.char_indices()
                    .take_while(|(i, _)| *i < 2000)
                    .map(|(_, c)| c)
                    .collect();
                results.push((final_url.to_string(), excerpt));

                let links = extract_links_from_html(&html, &final_url);
                for (href, anchor) in &links {
                    all_links.push((url.clone(), href.clone(), anchor.clone()));

                    // Enqueue subpages
                    if depth < max_depth && !visited.contains(href) {
                        if same_domain_only {
                            if let Ok(link_url) = Url::parse(href) {
                                let link_domain = link_url.domain().unwrap_or("");
                                if link_domain != start_domain {
                                    continue;
                                }
                            }
                        }
                        // Only follow HTML-like URLs (skip files)
                        let path = href.to_lowercase();
                        if path.ends_with(".pdf") || path.ends_with(".zip")
                            || path.ends_with(".png") || path.ends_with(".jpg")
                            || path.ends_with(".gif") || path.ends_with(".svg")
                            || path.ends_with(".css") || path.ends_with(".js")
                            || path.ends_with(".xml") || path.ends_with(".json")
                        {
                            continue;
                        }
                        queue.push_back((href.clone(), depth + 1));
                    }
                }
            }
            Err(e) => {
                errors.push(format!("{}: {}", url, e));
            }
        }
    }

    // Format output
    let mut output = format!(
        "### Crawl Results: {}\nPages fetched: {} | Errors: {} | Links indexed: {}\n",
        start_url, results.len(), errors.len(), all_links.len()
    );

    // Page contents
    output.push_str("\n---\n## Pages\n\n");
    for (i, (url, excerpt)) in results.iter().enumerate() {
        output.push_str(&format!("### {}. {}\n{}\n\n", i + 1, url, excerpt));
    }

    // Link index
    if !all_links.is_empty() {
        output.push_str("---\n## Link Index\n\n");
        for (source, href, anchor) in &all_links {
            let label = if anchor.is_empty() { href.as_str() } else { anchor.as_str() };
            output.push_str(&format!("- [{}]({}) ← {}\n", label, href, source));
        }
    }

    // Errors
    if !errors.is_empty() {
        output.push_str("\n---\n## Errors\n\n");
        for err in &errors {
            output.push_str(&format!("- {}\n", err));
        }
    }

    Ok(ToolOutput::text(output))
}
