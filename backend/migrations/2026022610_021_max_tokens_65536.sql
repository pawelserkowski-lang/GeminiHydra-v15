-- Increase max_tokens to 65536 (full Gemini 3.1 Pro output capacity)
UPDATE gh_settings SET max_tokens = 65536 WHERE id = 1;
ALTER TABLE gh_settings ALTER COLUMN max_tokens SET DEFAULT 65536;
