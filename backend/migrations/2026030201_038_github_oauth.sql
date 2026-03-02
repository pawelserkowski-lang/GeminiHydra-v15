-- GitHub OAuth token storage
CREATE TABLE IF NOT EXISTS gh_oauth_github (
    id INTEGER PRIMARY KEY DEFAULT 1,
    access_token TEXT NOT NULL,
    token_type TEXT NOT NULL DEFAULT 'bearer',
    scope TEXT NOT NULL DEFAULT '',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT gh_oauth_github_singleton CHECK (id = 1)
);
