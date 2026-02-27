-- Audit log for sensitive operations (delete session, change settings, pin model, etc.)
-- Jaskier Shared Pattern — audit

CREATE TABLE IF NOT EXISTS gh_audit_log (
    id SERIAL PRIMARY KEY,
    timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    action VARCHAR(100) NOT NULL,
    details JSONB,
    ip_address VARCHAR(45)
);

CREATE INDEX IF NOT EXISTS idx_gh_audit_log_timestamp ON gh_audit_log(timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_gh_audit_log_action ON gh_audit_log(action);
