-- Per-agent thinking level override + A/B testing fields
ALTER TABLE gh_agents ADD COLUMN IF NOT EXISTS thinking_level TEXT DEFAULT NULL;
ALTER TABLE gh_agents ADD COLUMN IF NOT EXISTS model_b TEXT DEFAULT NULL;
ALTER TABLE gh_agents ADD COLUMN IF NOT EXISTS ab_split REAL DEFAULT NULL;
