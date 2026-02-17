-- 20260218_009_rag_and_sandbox.sql

-- 1. Enable pgvector extension (requires pgvector installed on DB server)
CREATE EXTENSION IF NOT EXISTS vector;

-- 2. Create embeddings table
CREATE TABLE IF NOT EXISTS gh_file_embeddings (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    file_path TEXT NOT NULL,
    chunk_index INTEGER NOT NULL,
    content TEXT NOT NULL,
    embedding vector(768), -- Dimension for Gemini/Ollama embeddings (often 768 or 1536)
    created_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(file_path, chunk_index)
);

-- 3. Add sandbox settings
ALTER TABLE gh_settings ADD COLUMN IF NOT EXISTS use_docker_sandbox BOOLEAN DEFAULT FALSE;
