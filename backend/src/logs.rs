// Jaskier Shared Pattern — logs
// Centralized log endpoints for the Logs View.

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

#[derive(Deserialize)]
pub struct AuditLogsQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub action: Option<String>,
    pub search: Option<String>,
}

#[derive(Deserialize)]
pub struct FlyioLogsQuery {
    pub app: Option<String>,
}

#[derive(Deserialize)]
pub struct ActivityLogsQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Deserialize)]
pub struct UsageLogsQuery {
    pub agent_id: Option<String>,
    pub model: Option<String>,
    pub tier: Option<String>,
    pub days: Option<i64>,
    pub limit: Option<i64>,
}

#[derive(Deserialize)]
pub struct LeaderboardQuery {
    pub days: Option<i64>,
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

// ── GET /api/logs/audit ─────────────────────────────────────────────

pub async fn audit_logs(
    State(state): State<AppState>,
    Query(q): Query<AuditLogsQuery>,
) -> Json<Value> {
    let limit = q.limit.unwrap_or(50).min(200);
    let offset = q.offset.unwrap_or(0).max(0);

    // Count total
    let total: i64 = sqlx::query_scalar(
        concat!(
            "SELECT COUNT(*)::bigint FROM gh_audit_log",
            " WHERE ($1::text IS NULL OR action = $1)",
            " AND ($2::text IS NULL OR details::text ILIKE '%' || $2 || '%')",
        ),
    )
    .bind(q.action.as_deref())
    .bind(q.search.as_deref())
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    // Fetch rows
    let rows = sqlx::query_as::<_, (i32, chrono::DateTime<chrono::Utc>, String, Option<Value>, Option<String>)>(
        concat!(
            "SELECT id, timestamp, action, details, ip_address FROM gh_audit_log",
            " WHERE ($1::text IS NULL OR action = $1)",
            " AND ($2::text IS NULL OR details::text ILIKE '%' || $2 || '%')",
            " ORDER BY timestamp DESC LIMIT $3 OFFSET $4",
        ),
    )
    .bind(q.action.as_deref())
    .bind(q.search.as_deref())
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let logs: Vec<Value> = rows
        .into_iter()
        .map(|(id, ts, action, details, ip)| {
            json!({
                "id": id,
                "timestamp": ts.to_rfc3339(),
                "action": action,
                "details": details,
                "ip_address": ip,
            })
        })
        .collect();

    Json(json!({ "logs": logs, "total": total }))
}

// ── GET /api/logs/flyio ─────────────────────────────────────────────

pub async fn flyio_logs(
    State(state): State<AppState>,
    Query(q): Query<FlyioLogsQuery>,
) -> Json<Value> {
    let app = match q.app.as_deref() {
        Some(a) if !a.is_empty() => a.to_string(),
        _ => return Json(json!({ "error": "app parameter is required" })),
    };

    // Read PAT from env var (GH does not have service_tokens module)
    let token = match std::env::var("FLY_API_TOKEN") {
        Ok(t) if !t.is_empty() => t,
        _ => return Json(json!({ "error": "FLY_API_TOKEN not configured" })),
    };

    let query = json!({
        "query": format!(
            r#"query {{
                app(name: "{}") {{
                    name
                    status
                    currentRelease {{ version createdAt status }}
                    allocations {{
                        id
                        region
                        status
                        version
                        recentLogs(limit: 50) {{
                            id
                            message
                            timestamp
                            level
                            region
                        }}
                    }}
                }}
            }}"#,
            app
        )
    });

    let resp = state
        .client
        .post("https://api.fly.io/graphql")
        .header("authorization", format!("Bearer {}", token))
        .header("content-type", "application/json")
        .json(&query)
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await;

    match resp {
        Ok(r) if r.status().is_success() => {
            let body: Value = r.json().await.unwrap_or(json!({}));
            let app_data = body
                .get("data")
                .and_then(|d| d.get("app"))
                .cloned()
                .unwrap_or(json!(null));
            Json(json!({ "app": app_data }))
        }
        Ok(r) => {
            let status = r.status().as_u16();
            let text = r.text().await.unwrap_or_default();
            Json(json!({ "error": format!("Fly.io API error {}: {}", status, text) }))
        }
        Err(e) => Json(json!({ "error": format!("Fly.io request failed: {}", e) })),
    }
}

// ── GET /api/logs/activity ──────────────────────────────────────────

pub async fn activity_logs(
    State(state): State<AppState>,
    Query(q): Query<ActivityLogsQuery>,
) -> Json<Value> {
    let limit = q.limit.unwrap_or(50).min(200);
    let offset = q.offset.unwrap_or(0).max(0);

    let total: i64 = sqlx::query_scalar("SELECT COUNT(*)::bigint FROM gh_sessions")
        .fetch_one(&state.db)
        .await
        .unwrap_or(0);

    let rows = sqlx::query_as::<_, (
        uuid::Uuid,
        String,
        chrono::DateTime<chrono::Utc>,
        chrono::DateTime<chrono::Utc>,
    )>(
        concat!(
            "SELECT s.id, s.title, s.created_at, s.updated_at",
            " FROM gh_sessions s",
            " ORDER BY s.updated_at DESC",
            " LIMIT $1 OFFSET $2",
        ),
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    // Get message counts per session
    let session_ids: Vec<uuid::Uuid> = rows.iter().map(|r| r.0).collect();
    let msg_counts: std::collections::HashMap<uuid::Uuid, i64> = if !session_ids.is_empty() {
        sqlx::query_as::<_, (uuid::Uuid, i64)>(
            "SELECT session_id, COUNT(*)::bigint FROM gh_chat_messages WHERE session_id = ANY($1) GROUP BY session_id",
        )
        .bind(&session_ids)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default()
        .into_iter()
        .collect()
    } else {
        std::collections::HashMap::new()
    };

    let logs: Vec<Value> = rows
        .into_iter()
        .map(|(id, title, created, updated)| {
            let count = msg_counts.get(&id).copied().unwrap_or(0);
            json!({
                "id": id.to_string(),
                "title": title,
                "created_at": created.to_rfc3339(),
                "updated_at": updated.to_rfc3339(),
                "message_count": count,
            })
        })
        .collect();

    Json(json!({ "logs": logs, "total": total }))
}

// ── GET /api/logs/usage ───────────────────────────────────────────

pub async fn usage_logs(
    State(state): State<AppState>,
    Query(q): Query<UsageLogsQuery>,
) -> Json<Value> {
    let days = q.days.unwrap_or(7).min(90).max(1);
    let limit = q.limit.unwrap_or(200).min(1000);

    let rows = sqlx::query_as::<_, (
        i32, Option<String>, String, i32, i32, i32, i32, bool, Option<String>, chrono::DateTime<chrono::Utc>,
    )>(
        concat!(
            "SELECT id, agent_id, model, input_tokens, output_tokens, total_tokens, latency_ms, success, tier, created_at",
            " FROM gh_agent_usage",
            " WHERE created_at > NOW() - ($1::bigint || ' days')::interval",
            " AND ($2::text IS NULL OR agent_id = $2)",
            " AND ($3::text IS NULL OR model = $3)",
            " AND ($4::text IS NULL OR tier = $4)",
            " ORDER BY created_at DESC LIMIT $5",
        ),
    )
    .bind(days)
    .bind(q.agent_id.as_deref())
    .bind(q.model.as_deref())
    .bind(q.tier.as_deref())
    .bind(limit)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let logs: Vec<Value> = rows
        .into_iter()
        .map(|(id, agent_id, model, inp, out, total, lat, success, tier, created)| {
            json!({
                "id": id,
                "agent_id": agent_id,
                "model": model,
                "input_tokens": inp,
                "output_tokens": out,
                "total_tokens": total,
                "latency_ms": lat,
                "success": success,
                "tier": tier,
                "created_at": created.to_rfc3339(),
            })
        })
        .collect();

    Json(json!({ "logs": logs, "total": logs.len() }))
}

// ── GET /api/logs/leaderboard ─────────────────────────────────────

pub async fn leaderboard(
    State(state): State<AppState>,
    Query(q): Query<LeaderboardQuery>,
) -> Json<Value> {
    let days = q.days.unwrap_or(7).min(90).max(1);

    let rows = sqlx::query_as::<_, (Option<String>, i64, f64, f64, f64)>(
        concat!(
            "SELECT agent_id, COUNT(*)::bigint AS total_calls,",
            " AVG(latency_ms)::float8 AS avg_latency,",
            " (SUM(CASE WHEN success THEN 1 ELSE 0 END)::float8 / NULLIF(COUNT(*), 0)::float8 * 100) AS success_rate,",
            " AVG(total_tokens)::float8 AS avg_tokens",
            " FROM gh_agent_usage",
            " WHERE created_at > NOW() - ($1::bigint || ' days')::interval",
            " GROUP BY agent_id ORDER BY total_calls DESC",
        ),
    )
    .bind(days)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let entries: Vec<Value> = rows
        .into_iter()
        .map(|(agent_id, calls, avg_lat, success_rate, avg_tokens)| {
            json!({
                "agent_id": agent_id,
                "total_calls": calls,
                "avg_latency_ms": (avg_lat * 10.0).round() / 10.0,
                "success_rate": (success_rate * 10.0).round() / 10.0,
                "avg_tokens": (avg_tokens * 10.0).round() / 10.0,
            })
        })
        .collect();

    Json(json!({ "entries": entries, "days": days }))
}
