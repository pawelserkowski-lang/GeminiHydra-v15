-- OAuth tokens for Anthropic Claude MAX Plan (singleton row)
CREATE TABLE IF NOT EXISTS gh_oauth_tokens (
    id INTEGER PRIMARY KEY DEFAULT 1,
    access_token TEXT NOT NULL,
    refresh_token TEXT NOT NULL,
    expires_at BIGINT NOT NULL,
    scope TEXT DEFAULT '',
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    CONSTRAINT gh_oauth_tokens_singleton CHECK (id = 1)
);
