-- Enable trigram extension for ILIKE index support
-- Uses DO/EXCEPTION block for graceful skip on environments where pg_trgm is not available
DO $$
BEGIN
    CREATE EXTENSION IF NOT EXISTS pg_trgm;
EXCEPTION WHEN OTHERS THEN
    RAISE NOTICE 'pg_trgm extension not available, skipping trigram index';
END
$$;

-- GIN trigram index for content ILIKE searches on gh_chat_messages
CREATE INDEX IF NOT EXISTS idx_gh_messages_content_trgm
    ON gh_chat_messages USING gin (content gin_trgm_ops);
