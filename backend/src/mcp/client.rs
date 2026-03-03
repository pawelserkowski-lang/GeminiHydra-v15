// Jaskier Shared Pattern -- mcp/client
//! Lightweight MCP client manager using JSON-RPC 2.0.
//!
//! Supports HTTP transport (Streamable HTTP) and stdio transport (child process).
//! Connects to external MCP servers, discovers their tools, and proxies
//! `tools/call` requests so Gemini agents can use them.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::PgPool;
use tokio::sync::RwLock;

use super::config::{self, McpServerConfig};

/// Default timeout for tool execution (30 seconds).
const TOOL_CALL_TIMEOUT: Duration = Duration::from_secs(30);

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

// ── Transport ────────────────────────────────────────────────────────────────

/// Transport layer for communicating with an MCP server.
#[derive(Debug)]
enum McpTransport {
    Http {
        url: String,
        auth_token: Option<String>,
    },
    Stdio {
        _child: Box<tokio::sync::Mutex<tokio::process::Child>>,
        stdin: tokio::sync::Mutex<tokio::process::ChildStdin>,
        stdout: tokio::sync::Mutex<tokio::io::BufReader<tokio::process::ChildStdout>>,
    },
}

// ── Connection state ────────────────────────────────────────────────────────

/// An active connection to an MCP server.
#[derive(Debug)]
struct McpConnection {
    server_name: String,
    transport: McpTransport,
    timeout: Duration,
    tools: Vec<McpTool>,
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
    request_id: AtomicU64,
}

impl McpClientManager {
    pub fn new(db: PgPool, client: Client) -> Self {
        Self {
            connections: RwLock::new(HashMap::new()),
            db,
            client,
            request_id: AtomicU64::new(1),
        }
    }

    /// Get next unique JSON-RPC request ID.
    fn next_id(&self) -> u64 {
        self.request_id.fetch_add(1, Ordering::Relaxed)
    }

    // ── Startup: auto-register + connect to all enabled servers ────────

    /// Auto-register default MCP servers from `MCP_DEFAULT_SERVERS` env var.
    /// Idempotent — skips servers whose name already exists in DB.
    ///
    /// Env var format (JSON array):
    /// ```json
    /// [{"name":"brave","transport":"http","url":"https://...","auth_token":"..."}]
    /// ```
    /// On fly.io: `fly secrets set MCP_DEFAULT_SERVERS='[...]'`
    async fn ensure_default_servers(&self) {
        let env_val = match std::env::var("MCP_DEFAULT_SERVERS") {
            Ok(v) if !v.is_empty() => v,
            _ => return,
        };

        let defaults: Vec<serde_json::Value> = match serde_json::from_str(&env_val) {
            Ok(v) => v,
            Err(e) => {
                tracing::debug!("MCP: skipping MCP_DEFAULT_SERVERS (parse error: {e})");
                return;
            }
        };

        let existing = match config::list_mcp_servers(&self.db).await {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("MCP: failed to load servers for default check: {e}");
                return;
            }
        };
        let existing_names: std::collections::HashSet<String> =
            existing.iter().map(|s| s.name.clone()).collect();

        for server in &defaults {
            let name = match server["name"].as_str() {
                Some(n) if !n.is_empty() => n,
                _ => continue,
            };

            if existing_names.contains(name) {
                tracing::debug!(
                    "MCP: default server '{}' already registered, skipping",
                    name
                );
                continue;
            }

            let req = config::CreateMcpServer {
                name: name.to_string(),
                transport: server["transport"].as_str().unwrap_or("http").to_string(),
                command: server["command"].as_str().map(String::from),
                args: server["args"].as_array().map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                }),
                env_vars: server.get("env_vars").cloned(),
                url: server["url"].as_str().map(String::from),
                enabled: Some(server["enabled"].as_bool().unwrap_or(true)),
                auth_token: server["auth_token"].as_str().map(String::from),
                timeout_secs: server["timeout_secs"].as_i64().map(|v| v as i32),
            };

            match config::create_mcp_server_db(&self.db, &req).await {
                Ok(cfg) => tracing::info!(
                    "MCP: auto-registered default server '{}' (id={})",
                    name,
                    cfg.id
                ),
                Err(e) => tracing::warn!("MCP: failed to auto-register '{}': {e}", name),
            }
        }
    }

    /// Called once at backend startup -- auto-registers defaults, then connects to all enabled MCP servers.
    pub async fn startup_connect(&self) -> Result<(), String> {
        // Auto-register default servers from env (idempotent)
        self.ensure_default_servers().await;

        let servers = config::list_mcp_servers(&self.db)
            .await
            .map_err(|e| format!("Failed to load MCP servers: {e}"))?;

        let enabled: Vec<_> = servers.into_iter().filter(|s| s.enabled).collect();
        if enabled.is_empty() {
            tracing::info!("MCP: no enabled servers configured");
            return Ok(());
        }

        tracing::info!(
            "MCP: connecting to {} enabled server(s) in parallel",
            enabled.len()
        );

        // Connect concurrently — all connections are I/O-bound so this is safe
        // Uses join_all (concurrent on same task) rather than tokio::spawn (requires 'static)
        let results = futures_util::future::join_all(enabled.iter().map(|server| async move {
            let name = server.name.clone();
            let id = server.id.clone();
            match self.connect_server(server).await {
                Ok(()) => {
                    let tool_count = self.get_server_tools(&id).await.len();
                    tracing::info!("MCP: connected to '{}' ({} tools)", name, tool_count);
                }
                Err(e) => {
                    tracing::warn!("MCP: failed to connect to '{}': {}", name, e);
                }
            }
        }))
        .await;

        drop(results);

        let total_tools = self.list_all_tools().await.len();
        tracing::info!(
            "MCP: startup complete -- {} external tool(s) available",
            total_tools
        );
        Ok(())
    }

    // ── Connect / Disconnect ────────────────────────────────────────────

    /// Connect to a single MCP server: initialize + discover tools.
    pub async fn connect_server(&self, cfg: &McpServerConfig) -> Result<(), String> {
        let timeout = Duration::from_secs(cfg.timeout_secs.max(5) as u64);
        let sanitized_name = sanitize_server_name(&cfg.name);

        let (transport, tools) = match cfg.transport.as_str() {
            "http" => {
                let url = cfg.url.as_deref().ok_or("HTTP transport requires a URL")?;

                // Step 1: Initialize
                let _init_result = self
                    .http_jsonrpc(
                        url,
                        cfg.auth_token.as_deref(),
                        timeout,
                        "initialize",
                        json!({
                            "protocolVersion": "2025-03-26",
                            "capabilities": {
                                "tools": { "listChanged": true }
                            },
                            "clientInfo": {
                                "name": "GeminiHydra",
                                "version": "15.0.0"
                            }
                        }),
                    )
                    .await?;

                // Step 2: Send initialized notification
                let _ = self
                    .http_jsonrpc_notify(
                        url,
                        cfg.auth_token.as_deref(),
                        timeout,
                        "notifications/initialized",
                        json!({}),
                    )
                    .await;

                // Step 3: List tools
                let raw_tools = self
                    .http_list_tools(url, cfg.auth_token.as_deref(), timeout)
                    .await?;

                let transport = McpTransport::Http {
                    url: url.to_string(),
                    auth_token: cfg.auth_token.clone(),
                };

                let tools = build_tool_list(&raw_tools, &sanitized_name, &cfg.name, &cfg.id);
                (transport, tools)
            }
            "stdio" => {
                let command = cfg
                    .command
                    .as_deref()
                    .ok_or("stdio transport requires a command")?;

                let args: Vec<String> = serde_json::from_str(&cfg.args).unwrap_or_default();
                let env_vars: HashMap<String, String> =
                    serde_json::from_str(&cfg.env_vars).unwrap_or_default();

                let (transport, raw_tools) = self
                    .stdio_connect(command, &args, &env_vars, timeout)
                    .await?;

                let tools = build_tool_list(&raw_tools, &sanitized_name, &cfg.name, &cfg.id);
                (transport, tools)
            }
            other => {
                return Err(format!(
                    "Unsupported transport '{}' — use 'http' or 'stdio'",
                    other
                ));
            }
        };

        // Persist discovered tools to DB
        let db_tools: Vec<(String, Option<String>, String)> = tools
            .iter()
            .map(|t| {
                (
                    t.name.clone(),
                    t.description.clone(),
                    t.input_schema.to_string(),
                )
            })
            .collect();
        if let Err(e) = config::save_discovered_tools(&self.db, &cfg.id, &db_tools).await {
            tracing::error!(
                "MCP: failed to persist tools for '{}': {} — connection will still be usable but tools may not survive restart",
                cfg.name,
                e
            );
        }

        // Store connection
        let conn = Arc::new(McpConnection {
            server_name: cfg.name.clone(),
            transport,
            timeout,
            tools,
        });

        self.connections.write().await.insert(cfg.id.clone(), conn);
        Ok(())
    }

    /// Disconnect from a server (remove from active connections).
    pub async fn disconnect_server(&self, server_id: &str) {
        if let Some(conn) = self.connections.write().await.remove(server_id) {
            tracing::info!("MCP: disconnected server '{}'", conn.server_name);
            // For stdio, the child process is dropped when all Arc references are gone.
            // tokio::process::Child::drop kills the child.
        }
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
    /// Enforces a timeout of max(TOOL_CALL_TIMEOUT, server.timeout).
    pub async fn call_tool(
        &self,
        prefixed_name: &str,
        arguments: &Value,
    ) -> Result<String, String> {
        let (conn, original_name) = self.resolve_tool(prefixed_name).await.ok_or_else(|| {
            format!(
                "MCP tool '{}' not found in any connected server",
                prefixed_name
            )
        })?;

        let call_timeout = conn.timeout.max(TOOL_CALL_TIMEOUT);

        tokio::time::timeout(call_timeout, async {
            match &conn.transport {
                McpTransport::Http { url, auth_token } => {
                    let response = self
                        .http_jsonrpc(
                            url,
                            auth_token.as_deref(),
                            conn.timeout,
                            "tools/call",
                            json!({
                                "name": original_name,
                                "arguments": arguments
                            }),
                        )
                        .await?;

                    extract_tool_result(&response)
                }
                McpTransport::Stdio { stdin, stdout, .. } => {
                    self.stdio_call_tool(stdin, stdout, &original_name, arguments, conn.timeout)
                        .await
                }
            }
        })
        .await
        .map_err(|_| {
            format!(
                "MCP tool '{}' timed out after {}s",
                prefixed_name,
                call_timeout.as_secs()
            )
        })?
    }

    // ── HTTP JSON-RPC helpers ────────────────────────────────────────────

    async fn http_jsonrpc(
        &self,
        url: &str,
        auth_token: Option<&str>,
        timeout: Duration,
        method: &str,
        params: Value,
    ) -> Result<Value, String> {
        let id = self.next_id();
        let body = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });

        let mut req = self
            .client
            .post(url)
            .header("Content-Type", "application/json")
            // Streamable HTTP (MCP 2025-03-26): request JSON response, not SSE stream
            .header("Accept", "application/json")
            .timeout(timeout)
            .json(&body);

        if let Some(token) = auth_token {
            req = req.header("Authorization", format!("Bearer {}", token));
        }

        let response = req
            .send()
            .await
            .map_err(|e| format!("MCP HTTP request to '{}' failed: {}", url, e))?;

        let status = response.status();
        if !status.is_success() {
            let body_text = response.text().await.unwrap_or_default();
            return Err(format!(
                "MCP server returned HTTP {}: {}",
                status,
                truncate_str(&body_text, 500)
            ));
        }

        let json: Value = response
            .json()
            .await
            .map_err(|e| format!("MCP response is not valid JSON: {}", e))?;

        if let Some(error) = json.get("error") {
            return Err(format!("MCP JSON-RPC error: {}", error));
        }

        Ok(json.get("result").cloned().unwrap_or(json!(null)))
    }

    /// Send a JSON-RPC notification (no id, no response expected).
    async fn http_jsonrpc_notify(
        &self,
        url: &str,
        auth_token: Option<&str>,
        timeout: Duration,
        method: &str,
        params: Value,
    ) -> Result<(), String> {
        let body = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });

        let mut req = self
            .client
            .post(url)
            .header("Content-Type", "application/json")
            .timeout(timeout)
            .json(&body);

        if let Some(token) = auth_token {
            req = req.header("Authorization", format!("Bearer {}", token));
        }

        let _ = req
            .send()
            .await
            .map_err(|e| format!("MCP notification to '{}' failed: {}", url, e))?;

        Ok(())
    }

    async fn http_list_tools(
        &self,
        url: &str,
        auth_token: Option<&str>,
        timeout: Duration,
    ) -> Result<Vec<RawMcpTool>, String> {
        let result = self
            .http_jsonrpc(url, auth_token, timeout, "tools/list", json!({}))
            .await?;

        Ok(parse_tools_list(&result))
    }

    // ── Stdio transport helpers ──────────────────────────────────────────

    async fn stdio_connect(
        &self,
        command: &str,
        args: &[String],
        env_vars: &HashMap<String, String>,
        timeout: Duration,
    ) -> Result<(McpTransport, Vec<RawMcpTool>), String> {
        use tokio::io::{AsyncWriteExt, BufReader};
        use tokio::process::Command;

        let mut cmd = Command::new(command);
        cmd.args(args)
            .envs(env_vars)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Failed to spawn MCP stdio server '{}': {}", command, e))?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "Failed to capture stdin".to_string())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "Failed to capture stdout".to_string())?;

        let stdin_mutex = tokio::sync::Mutex::new(stdin);
        let stdout_mutex = tokio::sync::Mutex::new(BufReader::new(stdout));

        // Initialize — kill child on failure to prevent zombie processes
        let init_result = self
            .stdio_request(
                &stdin_mutex,
                &stdout_mutex,
                "initialize",
                json!({
                    "protocolVersion": "2025-03-26",
                    "capabilities": {
                        "tools": { "listChanged": true }
                    },
                    "clientInfo": {
                        "name": "GeminiHydra",
                        "version": "15.0.0"
                    }
                }),
                timeout,
            )
            .await;

        if let Err(e) = init_result {
            tracing::error!(
                "MCP stdio init failed for '{}', killing child process: {}",
                command,
                e
            );
            let _ = child.kill().await;
            return Err(e);
        }

        // Send initialized notification
        {
            let notif = json!({
                "jsonrpc": "2.0",
                "method": "notifications/initialized",
                "params": {}
            });
            let mut line = serde_json::to_string(&notif).unwrap_or_default();
            line.push('\n');
            let mut guard = stdin_mutex.lock().await;
            if let Err(e) = guard.write_all(line.as_bytes()).await {
                tracing::error!(
                    "MCP stdio notification failed for '{}', killing child process: {}",
                    command,
                    e
                );
                let _ = child.kill().await;
                return Err(format!("Failed to send initialized notification: {}", e));
            }
            if let Err(e) = guard.flush().await {
                tracing::error!(
                    "MCP stdio flush failed for '{}', killing child process: {}",
                    command,
                    e
                );
                let _ = child.kill().await;
                return Err(format!("Failed to flush stdin: {}", e));
            }
        }

        // List tools — kill child on failure
        let tools_result = self
            .stdio_request(
                &stdin_mutex,
                &stdout_mutex,
                "tools/list",
                json!({}),
                timeout,
            )
            .await;

        let tools_result = match tools_result {
            Ok(result) => result,
            Err(e) => {
                tracing::error!(
                    "MCP stdio tools/list failed for '{}', killing child process: {}",
                    command,
                    e
                );
                let _ = child.kill().await;
                return Err(e);
            }
        };

        let tools = parse_tools_list(&tools_result);

        let transport = McpTransport::Stdio {
            _child: Box::new(tokio::sync::Mutex::new(child)),
            stdin: stdin_mutex,
            stdout: stdout_mutex,
        };

        Ok((transport, tools))
    }

    async fn stdio_request(
        &self,
        stdin: &tokio::sync::Mutex<tokio::process::ChildStdin>,
        stdout: &tokio::sync::Mutex<tokio::io::BufReader<tokio::process::ChildStdout>>,
        method: &str,
        params: Value,
        timeout: Duration,
    ) -> Result<Value, String> {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt};

        let id = self.next_id();
        let id_str = id.to_string();
        let request = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });

        let mut line = serde_json::to_string(&request)
            .map_err(|e| format!("Failed to serialize JSON-RPC request: {}", e))?;
        line.push('\n');

        // Write request
        {
            let mut guard = stdin.lock().await;
            guard
                .write_all(line.as_bytes())
                .await
                .map_err(|e| format!("Failed to write to MCP stdio stdin: {}", e))?;
            guard
                .flush()
                .await
                .map_err(|e| format!("Failed to flush MCP stdio stdin: {}", e))?;
        }

        // Read response (line-delimited JSON-RPC)
        let response = tokio::time::timeout(timeout, async {
            let mut guard = stdout.lock().await;
            loop {
                let mut buf = String::new();
                let n = guard
                    .read_line(&mut buf)
                    .await
                    .map_err(|e| format!("MCP stdio read error: {}", e))?;
                if n == 0 {
                    return Err("MCP stdio: EOF while reading response".to_string());
                }
                let buf = buf.trim();
                if buf.is_empty() {
                    continue;
                }
                let parsed: Value = serde_json::from_str(buf)
                    .map_err(|e| format!("MCP stdio: invalid JSON response: {}", e))?;
                // Match by id (skip notifications)
                let resp_id = parsed.get("id");
                let matches = resp_id
                    .map(|v| v.as_u64() == Some(id) || v.as_str() == Some(&id_str))
                    .unwrap_or(false);

                if matches {
                    if let Some(error) = parsed.get("error") {
                        return Err(format!("MCP JSON-RPC error: {}", error));
                    }
                    return Ok(parsed.get("result").cloned().unwrap_or(json!(null)));
                }
                // else: notification or mismatched id, skip
            }
        })
        .await
        .map_err(|_| format!("MCP stdio: timeout waiting for response to '{}'", method))??;

        Ok(response)
    }

    async fn stdio_call_tool(
        &self,
        stdin: &tokio::sync::Mutex<tokio::process::ChildStdin>,
        stdout: &tokio::sync::Mutex<tokio::io::BufReader<tokio::process::ChildStdout>>,
        tool_name: &str,
        arguments: &Value,
        timeout: Duration,
    ) -> Result<String, String> {
        let result = self
            .stdio_request(
                stdin,
                stdout,
                "tools/call",
                json!({
                    "name": tool_name,
                    "arguments": arguments,
                }),
                timeout,
            )
            .await?;

        extract_tool_result(&result)
    }

    // ── Build Gemini tool declarations for MCP tools ────────────────────

    /// Generate Gemini `function_declarations` for all connected MCP tools.
    pub async fn build_gemini_tool_declarations(&self) -> Vec<Value> {
        let all_tools = self.list_all_tools().await;
        all_tools
            .iter()
            .map(|t| {
                let desc = t.description.as_deref().unwrap_or("External MCP tool");
                let full_desc = format!("[PREFERRED — MCP: {}] {}", t.server_name, desc);
                json!({
                    "name": t.prefixed_name,
                    "description": full_desc,
                    "parameters": t.input_schema,
                })
            })
            .collect()
    }
}

// ── Raw tool (before prefixing) ──────────────────────────────────────────────

struct RawMcpTool {
    name: String,
    description: Option<String>,
    input_schema: Value,
}

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Parse tools/list result into raw tool descriptors.
fn parse_tools_list(result: &Value) -> Vec<RawMcpTool> {
    let tools_array = result
        .get("tools")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    tools_array
        .iter()
        .filter_map(|t| {
            let name = t.get("name")?.as_str()?.to_string();
            if name.is_empty() {
                return None;
            }
            let description = t
                .get("description")
                .and_then(|d| d.as_str())
                .map(String::from);
            let input_schema = t
                .get("inputSchema")
                .cloned()
                .unwrap_or(json!({"type": "object", "properties": {}}));
            Some(RawMcpTool {
                name,
                description,
                input_schema,
            })
        })
        .collect()
}

/// Build prefixed McpTool list from raw tools.
fn build_tool_list(
    raw_tools: &[RawMcpTool],
    sanitized_name: &str,
    server_name: &str,
    server_id: &str,
) -> Vec<McpTool> {
    raw_tools
        .iter()
        .map(|t| {
            let prefixed = format!("mcp_{}_{}", sanitized_name, t.name);
            McpTool {
                name: t.name.clone(),
                prefixed_name: prefixed,
                server_name: server_name.to_string(),
                server_id: server_id.to_string(),
                description: t.description.clone(),
                input_schema: t.input_schema.clone(),
            }
        })
        .collect()
}

/// Extract text content from a tools/call result.
fn extract_tool_result(result: &Value) -> Result<String, String> {
    // Check for isError flag
    let is_error = result
        .get("isError")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // MCP tools/call result is { content: [{ type: "text", text: "..." }] }
    if let Some(content) = result.get("content")
        && let Some(arr) = content.as_array()
    {
        let mut text_parts: Vec<String> = Vec::new();
        for part in arr {
            let part_type = part.get("type").and_then(|v| v.as_str()).unwrap_or("");
            match part_type {
                "text" => {
                    if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                        text_parts.push(text.to_string());
                    }
                }
                "image" | "resource" => {
                    text_parts.push(format!("[{} content]", part_type));
                }
                _ => {}
            }
        }
        if !text_parts.is_empty() {
            let combined = text_parts.join("\n");
            return if is_error {
                Err(combined)
            } else {
                Ok(combined)
            };
        }
        let serialized = content.to_string();
        return if is_error {
            Err(serialized)
        } else {
            Ok(serialized)
        };
    }

    let fallback = result.to_string();
    if is_error {
        Err(fallback)
    } else {
        Ok(fallback)
    }
}

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

    #[test]
    fn test_extract_tool_result_text() {
        let result = json!({
            "content": [
                { "type": "text", "text": "Hello world" }
            ]
        });
        assert_eq!(extract_tool_result(&result), Ok("Hello world".to_string()));
    }

    #[test]
    fn test_extract_tool_result_error() {
        let result = json!({
            "content": [
                { "type": "text", "text": "Something failed" }
            ],
            "isError": true
        });
        assert_eq!(
            extract_tool_result(&result),
            Err("Something failed".to_string())
        );
    }
}
