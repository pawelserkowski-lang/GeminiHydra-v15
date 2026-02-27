-- ADK orchestration settings
ALTER TABLE gh_settings ADD COLUMN IF NOT EXISTS use_adk_orchestration BOOLEAN DEFAULT FALSE;
ALTER TABLE gh_settings ADD COLUMN IF NOT EXISTS adk_default_pattern TEXT DEFAULT 'hierarchical';

-- Extend A2A tasks for orchestration metadata
ALTER TABLE gh_a2a_tasks ADD COLUMN IF NOT EXISTS orchestration_pattern TEXT;
ALTER TABLE gh_a2a_tasks ADD COLUMN IF NOT EXISTS participating_agents TEXT[];
