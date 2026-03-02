-- Vercel OAuth token storage
CREATE TABLE IF NOT EXISTS gh_oauth_vercel (
    id INTEGER PRIMARY KEY DEFAULT 1,
    access_token TEXT NOT NULL,
    team_id TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT gh_oauth_vercel_singleton CHECK (id = 1)
);
