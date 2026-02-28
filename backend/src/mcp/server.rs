// Jaskier Shared Pattern -- mcp/server
//! MCP Server — exposes GeminiHydra's native tools as an MCP endpoint.
//!
//! External MCP clients can discover and call GeminiHydra's 31+ tools via
//! JSON-RPC 2.0 over HTTP POST at `/mcp`.
//!
//! Supported methods:
//! - `initialize` — server info + capabilities
//! - `notifications/initialized` — client ack (no-op)
//! - `tools/list` — list all available tools
//! - `tools/call` — execute a tool
//! - `resources/list` — list available resources (agents, sessions, system)
//! - `resources/read` — read a resource by URI
//! - `ping` — health check

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde_json::{json, Value};

use crate::state::AppState;
use crate::tools;

/// MCP JSON-RPC 2.0 endpoint handler.
///
/// Routes incoming JSON-RPC requests to the appropriate MCP method handler.
/// Supports both single requests and notifications (no `id` field).
pub async fn mcp_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<Value>,
) -> (StatusCode, Json<Value>) {
    let method = request.get("method").and_then(|m| m.as_str()).unwrap_or("");
    let id = request.get("id").cloned().unwrap_or(Value::Null);

    // Log inbound request (debug level to avoid noise)
    tracing::debug!(method = %method, "MCP server: incoming request");

    // Check for session ID header (optional, for stateful sessions)
    let _session_id = headers
        .get("mcp-session-id")
        .and_then(|v| v.to_str().ok());

    let result = match method {
        "initialize" => handle_initialize(&id),
        "notifications/initialized" => {
            // Client acknowledgment — no response needed for notifications
            return (StatusCode::OK, Json(json!({})));
        }
        "ping" => handle_ping(&id),
        "tools/list" => handle_tools_list(&state, &id).await,
        "tools/call" => handle_tools_call(&state, &request, &id).await,
        "resources/list" => handle_resources_list(&id),
        "resources/read" => handle_resources_read(&state, &request, &id).await,
        _ => json_rpc_error(id, -32601, &format!("Method not found: {}", method)),
    };

    (StatusCode::OK, Json(result))
}

// ── initialize ──────────────────────────────────────────────────────────────

fn handle_initialize(id: &Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": { "listChanged": false },
                "resources": { "subscribe": false, "listChanged": false }
            },
            "serverInfo": {
                "name": "GeminiHydra",
                "version": "15.0.0"
            },
            "instructions": "GeminiHydra v15 — Multi-Agent AI Swarm with 31+ tools for code analysis, file operations, git, GitHub, Vercel, Fly.io, and inter-agent delegation."
        }
    })
}

// ── ping ────────────────────────────────────────────────────────────────────

fn handle_ping(id: &Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {}
    })
}

// ── tools/list ──────────────────────────────────────────────────────────────

async fn handle_tools_list(state: &AppState, id: &Value) -> Value {
    let native_tools = build_mcp_tool_list();

    // Also include MCP tools from connected external servers
    let mcp_tools: Vec<Value> = state.mcp_client.build_gemini_tool_declarations().await;
    let external: Vec<Value> = mcp_tools
        .into_iter()
        .map(|t| {
            json!({
                "name": t.get("name").and_then(|n| n.as_str()).unwrap_or("unknown"),
                "description": t.get("description").and_then(|d| d.as_str()).unwrap_or(""),
                "inputSchema": t.get("parameters").cloned().unwrap_or(json!({"type": "object", "properties": {}})),
            })
        })
        .collect();

    let mut all_tools = native_tools;
    all_tools.extend(external);

    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "tools": all_tools
        }
    })
}

// ── tools/call ──────────────────────────────────────────────────────────────

async fn handle_tools_call(state: &AppState, request: &Value, id: &Value) -> Value {
    let params = request.get("params").cloned().unwrap_or(json!({}));
    let tool_name = params
        .get("name")
        .and_then(|n| n.as_str())
        .unwrap_or("");
    let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

    if tool_name.is_empty() {
        return json_rpc_error(id.clone(), -32602, "Missing 'name' in params");
    }

    tracing::info!(tool = %tool_name, "MCP server: tools/call");

    // Check if it's an MCP-proxied tool first
    if tool_name.starts_with("mcp_") {
        match state.mcp_client.call_tool(tool_name, &arguments).await {
            Ok(text) => {
                return json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [{ "type": "text", "text": text }],
                        "isError": false
                    }
                });
            }
            Err(e) => {
                return json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [{ "type": "text", "text": format!("Error: {}", e) }],
                        "isError": true
                    }
                });
            }
        }
    }

    // Read working_directory from settings for tool path resolution
    let wd: String = sqlx::query_scalar("SELECT working_directory FROM gh_settings WHERE id = 1")
        .fetch_one(&state.db)
        .await
        .unwrap_or_default();

    // Execute native tool
    match tools::execute_tool(tool_name, &arguments, state, &wd).await {
        Ok(output) => {
            let mut content = vec![json!({ "type": "text", "text": output.text })];

            // Include inline data if present (e.g., image analysis results)
            if let Some(data) = &output.inline_data {
                content.push(json!({
                    "type": "image",
                    "data": data.data,
                    "mimeType": data.mime_type,
                }));
            }

            json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "content": content,
                    "isError": false
                }
            })
        }
        Err(e) => json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "content": [{ "type": "text", "text": format!("Error: {}", e) }],
                "isError": true
            }
        }),
    }
}

// ── resources/list ──────────────────────────────────────────────────────────

fn handle_resources_list(id: &Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "resources": [
                {
                    "uri": "geminihydra://agents",
                    "name": "Agent List",
                    "description": "List of all Witcher agents with their specializations",
                    "mimeType": "application/json"
                },
                {
                    "uri": "geminihydra://sessions",
                    "name": "Chat Sessions",
                    "description": "List of chat sessions (summaries only, not messages)",
                    "mimeType": "application/json"
                },
                {
                    "uri": "geminihydra://system",
                    "name": "System Stats",
                    "description": "Current system snapshot (CPU, memory, uptime)",
                    "mimeType": "application/json"
                },
                {
                    "uri": "geminihydra://models",
                    "name": "Model Registry",
                    "description": "Available AI models and active pins",
                    "mimeType": "application/json"
                },
                {
                    "uri": "geminihydra://mcp/servers",
                    "name": "MCP Servers",
                    "description": "Configured MCP server connections and their tools",
                    "mimeType": "application/json"
                }
            ]
        }
    })
}

// ── resources/read ──────────────────────────────────────────────────────────

async fn handle_resources_read(state: &AppState, request: &Value, id: &Value) -> Value {
    let uri = request
        .pointer("/params/uri")
        .and_then(|u| u.as_str())
        .unwrap_or("");

    let content = match uri {
        "geminihydra://agents" => {
            let agents = state.agents.read().await;
            let list: Vec<Value> = agents
                .iter()
                .map(|a| {
                    json!({
                        "id": a.id,
                        "name": a.name,
                        "role": a.role,
                        "status": a.status,
                        "tier": a.tier,
                    })
                })
                .collect();
            serde_json::to_string_pretty(&list).unwrap_or_else(|_| "[]".to_string())
        }

        "geminihydra://sessions" => {
            let rows = sqlx::query_as::<_, crate::models::SessionSummaryRow>(
                "SELECT id, title, created_at, message_count FROM gh_sessions ORDER BY created_at DESC LIMIT 50",
            )
            .fetch_all(&state.db)
            .await
            .unwrap_or_default();
            let summaries: Vec<serde_json::Value> = rows.iter().map(|r| {
                json!({
                    "id": r.id.to_string(),
                    "title": r.title,
                    "created_at": r.created_at.to_rfc3339(),
                    "message_count": r.message_count,
                })
            }).collect();
            serde_json::to_string_pretty(&summaries).unwrap_or_else(|_| "[]".to_string())
        }

        "geminihydra://system" => {
            let snap = state.system_monitor.read().await;
            let uptime = state.start_time.elapsed().as_secs();
            json!({
                "cpu_usage_percent": snap.cpu_usage_percent,
                "memory_used_mb": snap.memory_used_mb,
                "memory_total_mb": snap.memory_total_mb,
                "platform": snap.platform,
                "uptime_seconds": uptime,
                "ready": state.is_ready(),
            })
            .to_string()
        }

        "geminihydra://models" => {
            let cache = state.model_cache.read().await;
            let fetched_ago = cache.fetched_at.map(|t| format!("{}s ago", t.elapsed().as_secs()));
            json!({
                "total_models": cache.models.len(),
                "fetched_ago": fetched_ago,
            })
            .to_string()
        }

        "geminihydra://mcp/servers" => {
            let all_tools = state.mcp_client.list_all_tools().await;
            let servers = crate::mcp::config::list_mcp_servers(&state.db)
                .await
                .unwrap_or_default();
            json!({
                "servers": servers.iter().map(|s| json!({
                    "id": s.id,
                    "name": s.name,
                    "transport": s.transport,
                    "enabled": s.enabled,
                })).collect::<Vec<_>>(),
                "total_external_tools": all_tools.len(),
            })
            .to_string()
        }

        _ => {
            return json_rpc_error(
                id.clone(),
                -32602,
                &format!("Unknown resource URI: {}", uri),
            );
        }
    };

    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "contents": [{
                "uri": uri,
                "mimeType": "application/json",
                "text": content
            }]
        }
    })
}

// ── Native tool list for MCP ────────────────────────────────────────────────

/// Build the MCP `tools/list` response for all native GeminiHydra tools.
/// Maps from Gemini `function_declarations` format to MCP `Tool` format.
fn build_mcp_tool_list() -> Vec<Value> {
    vec![
        mcp_tool("list_directory", "List files and subdirectories in a local directory with sizes and line counts.", json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Absolute path to the directory" },
                "show_hidden": { "type": "boolean", "description": "Include hidden files" }
            },
            "required": ["path"]
        })),
        mcp_tool("read_file", "Read a file from the local filesystem by its absolute path.", json!({
            "type": "object",
            "properties": { "path": { "type": "string", "description": "Absolute path to the file" } },
            "required": ["path"]
        })),
        mcp_tool("read_file_section", "Read specific line range from a file (1-indexed, inclusive). Max 500 lines.", json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Absolute path to the file" },
                "start_line": { "type": "integer", "description": "First line to read (1-indexed)" },
                "end_line": { "type": "integer", "description": "Last line to read (1-indexed)" }
            },
            "required": ["path", "start_line", "end_line"]
        })),
        mcp_tool("search_files", "Search for text/regex patterns across all files in a directory (recursive). Returns matching lines with file paths and line numbers.", json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Directory to search in" },
                "pattern": { "type": "string", "description": "Text or regex pattern (case-insensitive)" },
                "file_extensions": { "type": "string", "description": "Comma-separated extensions to filter" },
                "offset": { "type": "integer", "description": "Matches to skip (pagination)" },
                "limit": { "type": "integer", "description": "Max matches to return (default 80)" },
                "multiline": { "type": "boolean", "description": "Match across line boundaries" }
            },
            "required": ["path", "pattern"]
        })),
        mcp_tool("find_file", "Find files by glob pattern. Returns matching file paths with sizes.", json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Root directory to search" },
                "pattern": { "type": "string", "description": "Glob pattern like '*.tsx' or 'auth*'" }
            },
            "required": ["path", "pattern"]
        })),
        mcp_tool("get_code_structure", "Analyze code structure (functions, classes, structs) via AST. Supports Rust, TypeScript, JavaScript, Python, Go.", json!({
            "type": "object",
            "properties": { "path": { "type": "string", "description": "Absolute path to the source file" } },
            "required": ["path"]
        })),
        mcp_tool("write_file", "Write or create a file on the local filesystem.", json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Absolute path for the file" },
                "content": { "type": "string", "description": "Full file content" }
            },
            "required": ["path", "content"]
        })),
        mcp_tool("edit_file", "Edit an existing file by replacing a specific text section. Safer than write_file for modifications.", json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Absolute path to the file" },
                "old_text": { "type": "string", "description": "Exact text to find (must appear once)" },
                "new_text": { "type": "string", "description": "Replacement text" }
            },
            "required": ["path", "old_text", "new_text"]
        })),
        mcp_tool("diff_files", "Compare two files and show line-by-line differences in unified diff format.", json!({
            "type": "object",
            "properties": {
                "path_a": { "type": "string", "description": "First file path" },
                "path_b": { "type": "string", "description": "Second file path" }
            },
            "required": ["path_a", "path_b"]
        })),
        mcp_tool("execute_command", "Execute a shell command on the local machine. Use for build/test/npm/cargo operations.", json!({
            "type": "object",
            "properties": {
                "command": { "type": "string", "description": "Shell command (Windows cmd.exe)" },
                "working_directory": { "type": "string", "description": "Working directory" }
            },
            "required": ["command"]
        })),
    ]
}

/// Helper to build a single MCP tool object.
fn mcp_tool(name: &str, description: &str, input_schema: Value) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": input_schema,
    })
}

// ── JSON-RPC error helper ───────────────────────────────────────────────────

fn json_rpc_error(id: Value, code: i32, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message
        }
    })
}
