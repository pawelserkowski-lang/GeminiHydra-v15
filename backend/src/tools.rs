// backend/src/tools.rs
//! Tool execution module for Gemini function calling.
//!
//! Provides 6 local tools that Gemini agents can invoke:
//! - `execute_command` — run shell commands with timeout + safety filters
//! - `read_file` — read file contents (reuses files::read_file_for_context)
//! - `write_file` — create/overwrite files with size + path restrictions
//! - `list_directory` — list directory contents (reuses files::list_directory)
//! - `search_files` — search for text/regex patterns across files in a directory
//! - `get_code_structure` — analyze code AST without full read

use regex::Regex;
use serde_json::Value;
use std::time::Duration;
use tokio::process::Command;
use crate::state::AppState;

/// Max output bytes from a single command (50 KB).
const MAX_COMMAND_OUTPUT: usize = 50 * 1024;

/// Command execution timeout.
const COMMAND_TIMEOUT: Duration = Duration::from_secs(30);

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
pub async fn execute_tool(name: &str, args: &Value, state: &AppState) -> Result<String, String> {
    match name {
        "execute_command" => {
            let command = args["command"]
                .as_str()
                .ok_or("Missing required argument: command")?;
            tool_execute_command(command, state).await
        }
        "read_file" => {
            let path = args["path"]
                .as_str()
                .ok_or("Missing required argument: path")?;
            tool_read_file(path).await
        }
        "write_file" => {
            let path = args["path"]
                .as_str()
                .ok_or("Missing required argument: path")?;
            let content = args["content"]
                .as_str()
                .ok_or("Missing required argument: content")?;
            tool_write_file(path, content).await
        }
        "list_directory" => {
            let path = args["path"]
                .as_str()
                .ok_or("Missing required argument: path")?;
            let show_hidden = args["show_hidden"].as_bool().unwrap_or(false);
            tool_list_directory(path, show_hidden).await
        }
        "search_files" => {
            let path = args["path"]
                .as_str()
                .ok_or("Missing required argument: path")?;
            let pattern = args["pattern"]
                .as_str()
                .ok_or("Missing required argument: pattern")?;
            let extensions = args["file_extensions"].as_str();
            tool_search_files(path, pattern, extensions).await
        }
        "get_code_structure" => {
            let path = args["path"]
                .as_str()
                .ok_or("Missing required argument: path")?;
            tool_get_code_structure(path).await
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
            lines.push(format!("  {:>8}  {}", size, entry.name));
        }
    }

    Ok(format!("Directory: {}\n{}", path, lines.join("\n")))
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
const MAX_SEARCH_RESULTS: usize = 80;

/// Max directory depth for recursive search.
const MAX_SEARCH_DEPTH: usize = 8;

/// Directories to skip during search.
const SKIP_DIRS: &[&str] = &[
    "node_modules", "target", "dist", "build", ".git", "__pycache__",
    ".next", ".nuxt", "vendor", ".cache", "coverage", ".turbo",
];

async fn tool_search_files(
    path: &str,
    pattern: &str,
    extensions: Option<&str>,
) -> Result<String, String> {
    let dir = std::path::Path::new(path);
    if !dir.is_dir() {
        return Err(format!("'{}' is not a directory", path));
    }

    // Build regex: try as regex first, fall back to literal escape
    let re = Regex::new(&format!("(?i){}", pattern))
        .or_else(|_| Regex::new(&format!("(?i){}", regex::escape(pattern))))
        .map_err(|e| format!("Invalid pattern '{}': {}", pattern, e))?;

    // Parse extension filter
    let ext_filter: Option<Vec<String>> = extensions.map(|e| {
        e.split(',')
            .map(|s| s.trim().trim_start_matches('.').to_lowercase())
            .filter(|s| !s.is_empty())
            .collect()
    });

    let mut results = Vec::new();
    let mut files_searched: usize = 0;
    let mut stack: Vec<(std::path::PathBuf, usize)> = vec![(dir.to_path_buf(), 0)];

    while let Some((current_dir, depth)) = stack.pop() {
        if depth > MAX_SEARCH_DEPTH || results.len() >= MAX_SEARCH_RESULTS {
            break;
        }

        let mut entries = match tokio::fs::read_dir(&current_dir).await {
            Ok(e) => e,
            Err(_) => continue,
        };

        while let Ok(Some(entry)) = entries.next_entry().await {
            if results.len() >= MAX_SEARCH_RESULTS {
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
                if !crate::files::is_text_extension(&ext) {
                    continue;
                }

                // Read and search
                if let Ok(content) = tokio::fs::read_to_string(&entry_path).await {
                    files_searched += 1;
                    for (line_num, line) in content.lines().enumerate() {
                        if results.len() >= MAX_SEARCH_RESULTS {
                            break;
                        }
                        if re.is_match(line) {
                            let trimmed = line.trim();
                            // Truncate very long lines
                            let display = if trimmed.len() > 200 {
                                format!("{}...", &trimmed[..200])
                            } else {
                                trimmed.to_string()
                            };
                            results.push(format!(
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

    if results.is_empty() {
        Ok(format!(
            "No matches found for '{}' in '{}' ({} files searched)",
            pattern, path, files_searched
        ))
    } else {
        let truncated = if results.len() >= MAX_SEARCH_RESULTS {
            format!(" (truncated at {} results)", MAX_SEARCH_RESULTS)
        } else {
            String::new()
        };
        Ok(format!(
            "Found {} match(es) for '{}' in '{}' ({} files searched){}:\n\n{}",
            results.len(),
            pattern,
            path,
            files_searched,
            truncated,
            results.join("\n")
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
