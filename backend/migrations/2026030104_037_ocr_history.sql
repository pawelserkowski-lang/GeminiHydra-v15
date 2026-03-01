-- OCR History — persists OCR results for search and re-use
CREATE TABLE IF NOT EXISTS gh_ocr_history (
    id               UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    filename         TEXT,
    mime_type        TEXT NOT NULL,
    preset           TEXT,
    text             TEXT NOT NULL,
    pages_json       JSONB NOT NULL DEFAULT '[]',
    total_pages      INTEGER NOT NULL DEFAULT 1,
    confidence       DOUBLE PRECISION,
    provider         TEXT NOT NULL,
    processing_time_ms BIGINT NOT NULL DEFAULT 0,
    detected_preset  TEXT,
    structured_data  JSONB,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_gh_ocr_hist_created ON gh_ocr_history(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_gh_ocr_hist_filename ON gh_ocr_history(filename);
