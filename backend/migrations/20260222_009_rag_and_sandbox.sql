-- 20260222_009_rag_and_sandbox.sql

-- 1. Add sandbox settings (always needed)
ALTER TABLE gh_settings ADD COLUMN IF NOT EXISTS use_docker_sandbox BOOLEAN DEFAULT FALSE;

-- 2. Enable pgvector extension + embeddings table (optional — skipped if pgvector not installed)
DO $$
BEGIN
    CREATE EXTENSION IF NOT EXISTS vector;
    CREATE TABLE IF NOT EXISTS gh_file_embeddings (
        id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
        file_path TEXT NOT NULL,
        chunk_index INTEGER NOT NULL,
        content TEXT NOT NULL,
        embedding vector(768),
        created_at TIMESTAMPTZ DEFAULT NOW(),
        UNIQUE(file_path, chunk_index)
    );
EXCEPTION WHEN OTHERS THEN
    RAISE NOTICE 'pgvector not available — skipping gh_file_embeddings table';
END;
$$;
