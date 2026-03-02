-- Generic service token storage (Fly.io PAT, etc.)
CREATE TABLE IF NOT EXISTS gh_service_tokens (
    id SERIAL PRIMARY KEY,
    service TEXT NOT NULL UNIQUE,
    encrypted_token TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
