// backend/src/tools.rs
//! Tool execution module for Gemini function calling.
//!
//! Provides 4 local tools that Gemini agents can invoke:
//! - `execute_command` — run shell commands with timeout + safety filters
//! - `read_file` — read file contents (reuses files::read_file_for_context)
//! - `write_file` — create/overwrite files with size + path restrictions
//! - `list_directory` — list directory contents (reuses files::list_directory)

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
    "rm -rf /", // Still dangerous if volume mounted
    "format c:",
    ":(){:|:&};:",
    "dd if=/dev/zero",
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
// get_code_structure
// ---------------------------------------------------------------------------

async fn tool_get_code_structure(path: &str) -> Result<String, String> {
    // Read file (reuse context reader for safety/limits)
    let ctx = crate::files::read_file_for_context(path)
        .await
        .map_err(|e| format!("Cannot read file '{}': {}", e.path, e.reason))?;

    // Analyze
    if let Some(structure) = crate::analysis::analyze_file(&ctx.path, &ctx.content) {
        let mut out = format!("### Code Structure: {}\n", ctx.path);
        if structure.symbols.is_empty() {
            out.push_str("(No symbols detected or language not supported)");
        } else {
            for sym in structure.symbols {
                out.push_str(&format!("- [L{}] {} {}\n", sym.line, sym.kind, sym.name));
                // Optional: include signature if short?
                // out.push_str(&format!("  `{}`\n", sym.signature));
            }
        }
        Ok(out)
    } else {
        Ok(format!("Could not analyze structure for '{}' (unsupported language?)", path))
    }
}
