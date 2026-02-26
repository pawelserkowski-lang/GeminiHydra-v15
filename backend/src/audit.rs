// Jaskier Shared Pattern — audit
//! Audit logging for sensitive operations.
//!
//! Writes structured entries to `gh_audit_log` for post-hoc review.
//! Failures are logged but never propagate — auditing must not break
//! the request that triggered it.

use serde_json::Value;
use sqlx::PgPool;

/// Insert an audit log entry. Failures are silently logged (fire-and-forget).
///
/// # Arguments
/// * `pool` — Database connection pool.
/// * `action` — Machine-readable action name (e.g. "delete_session", "pin_model").
/// * `details` — Arbitrary JSON payload with context (IDs, old/new values, etc.).
/// * `ip` — Client IP address if available.
pub async fn log_audit(pool: &PgPool, action: &str, details: Value, ip: Option<&str>) {
    if let Err(e) = sqlx::query(
        "INSERT INTO gh_audit_log (action, details, ip_address) VALUES ($1, $2, $3)",
    )
    .bind(action)
    .bind(&details)
    .bind(ip)
    .execute(pool)
    .await
    {
        tracing::warn!(action = %action, "audit log insert failed: {}", e);
    }
}
