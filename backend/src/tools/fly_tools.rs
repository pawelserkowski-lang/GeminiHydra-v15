// Jaskier Shared Pattern — Fly.io Tools
// Agent tools for Fly.io API interactions (read-only).
// Reads PAT from gh_service_tokens table via service_tokens module.

use serde_json::{Value, json};

use crate::service_tokens;
use crate::state::AppState;

const FLY_API_BASE: &str = "https://api.machines.dev/v1";
const FLY_GQL_URL: &str = "https://api.fly.io/graphql";
const SERVICE_NAME: &str = "flyio";

// ═══════════════════════════════════════════════════════════════════════
//  Tool execution
// ═══════════════════════════════════════════════════════════════════════

pub async fn execute(tool_name: &str, input: &Value, state: &AppState) -> Result<String, String> {
    let token = match service_tokens::get_service_token(state, SERVICE_NAME).await {
        Some(t) => t,
        None => {
            return Err(
                "Fly.io not configured. Please add your Fly.io PAT via Settings > Service Tokens (service name: flyio).".to_string(),
            )
        }
    };

    let client = &state.client;

    match tool_name {
        "fly_list_apps" => exec_list_apps(client, &token, input).await,
        "fly_get_status" => exec_get_status(client, &token, input).await,
        "fly_get_logs" => exec_get_logs(client, &token, input).await,
        _ => Err(format!("Unknown Fly.io tool: {}", tool_name)),
    }
}

// ── Individual tool implementations ──────────────────────────────────────

async fn exec_list_apps(
    client: &reqwest::Client,
    token: &str,
    input: &Value,
) -> Result<String, String> {
    let org_slug = input
        .get("org_slug")
        .and_then(|v| v.as_str())
        .unwrap_or("personal");

    // Use GraphQL API for listing apps (Machines API doesn't have a list-all endpoint)
    let query = json!({
        "query": format!(
            r#"query {{
                apps(type: "container", organizationSlug: "{}") {{
                    nodes {{
                        id
                        name
                        status
                        organization {{ slug }}
                        currentRelease {{ version createdAt }}
                        hostname
                    }}
                }}
            }}"#,
            org_slug
        )
    });

    let body = fly_graphql(client, token, &query).await?;
    let apps = body
        .get("data")
        .and_then(|d| d.get("apps"))
        .and_then(|a| a.get("nodes"))
        .and_then(|n| n.as_array())
        .map(|arr| {
            arr.iter()
                .map(|a| {
                    json!({
                        "name": a.get("name"),
                        "status": a.get("status"),
                        "organization": a.get("organization").and_then(|o| o.get("slug")),
                        "hostname": a.get("hostname"),
                        "current_release": a.get("currentRelease"),
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Ok(serde_json::to_string_pretty(&apps).unwrap_or_else(|_| "[]".to_string()))
}

async fn exec_get_status(
    client: &reqwest::Client,
    token: &str,
    input: &Value,
) -> Result<String, String> {
    let app_name = input.get("app_name").and_then(|v| v.as_str()).unwrap_or("");

    if app_name.is_empty() {
        return Err("app_name is required".to_string());
    }

    // Get machines for this app via Machines API
    let url = format!("{}/apps/{}/machines", FLY_API_BASE, app_name);

    let body = fly_get(client, token, &url).await?;
    let machines = body
        .as_array()
        .map(|arr| {
            arr.iter()
                .map(|m| {
                    json!({
                        "id": m.get("id"),
                        "name": m.get("name"),
                        "state": m.get("state"),
                        "region": m.get("region"),
                        "instance_id": m.get("instance_id"),
                        "image": m.get("config").and_then(|c| c.get("image")),
                        "created_at": m.get("created_at"),
                        "updated_at": m.get("updated_at"),
                        "checks": m.get("checks"),
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let result = json!({
        "app": app_name,
        "machine_count": machines.len(),
        "machines": machines,
    });
    Ok(serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".to_string()))
}

async fn exec_get_logs(
    client: &reqwest::Client,
    token: &str,
    input: &Value,
) -> Result<String, String> {
    let app_name = input.get("app_name").and_then(|v| v.as_str()).unwrap_or("");

    if app_name.is_empty() {
        return Err("app_name is required".to_string());
    }

    // Use GraphQL for logs (no REST endpoint for historical logs)
    let query = json!({
        "query": format!(
            r#"query {{
                app(name: "{}") {{
                    name
                    status
                    currentRelease {{ version createdAt status }}
                    allocations {{
                        id
                        region
                        status
                        version
                        desiredStatus
                        recentLogs(limit: 50) {{
                            id
                            message
                            timestamp
                            level
                            region
                        }}
                    }}
                }}
            }}"#,
            app_name
        )
    });

    let body = fly_graphql(client, token, &query).await?;
    let app_data = body.get("data").and_then(|d| d.get("app"));

    match app_data {
        Some(app) => {
            let result = json!({
                "name": app.get("name"),
                "status": app.get("status"),
                "current_release": app.get("currentRelease"),
                "allocations": app.get("allocations"),
            });
            Ok(serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".to_string()))
        }
        None => {
            // Check for errors
            let errors = body.get("errors");
            let msg = errors
                .and_then(|e| e.as_array())
                .and_then(|arr| arr.first())
                .and_then(|e| e.get("message"))
                .and_then(|m| m.as_str())
                .unwrap_or("App not found or no data returned");
            Err(msg.to_string())
        }
    }
}

// ── HTTP helpers ─────────────────────────────────────────────────────────

async fn fly_get(client: &reqwest::Client, token: &str, url: &str) -> Result<Value, String> {
    let resp = client
        .get(url)
        .header("authorization", format!("Bearer {}", token))
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| format!("Fly.io API request failed: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Fly.io API error {}: {}", status, body));
    }

    resp.json::<Value>()
        .await
        .map_err(|e| format!("Failed to parse Fly.io response: {}", e))
}

async fn fly_graphql(
    client: &reqwest::Client,
    token: &str,
    query: &Value,
) -> Result<Value, String> {
    let resp = client
        .post(FLY_GQL_URL)
        .header("authorization", format!("Bearer {}", token))
        .header("content-type", "application/json")
        .json(query)
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| format!("Fly.io GraphQL request failed: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Fly.io GraphQL error {}: {}", status, body));
    }

    resp.json::<Value>()
        .await
        .map_err(|e| format!("Failed to parse Fly.io response: {}", e))
}
