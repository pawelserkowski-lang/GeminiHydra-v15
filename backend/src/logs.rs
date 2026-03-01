// Jaskier Shared Pattern — logs
// Backend log endpoints for the Logs View.

use axum::extract::{Query, State};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::state::AppState;

// ── Query parameters ────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct BackendLogsQuery {
    pub limit: Option<usize>,
    pub level: Option<String>,
    pub search: Option<String>,
}

// ── GET /api/logs/backend ───────────────────────────────────────────

pub async fn backend_logs(
    State(state): State<AppState>,
    Query(q): Query<BackendLogsQuery>,
) -> Json<Value> {
    let limit = q.limit.unwrap_or(200).min(500);
    let entries = state.log_buffer.recent(
        limit,
        q.level.as_deref(),
        q.search.as_deref(),
    );
    Json(json!({ "logs": entries, "total": entries.len() }))
}

// ── DELETE /api/logs/backend ────────────────────────────────────────

pub async fn clear_backend_logs(State(state): State<AppState>) -> Json<Value> {
    state.log_buffer.clear();
    Json(json!({ "cleared": true }))
}
