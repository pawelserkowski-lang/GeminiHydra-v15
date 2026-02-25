-- Expression index for LOWER(agent) queries on gh_memories
-- Optimizes WHERE LOWER(agent) = LOWER($1) in sessions.rs
CREATE INDEX IF NOT EXISTS idx_gh_mem_agent_lower ON gh_memories (LOWER(agent));
