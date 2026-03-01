-- Token usage tracking per agent
CREATE TABLE IF NOT EXISTS gh_agent_usage (
    id SERIAL PRIMARY KEY,
    agent_id TEXT,
    model TEXT NOT NULL,
    input_tokens INT DEFAULT 0,
    output_tokens INT DEFAULT 0,
    total_tokens INT DEFAULT 0,
    latency_ms INT DEFAULT 0,
    success BOOLEAN DEFAULT TRUE,
    tier TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_gh_agent_usage_agent ON gh_agent_usage(agent_id);
CREATE INDEX IF NOT EXISTS idx_gh_agent_usage_created ON gh_agent_usage(created_at);
