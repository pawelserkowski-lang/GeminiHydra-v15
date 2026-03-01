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
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet, VecDeque};
use std::net::IpAddr;
use std::time::{Duration, Instant};
use tokio::process::Command;
use tokio::task::JoinSet;
use url::Url;
use crate::state::AppState;

/// Max output bytes from a single command (50 KB).
const MAX_COMMAND_OUTPUT: usize = 50 * 1024;

/// Command execution timeout.
const COMMAND_TIMEOUT: Duration = Duration::from_secs(30);

// ---------------------------------------------------------------------------
// Web Scraping Constants (v2 — 50 improvements)
// ---------------------------------------------------------------------------
const MAX_PAGE_SIZE: usize = 5 * 1024 * 1024;
const FETCH_TIMEOUT: Duration = Duration::from_secs(30);
const DEFAULT_CRAWL_DELAY_MS: u64 = 300;
const MAX_CRAWL_DEPTH: u32 = 5;
const MAX_CRAWL_PAGES: usize = 50;
const MAX_CONCURRENT: usize = 5;
const MAX_TOTAL_CRAWL_SECS: u64 = 180;
const MAX_RETRY_ATTEMPTS: u32 = 3;
const WEB_USER_AGENT: &str = "Jaskier-Bot/1.0 (AI Agent Tool)";

const TRACKING_PARAMS: &[&str] = &[
    "utm_source", "utm_medium", "utm_campaign", "utm_term", "utm_content",
    "fbclid", "gclid", "mc_cid", "mc_eid", "ref", "_ga",
];

const SKIP_EXTENSIONS: &[&str] = &[
    ".pdf", ".zip", ".tar", ".gz", ".rar", ".7z",
    ".png", ".jpg", ".jpeg", ".gif", ".svg", ".webp", ".ico", ".bmp",
    ".css", ".js", ".woff", ".woff2", ".ttf", ".eot",
    ".xml", ".json", ".rss", ".atom",
    ".mp3", ".mp4", ".avi", ".mov", ".wmv", ".flv",
    ".doc", ".docx", ".xls", ".xlsx", ".ppt", ".pptx",
    ".exe", ".dmg", ".apk", ".deb", ".rpm",
];

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

/// Tool metadata for runtime discovery.
pub struct ToolInfo {
    pub name: &'static str,
    pub category: &'static str,
}

/// List all registered tool names with categories (for diagnostics and MCP server).
pub fn list_available_tools() -> Vec<ToolInfo> {
    vec![
        // Filesystem tools
        ToolInfo { name: "execute_command", category: "filesystem" },
        ToolInfo { name: "read_file", category: "filesystem" },
        ToolInfo { name: "write_file", category: "filesystem" },
        ToolInfo { name: "edit_file", category: "filesystem" },
        ToolInfo { name: "list_directory", category: "filesystem" },
        ToolInfo { name: "search_files", category: "filesystem" },
        ToolInfo { name: "get_code_structure", category: "filesystem" },
        ToolInfo { name: "read_file_section", category: "filesystem" },
        ToolInfo { name: "find_file", category: "filesystem" },
        ToolInfo { name: "diff_files", category: "filesystem" },
        // Document tools
        ToolInfo { name: "read_pdf", category: "document" },
        ToolInfo { name: "analyze_image", category: "document" },
        ToolInfo { name: "ocr_document", category: "document" },
        // Web tools
        ToolInfo { name: "fetch_webpage", category: "web" },
        ToolInfo { name: "crawl_website", category: "web" },
        // Git tools
        ToolInfo { name: "git_status", category: "git" },
        ToolInfo { name: "git_log", category: "git" },
        ToolInfo { name: "git_diff", category: "git" },
        ToolInfo { name: "git_branch", category: "git" },
        ToolInfo { name: "git_commit", category: "git" },
        // A2A delegation
        ToolInfo { name: "call_agent", category: "a2a" },
        // MCP proxy
        ToolInfo { name: "list_mcp_tools", category: "mcp" },
        ToolInfo { name: "execute_mcp_tool", category: "mcp" },
    ]
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
            tool_fetch_webpage(args, &state.client).await
        }
        "crawl_website" => {
            tool_crawl_website(args, &state.client).await
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
// Web Scraping Tools (v2 — 50 improvements)
// ---------------------------------------------------------------------------

// --- Types ---

struct WebFetchResult {
    url: String,
    title: String,
    text: String,
    metadata: WebPageMetadata,
    links: Vec<WebCategorizedLink>,
    content_hash: String,
}

#[derive(Default)]
struct WebPageMetadata {
    description: String,
    og_title: String,
    og_description: String,
    og_image: String,
    canonical_url: String,
    language: String,
    json_ld: Vec<String>,
}

#[derive(Clone)]
enum WebLinkType { Internal, External, Resource }

#[derive(Clone)]
struct WebCategorizedLink {
    href: String,
    anchor: String,
    link_type: WebLinkType,
}

struct WebRobotsRules {
    disallow: Vec<String>,
    allow: Vec<String>,
    crawl_delay: Option<f64>,
    sitemaps: Vec<String>,
}

struct WebPageResult {
    url: String,
    title: String,
    text_excerpt: String,
    content_hash: String,
    metadata: WebPageMetadata,
    links: Vec<WebCategorizedLink>,
}

struct WebExtractionOptions {
    include_links: bool,
    include_metadata: bool,
    include_images: bool,
    max_text_length: usize,
}

// --- URL validation, normalization, SSRF prevention ---

fn web_validate_url(raw: &str) -> Result<Url, String> {
    let parsed = Url::parse(raw).map_err(|e| format!("Invalid URL '{}': {}", raw, e))?;
    match parsed.scheme() {
        "http" | "https" => {}
        other => return Err(format!("Unsupported scheme '{}' — only http/https", other)),
    }
    // SSRF prevention
    if let Some(host) = parsed.host_str() {
        let lower = host.to_lowercase();
        if lower == "localhost"
            || lower == "metadata.google.internal"
            || lower.ends_with(".internal")
            || lower == "169.254.169.254"
        {
            return Err(format!("Blocked host: {}", host));
        }
        if let Ok(ip) = host.parse::<IpAddr>() {
            let is_private = match ip {
                IpAddr::V4(v4) => v4.is_loopback() || v4.is_private() || v4.is_link_local(),
                IpAddr::V6(v6) => v6.is_loopback(),
            };
            if is_private {
                return Err(format!("Blocked private/loopback IP: {}", ip));
            }
        }
    }
    Ok(parsed)
}

fn web_normalize_url(url: &Url) -> String {
    let mut normalized = url.clone();
    normalized.set_fragment(None);
    // Strip tracking params
    let pairs: Vec<(String, String)> = normalized
        .query_pairs()
        .filter(|(k, _)| !TRACKING_PARAMS.contains(&k.as_ref()))
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    if pairs.is_empty() {
        normalized.set_query(None);
    } else {
        let mut sorted = pairs;
        sorted.sort_by(|a, b| a.0.cmp(&b.0));
        let qs: Vec<String> = sorted.iter().map(|(k, v)| {
            if v.is_empty() { k.clone() } else { format!("{}={}", k, v) }
        }).collect();
        normalized.set_query(Some(&qs.join("&")));
    }
    // Remove trailing slash for non-root paths
    let mut s = normalized.to_string();
    if s.ends_with('/') && normalized.path() != "/" {
        s.pop();
    }
    s
}

fn web_should_skip_url(path: &str) -> bool {
    let lower = path.to_lowercase();
    SKIP_EXTENSIONS.iter().any(|ext| lower.ends_with(ext))
}

// --- robots.txt ---

async fn web_fetch_robots(client: &reqwest::Client, base: &Url) -> Option<WebRobotsRules> {
    let robots_url = format!("{}://{}/robots.txt", base.scheme(), base.authority());
    let resp = client
        .get(&robots_url)
        .header("User-Agent", WEB_USER_AGENT)
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() { return None; }
    let text = resp.text().await.ok()?;
    Some(web_parse_robots(&text))
}

fn web_parse_robots(text: &str) -> WebRobotsRules {
    let mut rules = WebRobotsRules {
        disallow: Vec::new(),
        allow: Vec::new(),
        crawl_delay: None,
        sitemaps: Vec::new(),
    };
    let mut in_our_section = false;
    let mut in_any_section = false;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') { continue; }
        let lower = line.to_lowercase();
        if lower.starts_with("user-agent:") {
            let agent = line[11..].trim().to_lowercase();
            in_any_section = true;
            in_our_section = agent == "*" || agent.contains("jaskier");
        } else if lower.starts_with("sitemap:") {
            let url = line[8..].trim();
            if !url.is_empty() {
                rules.sitemaps.push(url.to_string());
            }
        } else if in_our_section || !in_any_section {
            if lower.starts_with("disallow:") {
                let path = line[9..].trim();
                if !path.is_empty() { rules.disallow.push(path.to_string()); }
            } else if lower.starts_with("allow:") {
                let path = line[6..].trim();
                if !path.is_empty() { rules.allow.push(path.to_string()); }
            } else if lower.starts_with("crawl-delay:") {
                if let Ok(d) = line[12..].trim().parse::<f64>() {
                    rules.crawl_delay = Some(d);
                }
            }
        }
    }
    rules
}

fn web_is_allowed_by_robots(rules: &WebRobotsRules, path: &str) -> bool {
    // Allow rules take precedence over disallow for same-length match
    for a in &rules.allow {
        if path.starts_with(a.as_str()) { return true; }
    }
    for d in &rules.disallow {
        if path.starts_with(d.as_str()) { return false; }
    }
    true
}

// --- Sitemap ---

async fn web_fetch_sitemap(client: &reqwest::Client, base: &Url, robots: &Option<WebRobotsRules>) -> Vec<String> {
    let mut sitemap_urls: Vec<String> = Vec::new();
    let mut candidates = Vec::new();
    if let Some(r) = robots {
        candidates.extend(r.sitemaps.clone());
    }
    if candidates.is_empty() {
        candidates.push(format!("{}://{}/sitemap.xml", base.scheme(), base.authority()));
    }
    for sm_url in &candidates {
        if let Ok(resp) = client
            .get(sm_url)
            .header("User-Agent", WEB_USER_AGENT)
            .timeout(Duration::from_secs(15))
            .send()
            .await
        {
            if resp.status().is_success() {
                if let Ok(body) = resp.text().await {
                    web_parse_sitemap_xml(&body, &mut sitemap_urls);
                }
            }
        }
    }
    sitemap_urls
}

fn web_parse_sitemap_xml(xml: &str, out: &mut Vec<String>) {
    // Simple regex-based XML parsing for <loc> tags
    let loc_re = Regex::new(r"<loc>\s*(.*?)\s*</loc>").expect("loc regex is valid");
    let is_index = xml.contains("<sitemapindex");
    for cap in loc_re.captures_iter(xml) {
        if let Some(m) = cap.get(1) {
            let url = m.as_str().trim();
            if !url.is_empty() {
                if is_index {
                    // Sitemap index — these are sub-sitemaps; we just collect the URLs
                    out.push(url.to_string());
                } else {
                    out.push(url.to_string());
                }
            }
        }
    }
}

// --- HTTP fetch with retry ---

async fn web_fetch_with_retry(
    client: &reqwest::Client,
    url: &str,
    custom_headers: &HashMap<String, String>,
) -> Result<(String, Url, u16), String> {
    let parsed = web_validate_url(url)?;
    let mut last_err = String::new();

    for attempt in 0..MAX_RETRY_ATTEMPTS {
        if attempt > 0 {
            let delay = Duration::from_millis(500 * 2u64.pow(attempt));
            tokio::time::sleep(delay).await;
        }
        let mut req = client
            .get(parsed.as_str())
            .header("User-Agent", WEB_USER_AGENT)
            .header("Accept", "text/html,application/xhtml+xml,*/*;q=0.8")
            .header("Accept-Language", "en-US,en;q=0.9,pl;q=0.8")
            .timeout(FETCH_TIMEOUT);
        for (k, v) in custom_headers {
            req = req.header(k.as_str(), v.as_str());
        }
        match req.send().await {
            Ok(resp) => {
                let status = resp.status().as_u16();
                if status == 429 || (status >= 500 && status < 600) {
                    last_err = format!("HTTP {} for '{}'", status, url);
                    continue; // retry
                }
                if !resp.status().is_success() {
                    return Err(format!("HTTP {} for '{}'", status, url));
                }
                // Content-Type check
                let ct = resp.headers().get("content-type")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("");
                if !ct.is_empty() && !ct.contains("text/") && !ct.contains("html") && !ct.contains("xml") {
                    return Err(format!("Non-HTML content: {} for '{}'", ct, url));
                }
                if let Some(len) = resp.content_length() {
                    if len as usize > MAX_PAGE_SIZE {
                        return Err(format!("Response too large: {} bytes", len));
                    }
                }
                let final_url = resp.url().clone();
                let bytes = resp.bytes().await
                    .map_err(|e| format!("Read body failed for '{}': {}", url, e))?;
                if bytes.len() > MAX_PAGE_SIZE {
                    return Err(format!("Response too large: {} bytes", bytes.len()));
                }
                let body = String::from_utf8_lossy(&bytes).to_string();
                return Ok((body, final_url, status));
            }
            Err(e) => {
                last_err = format!("Fetch '{}': {}", url, e);
                if e.is_timeout() { continue; }
                return Err(last_err);
            }
        }
    }
    Err(format!("Failed after {} retries: {}", MAX_RETRY_ATTEMPTS, last_err))
}

// --- Enhanced HTML → text extraction ---

fn web_extract_text(html: &str, opts: &WebExtractionOptions) -> String {
    let doc = Html::parse_document(html);

    // Title
    let title = Selector::parse("title").ok()
        .and_then(|sel| doc.select(&sel).next())
        .map(|el| el.text().collect::<String>());

    // Content priority: article > main > body
    let content_el = ["article", "main", "body"].iter()
        .find_map(|tag| {
            Selector::parse(tag).ok()
                .and_then(|sel| doc.select(&sel).next())
        });

    let mut raw = String::new();
    if let Some(el) = content_el {
        web_collect_element_text(el, &mut raw, opts);
    }

    // Collapse whitespace
    let lines: Vec<&str> = raw.lines().map(|l| l.trim_end()).collect();
    let mut output = String::new();
    let mut blank_count = 0;
    for line in lines {
        if line.is_empty() {
            blank_count += 1;
            if blank_count <= 2 { output.push('\n'); }
        } else {
            blank_count = 0;
            output.push_str(line);
            output.push('\n');
        }
    }

    let text = output.trim().to_string();
    if let Some(t) = title {
        let t = t.trim();
        if !t.is_empty() {
            return format!("# {}\n\n{}", t, text);
        }
    }
    text
}

fn web_collect_element_text(element: scraper::ElementRef, out: &mut String, opts: &WebExtractionOptions) {
    let tag = element.value().name();

    // Skip noise
    if matches!(tag, "script" | "style" | "noscript" | "svg" | "iframe" | "nav" | "footer" | "header") {
        return;
    }

    match tag {
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
            let level: usize = tag[1..].parse().unwrap_or(1);
            let prefix = "#".repeat(level);
            let text: String = element.text().collect::<Vec<_>>().join(" ");
            let text = text.trim();
            if !text.is_empty() {
                out.push_str(&format!("\n{} {}\n\n", prefix, text));
            }
            return; // don't recurse into heading children
        }
        "pre" | "code" => {
            let text: String = element.text().collect::<Vec<_>>().join("");
            if !text.trim().is_empty() {
                // Detect language from class
                let lang = element.value().attr("class")
                    .and_then(|c| c.split_whitespace()
                        .find(|cls| cls.starts_with("language-") || cls.starts_with("lang-"))
                        .map(|cls| cls.split('-').nth(1).unwrap_or("")))
                    .unwrap_or("");
                out.push_str(&format!("\n```{}\n{}\n```\n", lang, text.trim()));
            }
            return;
        }
        "table" => {
            web_extract_table(element, out);
            return;
        }
        "img" if opts.include_images => {
            let alt = element.value().attr("alt").unwrap_or("").trim();
            if !alt.is_empty() {
                let src = element.value().attr("src").unwrap_or("");
                out.push_str(&format!("![{}]({})", alt, src));
            }
            return;
        }
        "a" if opts.include_links => {
            let href = element.value().attr("href").unwrap_or("").trim();
            let text: String = element.text().collect::<Vec<_>>().join(" ");
            let text = text.trim();
            if !text.is_empty() && !href.is_empty()
                && !href.starts_with('#') && !href.starts_with("javascript:")
            {
                out.push_str(&format!("[{}]({})", text, href));
            } else if !text.is_empty() {
                out.push_str(text);
            }
            return;
        }
        "details" => {
            // Expand details/summary
            if let Ok(sum_sel) = Selector::parse("summary") {
                if let Some(summary) = element.select(&sum_sel).next() {
                    let text: String = summary.text().collect::<Vec<_>>().join(" ");
                    out.push_str(&format!("\n**{}**\n", text.trim()));
                }
            }
        }
        "dl" => {
            web_extract_definition_list(element, out);
            return;
        }
        "li" => {
            out.push_str("- ");
        }
        "br" => {
            out.push('\n');
            return;
        }
        "hr" => {
            out.push_str("\n---\n");
            return;
        }
        "p" | "div" | "section" | "article" | "main" | "blockquote" => {
            out.push('\n');
        }
        _ => {}
    }

    for child in element.children() {
        match child.value() {
            scraper::node::Node::Text(text) => {
                let t = text.text.trim();
                if !t.is_empty() {
                    out.push_str(t);
                    out.push(' ');
                }
            }
            scraper::node::Node::Element(_) => {
                if let Some(child_el) = scraper::ElementRef::wrap(child) {
                    web_collect_element_text(child_el, out, opts);
                }
            }
            _ => {}
        }
    }

    if matches!(tag, "p" | "div" | "section" | "article" | "main" | "blockquote" | "li") {
        out.push('\n');
    }
}

fn web_extract_table(table: scraper::ElementRef, out: &mut String) {
    let row_sel = Selector::parse("tr").expect("tr selector is valid");
    let th_sel = Selector::parse("th").expect("th selector is valid");
    let td_sel = Selector::parse("td").expect("td selector is valid");

    let mut rows: Vec<Vec<String>> = Vec::new();
    for row in table.select(&row_sel) {
        let mut cells: Vec<String> = Vec::new();
        for cell in row.select(&th_sel).chain(row.select(&td_sel)) {
            let text: String = cell.text().collect::<Vec<_>>().join(" ");
            cells.push(text.trim().replace('|', "\\|").to_string());
        }
        if !cells.is_empty() { rows.push(cells); }
    }
    if rows.is_empty() { return; }

    let max_cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    out.push('\n');
    for (i, row) in rows.iter().enumerate() {
        out.push('|');
        for j in 0..max_cols {
            let cell = row.get(j).map(|s| s.as_str()).unwrap_or("");
            out.push_str(&format!(" {} |", cell));
        }
        out.push('\n');
        if i == 0 {
            out.push('|');
            for _ in 0..max_cols {
                out.push_str(" --- |");
            }
            out.push('\n');
        }
    }
    out.push('\n');
}

fn web_extract_definition_list(dl: scraper::ElementRef, out: &mut String) {
    let dt_sel = Selector::parse("dt").expect("dt selector is valid");
    let dd_sel = Selector::parse("dd").expect("dd selector is valid");
    out.push('\n');
    for dt in dl.select(&dt_sel) {
        let text: String = dt.text().collect::<Vec<_>>().join(" ");
        out.push_str(&format!("**{}**\n", text.trim()));
    }
    for dd in dl.select(&dd_sel) {
        let text: String = dd.text().collect::<Vec<_>>().join(" ");
        out.push_str(&format!(": {}\n", text.trim()));
    }
    out.push('\n');
}

// --- Metadata extraction ---

fn web_extract_metadata(html: &str, base_url: &Url) -> WebPageMetadata {
    let doc = Html::parse_document(html);
    let mut meta = WebPageMetadata::default();

    if let Ok(sel) = Selector::parse("meta") {
        for el in doc.select(&sel) {
            let name = el.value().attr("name").or_else(|| el.value().attr("property")).unwrap_or("");
            let content = el.value().attr("content").unwrap_or("");
            match name {
                "description" => meta.description = content.to_string(),
                "og:title" => meta.og_title = content.to_string(),
                "og:description" => meta.og_description = content.to_string(),
                "og:image" => meta.og_image = content.to_string(),
                _ => {}
            }
        }
    }
    if let Ok(sel) = Selector::parse("link[rel='canonical']") {
        if let Some(el) = doc.select(&sel).next() {
            if let Some(href) = el.value().attr("href") {
                meta.canonical_url = href.to_string();
            }
        }
    }
    if let Ok(sel) = Selector::parse("html") {
        if let Some(el) = doc.select(&sel).next() {
            if let Some(lang) = el.value().attr("lang") {
                meta.language = lang.to_string();
            }
        }
    }
    // JSON-LD
    if let Ok(sel) = Selector::parse("script[type='application/ld+json']") {
        for el in doc.select(&sel) {
            let text: String = el.text().collect();
            let text = text.trim();
            if !text.is_empty() && text.len() < 5000 {
                meta.json_ld.push(text.to_string());
            }
        }
    }
    let _ = base_url; // reserved for future relative URL resolution in metadata
    meta
}

// --- Link extraction & categorization ---

fn web_extract_links(html: &str, base_url: &Url) -> Vec<WebCategorizedLink> {
    let doc = Html::parse_document(html);
    let mut links = Vec::new();
    let mut seen = HashSet::new();
    let base_domain = base_url.domain().unwrap_or("");

    if let Ok(sel) = Selector::parse("a[href]") {
        for el in doc.select(&sel) {
            if let Some(href) = el.value().attr("href") {
                let href = href.trim();
                if href.is_empty()
                    || href.starts_with('#')
                    || href.starts_with("javascript:")
                    || href.starts_with("mailto:")
                    || href.starts_with("tel:")
                    || href.starts_with("data:")
                {
                    continue;
                }
                let resolved = match base_url.join(href) {
                    Ok(u) => web_normalize_url(&u),
                    Err(_) => continue,
                };
                if seen.contains(&resolved) { continue; }
                seen.insert(resolved.clone());

                let anchor: String = el.text().collect::<Vec<_>>().join(" ");
                let anchor = anchor.trim().to_string();

                let link_type = if web_should_skip_url(&resolved) {
                    WebLinkType::Resource
                } else if let Ok(u) = Url::parse(&resolved) {
                    if u.domain().unwrap_or("") == base_domain {
                        WebLinkType::Internal
                    } else {
                        WebLinkType::External
                    }
                } else {
                    WebLinkType::External
                };

                links.push(WebCategorizedLink { href: resolved, anchor, link_type });
            }
        }
    }
    links
}

// --- Utility ---

fn web_content_hash(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    format!("{:x}", hasher.finalize())[..16].to_string()
}

fn web_truncate_text(text: &str, max_len: usize) -> String {
    if text.len() <= max_len { return text.to_string(); }
    text.char_indices()
        .take_while(|(i, _)| *i < max_len)
        .map(|(_, c)| c)
        .collect::<String>()
        + "…"
}

// --- Output formatting ---

fn web_format_fetch_output(result: &WebFetchResult, opts: &WebExtractionOptions, as_json: bool) -> String {
    if as_json {
        let links_json: Vec<Value> = if opts.include_links {
            result.links.iter().map(|l| json!({
                "href": l.href,
                "anchor": l.anchor,
                "type": match l.link_type { WebLinkType::Internal => "internal", WebLinkType::External => "external", WebLinkType::Resource => "resource" },
            })).collect()
        } else { vec![] };
        let mut obj = json!({
            "url": result.url,
            "title": result.title,
            "content_hash": result.content_hash,
            "text": web_truncate_text(&result.text, opts.max_text_length),
        });
        if opts.include_links { obj["links"] = json!(links_json); }
        if opts.include_metadata {
            obj["metadata"] = json!({
                "description": result.metadata.description,
                "og_title": result.metadata.og_title,
                "og_description": result.metadata.og_description,
                "og_image": result.metadata.og_image,
                "canonical_url": result.metadata.canonical_url,
                "language": result.metadata.language,
            });
        }
        serde_json::to_string_pretty(&obj).unwrap_or_else(|_| format!("{:?}", obj))
    } else {
        let mut out = format!("## {}\n**URL**: {}\n**Hash**: {}\n\n",
            result.title, result.url, result.content_hash);
        if opts.include_metadata {
            let m = &result.metadata;
            if !m.description.is_empty() { out.push_str(&format!("**Description**: {}\n", m.description)); }
            if !m.language.is_empty() { out.push_str(&format!("**Language**: {}\n", m.language)); }
            if !m.canonical_url.is_empty() { out.push_str(&format!("**Canonical**: {}\n", m.canonical_url)); }
            out.push('\n');
        }
        out.push_str(&web_truncate_text(&result.text, opts.max_text_length));
        if opts.include_links && !result.links.is_empty() {
            out.push_str("\n\n---\n### Links\n\n");
            let internal: Vec<_> = result.links.iter().filter(|l| matches!(l.link_type, WebLinkType::Internal)).collect();
            let external: Vec<_> = result.links.iter().filter(|l| matches!(l.link_type, WebLinkType::External)).collect();
            if !internal.is_empty() {
                out.push_str(&format!("**Internal ({}):**\n", internal.len()));
                for l in &internal {
                    let label = if l.anchor.is_empty() { &l.href } else { &l.anchor };
                    out.push_str(&format!("- [{}]({})\n", label, l.href));
                }
            }
            if !external.is_empty() {
                out.push_str(&format!("\n**External ({}):**\n", external.len()));
                for l in &external {
                    let label = if l.anchor.is_empty() { &l.href } else { &l.anchor };
                    out.push_str(&format!("- [{}]({})\n", label, l.href));
                }
            }
        }
        out
    }
}

fn web_format_crawl_output(
    results: &[WebPageResult],
    errors: &[String],
    start_url: &str,
    elapsed_secs: f64,
    as_json: bool,
) -> String {
    let total_links: usize = results.iter().map(|r| r.links.len()).sum();
    if as_json {
        let pages: Vec<Value> = results.iter().map(|r| {
            let links: Vec<Value> = r.links.iter().map(|l| json!({
                "href": l.href, "anchor": l.anchor,
                "type": match l.link_type { WebLinkType::Internal => "internal", WebLinkType::External => "external", WebLinkType::Resource => "resource" },
            })).collect();
            let mut page = json!({
                "url": r.url, "title": r.title, "content_hash": r.content_hash,
                "text_excerpt": r.text_excerpt, "links": links,
            });
            if !r.metadata.language.is_empty() || !r.metadata.description.is_empty() {
                page["metadata"] = json!({
                    "description": r.metadata.description,
                    "language": r.metadata.language,
                    "canonical_url": r.metadata.canonical_url,
                });
            }
            page
        }).collect();
        let obj = json!({
            "start_url": start_url,
            "pages_fetched": results.len(),
            "total_links": total_links,
            "errors": errors.len(),
            "elapsed_seconds": (elapsed_secs * 10.0).round() / 10.0,
            "pages": pages,
            "crawl_errors": errors,
        });
        serde_json::to_string_pretty(&obj).unwrap_or_else(|_| "{}".to_string())
    } else {
        let mut out = format!(
            "## Crawl: {}\n**Pages**: {} | **Links**: {} | **Errors**: {} | **Time**: {:.1}s\n\n",
            start_url, results.len(), total_links, errors.len(), elapsed_secs
        );
        for (i, r) in results.iter().enumerate() {
            out.push_str(&format!("### {}. {} ({})\n{}\n\n", i + 1, r.title, r.url, r.text_excerpt));
        }
        if !results.is_empty() {
            out.push_str("---\n### Link Index\n\n");
            for r in results {
                for l in &r.links {
                    if matches!(l.link_type, WebLinkType::Internal) {
                        let label = if l.anchor.is_empty() { &l.href } else { &l.anchor };
                        out.push_str(&format!("- [{}]({}) ← {}\n", label, l.href, r.url));
                    }
                }
            }
        }
        if !errors.is_empty() {
            out.push_str("\n---\n### Errors\n\n");
            for e in errors { out.push_str(&format!("- {}\n", e)); }
        }
        out
    }
}

// --- Tool entry points ---

async fn tool_fetch_webpage(args: &Value, client: &reqwest::Client) -> Result<ToolOutput, String> {
    let url = args["url"].as_str().ok_or("Missing 'url'")?;
    let extract_links = args["extract_links"].as_bool().unwrap_or(true);
    let extract_metadata = args["extract_metadata"].as_bool().unwrap_or(false);
    let include_images = args["include_images"].as_bool().unwrap_or(false);
    let output_format = args["output_format"].as_str().unwrap_or("text");
    let max_text_length = args["max_text_length"].as_u64().unwrap_or(0) as usize;
    let custom_headers: HashMap<String, String> = args["headers"].as_object()
        .map(|m| m.iter().filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string()))).collect())
        .unwrap_or_default();

    let (html, final_url, _status) = web_fetch_with_retry(client, url, &custom_headers).await?;

    let opts = WebExtractionOptions {
        include_links: extract_links,
        include_metadata: extract_metadata,
        include_images,
        max_text_length: if max_text_length == 0 { usize::MAX } else { max_text_length },
    };

    let text = web_extract_text(&html, &opts);
    let title_sel = Selector::parse("title").ok()
        .and_then(|sel| Html::parse_document(&html).select(&sel).next()
            .map(|el| el.text().collect::<String>()));
    let title = title_sel.unwrap_or_default().trim().to_string();
    let content_hash = web_content_hash(&text);
    let metadata = if extract_metadata { web_extract_metadata(&html, &final_url) } else { WebPageMetadata::default() };
    let links = if extract_links { web_extract_links(&html, &final_url) } else { Vec::new() };

    let result = WebFetchResult {
        url: final_url.to_string(),
        title,
        text,
        metadata,
        links,
        content_hash,
    };

    let output = web_format_fetch_output(&result, &opts, output_format == "json");
    Ok(ToolOutput::text(output))
}

async fn tool_crawl_website(args: &Value, client: &reqwest::Client) -> Result<ToolOutput, String> {
    let start_url = args["url"].as_str().ok_or("Missing 'url'")?;
    let max_depth = (args["max_depth"].as_u64().unwrap_or(1) as u32).min(MAX_CRAWL_DEPTH);
    let max_pages = (args["max_pages"].as_u64().unwrap_or(10) as usize).min(MAX_CRAWL_PAGES);
    let same_domain = args["same_domain_only"].as_bool().unwrap_or(true);
    let path_prefix = args["path_prefix"].as_str().unwrap_or("");
    let respect_robots = args["respect_robots_txt"].as_bool().unwrap_or(true);
    let use_sitemap = args["use_sitemap"].as_bool().unwrap_or(false);
    let concurrent = (args["concurrent_requests"].as_u64().unwrap_or(1) as usize).min(MAX_CONCURRENT);
    let delay_ms = args["delay_ms"].as_u64().unwrap_or(DEFAULT_CRAWL_DELAY_MS);
    let max_total_secs = (args["max_total_seconds"].as_u64().unwrap_or(MAX_TOTAL_CRAWL_SECS)).min(MAX_TOTAL_CRAWL_SECS);
    let output_format = args["output_format"].as_str().unwrap_or("text");
    let max_text_length = args["max_text_length"].as_u64().unwrap_or(2000) as usize;
    let include_metadata = args["include_metadata"].as_bool().unwrap_or(false);
    let custom_headers: HashMap<String, String> = args["headers"].as_object()
        .map(|m| m.iter().filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string()))).collect())
        .unwrap_or_default();
    let exclude_patterns: Vec<String> = args["exclude_patterns"].as_array()
        .map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
        .unwrap_or_default();

    let start_parsed = web_validate_url(start_url)?;
    let start_domain = start_parsed.domain().unwrap_or("").to_string();
    let started = Instant::now();

    // Robots.txt
    let robots = if respect_robots {
        web_fetch_robots(client, &start_parsed).await
    } else {
        None
    };

    // Effective crawl delay
    let effective_delay = if let Some(ref r) = robots {
        r.crawl_delay.map(|d| (d * 1000.0) as u64).unwrap_or(delay_ms)
    } else {
        delay_ms
    };

    // Seed queue
    let mut visited: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<(String, u32)> = VecDeque::new();
    let mut results: Vec<WebPageResult> = Vec::new();
    let mut errors: Vec<String> = Vec::new();
    let mut content_hashes: HashSet<String> = HashSet::new();

    // Sitemap seeding
    if use_sitemap {
        let sitemap_urls = web_fetch_sitemap(client, &start_parsed, &robots).await;
        for su in sitemap_urls {
            if !path_prefix.is_empty() {
                if let Ok(u) = Url::parse(&su) {
                    if !u.path().starts_with(path_prefix) { continue; }
                }
            }
            queue.push_back((su, 0));
        }
    }

    queue.push_back((start_parsed.to_string(), 0));

    let opts = WebExtractionOptions {
        include_links: true,
        include_metadata,
        include_images: false,
        max_text_length,
    };

    while !queue.is_empty() && results.len() < max_pages {
        if started.elapsed().as_secs() > max_total_secs { break; }

        // Batch up to `concurrent` URLs
        let mut batch: Vec<(String, u32)> = Vec::new();
        while batch.len() < concurrent {
            if let Some((url, depth)) = queue.pop_front() {
                let normalized = if let Ok(u) = Url::parse(&url) { web_normalize_url(&u) } else { url.clone() };
                if visited.contains(&normalized) { continue; }
                visited.insert(normalized.clone());
                batch.push((normalized, depth));
            } else {
                break;
            }
        }
        if batch.is_empty() { break; }

        if concurrent > 1 && batch.len() > 1 {
            // Concurrent fetch
            let mut join_set = JoinSet::new();
            for (url, depth) in batch {
                let client = client.clone();
                let headers = custom_headers.clone();
                join_set.spawn(async move {
                    let result = web_fetch_with_retry(&client, &url, &headers).await;
                    (url, depth, result)
                });
            }
            while let Some(res) = join_set.join_next().await {
                if let Ok((url, depth, fetch_result)) = res {
                    match fetch_result {
                        Ok((html, final_url, _)) => {
                            let pr = web_process_page(&html, &final_url, &url, max_text_length, &opts);
                            if content_hashes.contains(&pr.content_hash) { continue; }
                            content_hashes.insert(pr.content_hash.clone());

                            // Enqueue discovered links
                            if depth < max_depth {
                                for link in &pr.links {
                                    if !matches!(link.link_type, WebLinkType::Internal) && same_domain { continue; }
                                    if web_should_skip_url(&link.href) { continue; }
                                    if !path_prefix.is_empty() {
                                        if let Ok(u) = Url::parse(&link.href) {
                                            if !u.path().starts_with(path_prefix) { continue; }
                                        }
                                    }
                                    if !exclude_patterns.is_empty() && exclude_patterns.iter().any(|p| link.href.contains(p)) { continue; }
                                    if same_domain {
                                        if let Ok(u) = Url::parse(&link.href) {
                                            if u.domain().unwrap_or("") != start_domain { continue; }
                                        }
                                    }
                                    if let Some(ref r) = robots {
                                        if let Ok(u) = Url::parse(&link.href) {
                                            if !web_is_allowed_by_robots(r, u.path()) { continue; }
                                        }
                                    }
                                    queue.push_back((link.href.clone(), depth + 1));
                                }
                            }
                            results.push(pr);
                        }
                        Err(e) => errors.push(e),
                    }
                }
            }
        } else {
            // Sequential fetch
            for (url, depth) in batch {
                if started.elapsed().as_secs() > max_total_secs || results.len() >= max_pages { break; }

                if let Some(ref r) = robots {
                    if let Ok(u) = Url::parse(&url) {
                        if !web_is_allowed_by_robots(r, u.path()) { continue; }
                    }
                }

                match web_fetch_with_retry(client, &url, &custom_headers).await {
                    Ok((html, final_url, _)) => {
                        let pr = web_process_page(&html, &final_url, &url, max_text_length, &opts);
                        if content_hashes.contains(&pr.content_hash) { continue; }
                        content_hashes.insert(pr.content_hash.clone());

                        if depth < max_depth {
                            for link in &pr.links {
                                if !matches!(link.link_type, WebLinkType::Internal) && same_domain { continue; }
                                if web_should_skip_url(&link.href) { continue; }
                                if !path_prefix.is_empty() {
                                    if let Ok(u) = Url::parse(&link.href) {
                                        if !u.path().starts_with(path_prefix) { continue; }
                                    }
                                }
                                if !exclude_patterns.is_empty() && exclude_patterns.iter().any(|p| link.href.contains(p)) { continue; }
                                if same_domain {
                                    if let Ok(u) = Url::parse(&link.href) {
                                        if u.domain().unwrap_or("") != start_domain { continue; }
                                    }
                                }
                                if let Some(ref r) = robots {
                                    if let Ok(u) = Url::parse(&link.href) {
                                        if !web_is_allowed_by_robots(r, u.path()) { continue; }
                                    }
                                }
                                queue.push_back((link.href.clone(), depth + 1));
                            }
                        }
                        results.push(pr);
                    }
                    Err(e) => errors.push(e),
                }

                if effective_delay > 0 {
                    tokio::time::sleep(Duration::from_millis(effective_delay)).await;
                }
            }
        }
    }

    let elapsed = started.elapsed().as_secs_f64();
    let output = web_format_crawl_output(&results, &errors, start_url, elapsed, output_format == "json");
    Ok(ToolOutput::text(output))
}

fn web_process_page(
    html: &str,
    final_url: &Url,
    _original_url: &str,
    max_text_length: usize,
    opts: &WebExtractionOptions,
) -> WebPageResult {
    let text = web_extract_text(html, opts);
    let title = Selector::parse("title").ok()
        .and_then(|sel| Html::parse_document(html).select(&sel).next()
            .map(|el| el.text().collect::<String>()))
        .unwrap_or_default()
        .trim()
        .to_string();
    let content_hash = web_content_hash(&text);
    let excerpt = web_truncate_text(&text, max_text_length);
    let metadata = if opts.include_metadata { web_extract_metadata(html, final_url) } else { WebPageMetadata::default() };
    let links = web_extract_links(html, final_url);

    WebPageResult { url: final_url.to_string(), title, text_excerpt: excerpt, content_hash, metadata, links }
}
