// Jaskier Shared Pattern -- mcp/client
//! Lightweight MCP client manager using JSON-RPC 2.0 over HTTP.
//!
//! Connects to external MCP servers, discovers their tools, and proxies
//! `tools/call` requests so Gemini agents can use them.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::PgPool;
use tokio::sync::RwLock;

use super::config::{self, McpServerConfig};

// ── MCP Tool descriptor ─────────────────────────────────────────────────────

/// A tool discovered from an MCP server, enriched with routing metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    /// Original tool name from the MCP server.
    pub name: String,
    /// Prefixed name used in Gemini tool dispatch: `mcp_{server_name}_{tool_name}`.
    pub prefixed_name: String,
    /// Human-readable server name (for UI display).
    pub server_name: String,
    /// Server ID (DB primary key).
    pub server_id: String,
    /// Tool description from the MCP server.
    pub description: Option<String>,
    /// JSON Schema for tool input parameters.
    pub input_schema: Value,
}

// ── Connection state ────────────────────────────────────────────────────────

/// An active connection to an MCP server.
#[derive(Debug)]
struct McpConnection {
    server_id: String,
    server_name: String,
    url: String,
    auth_token: Option<String>,
    timeout: Duration,
    tools: Vec<McpTool>,
    /// MCP session ID returned by `initialize`, sent as `Mcp-Session-Id` header.
    session_id: Option<String>,
}

// ── Client Manager ──────────────────────────────────────────────────────────

/// Manages connections to external MCP servers.
///
/// Thread-safe (Clone-friendly via Arc internals). Stores active connections
/// keyed by server ID. Tools from all connected servers are merged and
/// available via `list_all_tools()`.
pub struct McpClientManager {
    connections: RwLock<HashMap<String, Arc<McpConnection>>>,
    db: PgPool,
    client: Client,
}

impl McpClientManager {
    pub fn new(db: PgPool, client: Client) -> Self {
        Self {
            connections: RwLock::new(HashMap::new()),
            db,
            client,
        }
    }

    // ── Startup: connect to all enabled servers ─────────────────────────

    /// Called once at backend startup -- connects to all enabled MCP servers.
    pub async fn startup_connect(&self) -> Result<(), String> {
        let servers = config::list_mcp_servers(&self.db)
            .await
            .map_err(|e| format!("Failed to load MCP servers: {e}"))?;

        let enabled: Vec<_> = servers.into_iter().filter(|s| s.enabled).collect();
        if enabled.is_empty() {
            tracing::info!("MCP: no enabled servers configured");
            return Ok(());
        }

        tracing::info!("MCP: connecting to {} enabled server(s)", enabled.len());

        for server in &enabled {
            match self.connect_server(server).await {
                Ok(()) => {
                    tracing::info!(
                        "MCP: connected to '{}' ({} tools)",
                        server.name,
                        self.get_server_tools(&server.id).await.len()
                    );
                }
                Err(e) => {
                    tracing::warn!("MCP: failed to connect to '{}': {}", server.name, e);
                }
            }
        }

        let total_tools = self.list_all_tools().await.len();
        tracing::info!("MCP: startup complete -- {} external tool(s) available", total_tools);
        Ok(())
    }

    // ── Connect / Disconnect ────────────────────────────────────────────

    /// Connect to a single MCP server: initialize + discover tools.
    pub async fn connect_server(&self, cfg: &McpServerConfig) -> Result<(), String> {
        if cfg.transport != "http" {
            return Err(format!(
                "Only HTTP transport is supported (got '{}').",
                cfg.transport
            ));
        }

        let url = cfg
            .url
            .as_deref()
            .ok_or("HTTP transport requires a URL")?;

        let timeout = Duration::from_secs(cfg.timeout_secs.max(5) as u64);

        // Step 1: Initialize
        let init_response = self
            .json_rpc_call(url, cfg.auth_token.as_deref(), timeout, json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "protocolVersion": "2024-11-05",
                    "capabilities": {
                        "tools": { "listChanged": true }
                    },
                    "clientInfo": {
                        "name": "GeminiHydra",
                        "version": "15.0.0"
                    }
                }
            }))
            .await?;

        let session_id = init_response
            .get("_mcp_session_id")
            .and_then(|v| v.as_str())
            .map(String::from);

        let server_name = init_response
            .pointer("/result/serverInfo/name")
            .and_then(|v| v.as_str())
            .unwrap_or(&cfg.name);

        tracing::debug!(
            "MCP: initialized '{}' (protocol version: {})",
            server_name,
            init_response
                .pointer("/result/protocolVersion")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
        );

        // Step 2: List tools
        let tools_response = self
            .json_rpc_call(url, cfg.auth_token.as_deref(), timeout, json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "tools/list"
            }))
            .await?;

        let raw_tools = tools_response
            .pointer("/result/tools")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        let sanitized_name = sanitize_server_name(&cfg.name);

        let tools: Vec<McpTool> = raw_tools
            .iter()
            .filter_map(|t| {
                let name = t.get("name")?.as_str()?.to_string();
                let description = t.get("description").and_then(|d| d.as_str()).map(String::from);
                let input_schema = t.get("inputSchema").cloned().unwrap_or(json!({"type": "object", "properties": {}}));
                let prefixed = format!("mcp_{}_{}", sanitized_name, name);

                Some(McpTool {
                    name,
                    prefixed_name: prefixed,
                    server_name: cfg.name.clone(),
                    server_id: cfg.id.clone(),
                    description,
                    input_schema,
                })
            })
            .collect();

        // Step 3: Persist discovered tools to DB
        let db_tools: Vec<(String, Option<String>, String)> = tools
            .iter()
            .map(|t| (t.name.clone(), t.description.clone(), t.input_schema.to_string()))
            .collect();
        if let Err(e) = config::save_discovered_tools(&self.db, &cfg.id, &db_tools).await {
            tracing::warn!("MCP: failed to persist tools for '{}': {}", cfg.name, e);
        }

        // Step 4: Store connection
        let conn = Arc::new(McpConnection {
            server_id: cfg.id.clone(),
            server_name: cfg.name.clone(),
            url: url.to_string(),
            auth_token: cfg.auth_token.clone(),
            timeout,
            tools,
            session_id,
        });

        self.connections.write().await.insert(cfg.id.clone(), conn);
        Ok(())
    }

    /// Disconnect from a server (remove from active connections).
    pub async fn disconnect_server(&self, server_id: &str) {
        self.connections.write().await.remove(server_id);
        tracing::info!("MCP: disconnected server {}", server_id);
    }

    // ── Tool access ─────────────────────────────────────────────────────

    /// Get all tools from a specific connected server.
    pub async fn get_server_tools(&self, server_id: &str) -> Vec<McpTool> {
        let lock = self.connections.read().await;
        lock.get(server_id)
            .map(|c| c.tools.clone())
            .unwrap_or_default()
    }

    /// Get all tools from all connected servers.
    pub async fn list_all_tools(&self) -> Vec<McpTool> {
        let lock = self.connections.read().await;
        lock.values()
            .flat_map(|c| c.tools.iter().cloned())
            .collect()
    }

    /// Find the connection and original tool name for a prefixed tool name.
    async fn resolve_tool(&self, prefixed_name: &str) -> Option<(Arc<McpConnection>, String)> {
        let lock = self.connections.read().await;
        for conn in lock.values() {
            for tool in &conn.tools {
                if tool.prefixed_name == prefixed_name {
                    return Some((conn.clone(), tool.name.clone()));
                }
            }
        }
        None
    }

    // ── Call tool ───────────────────────────────────────────────────────

    /// Call a tool on a connected MCP server by its prefixed name.
    pub async fn call_tool(
        &self,
        prefixed_name: &str,
        arguments: &Value,
    ) -> Result<String, String> {
        let (conn, original_name) = self
            .resolve_tool(prefixed_name)
            .await
            .ok_or_else(|| format!("MCP tool '{}' not found in any connected server", prefixed_name))?;

        let response = self
            .json_rpc_call(
                &conn.url,
                conn.auth_token.as_deref(),
                conn.timeout,
                json!({
                    "jsonrpc": "2.0",
                    "id": 3,
                    "method": "tools/call",
                    "params": {
                        "name": original_name,
                        "arguments": arguments
                    }
                }),
            )
            .await?;

        // Extract result content
        if let Some(error) = response.get("error") {
            let msg = error.get("message").and_then(|m| m.as_str()).unwrap_or("Unknown MCP error");
            let code = error.get("code").and_then(|c| c.as_i64()).unwrap_or(-1);
            return Err(format!("MCP error {}: {}", code, msg));
        }

        // MCP tools/call result is { content: [{ type: "text", text: "..." }] }
        if let Some(content) = response.pointer("/result/content") {
            if let Some(arr) = content.as_array() {
                let texts: Vec<&str> = arr
                    .iter()
                    .filter_map(|c| {
                        if c.get("type").and_then(|t| t.as_str()) == Some("text") {
                            c.get("text").and_then(|t| t.as_str())
                        } else {
                            None
                        }
                    })
                    .collect();
                if !texts.is_empty() {
                    return Ok(texts.join("\n"));
                }
            }
            return Ok(content.to_string());
        }

        Ok(response.get("result").map(|r| r.to_string()).unwrap_or_else(|| "{}".to_string()))
    }

    // ── JSON-RPC transport ──────────────────────────────────────────────

    async fn json_rpc_call(
        &self,
        url: &str,
        auth_token: Option<&str>,
        timeout: Duration,
        body: Value,
    ) -> Result<Value, String> {
        let mut req = self
            .client
            .post(url)
            .header("Content-Type", "application/json")
            .timeout(timeout)
            .json(&body);

        if let Some(token) = auth_token {
            req = req.header("Authorization", format!("Bearer {}", token));
        }

        let response = req.send().await.map_err(|e| {
            format!("MCP HTTP request to '{}' failed: {}", url, e)
        })?;

        let session_id = response
            .headers()
            .get("mcp-session-id")
            .and_then(|v| v.to_str().ok())
            .map(String::from);

        let status = response.status();
        if !status.is_success() {
            let body_text = response.text().await.unwrap_or_default();
            return Err(format!(
                "MCP server returned HTTP {}: {}",
                status,
                truncate_str(&body_text, 500)
            ));
        }

        let mut json: Value = response.json().await.map_err(|e| {
            format!("MCP response is not valid JSON: {}", e)
        })?;

        if let Some(sid) = session_id {
            json["_mcp_session_id"] = Value::String(sid);
        }

        Ok(json)
    }

    // ── Build Gemini tool declarations for MCP tools ────────────────────

    /// Generate Gemini `function_declarations` for all connected MCP tools.
    pub async fn build_gemini_tool_declarations(&self) -> Vec<Value> {
        let all_tools = self.list_all_tools().await;
        all_tools
            .iter()
            .map(|t| {
                let desc = t.description.as_deref().unwrap_or("External MCP tool");
                let full_desc = format!("[MCP: {}] {}", t.server_name, desc);
                json!({
                    "name": t.prefixed_name,
                    "description": full_desc,
                    "parameters": t.input_schema,
                })
            })
            .collect()
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn sanitize_server_name(name: &str) -> String {
    let mut result = String::with_capacity(name.len());
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            result.push(ch.to_ascii_lowercase());
        } else if !result.ends_with('_') {
            result.push('_');
        }
    }
    result.trim_end_matches('_').to_string()
}

fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let boundary = s
            .char_indices()
            .take_while(|(i, _)| *i < max_len)
            .last()
            .map(|(i, c)| i + c.len_utf8())
            .unwrap_or(max_len);
        format!("{}...", &s[..boundary])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_server_name() {
        assert_eq!(sanitize_server_name("my-server"), "my_server");
        assert_eq!(sanitize_server_name("My Server 2"), "my_server_2");
        assert_eq!(sanitize_server_name("a--b"), "a_b");
        assert_eq!(sanitize_server_name("simple"), "simple");
        assert_eq!(sanitize_server_name("UPPER"), "upper");
    }

    #[test]
    fn test_truncate_str() {
        assert_eq!(truncate_str("hello", 10), "hello");
        assert_eq!(truncate_str("hello world", 5), "hello...");
    }
}
