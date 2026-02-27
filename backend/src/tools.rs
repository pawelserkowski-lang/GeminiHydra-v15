// backend/src/tools.rs
//! Tool execution module for Gemini function calling.
//!
//! Provides 10 local tools that Gemini agents can invoke:
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

use regex::Regex;
use serde_json::Value;
use std::time::Duration;
use tokio::process::Command;
use crate::state::AppState;

/// Max output bytes from a single command (50 KB).
const MAX_COMMAND_OUTPUT: usize = 50 * 1024;

/// Command execution timeout.
const COMMAND_TIMEOUT: Duration = Duration::from_secs(30);

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

/// Central dispatcher — routes tool call to the appropriate handler.
/// Returns `ToolOutput` supporting text + optional multimodal data (Gemini 3).
pub async fn execute_tool(name: &str, args: &Value, state: &AppState) -> Result<ToolOutput, String> {
    match name {
        "execute_command" => {
            let command = args["command"]
                .as_str()
                .ok_or("Missing required argument: command")?;
            tool_execute_command(command, state).await.map(ToolOutput::text)
        }
        "read_file" => {
            let path = args["path"]
                .as_str()
                .ok_or("Missing required argument: path")?;
            tool_read_file(path).await.map(ToolOutput::text)
        }
        "write_file" => {
            let path = args["path"]
                .as_str()
                .ok_or("Missing required argument: path")?;
            let content = args["content"]
                .as_str()
                .ok_or("Missing required argument: content")?;
            tool_write_file(path, content).await.map(ToolOutput::text)
        }
        "edit_file" => {
            let path = args["path"]
                .as_str()
                .ok_or("Missing required argument: path")?;
            let old_text = args["old_text"]
                .as_str()
                .ok_or("Missing required argument: old_text")?;
            let new_text = args["new_text"]
                .as_str()
                .ok_or("Missing required argument: new_text")?;
            tool_edit_file(path, old_text, new_text).await.map(ToolOutput::text)
        }
        "list_directory" => {
            let path = args["path"]
                .as_str()
                .ok_or("Missing required argument: path")?;
            let show_hidden = args["show_hidden"].as_bool().unwrap_or(false);
            tool_list_directory(path, show_hidden).await.map(ToolOutput::text)
        }
        "search_files" => {
            let path = args["path"]
                .as_str()
                .ok_or("Missing required argument: path")?;
            let pattern = args["pattern"]
                .as_str()
                .ok_or("Missing required argument: pattern")?;
            let extensions = args["file_extensions"].as_str();
            let offset = args["offset"].as_u64().unwrap_or(0) as usize;
            let limit = args["limit"].as_u64().unwrap_or(80) as usize;
            let multiline = args["multiline"].as_bool().unwrap_or(false);
            tool_search_files(path, pattern, extensions, offset, limit, multiline).await.map(ToolOutput::text)
        }
        "get_code_structure" => {
            let path = args["path"]
                .as_str()
                .ok_or("Missing required argument: path")?;
            tool_get_code_structure(path).await.map(ToolOutput::text)
        }
        "read_file_section" => {
            let path = args["path"]
                .as_str()
                .ok_or("Missing required argument: path")?;
            let start_line = args["start_line"]
                .as_u64()
                .ok_or("Missing required argument: start_line")? as usize;
            let end_line = args["end_line"]
                .as_u64()
                .ok_or("Missing required argument: end_line")? as usize;
            tool_read_file_section(path, start_line, end_line).await.map(ToolOutput::text)
        }
        "find_file" => {
            let path = args["path"]
                .as_str()
                .ok_or("Missing required argument: path")?;
            let pattern = args["pattern"]
                .as_str()
                .ok_or("Missing required argument: pattern")?;
            tool_find_file(path, pattern).await.map(ToolOutput::text)
        }
        "diff_files" => {
            let path_a = args["path_a"]
                .as_str()
                .ok_or("Missing required argument: path_a")?;
            let path_b = args["path_b"]
                .as_str()
                .ok_or("Missing required argument: path_b")?;
            tool_diff_files(path_a, path_b).await.map(ToolOutput::text)
        }
        _ => Err(format!("Unknown tool: {}", name)),
    }
}

// ---------------------------------------------------------------------------
// execute_command
// ---------------------------------------------------------------------------

async fn tool_execute_command(command: &str, state: &AppState) -> Result<String, String> {
    let lower = command.to_lowercase();
    for pattern in BLOCKED_PATTERNS {
        if lower.contains(pattern) {
            return Err(format!("Blocked dangerous command pattern: {}", pattern));
        }
    }

    // Check sandbox setting
    let use_sandbox = sqlx::query_scalar::<_, bool>("SELECT use_docker_sandbox FROM gh_settings WHERE id = 1")
        .fetch_one(&state.db)
        .await
        .unwrap_or(false);

    let output_res = if use_sandbox {
        // Run in Docker
        // Mount current directory to /app
        let current_dir = std::env::current_dir()
            .map_err(|e| format!("Cannot determines current dir: {}", e))?
            .to_string_lossy()
            .to_string();

        let docker_args = [
            "run",
            "--rm",
            "-v", &format!("{}:/app", current_dir),
            "-w", "/app",
            "alpine:latest", // Lightweight base image
            "sh", "-c", command
        ];

        tokio::time::timeout(COMMAND_TIMEOUT, Command::new("docker").args(docker_args).output()).await
    } else {
        // Run locally
        tokio::time::timeout(COMMAND_TIMEOUT, run_command(command)).await
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

async fn run_command(command: &str) -> std::io::Result<std::process::Output> {
    if cfg!(target_os = "windows") {
        Command::new("cmd").args(["/C", command]).output().await
    } else {
        Command::new("sh").args(["-c", command]).output().await
    }
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
        return Err(format!(
            "old_text not found in {}. Make sure the text matches exactly (including whitespace).",
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
