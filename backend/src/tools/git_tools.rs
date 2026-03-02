// tools/git_tools.rs
// Jaskier Shared Pattern -- git_tools
//! Git operations tools for agent function calling.
//! Uses shell commands via tokio::process::Command (no C dependency).

use std::path::Path;
use std::time::Duration;
use tokio::process::Command;

/// Git command timeout.
const GIT_TIMEOUT: Duration = Duration::from_secs(15);

/// Maximum output length.
const MAX_OUTPUT_CHARS: usize = 6000;

/// Run a git command in the given repo path, return stdout.
async fn run_git(repo_path: &str, args: &[&str]) -> Result<String, String> {
    let repo = Path::new(repo_path);

    // Validate repo exists
    if !repo.exists() || !repo.is_dir() {
        return Err(format!("Not a directory: {}", repo_path));
    }

    // Validate it's a git repo
    if !repo.join(".git").exists() && !repo.join("HEAD").exists() {
        return Err(format!("Not a git repository: {}", repo_path));
    }

    let output = tokio::time::timeout(GIT_TIMEOUT, async {
        Command::new("git")
            .args(args)
            .current_dir(repo)
            .output()
            .await
            .map_err(|e| format!("Failed to execute git: {}", e))
    })
    .await
    .map_err(|_| "Git command timed out (15s)".to_string())??;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        if stdout.len() > MAX_OUTPUT_CHARS {
            let truncated: String = stdout.chars().take(MAX_OUTPUT_CHARS - 40).collect();
            Ok(format!("{}\n\n[... truncated ...]", truncated))
        } else {
            Ok(stdout)
        }
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        Err(format!("git error: {}", stderr))
    }
}

/// Show working tree status.
pub async fn tool_git_status(repo_path: &str) -> Result<String, String> {
    let porcelain = run_git(repo_path, &["status", "--porcelain=v2", "--branch"]).await?;
    let readable = run_git(repo_path, &["status", "--short", "--branch"]).await?;
    Ok(format!(
        "### Git Status: {}\n\n{}\n\n### Porcelain v2:\n{}",
        repo_path, readable, porcelain
    ))
}

/// Show commit history.
pub async fn tool_git_log(repo_path: &str, count: Option<u32>) -> Result<String, String> {
    let n = count.unwrap_or(20).min(50);
    let n_str = format!("-{}", n);
    let log = run_git(
        repo_path,
        &[
            "log",
            &n_str,
            "--oneline",
            "--graph",
            "--decorate",
            "--date=short",
        ],
    )
    .await?;
    Ok(format!(
        "### Git Log (last {} commits): {}\n\n{}",
        n, repo_path, log
    ))
}

/// Show changes (diff).
pub async fn tool_git_diff(repo_path: &str, target: Option<&str>) -> Result<String, String> {
    let args: Vec<&str> = match target {
        Some("staged") | Some("--staged") => vec!["diff", "--staged", "--stat"],
        Some("--stat") => vec!["diff", "--stat"],
        Some(t) => vec!["diff", t],
        None => vec!["diff", "--stat"],
    };
    let diff = run_git(repo_path, &args).await?;
    let label = target.unwrap_or("working tree");
    Ok(format!(
        "### Git Diff ({}): {}\n\n{}",
        label, repo_path, diff
    ))
}

/// List, create, or switch branches.
pub async fn tool_git_branch(repo_path: &str, action: Option<&str>) -> Result<String, String> {
    match action.unwrap_or("list") {
        "list" => {
            let branches = run_git(
                repo_path,
                &[
                    "branch",
                    "-a",
                    "--format=%(HEAD) %(refname:short) %(upstream:short) %(objectname:short)",
                ],
            )
            .await?;
            Ok(format!(
                "### Git Branches: {}\n\n{}",
                repo_path, branches
            ))
        }
        a if a.starts_with("create:") => {
            let name = a.strip_prefix("create:").unwrap_or("").trim();
            if name.is_empty() {
                return Err("Branch name required. Usage: create:branch-name".into());
            }
            // Validate branch name
            if name.contains(' ') || name.contains("..") || name.starts_with('-') {
                return Err(format!("Invalid branch name: {}", name));
            }
            let result = run_git(repo_path, &["checkout", "-b", name]).await?;
            Ok(format!(
                "### Created and switched to branch: {}\n\n{}",
                name, result
            ))
        }
        a if a.starts_with("switch:") => {
            let name = a.strip_prefix("switch:").unwrap_or("").trim();
            if name.is_empty() {
                return Err("Branch name required. Usage: switch:branch-name".into());
            }
            let result = run_git(repo_path, &["checkout", name]).await?;
            Ok(format!(
                "### Switched to branch: {}\n\n{}",
                name, result
            ))
        }
        other => Err(format!(
            "Unknown branch action: '{}'. Use 'list', 'create:name', or 'switch:name'",
            other
        )),
    }
}

/// Stage files and commit changes. NO PUSH — too dangerous for agent tools.
pub async fn tool_git_commit(
    repo_path: &str,
    message: &str,
    files: Option<&str>,
) -> Result<String, String> {
    if message.is_empty() {
        return Err("Commit message cannot be empty".into());
    }

    // Stage files
    match files {
        Some("all") | Some(".") => {
            run_git(repo_path, &["add", "-A"]).await?;
        }
        Some(file_list) => {
            for file in file_list.split(',') {
                let file = file.trim();
                if !file.is_empty() {
                    run_git(repo_path, &["add", file]).await?;
                }
            }
        }
        None => {
            // Check if anything is staged
            let staged = run_git(repo_path, &["diff", "--staged", "--name-only"]).await?;
            if staged.trim().is_empty() {
                return Err("Nothing staged for commit. Use 'files' parameter to stage files (e.g., 'all' or 'file1.rs,file2.rs')".into());
            }
        }
    }

    // Commit
    let result = run_git(repo_path, &["commit", "-m", message]).await?;
    Ok(format!("### Git Commit\n\n{}", result))
}
