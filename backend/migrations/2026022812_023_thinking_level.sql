-- Add thinking_level setting for Gemini 3 thinkingConfig
-- Valid values: 'none', 'minimal', 'low', 'medium', 'high'
ALTER TABLE gh_settings ADD COLUMN IF NOT EXISTS thinking_level TEXT NOT NULL DEFAULT 'medium';
