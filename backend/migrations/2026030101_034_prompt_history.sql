-- Global prompt history — persists across sessions, survives restarts
CREATE TABLE IF NOT EXISTS gh_prompt_history (
    id SERIAL PRIMARY KEY,
    content TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_gh_prompt_history_created ON gh_prompt_history(created_at DESC);
