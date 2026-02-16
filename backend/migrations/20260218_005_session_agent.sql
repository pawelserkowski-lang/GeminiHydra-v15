-- GeminiHydra v15: Lock agent per session
-- When an agent is assigned to a session, all follow-up messages stay with that agent.
ALTER TABLE gh_sessions ADD COLUMN IF NOT EXISTS agent_id TEXT;
