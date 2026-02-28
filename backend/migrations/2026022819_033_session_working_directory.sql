-- Per-session working directory (overrides global setting per session)
-- Empty string = inherit from gh_settings.working_directory
ALTER TABLE gh_sessions ADD COLUMN IF NOT EXISTS working_directory TEXT NOT NULL DEFAULT '';
