-- GeminiHydra v15 — initial schema

-- gh_settings (singleton)
CREATE TABLE IF NOT EXISTS gh_settings (
    id INTEGER PRIMARY KEY DEFAULT 1 CHECK (id = 1),
    temperature DOUBLE PRECISION NOT NULL DEFAULT 1.0,
    max_tokens INTEGER NOT NULL DEFAULT 65536,
    default_model TEXT NOT NULL DEFAULT 'gemini-3-flash-preview',
    language TEXT NOT NULL DEFAULT 'en',
    theme TEXT NOT NULL DEFAULT 'dark',
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
INSERT INTO gh_settings (id) VALUES (1) ON CONFLICT DO NOTHING;

-- gh_chat_messages
CREATE TABLE IF NOT EXISTS gh_chat_messages (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    model TEXT,
    agent TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_gh_chat_created ON gh_chat_messages (created_at DESC);

-- gh_memories
CREATE TABLE IF NOT EXISTS gh_memories (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    agent TEXT NOT NULL,
    content TEXT NOT NULL,
    importance DOUBLE PRECISION NOT NULL DEFAULT 0.5,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_gh_mem_agent ON gh_memories (agent);
CREATE INDEX IF NOT EXISTS idx_gh_mem_importance ON gh_memories (importance DESC);

-- gh_knowledge_nodes + gh_knowledge_edges
CREATE TABLE IF NOT EXISTS gh_knowledge_nodes (
    id TEXT PRIMARY KEY,
    node_type TEXT NOT NULL,
    label TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS gh_knowledge_edges (
    source TEXT NOT NULL REFERENCES gh_knowledge_nodes(id) ON DELETE CASCADE,
    target TEXT NOT NULL REFERENCES gh_knowledge_nodes(id) ON DELETE CASCADE,
    label TEXT NOT NULL,
    PRIMARY KEY (source, target, label)
);
