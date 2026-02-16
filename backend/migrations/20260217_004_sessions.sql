-- GeminiHydra v15: Session persistence
CREATE TABLE IF NOT EXISTS gh_sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    title TEXT NOT NULL DEFAULT 'New Chat',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Add session FK to existing messages
ALTER TABLE gh_chat_messages ADD COLUMN IF NOT EXISTS session_id UUID REFERENCES gh_sessions(id) ON DELETE CASCADE;

-- Create a "Legacy Chat" session for existing orphan messages
INSERT INTO gh_sessions (id, title) VALUES ('00000000-0000-0000-0000-000000000001', 'Legacy Chat');
UPDATE gh_chat_messages SET session_id = '00000000-0000-0000-0000-000000000001' WHERE session_id IS NULL;

CREATE INDEX IF NOT EXISTS idx_gh_sessions_updated ON gh_sessions(updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_gh_messages_session ON gh_chat_messages(session_id, created_at);
