// Jaskier Shared Pattern -- mcp/config
//! MCP server configuration: CRUD for gh_mcp_servers + gh_mcp_discovered_tools.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{FromRow, PgPool};

use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct McpServerConfig {
    pub id: String,
    pub name: String,
    pub transport: String,
    pub command: Option<String>,
    pub args: String,
    pub env_vars: String,
    pub url: Option<String>,
    pub enabled: bool,
    pub auth_token: Option<String>,
    pub timeout_secs: i32,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct McpDiscoveredTool {
    pub id: String,
    pub server_id: String,
    pub tool_name: String,
    pub description: Option<String>,
    pub input_schema: String,
    pub discovered_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateMcpServer {
    pub name: String,
    pub transport: String,
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    pub env_vars: Option<Value>,
    pub url: Option<String>,
    pub enabled: Option<bool>,
    pub auth_token: Option<String>,
    pub timeout_secs: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateMcpServer {
    pub name: Option<String>,
    pub transport: Option<String>,
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    pub env_vars: Option<Value>,
    pub url: Option<String>,
    pub enabled: Option<bool>,
    pub auth_token: Option<String>,
    pub timeout_secs: Option<i32>,
}

// ── DB functions ──────────────────────────────────────────────────────────

pub async fn list_mcp_servers(db: &PgPool) -> Result<Vec<McpServerConfig>, sqlx::Error> {
    sqlx::query_as::<_, McpServerConfig>("SELECT * FROM gh_mcp_servers ORDER BY created_at ASC")
        .fetch_all(db)
        .await
}

pub async fn get_mcp_server(db: &PgPool, id: &str) -> Result<Option<McpServerConfig>, sqlx::Error> {
    sqlx::query_as::<_, McpServerConfig>("SELECT * FROM gh_mcp_servers WHERE id = $1")
        .bind(id)
        .fetch_optional(db)
        .await
}

pub async fn create_mcp_server_db(db: &PgPool, req: &CreateMcpServer) -> Result<McpServerConfig, sqlx::Error> {
    let args_json = serde_json::to_string(&req.args.as_deref().unwrap_or(&[]))
        .unwrap_or_else(|_| "[]".to_string());
    let env_json = req.env_vars.as_ref().map(|v| v.to_string()).unwrap_or_else(|| "{}".to_string());

    sqlx::query_as::<_, McpServerConfig>(
        "INSERT INTO gh_mcp_servers (name, transport, command, args, env_vars, url, enabled, auth_token, timeout_secs) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) RETURNING *",
    )
    .bind(&req.name)
    .bind(&req.transport)
    .bind(&req.command)
    .bind(&args_json)
    .bind(&env_json)
    .bind(&req.url)
    .bind(req.enabled.unwrap_or(true))
    .bind(&req.auth_token)
    .bind(req.timeout_secs.unwrap_or(30))
    .fetch_one(db)
    .await
}

pub async fn update_mcp_server_db(db: &PgPool, id: &str, req: &UpdateMcpServer) -> Result<Option<McpServerConfig>, sqlx::Error> {
    let current = match get_mcp_server(db, id).await? {
        Some(c) => c,
        None => return Ok(None),
    };
    let name = req.name.as_deref().unwrap_or(&current.name);
    let transport = req.transport.as_deref().unwrap_or(&current.transport);
    let command = req.command.as_deref().or(current.command.as_deref());
    let args = req.args.as_ref()
        .map(|a| serde_json::to_string(a).unwrap_or_else(|_| "[]".to_string()))
        .unwrap_or(current.args.clone());
    let env_vars = req.env_vars.as_ref()
        .map(|v| v.to_string())
        .unwrap_or(current.env_vars.clone());
    let url = req.url.as_deref().or(current.url.as_deref());
    let enabled = req.enabled.unwrap_or(current.enabled);
    let auth_token = req.auth_token.as_deref().or(current.auth_token.as_deref());
    let timeout_secs = req.timeout_secs.unwrap_or(current.timeout_secs);

    sqlx::query_as::<_, McpServerConfig>(
        "UPDATE gh_mcp_servers SET name=$1, transport=$2, command=$3, args=$4, env_vars=$5, url=$6, enabled=$7, auth_token=$8, timeout_secs=$9, updated_at=NOW() WHERE id=$10 RETURNING *",
    )
    .bind(name).bind(transport).bind(command).bind(&args).bind(&env_vars)
    .bind(url).bind(enabled).bind(auth_token).bind(timeout_secs).bind(id)
    .fetch_optional(db)
    .await
}

pub async fn delete_mcp_server_db(db: &PgPool, id: &str) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("DELETE FROM gh_mcp_servers WHERE id = $1")
        .bind(id).execute(db).await?;
    Ok(result.rows_affected() > 0)
}

/// Save discovered tools for a server (replace all).
/// Each tuple is (tool_name, description, input_schema_json).
pub async fn save_discovered_tools(
    db: &PgPool,
    server_id: &str,
    tools: &[(String, Option<String>, String)],
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM gh_mcp_discovered_tools WHERE server_id = $1")
        .bind(server_id).execute(db).await?;
    for (name, desc, schema) in tools {
        sqlx::query(
            "INSERT INTO gh_mcp_discovered_tools (server_id, tool_name, description, input_schema) VALUES ($1, $2, $3, $4)",
        )
        .bind(server_id).bind(name).bind(desc.as_deref()).bind(schema)
        .execute(db).await?;
    }
    Ok(())
}

pub async fn list_discovered_tools(db: &PgPool, server_id: &str) -> Result<Vec<McpDiscoveredTool>, sqlx::Error> {
    sqlx::query_as::<_, McpDiscoveredTool>(
        "SELECT * FROM gh_mcp_discovered_tools WHERE server_id = $1 ORDER BY tool_name ASC",
    )
    .bind(server_id).fetch_all(db).await
}

pub async fn list_all_discovered_tools(db: &PgPool) -> Result<Vec<McpDiscoveredTool>, sqlx::Error> {
    sqlx::query_as::<_, McpDiscoveredTool>(
        "SELECT * FROM gh_mcp_discovered_tools ORDER BY server_id, tool_name ASC",
    )
    .fetch_all(db).await
}

// ── HTTP Handlers ──────────────────────────────────────────────────────────

pub async fn mcp_server_list(State(state): State<AppState>) -> (StatusCode, Json<Value>) {
    match list_mcp_servers(&state.db).await {
        Ok(servers) => {
            let val = serde_json::to_value(&servers).unwrap_or_else(|_| json!([]));
            (StatusCode::OK, Json(json!({ "servers": val })))
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": format!("DB error: {}", e) }))),
    }
}

pub async fn mcp_server_create(State(state): State<AppState>, Json(body): Json<CreateMcpServer>) -> (StatusCode, Json<Value>) {
    if body.name.trim().is_empty() {
        return (StatusCode::BAD_REQUEST, Json(json!({ "error": "Server name is required" })));
    }
    if body.transport != "stdio" && body.transport != "http" {
        return (StatusCode::BAD_REQUEST, Json(json!({ "error": "Transport must be stdio or http" })));
    }
    match create_mcp_server_db(&state.db, &body).await {
        Ok(server) => {
            let val = serde_json::to_value(&server).unwrap_or_else(|_| json!({"error": "serialization failed"}));
            (StatusCode::CREATED, Json(val))
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": format!("Failed to create: {}", e) }))),
    }
}

pub async fn mcp_server_update(State(state): State<AppState>, Path(id): Path<String>, Json(body): Json<UpdateMcpServer>) -> (StatusCode, Json<Value>) {
    match update_mcp_server_db(&state.db, &id, &body).await {
        Ok(Some(server)) => {
            let val = serde_json::to_value(&server).unwrap_or_else(|_| json!({"error": "serialization failed"}));
            (StatusCode::OK, Json(val))
        }
        Ok(None) => (StatusCode::NOT_FOUND, Json(json!({ "error": "MCP server not found" }))),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": format!("Failed to update: {}", e) }))),
    }
}

pub async fn mcp_server_delete(State(state): State<AppState>, Path(id): Path<String>) -> (StatusCode, Json<Value>) {
    state.mcp_client.disconnect_server(&id).await;
    match delete_mcp_server_db(&state.db, &id).await {
        Ok(true) => (StatusCode::OK, Json(json!({ "deleted": true }))),
        Ok(false) => (StatusCode::NOT_FOUND, Json(json!({ "error": "MCP server not found" }))),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": format!("Failed to delete: {}", e) }))),
    }
}

pub async fn mcp_server_connect(State(state): State<AppState>, Path(id): Path<String>) -> (StatusCode, Json<Value>) {
    let server = match get_mcp_server(&state.db, &id).await {
        Ok(Some(s)) => s,
        Ok(None) => return (StatusCode::NOT_FOUND, Json(json!({ "error": "MCP server not found" }))),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": format!("DB error: {}", e) }))),
    };
    match state.mcp_client.connect_server(&server).await {
        Ok(()) => {
            let tools = state.mcp_client.get_server_tools(&id).await;
            (StatusCode::OK, Json(json!({
                "connected": true,
                "tools_discovered": tools.len(),
                "tools": tools.iter().map(|t| json!({"name": t.name, "prefixed_name": t.prefixed_name, "description": t.description})).collect::<Vec<_>>()
            })))
        }
        Err(e) => (StatusCode::BAD_GATEWAY, Json(json!({ "error": format!("Failed to connect: {}", e) }))),
    }
}

pub async fn mcp_server_disconnect(State(state): State<AppState>, Path(id): Path<String>) -> (StatusCode, Json<Value>) {
    state.mcp_client.disconnect_server(&id).await;
    (StatusCode::OK, Json(json!({ "disconnected": true })))
}

pub async fn mcp_server_tools(State(state): State<AppState>, Path(id): Path<String>) -> (StatusCode, Json<Value>) {
    let live_tools = state.mcp_client.get_server_tools(&id).await;
    if !live_tools.is_empty() {
        let tools_val: Vec<Value> = live_tools.iter().map(|t| json!({"name": t.name, "prefixed_name": t.prefixed_name, "description": t.description, "input_schema": t.input_schema, "source": "live"})).collect();
        return (StatusCode::OK, Json(json!({ "tools": tools_val, "source": "live" })));
    }
    match list_discovered_tools(&state.db, &id).await {
        Ok(tools) => {
            let tools_val: Vec<Value> = tools.iter().map(|t| json!({"name": t.tool_name, "description": t.description, "input_schema": t.input_schema, "source": "db"})).collect();
            (StatusCode::OK, Json(json!({ "tools": tools_val, "source": "db" })))
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": format!("Failed to list tools: {}", e) }))),
    }
}

pub async fn mcp_all_tools(State(state): State<AppState>) -> (StatusCode, Json<Value>) {
    let tools = state.mcp_client.list_all_tools().await;
    let tools_val: Vec<Value> = tools.iter().map(|t| json!({"name": t.name, "prefixed_name": t.prefixed_name, "server_name": t.server_name, "server_id": t.server_id, "description": t.description, "input_schema": t.input_schema})).collect();
    (StatusCode::OK, Json(json!({ "tools": tools_val, "total": tools_val.len() })))
}
