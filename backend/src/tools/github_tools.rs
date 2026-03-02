// Jaskier Shared Pattern — GitHub Tools
// Agent tools for GitHub API interactions.
// Reads token from gh_oauth_github table via oauth_github module.

use serde_json::{json, Value};

use crate::oauth_github;
use crate::state::AppState;

const GITHUB_API_BASE: &str = "https://api.github.com";
const USER_AGENT: &str = "GeminiHydra/15.0";

/// Validate that a user-provided GitHub parameter (owner, repo, etc.)
/// does not contain characters that could cause URL injection.
fn validate_github_param<'a>(param: &'a str, name: &str) -> Result<&'a str, String> {
    if param.contains('/')
        || param.contains('\\')
        || param.contains(' ')
        || param.contains('?')
        || param.contains('#')
    {
        return Err(format!(
            "Invalid GitHub {}: contains special characters",
            name
        ));
    }
    Ok(param)
}

// ═══════════════════════════════════════════════════════════════════════
//  Tool execution
// ═══════════════════════════════════════════════════════════════════════

pub async fn execute(
    tool_name: &str,
    input: &Value,
    state: &AppState,
) -> Result<String, String> {
    let token = match oauth_github::get_github_access_token(state).await {
        Some(t) => t,
        None => {
            return Err(
                "GitHub not authenticated. Please connect your GitHub account via Settings > GitHub OAuth.".to_string(),
            )
        }
    };

    let client = &state.client;

    match tool_name {
        "github_list_repos" => exec_list_repos(client, &token, input).await,
        "github_get_repo" => exec_get_repo(client, &token, input).await,
        "github_list_issues" => exec_list_issues(client, &token, input).await,
        "github_get_issue" => exec_get_issue(client, &token, input).await,
        "github_create_issue" => exec_create_issue(client, &token, input).await,
        "github_create_pr" => exec_create_pr(client, &token, input).await,
        _ => Err(format!("Unknown GitHub tool: {}", tool_name)),
    }
}

// ── Individual tool implementations ──────────────────────────────────────

async fn exec_list_repos(
    client: &reqwest::Client,
    token: &str,
    input: &Value,
) -> Result<String, String> {
    let sort = input
        .get("sort")
        .and_then(|v| v.as_str())
        .unwrap_or("updated");
    let per_page = input
        .get("per_page")
        .and_then(|v| v.as_u64())
        .unwrap_or(30)
        .min(100);

    let url = format!("{}/user/repos?sort={}&per_page={}", GITHUB_API_BASE, sort, per_page);

    let body = github_get(client, token, &url).await?;
    let repos = body
        .as_array()
        .map(|arr| {
            arr.iter()
                .map(|r| {
                    json!({
                        "full_name": r.get("full_name"),
                        "description": r.get("description"),
                        "language": r.get("language"),
                        "stargazers_count": r.get("stargazers_count"),
                        "private": r.get("private"),
                        "updated_at": r.get("updated_at"),
                        "html_url": r.get("html_url"),
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Ok(serde_json::to_string_pretty(&repos).unwrap_or_else(|_| "[]".to_string()))
}

async fn exec_get_repo(
    client: &reqwest::Client,
    token: &str,
    input: &Value,
) -> Result<String, String> {
    let owner = input.get("owner").and_then(|v| v.as_str()).unwrap_or("");
    let repo = input.get("repo").and_then(|v| v.as_str()).unwrap_or("");

    if owner.is_empty() || repo.is_empty() {
        return Err("owner and repo are required".to_string());
    }

    let owner = validate_github_param(owner, "owner")?;
    let repo = validate_github_param(repo, "repo")?;

    let url = format!("{}/repos/{}/{}", GITHUB_API_BASE, owner, repo);

    let body = github_get(client, token, &url).await?;
    let summary = json!({
        "full_name": body.get("full_name"),
        "description": body.get("description"),
        "language": body.get("language"),
        "stargazers_count": body.get("stargazers_count"),
        "forks_count": body.get("forks_count"),
        "open_issues_count": body.get("open_issues_count"),
        "default_branch": body.get("default_branch"),
        "private": body.get("private"),
        "html_url": body.get("html_url"),
        "created_at": body.get("created_at"),
        "updated_at": body.get("updated_at"),
        "topics": body.get("topics"),
    });
    Ok(serde_json::to_string_pretty(&summary).unwrap_or_else(|_| "{}".to_string()))
}

async fn exec_list_issues(
    client: &reqwest::Client,
    token: &str,
    input: &Value,
) -> Result<String, String> {
    let owner = input.get("owner").and_then(|v| v.as_str()).unwrap_or("");
    let repo = input.get("repo").and_then(|v| v.as_str()).unwrap_or("");
    let state_filter = input
        .get("state")
        .and_then(|v| v.as_str())
        .unwrap_or("open");

    if owner.is_empty() || repo.is_empty() {
        return Err("owner and repo are required".to_string());
    }

    let owner = validate_github_param(owner, "owner")?;
    let repo = validate_github_param(repo, "repo")?;

    let url = format!(
        "{}/repos/{}/{}/issues?state={}&per_page=30",
        GITHUB_API_BASE, owner, repo, state_filter
    );

    let body = github_get(client, token, &url).await?;
    let issues = body
        .as_array()
        .map(|arr| {
            arr.iter()
                .map(|i| {
                    json!({
                        "number": i.get("number"),
                        "title": i.get("title"),
                        "state": i.get("state"),
                        "user": i.get("user").and_then(|u| u.get("login")),
                        "labels": i.get("labels").and_then(|l| l.as_array()).map(|arr| {
                            arr.iter().filter_map(|l| l.get("name")).collect::<Vec<_>>()
                        }),
                        "created_at": i.get("created_at"),
                        "comments": i.get("comments"),
                        "html_url": i.get("html_url"),
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Ok(serde_json::to_string_pretty(&issues).unwrap_or_else(|_| "[]".to_string()))
}

async fn exec_get_issue(
    client: &reqwest::Client,
    token: &str,
    input: &Value,
) -> Result<String, String> {
    let owner = input.get("owner").and_then(|v| v.as_str()).unwrap_or("");
    let repo = input.get("repo").and_then(|v| v.as_str()).unwrap_or("");
    let number = input.get("number").and_then(|v| v.as_u64()).unwrap_or(0);

    if owner.is_empty() || repo.is_empty() || number == 0 {
        return Err("owner, repo, and number are required".to_string());
    }

    let owner = validate_github_param(owner, "owner")?;
    let repo = validate_github_param(repo, "repo")?;

    let issue_url = format!(
        "{}/repos/{}/{}/issues/{}",
        GITHUB_API_BASE, owner, repo, number
    );
    let comments_url = format!(
        "{}/repos/{}/{}/issues/{}/comments",
        GITHUB_API_BASE, owner, repo, number
    );

    let (issue_result, comments_result) = tokio::join!(
        github_get(client, token, &issue_url),
        github_get(client, token, &comments_url)
    );

    let issue = issue_result?;

    let comments = match comments_result {
        Ok(body) => body
            .as_array()
            .map(|arr| {
                arr.iter()
                    .map(|c| {
                        json!({
                            "user": c.get("user").and_then(|u| u.get("login")),
                            "body": c.get("body"),
                            "created_at": c.get("created_at"),
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default(),
        Err(_) => vec![],
    };

    let result = json!({
        "number": issue.get("number"),
        "title": issue.get("title"),
        "state": issue.get("state"),
        "body": issue.get("body"),
        "user": issue.get("user").and_then(|u| u.get("login")),
        "labels": issue.get("labels"),
        "created_at": issue.get("created_at"),
        "html_url": issue.get("html_url"),
        "comments": comments,
    });

    Ok(serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".to_string()))
}

async fn exec_create_issue(
    client: &reqwest::Client,
    token: &str,
    input: &Value,
) -> Result<String, String> {
    let owner = input.get("owner").and_then(|v| v.as_str()).unwrap_or("");
    let repo = input.get("repo").and_then(|v| v.as_str()).unwrap_or("");
    let title = input.get("title").and_then(|v| v.as_str()).unwrap_or("");
    let body = input.get("body").and_then(|v| v.as_str()).unwrap_or("");

    if owner.is_empty() || repo.is_empty() || title.is_empty() {
        return Err("owner, repo, and title are required".to_string());
    }

    let owner = validate_github_param(owner, "owner")?;
    let repo = validate_github_param(repo, "repo")?;

    let url = format!("{}/repos/{}/{}/issues", GITHUB_API_BASE, owner, repo);

    let resp = github_post(client, token, &url, &json!({ "title": title, "body": body })).await?;
    let result = json!({
        "number": resp.get("number"),
        "title": resp.get("title"),
        "html_url": resp.get("html_url"),
        "state": resp.get("state"),
    });
    Ok(serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".to_string()))
}

async fn exec_create_pr(
    client: &reqwest::Client,
    token: &str,
    input: &Value,
) -> Result<String, String> {
    let owner = input.get("owner").and_then(|v| v.as_str()).unwrap_or("");
    let repo = input.get("repo").and_then(|v| v.as_str()).unwrap_or("");
    let title = input.get("title").and_then(|v| v.as_str()).unwrap_or("");
    let body = input.get("body").and_then(|v| v.as_str()).unwrap_or("");
    let head = input.get("head").and_then(|v| v.as_str()).unwrap_or("");
    let base = input.get("base").and_then(|v| v.as_str()).unwrap_or("main");

    if owner.is_empty() || repo.is_empty() || title.is_empty() || head.is_empty() {
        return Err("owner, repo, title, and head are required".to_string());
    }

    let owner = validate_github_param(owner, "owner")?;
    let repo = validate_github_param(repo, "repo")?;

    let url = format!("{}/repos/{}/{}/pulls", GITHUB_API_BASE, owner, repo);

    let resp = github_post(
        client,
        token,
        &url,
        &json!({
            "title": title,
            "body": body,
            "head": head,
            "base": base,
        }),
    )
    .await?;
    let result = json!({
        "number": resp.get("number"),
        "title": resp.get("title"),
        "html_url": resp.get("html_url"),
        "state": resp.get("state"),
    });
    Ok(serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".to_string()))
}

// ── HTTP helpers ─────────────────────────────────────────────────────────

async fn github_get(
    client: &reqwest::Client,
    token: &str,
    url: &str,
) -> Result<Value, String> {
    let resp = client
        .get(url)
        .header("authorization", format!("Bearer {}", token))
        .header("accept", "application/vnd.github+json")
        .header("user-agent", USER_AGENT)
        .header("x-github-api-version", "2022-11-28")
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| format!("GitHub API request failed: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_else(|e| format!("(failed to read response body: {})", e));
        return Err(format!("GitHub API error {}: {}", status, body));
    }

    resp.json::<Value>()
        .await
        .map_err(|e| format!("Failed to parse GitHub response: {}", e))
}

async fn github_post(
    client: &reqwest::Client,
    token: &str,
    url: &str,
    body: &Value,
) -> Result<Value, String> {
    let resp = client
        .post(url)
        .header("authorization", format!("Bearer {}", token))
        .header("accept", "application/vnd.github+json")
        .header("user-agent", USER_AGENT)
        .header("x-github-api-version", "2022-11-28")
        .json(body)
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| format!("GitHub API request failed: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_else(|e| format!("(failed to read response body: {})", e));
        return Err(format!("GitHub API error {}: {}", status, body));
    }

    resp.json::<Value>()
        .await
        .map_err(|e| format!("Failed to parse GitHub response: {}", e))
}
