// Jaskier Shared Pattern -- mcp
//! MCP (Model Context Protocol) support — client + server.
//!
//! **Client** (`McpClientManager`): connects to external MCP servers, discovers
//! their tools, and proxies `tools/call` requests so Gemini agents can use them.
//!
//! **Server** (`mcp_handler`): exposes GeminiHydra's 31+ native tools as an MCP
//! endpoint that external clients can call via JSON-RPC 2.0 over HTTP.
//!
//! **Config** (`config`): CRUD for `gh_mcp_servers` + `gh_mcp_discovered_tools`.
//!
//! Protocol: JSON-RPC 2.0 over HTTP (lightweight, no stdio transport needed).
//! Spec: <https://spec.modelcontextprotocol.io/2024-11-05/>

pub mod client;
pub mod config;
pub mod server;

use crate::{auth, state::AppState};
use axum::{
    Router, middleware,
    routing::{get, patch, post},
};

pub fn mcp_router(state: AppState) -> Router<AppState> {
    Router::new()
        .route(
            "/api/mcp/servers",
            get(config::mcp_server_list).post(config::mcp_server_create),
        )
        .route(
            "/api/mcp/servers/{id}",
            patch(config::mcp_server_update).delete(config::mcp_server_delete),
        )
        .route(
            "/api/mcp/servers/{id}/connect",
            post(config::mcp_server_connect),
        )
        .route(
            "/api/mcp/servers/{id}/disconnect",
            post(config::mcp_server_disconnect),
        )
        .route("/api/mcp/servers/{id}/tools", get(config::mcp_server_tools))
        .route("/api/mcp/tools", get(config::mcp_all_tools))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth::require_auth,
        ))
}
