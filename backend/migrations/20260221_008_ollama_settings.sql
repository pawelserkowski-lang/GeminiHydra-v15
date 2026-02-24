-- 20260218_008_ollama_settings.sql
ALTER TABLE gh_settings ADD COLUMN IF NOT EXISTS ollama_url TEXT NOT NULL DEFAULT 'http://localhost:11434';
