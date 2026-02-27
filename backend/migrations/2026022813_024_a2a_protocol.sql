-- A2A v0.3 Protocol — Agent-to-Agent communication tables

-- Task tracking (A2A Task lifecycle)
CREATE TABLE IF NOT EXISTS gh_a2a_tasks (
    id TEXT PRIMARY KEY,
    parent_task_id TEXT REFERENCES gh_a2a_tasks(id),
    agent_id TEXT NOT NULL,
    caller_agent_id TEXT,
    status TEXT NOT NULL DEFAULT 'submitted',
    prompt TEXT NOT NULL,
    result TEXT,
    error_message TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Message history per task
CREATE TABLE IF NOT EXISTS gh_a2a_messages (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL REFERENCES gh_a2a_tasks(id) ON DELETE CASCADE,
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    agent_id TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Artifacts produced by tasks
CREATE TABLE IF NOT EXISTS gh_a2a_artifacts (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL REFERENCES gh_a2a_tasks(id) ON DELETE CASCADE,
    name TEXT,
    mime_type TEXT NOT NULL DEFAULT 'text/plain',
    data TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_a2a_tasks_status ON gh_a2a_tasks(status);
CREATE INDEX IF NOT EXISTS idx_a2a_tasks_agent ON gh_a2a_tasks(agent_id);
CREATE INDEX IF NOT EXISTS idx_a2a_messages_task ON gh_a2a_messages(task_id);
CREATE INDEX IF NOT EXISTS idx_a2a_artifacts_task ON gh_a2a_artifacts(task_id);
