// Jaskier Shared Pattern — Vercel Tools
// Agent tools for Vercel API interactions.
// Reads token from gh_oauth_vercel table via oauth_vercel module.

use serde_json::{Value, json};

use crate::oauth_vercel;
use crate::state::AppState;

const VERCEL_API_BASE: &str = "https://api.vercel.com";

// ═══════════════════════════════════════════════════════════════════════
//  Tool execution
// ═══════════════════════════════════════════════════════════════════════

pub async fn execute(tool_name: &str, input: &Value, state: &AppState) -> Result<String, String> {
    let (token, team_id) = match oauth_vercel::get_vercel_access_token(state).await {
        Some(t) => t,
        None => {
            return Err(
                "Vercel not authenticated. Please connect your Vercel account via Settings > Vercel OAuth.".to_string(),
            )
        }
    };

    let client = &state.client;

    match tool_name {
        "vercel_list_projects" => {
            exec_list_projects(client, &token, team_id.as_deref(), input).await
        }
        "vercel_get_deployment" => {
            exec_get_deployment(client, &token, team_id.as_deref(), input).await
        }
        "vercel_deploy" => exec_deploy(client, &token, team_id.as_deref(), input).await,
        _ => Err(format!("Unknown Vercel tool: {}", tool_name)),
    }
}

// ── Individual tool implementations ──────────────────────────────────────

async fn exec_list_projects(
    client: &reqwest::Client,
    token: &str,
    team_id: Option<&str>,
    input: &Value,
) -> Result<String, String> {
    let limit = input
        .get("limit")
        .and_then(|v| v.as_u64())
        .unwrap_or(20)
        .min(100);

    let mut url = format!("{}/v9/projects?limit={}", VERCEL_API_BASE, limit);
    if let Some(tid) = team_id {
        url.push_str(&format!("&teamId={}", tid));
    }

    let body = vercel_get(client, token, &url).await?;
    let projects = body
        .get("projects")
        .and_then(|p| p.as_array())
        .map(|arr| {
            arr.iter()
                .map(|p| {
                    json!({
                        "name": p.get("name"),
                        "id": p.get("id"),
                        "framework": p.get("framework"),
                        "updated_at": p.get("updatedAt"),
                        "latest_deployments": p.get("latestDeployments").and_then(|d| d.as_array()).map(|arr| {
                            arr.iter().take(1).map(|d| {
                                json!({
                                    "id": d.get("id"),
                                    "state": d.get("state"),
                                    "url": d.get("url"),
                                    "created_at": d.get("createdAt"),
                                })
                            }).collect::<Vec<_>>()
                        }),
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Ok(serde_json::to_string_pretty(&projects).unwrap_or_else(|_| "[]".to_string()))
}

async fn exec_get_deployment(
    client: &reqwest::Client,
    token: &str,
    team_id: Option<&str>,
    input: &Value,
) -> Result<String, String> {
    let deployment_id = input
        .get("deployment_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if deployment_id.is_empty() {
        return Err("deployment_id is required".to_string());
    }

    let mut url = format!("{}/v13/deployments/{}", VERCEL_API_BASE, deployment_id);
    if let Some(tid) = team_id {
        url.push_str(&format!("?teamId={}", tid));
    }

    let body = vercel_get(client, token, &url).await?;
    let result = json!({
        "id": body.get("id"),
        "name": body.get("name"),
        "url": body.get("url"),
        "state": body.get("readyState"),
        "target": body.get("target"),
        "created_at": body.get("createdAt"),
        "ready": body.get("ready"),
        "build_errors": body.get("buildErrors"),
        "git_source": body.get("gitSource"),
    });
    Ok(serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".to_string()))
}

async fn exec_deploy(
    client: &reqwest::Client,
    token: &str,
    team_id: Option<&str>,
    input: &Value,
) -> Result<String, String> {
    let project = input.get("project").and_then(|v| v.as_str()).unwrap_or("");
    let target = input
        .get("target")
        .and_then(|v| v.as_str())
        .unwrap_or("preview");

    if project.is_empty() {
        return Err("project is required".to_string());
    }

    let mut url = format!("{}/v13/deployments", VERCEL_API_BASE);
    if let Some(tid) = team_id {
        url.push_str(&format!("?teamId={}", tid));
    }

    let body = json!({
        "name": project,
        "target": target,
    });

    let resp = vercel_post(client, token, &url, &body).await?;
    let result = json!({
        "id": resp.get("id"),
        "url": resp.get("url"),
        "state": resp.get("readyState"),
        "target": resp.get("target"),
        "created_at": resp.get("createdAt"),
    });
    Ok(serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".to_string()))
}

// ── HTTP helpers ─────────────────────────────────────────────────────────

async fn vercel_get(client: &reqwest::Client, token: &str, url: &str) -> Result<Value, String> {
    let resp = client
        .get(url)
        .header("authorization", format!("Bearer {}", token))
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| format!("Vercel API request failed: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Vercel API error {}: {}", status, body));
    }

    resp.json::<Value>()
        .await
        .map_err(|e| format!("Failed to parse Vercel response: {}", e))
}

async fn vercel_post(
    client: &reqwest::Client,
    token: &str,
    url: &str,
    body: &Value,
) -> Result<Value, String> {
    let resp = client
        .post(url)
        .header("authorization", format!("Bearer {}", token))
        .json(body)
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await
        .map_err(|e| format!("Vercel API request failed: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Vercel API error {}: {}", status, body));
    }

    resp.json::<Value>()
        .await
        .map_err(|e| format!("Failed to parse Vercel response: {}", e))
}
