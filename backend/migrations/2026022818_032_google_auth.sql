-- Google Auth credentials (replaces Anthropic OAuth)
-- Stores either Google OAuth tokens OR user-provided API key (encrypted)

DROP TABLE IF EXISTS gh_oauth_tokens;

CREATE TABLE IF NOT EXISTS gh_google_auth (
    id INTEGER PRIMARY KEY DEFAULT 1,
    auth_method TEXT NOT NULL DEFAULT 'api_key',
    access_token TEXT DEFAULT '',
    refresh_token TEXT DEFAULT '',
    expires_at BIGINT DEFAULT 0,
    api_key_encrypted TEXT DEFAULT '',
    user_email TEXT DEFAULT '',
    user_name TEXT DEFAULT '',
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    CONSTRAINT gh_google_auth_singleton CHECK (id = 1)
);
