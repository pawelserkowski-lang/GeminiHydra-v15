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
