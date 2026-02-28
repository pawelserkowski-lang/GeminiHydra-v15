-- MCP (Model Context Protocol) support tables
-- Phase 9+10: MCP Client & Server for GeminiHydra v15

-- Configured MCP server connections
CREATE TABLE IF NOT EXISTS gh_mcp_servers (
    id          TEXT PRIMARY KEY DEFAULT gen_random_uuid()::TEXT,
    name        TEXT NOT NULL UNIQUE,
    transport   TEXT NOT NULL CHECK (transport IN ('stdio', 'http')),
    command     TEXT,                          -- stdio: command to spawn
    args        TEXT NOT NULL DEFAULT '[]',    -- stdio: JSON array of arguments
    env_vars    TEXT NOT NULL DEFAULT '{}',    -- stdio: JSON object of env vars
    url         TEXT,                          -- http: endpoint URL
    enabled     BOOLEAN NOT NULL DEFAULT true,
    auth_token  TEXT,                          -- http: Bearer token
    timeout_secs INTEGER NOT NULL DEFAULT 30,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Tools discovered from connected MCP servers
CREATE TABLE IF NOT EXISTS gh_mcp_discovered_tools (
    id            TEXT PRIMARY KEY DEFAULT gen_random_uuid()::TEXT,
    server_id     TEXT NOT NULL REFERENCES gh_mcp_servers(id) ON DELETE CASCADE,
    tool_name     TEXT NOT NULL,
    description   TEXT,
    input_schema  TEXT NOT NULL DEFAULT '{}',
    discovered_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(server_id, tool_name)
);
