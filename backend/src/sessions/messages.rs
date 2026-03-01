//! Message-level handlers: global history CRUD and per-session message operations.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde_json::{json, Value};

use crate::models::{ChatMessage, ChatMessageRow};
use crate::state::AppState;

use super::{AddMessageRequest, HistoryParams, PaginationParams, SearchQuery, MAX_MESSAGE_LENGTH};

// ============================================================================
// History handlers (global, not session-scoped)
// ============================================================================

/// GET /api/history?limit=50&offset=0
#[utoipa::path(get, path = "/api/history", tag = "history",
    params(
        ("limit" = Option<i64>, Query, description = "Max messages to return (default 50, max 500)"),
        ("offset" = Option<i64>, Query, description = "Number of messages to skip (default 0)"),
    ),
    responses((status = 200, description = "Chat history messages", body = Value))
)]
pub async fn get_history(
    State(state): State<AppState>,
    Query(params): Query<HistoryParams>,
) -> Result<Json<Value>, StatusCode> {
    let limit = params.limit.unwrap_or(50).clamp(1, 500);
    let offset = params.offset.unwrap_or(0).max(0);

    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM gh_chat_messages")
        .fetch_one(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let rows = sqlx::query_as::<_, ChatMessageRow>(
        "SELECT * FROM (\
            SELECT id, role, content, model, agent, created_at \
            FROM gh_chat_messages ORDER BY created_at DESC LIMIT $1 OFFSET $2\
        ) sub ORDER BY created_at ASC",
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let messages: Vec<ChatMessage> = rows.into_iter().map(super::row_to_message).collect();
    let returned = messages.len();

    Ok(Json(json!({
        "messages": messages,
        "total": total,
        "returned": returned,
        "limit": limit,
        "offset": offset,
    })))
}

/// GET /api/history/search?q=...
#[utoipa::path(get, path = "/api/history/search", tag = "history",
    responses((status = 200, description = "Search results", body = Value))
)]
pub async fn search_history(
    State(state): State<AppState>,
    Query(params): Query<SearchQuery>,
) -> Result<Json<Value>, StatusCode> {
    let pattern = format!("%{}%", params.q);

    let rows = sqlx::query_as::<_, ChatMessageRow>(
        "SELECT id, role, content, model, agent, created_at \
         FROM gh_chat_messages WHERE content ILIKE $1 ORDER BY created_at ASC",
    )
    .bind(&pattern)
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let results: Vec<ChatMessage> = rows.into_iter().map(super::row_to_message).collect();
    let count = results.len();

    Ok(Json(json!({
        "query": params.q,
        "results": results,
        "count": count,
    })))
}

/// POST /api/history — add a single message
#[utoipa::path(post, path = "/api/history", tag = "history",
    request_body = AddMessageRequest,
    responses((status = 200, description = "Message added", body = Value))
)]
pub async fn add_message(
    State(state): State<AppState>,
    Json(body): Json<AddMessageRequest>,
) -> Result<Json<Value>, StatusCode> {
    if body.content.len() > MAX_MESSAGE_LENGTH {
        tracing::warn!(
            "add_message: content exceeds {} chars (got {})",
            MAX_MESSAGE_LENGTH,
            body.content.len()
        );
        return Err(StatusCode::BAD_REQUEST);
    }

    let row = sqlx::query_as::<_, ChatMessageRow>(
        "INSERT INTO gh_chat_messages (role, content, model, agent) \
         VALUES ($1, $2, $3, $4) \
         RETURNING id, role, content, model, agent, created_at",
    )
    .bind(&body.role)
    .bind(&body.content)
    .bind(&body.model)
    .bind(&body.agent)
    .fetch_one(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let msg = super::row_to_message(row);
    Ok(Json(json!(msg)))
}

/// DELETE /api/history — clear all history
#[utoipa::path(delete, path = "/api/history", tag = "history",
    responses((status = 200, description = "History cleared", body = Value))
)]
pub async fn clear_history(
    State(state): State<AppState>,
) -> Result<Json<Value>, StatusCode> {
    sqlx::query("DELETE FROM gh_chat_messages")
        .execute(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(json!({ "cleared": true })))
}

// ============================================================================
// Per-session message handlers
// ============================================================================

/// GET /api/sessions/:id/messages?limit=50&offset=0
///
/// Paginated message history for a session. Returns messages in chronological
/// order with total count for client-side pagination controls.
#[utoipa::path(get, path = "/api/sessions/{id}/messages", tag = "sessions",
    params(
        ("id" = String, Path, description = "Session UUID"),
        ("limit" = Option<i64>, Query, description = "Max messages to return (default 50, max 500)"),
        ("offset" = Option<i64>, Query, description = "Number of messages to skip (default 0)"),
    ),
    responses(
        (status = 200, description = "Paginated messages", body = Value),
        (status = 404, description = "Session not found")
    )
)]
pub async fn get_session_messages(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<Value>, StatusCode> {
    let session_id: uuid::Uuid = id.parse().map_err(|_| StatusCode::BAD_REQUEST)?;
    let limit = params.limit.unwrap_or(50).clamp(1, 500);
    let offset = params.offset.unwrap_or(0).max(0);

    // Verify session exists
    sqlx::query("SELECT 1 FROM gh_sessions WHERE id = $1")
        .bind(session_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    // Total message count for this session
    let total: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM gh_chat_messages WHERE session_id = $1",
    )
    .bind(session_id)
    .fetch_one(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Fetch paginated messages in chronological order
    let rows = sqlx::query_as::<_, ChatMessageRow>(
        "SELECT id, role, content, model, agent, created_at \
         FROM gh_chat_messages WHERE session_id = $1 \
         ORDER BY created_at ASC LIMIT $2 OFFSET $3",
    )
    .bind(session_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let messages: Vec<ChatMessage> = rows.into_iter().map(super::row_to_message).collect();
    let returned = messages.len();

    Ok(Json(json!({
        "session_id": id,
        "messages": messages,
        "total": total,
        "returned": returned,
        "limit": limit,
        "offset": offset,
    })))
}

/// POST /api/sessions/:id/messages
#[utoipa::path(post, path = "/api/sessions/{id}/messages", tag = "sessions",
    params(("id" = String, Path, description = "Session UUID")),
    request_body = AddMessageRequest,
    responses(
        (status = 201, description = "Message added", body = Value),
        (status = 404, description = "Session not found")
    )
)]
pub async fn add_session_message(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<AddMessageRequest>,
) -> Result<(StatusCode, Json<Value>), StatusCode> {
    if body.content.len() > MAX_MESSAGE_LENGTH {
        tracing::warn!(
            "add_session_message: content exceeds {} chars (got {})",
            MAX_MESSAGE_LENGTH,
            body.content.len()
        );
        return Err(StatusCode::BAD_REQUEST);
    }

    let session_id: uuid::Uuid = id.parse().map_err(|_| StatusCode::BAD_REQUEST)?;

    // Verify session exists
    sqlx::query("SELECT 1 FROM gh_sessions WHERE id = $1")
        .bind(session_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let row = sqlx::query_as::<_, ChatMessageRow>(
        "INSERT INTO gh_chat_messages (session_id, role, content, model, agent) \
         VALUES ($1, $2, $3, $4, $5) \
         RETURNING id, role, content, model, agent, created_at",
    )
    .bind(session_id)
    .bind(&body.role)
    .bind(&body.content)
    .bind(&body.model)
    .bind(&body.agent)
    .fetch_one(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Update session timestamp
    sqlx::query("UPDATE gh_sessions SET updated_at = NOW() WHERE id = $1")
        .bind(session_id)
        .execute(&state.db)
        .await
        .ok();

    let msg = super::row_to_message(row);
    Ok((StatusCode::CREATED, Json(json!(msg))))
}
