-- Working directory setting for filesystem tools
-- Empty string = no working directory set (tools use absolute paths only)
ALTER TABLE gh_settings ADD COLUMN IF NOT EXISTS working_directory TEXT NOT NULL DEFAULT '';
