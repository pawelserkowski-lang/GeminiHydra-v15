-- 20260227_016_remove_ollama.sql
-- Drop the ollama_url column as Ollama support has been fully removed from GeminiHydra.

ALTER TABLE gh_settings DROP COLUMN IF EXISTS ollama_url;
