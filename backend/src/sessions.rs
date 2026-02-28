//! Session, History, Settings & Memory endpoints.
//!
//! This module is kept separate from `handlers.rs` to avoid merge conflicts.
//! It owns the memory / knowledge-graph response models and all related API handlers.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use utoipa::ToSchema;

use crate::models::{
    AppSettings, ChatMessage, ChatMessageRow, CreateSessionRequest, KnowledgeEdgeRow,
    KnowledgeNodeRow, MemoryRow, RatingRequest, RatingResponse, Session, SessionRow,
    SessionSummary, SessionSummaryRow, SettingsRow, UnlockAgentResponse, UpdateSessionRequest,
};
use crate::state::AppState;

// ── Input length limits — Jaskier Shared Pattern ────────────────────────────

const MAX_TITLE_LENGTH: usize = 200;
const MAX_MESSAGE_LENGTH: usize = 50_000; // 50KB

// ============================================================================
// Response models
// ============================================================================

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MemoryEntry {
    pub id: String,
    pub agent: String,
    pub content: String,
    pub importance: f64,
    pub timestamp: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
pub struct KnowledgeNode {
    pub id: String,
    pub node_type: String,
    pub label: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
pub struct KnowledgeEdge {
    pub source: String,
    pub target: String,
    pub label: String,
}

// ── Request / query structs ─────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct AddMessageRequest {
    pub role: String,
    pub content: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub agent: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct HistoryParams {
    /// Max messages to return (default 50, max 500).
    #[serde(default)]
    pub limit: Option<i64>,
    /// Number of messages to skip (default 0).
    #[serde(default)]
    pub offset: Option<i64>,
}

/// Pagination query params for session/message listing.
/// Backwards-compatible: all fields optional with sensible defaults.
/// Supports both offset-based (`offset`) and cursor-based (`after`) pagination.
#[derive(Debug, Deserialize)]
pub struct PaginationParams {
    /// Max items to return (clamped to 500).
    #[serde(default)]
    pub limit: Option<i64>,
    /// Number of items to skip (offset-based pagination).
    #[serde(default)]
    pub offset: Option<i64>,
    /// Cursor-based pagination: return sessions created before this session ID.
    /// When provided, `offset` is ignored.
    #[serde(default)]
    pub after: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct MemoryQueryParams {
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(default, alias = "topK")]
    pub top_k: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct ClearMemoryParams {
    #[serde(default)]
    pub agent: Option<String>,
}

/// Partial settings for PATCH merge.
#[derive(Debug, Deserialize, ToSchema)]
pub struct PartialSettings {
    #[serde(default)]
    pub temperature: Option<f64>,
    #[serde(default)]
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub default_model: Option<String>,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub theme: Option<String>,
    #[serde(default)]
    pub welcome_message: Option<String>,
    #[serde(default)]
    pub use_docker_sandbox: Option<bool>,
    /// #46 — topP for Gemini generationConfig
    #[serde(default)]
    pub top_p: Option<f64>,
    /// #47 — Response style: 'concise', 'balanced', 'detailed', 'technical'
    #[serde(default)]
    pub response_style: Option<String>,
    /// #49 — Max tool call iterations per request
    #[serde(default)]
    pub max_iterations: Option<i32>,
    /// Gemini 3 thinking level: 'none', 'minimal', 'low', 'medium', 'high'
    #[serde(default)]
    pub thinking_level: Option<String>,
    /// Working directory for filesystem tools (empty = absolute paths only)
    #[serde(default)]
    pub working_directory: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct AddMemoryRequest {
    pub agent: String,
    pub content: String,
    pub importance: f64,
}

// ============================================================================
// Conversions
// ============================================================================

fn row_to_message(row: ChatMessageRow) -> ChatMessage {
    ChatMessage {
        id: row.id.to_string(),
        role: row.role,
        content: row.content,
        model: row.model,
        timestamp: row.created_at.to_rfc3339(),
        agent: row.agent,
    }
}

fn row_to_settings(row: SettingsRow) -> AppSettings {
    AppSettings {
        temperature: row.temperature,
        max_tokens: row.max_tokens as u32,
        default_model: row.default_model,
        language: row.language,
        theme: row.theme,
        welcome_message: row.welcome_message,
        use_docker_sandbox: row.use_docker_sandbox,
        top_p: if row.top_p == 0.0 { 0.95 } else { row.top_p },
        response_style: if row.response_style.is_empty() { "balanced".to_string() } else { row.response_style },
        max_iterations: if row.max_iterations == 0 { 10 } else { row.max_iterations },
        thinking_level: if row.thinking_level.is_empty() { "medium".to_string() } else { row.thinking_level },
        working_directory: row.working_directory,
    }
}

fn row_to_memory(row: MemoryRow) -> MemoryEntry {
    MemoryEntry {
        id: row.id.to_string(),
        agent: row.agent,
        content: row.content,
        importance: row.importance,
        timestamp: row.created_at.to_rfc3339(),
    }
}

fn row_to_node(row: KnowledgeNodeRow) -> KnowledgeNode {
    KnowledgeNode {
        id: row.id,
        node_type: row.node_type,
        label: row.label,
    }
}

fn row_to_edge(row: KnowledgeEdgeRow) -> KnowledgeEdge {
    KnowledgeEdge {
        source: row.source,
        target: row.target,
        label: row.label,
    }
}

// ============================================================================
// Route builder — merge this into the main Router
// ============================================================================

pub fn session_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/api/history",
            get(get_history).post(add_message).delete(clear_history),
        )
        .route("/api/history/search", get(search_history))
        .route("/api/settings", get(get_settings).patch(update_settings))
        .route("/api/settings/reset", post(reset_settings))
        .route(
            "/api/memory/memories",
            get(list_memories).post(add_memory).delete(clear_memories),
        )
        .route("/api/memory/graph", get(get_knowledge_graph))
        .route("/api/memory/graph/nodes", post(add_knowledge_node))
        .route("/api/memory/graph/edges", post(add_graph_edge))
        // Session CRUD
        .route("/api/sessions", get(list_sessions).post(create_session))
        .route(
            "/api/sessions/{id}",
            get(get_session).patch(update_session).delete(delete_session),
        )
        .route(
            "/api/sessions/{id}/messages",
            get(get_session_messages).post(add_session_message),
        )
        .route(
            "/api/sessions/{id}/generate-title",
            post(generate_session_title),
        )
        .route(
            "/api/sessions/{id}/unlock",
            post(unlock_session_agent),
        )
        .route("/api/ratings", post(rate_message))
}

// ============================================================================
// History handlers
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

    let messages: Vec<ChatMessage> = rows.into_iter().map(row_to_message).collect();
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

    let results: Vec<ChatMessage> = rows.into_iter().map(row_to_message).collect();
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
        tracing::warn!("add_message: content exceeds {} chars (got {})", MAX_MESSAGE_LENGTH, body.content.len());
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

    let msg = row_to_message(row);
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
// Session CRUD handlers
// ============================================================================

/// GET /api/sessions?limit=100&offset=0
/// Also supports cursor-based pagination: ?after=<session_id>&limit=20
#[utoipa::path(get, path = "/api/sessions", tag = "sessions",
    params(
        ("limit" = Option<i64>, Query, description = "Max sessions to return (default 100, max 500)"),
        ("offset" = Option<i64>, Query, description = "Number of sessions to skip (default 0)"),
        ("after" = Option<String>, Query, description = "Cursor: return sessions after this session ID (by updated_at)"),
    ),
    responses((status = 200, description = "List of session summaries", body = Vec<SessionSummary>))
)]
pub async fn list_sessions(
    State(state): State<AppState>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<Value>, StatusCode> {
    let limit = params.limit.unwrap_or(100).clamp(1, 500);

    // Cursor-based pagination: when `after` is provided, use it instead of offset
    if let Some(ref after_id) = params.after {
        let cursor_id = uuid::Uuid::parse_str(after_id).map_err(|_| StatusCode::BAD_REQUEST)?;

        let rows = sqlx::query_as::<_, SessionSummaryRow>(
            "SELECT s.id, s.title, s.created_at, \
             (SELECT COUNT(*) FROM gh_chat_messages WHERE session_id = s.id) as message_count \
             FROM gh_sessions s \
             WHERE s.updated_at < (SELECT updated_at FROM gh_sessions WHERE id = $1) \
             ORDER BY s.updated_at DESC \
             LIMIT $2",
        )
        .bind(cursor_id)
        .bind(limit)
        .fetch_all(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let summaries: Vec<SessionSummary> = rows
            .iter()
            .map(|r| SessionSummary {
                id: r.id.to_string(),
                title: r.title.clone(),
                created_at: r.created_at.to_rfc3339(),
                message_count: r.message_count as usize,
            })
            .collect();

        let has_more = summaries.len() as i64 == limit;
        let next_cursor = summaries.last().map(|s| s.id.clone());

        return Ok(Json(serde_json::to_value(serde_json::json!({
            "sessions": summaries,
            "has_more": has_more,
            "next_cursor": next_cursor,
        }))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?));
    }

    // Offset-based pagination (backwards compatible)
    let offset = params.offset.unwrap_or(0).max(0);

    let rows = sqlx::query_as::<_, SessionSummaryRow>(
        "SELECT s.id, s.title, s.created_at, \
         (SELECT COUNT(*) FROM gh_chat_messages WHERE session_id = s.id) as message_count \
         FROM gh_sessions s ORDER BY s.updated_at DESC \
         LIMIT $1 OFFSET $2",
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let summaries: Vec<SessionSummary> = rows
        .into_iter()
        .map(|r| SessionSummary {
            id: r.id.to_string(),
            title: r.title,
            created_at: r.created_at.to_rfc3339(),
            message_count: r.message_count as usize,
        })
        .collect();

    Ok(Json(serde_json::to_value(summaries).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?))
}

/// POST /api/sessions
#[utoipa::path(post, path = "/api/sessions", tag = "sessions",
    request_body = CreateSessionRequest,
    responses((status = 201, description = "Session created", body = Session))
)]
pub async fn create_session(
    State(state): State<AppState>,
    Json(req): Json<CreateSessionRequest>,
) -> Result<(StatusCode, Json<Value>), StatusCode> {
    if req.title.len() > MAX_TITLE_LENGTH {
        tracing::warn!("create_session: title exceeds {} chars (got {})", MAX_TITLE_LENGTH, req.title.len());
        return Err(StatusCode::BAD_REQUEST);
    }

    let row = sqlx::query_as::<_, SessionRow>(
        "INSERT INTO gh_sessions (title) VALUES ($1) \
         RETURNING id, title, created_at, updated_at",
    )
    .bind(&req.title)
    .fetch_one(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let session = Session {
        id: row.id.to_string(),
        title: row.title,
        created_at: row.created_at.to_rfc3339(),
        messages: Vec::new(),
    };

    Ok((StatusCode::CREATED, Json(serde_json::to_value(session).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?)))
}

/// GET /api/sessions/:id?limit=200&offset=0
#[utoipa::path(get, path = "/api/sessions/{id}", tag = "sessions",
    params(
        ("id" = String, Path, description = "Session UUID"),
        ("limit" = Option<i64>, Query, description = "Max messages to return (default 200, max 500)"),
        ("offset" = Option<i64>, Query, description = "Number of messages to skip (default 0)"),
    ),
    responses(
        (status = 200, description = "Session with messages", body = Session),
        (status = 404, description = "Session not found")
    )
)]
pub async fn get_session(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<Value>, StatusCode> {
    let session_id: uuid::Uuid = id.parse().map_err(|_| StatusCode::BAD_REQUEST)?;
    let msg_limit = params.limit.unwrap_or(200).clamp(1, 500);
    let msg_offset = params.offset.unwrap_or(0).max(0);

    let session_row = sqlx::query_as::<_, SessionRow>(
        "SELECT id, title, created_at, updated_at FROM gh_sessions WHERE id = $1",
    )
    .bind(session_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    // Total message count for pagination metadata
    let total_messages: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM gh_chat_messages WHERE session_id = $1",
    )
    .bind(session_id)
    .fetch_one(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Fetch the most recent N messages (subquery DESC, then re-sort ASC)
    let message_rows = sqlx::query_as::<_, ChatMessageRow>(
        "SELECT * FROM (\
            SELECT id, role, content, model, agent, created_at \
            FROM gh_chat_messages WHERE session_id = $1 \
            ORDER BY created_at DESC LIMIT $2 OFFSET $3\
        ) sub ORDER BY created_at ASC",
    )
    .bind(session_id)
    .bind(msg_limit)
    .bind(msg_offset)
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let messages: Vec<ChatMessage> = message_rows.into_iter().map(row_to_message).collect();

    let session = Session {
        id: session_row.id.to_string(),
        title: session_row.title,
        created_at: session_row.created_at.to_rfc3339(),
        messages,
    };

    let mut result = serde_json::to_value(session).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    // Extend session JSON with pagination metadata
    if let Some(obj) = result.as_object_mut() {
        obj.insert("total_messages".to_string(), json!(total_messages));
        obj.insert("limit".to_string(), json!(msg_limit));
        obj.insert("offset".to_string(), json!(msg_offset));
    }

    Ok(Json(result))
}

/// PATCH /api/sessions/:id
#[utoipa::path(patch, path = "/api/sessions/{id}", tag = "sessions",
    params(("id" = String, Path, description = "Session UUID")),
    request_body = UpdateSessionRequest,
    responses(
        (status = 200, description = "Session updated", body = SessionSummary),
        (status = 404, description = "Session not found")
    )
)]
pub async fn update_session(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateSessionRequest>,
) -> Result<Json<Value>, StatusCode> {
    if req.title.len() > MAX_TITLE_LENGTH {
        tracing::warn!("update_session: title exceeds {} chars (got {})", MAX_TITLE_LENGTH, req.title.len());
        return Err(StatusCode::BAD_REQUEST);
    }

    let session_id: uuid::Uuid = id.parse().map_err(|_| StatusCode::BAD_REQUEST)?;

    let row = sqlx::query_as::<_, SessionRow>(
        "UPDATE gh_sessions SET title = $1, updated_at = NOW() WHERE id = $2 \
         RETURNING id, title, created_at, updated_at",
    )
    .bind(&req.title)
    .bind(session_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    let summary = SessionSummary {
        id: row.id.to_string(),
        title: row.title,
        created_at: row.created_at.to_rfc3339(),
        message_count: 0,
    };

    Ok(Json(serde_json::to_value(summary).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?))
}

/// DELETE /api/sessions/:id
#[utoipa::path(delete, path = "/api/sessions/{id}", tag = "sessions",
    params(("id" = String, Path, description = "Session UUID")),
    responses(
        (status = 200, description = "Session deleted", body = Value),
        (status = 404, description = "Session not found")
    )
)]
pub async fn delete_session(
    State(state): State<AppState>,
    axum::extract::ConnectInfo(addr): axum::extract::ConnectInfo<std::net::SocketAddr>,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    let session_id: uuid::Uuid = id.parse().map_err(|_| StatusCode::BAD_REQUEST)?;

    let result = sqlx::query("DELETE FROM gh_sessions WHERE id = $1")
        .bind(session_id)
        .execute(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if result.rows_affected() == 0 {
        return Err(StatusCode::NOT_FOUND);
    }

    crate::audit::log_audit(
        &state.db,
        "delete_session",
        serde_json::json!({ "session_id": id }),
        Some(&addr.ip().to_string()),
    )
    .await;

    Ok(Json(json!({ "status": "deleted", "id": id })))
}

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

    let messages: Vec<ChatMessage> = rows.into_iter().map(row_to_message).collect();
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
        tracing::warn!("add_session_message: content exceeds {} chars (got {})", MAX_MESSAGE_LENGTH, body.content.len());
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

    let msg = row_to_message(row);
    Ok((StatusCode::CREATED, Json(json!(msg))))
}

// ============================================================================
// AI title generation — Jaskier Shared Pattern
// ============================================================================

/// POST /api/sessions/:id/generate-title
///
/// Reads the first user message from the session and asks Gemini Flash
/// to produce a concise 3-7 word title. Updates the DB and returns the title.
#[utoipa::path(post, path = "/api/sessions/{id}/generate-title", tag = "sessions",
    params(("id" = String, Path, description = "Session UUID")),
    responses(
        (status = 200, description = "AI-generated title", body = Value),
        (status = 404, description = "Session not found or no user messages"),
        (status = 503, description = "No API key configured")
    )
)]
pub async fn generate_session_title(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    let session_id: uuid::Uuid = id.parse().map_err(|_| StatusCode::BAD_REQUEST)?;

    // Fetch first user message
    let first_msg = sqlx::query_scalar::<_, String>(
        "SELECT content FROM gh_chat_messages \
         WHERE session_id = $1 AND role = 'user' \
         ORDER BY created_at ASC LIMIT 1",
    )
    .bind(session_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    // Get Google credential (API key or OAuth token)
    let (api_key, is_oauth) = match crate::oauth::get_google_credential(&state).await {
        Some(cred) => cred,
        None => {
            tracing::warn!("generate_session_title: no Google credential");
            return Err(StatusCode::SERVICE_UNAVAILABLE);
        }
    };

    // Truncate message to ~500 chars for the prompt (safe UTF-8 boundary)
    let snippet: &str = if first_msg.len() > 500 {
        let end = first_msg.char_indices()
            .take_while(|(i, _)| *i < 500)
            .last()
            .map(|(i, c)| i + c.len_utf8())
            .unwrap_or(500.min(first_msg.len()));
        &first_msg[..end]
    } else {
        &first_msg
    };
    let prompt = format!(
        "Generate a concise 3-7 word title for a chat that starts with this message. \
         Return ONLY the title text, no quotes, no explanation.\n\nMessage: {}",
        snippet
    );

    let model = "gemini-2.0-flash";
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent",
        model
    );
    let parsed_url = match reqwest::Url::parse(&url) {
        Ok(u) if u.scheme() == "https" => u,
        _ => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    };

    let body = json!({
        "contents": [{ "parts": [{ "text": prompt }] }],
        "generationConfig": { "temperature": 1.0, "maxOutputTokens": 256 }
    });

    let res = crate::oauth::apply_google_auth(
            state.client.post(parsed_url), &api_key, is_oauth,
        )
        .json(&body)
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| {
            tracing::error!("generate_session_title: API call failed: {}", e);
            StatusCode::BAD_GATEWAY
        })?;

    if !res.status().is_success() {
        tracing::error!("generate_session_title: API returned {}", res.status());
        return Err(StatusCode::BAD_GATEWAY);
    }

    let json_resp: Value = res.json().await.map_err(|_| StatusCode::BAD_GATEWAY)?;
    let raw_title = json_resp
        .get("candidates")
        .and_then(|c| c.get(0))
        .and_then(|c0| c0.get("content"))
        .and_then(|ct| ct.get("parts"))
        .and_then(|p| p.get(0))
        .and_then(|p0| p0.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or("");
    let raw_title = raw_title.trim().trim_matches('"').trim();

    if raw_title.is_empty() {
        tracing::warn!(
            "generate_session_title: Gemini response missing text — {}",
            crate::handlers::gemini_diagnose(&json_resp)
        );
        return Err(StatusCode::BAD_GATEWAY);
    }

    // Sanitize: cap at MAX_TITLE_LENGTH
    let title: String = raw_title.chars().take(MAX_TITLE_LENGTH).collect();

    // Update session title in DB
    sqlx::query("UPDATE gh_sessions SET title = $1, updated_at = NOW() WHERE id = $2")
        .bind(&title)
        .bind(session_id)
        .execute(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    tracing::info!("generate_session_title: session {} → {:?}", session_id, title);
    Ok(Json(json!({ "title": title })))
}

// ============================================================================
// Settings handlers
// ============================================================================

/// GET /api/settings
#[utoipa::path(get, path = "/api/settings", tag = "settings",
    responses((status = 200, description = "Current application settings", body = AppSettings))
)]
pub async fn get_settings(
    State(state): State<AppState>,
) -> Result<Json<AppSettings>, StatusCode> {
    let row = sqlx::query_as::<_, SettingsRow>(
        "SELECT temperature, max_tokens, default_model, language, theme, welcome_message, \
         use_docker_sandbox, top_p, response_style, max_iterations, thinking_level, working_directory \
         FROM gh_settings WHERE id = 1",
    )
    .fetch_one(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(row_to_settings(row)))
}

/// PATCH /api/settings — partial update (read-modify-write)
#[utoipa::path(patch, path = "/api/settings", tag = "settings",
    responses((status = 200, description = "Updated settings", body = AppSettings))
)]
pub async fn update_settings(
    State(state): State<AppState>,
    axum::extract::ConnectInfo(addr): axum::extract::ConnectInfo<std::net::SocketAddr>,
    Json(patch): Json<PartialSettings>,
) -> Result<Json<AppSettings>, StatusCode> {
    // Limit string field sizes to prevent uncontrolled memory allocation
    if patch.welcome_message.as_ref().is_some_and(|s| s.len() > 10_000)
        || patch.default_model.as_ref().is_some_and(|s| s.len() > 200)
        || patch.response_style.as_ref().is_some_and(|s| !["concise", "balanced", "detailed", "technical"].contains(&s.as_str()))
        || patch.thinking_level.as_ref().is_some_and(|s| !["none", "minimal", "low", "medium", "high"].contains(&s.as_str()))
    {
        return Err(StatusCode::PAYLOAD_TOO_LARGE);
    }

    let current = sqlx::query_as::<_, SettingsRow>(
        "SELECT temperature, max_tokens, default_model, language, theme, welcome_message, \
         use_docker_sandbox, top_p, response_style, max_iterations, thinking_level, working_directory \
         FROM gh_settings WHERE id = 1",
    )
    .fetch_one(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let temperature = patch.temperature.unwrap_or(current.temperature);
    let max_tokens = patch.max_tokens.map(|v| v as i32).unwrap_or(current.max_tokens);
    let default_model = patch.default_model.unwrap_or(current.default_model);
    let language = patch.language.unwrap_or(current.language);
    let theme = patch.theme.unwrap_or(current.theme);
    let welcome_message = patch.welcome_message.unwrap_or(current.welcome_message);
    let use_docker_sandbox = patch.use_docker_sandbox.unwrap_or(current.use_docker_sandbox);
    let top_p = patch.top_p.unwrap_or(current.top_p);
    let response_style = patch.response_style.unwrap_or(current.response_style);
    let max_iterations = patch.max_iterations.unwrap_or(current.max_iterations);
    let thinking_level = patch.thinking_level.unwrap_or(current.thinking_level);
    let working_directory = patch.working_directory.unwrap_or(current.working_directory);

    // Validate working_directory if non-empty
    if !working_directory.is_empty() && !std::path::Path::new(&working_directory).is_dir() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let row = sqlx::query_as::<_, SettingsRow>(
        "UPDATE gh_settings SET temperature=$1, max_tokens=$2, default_model=$3, \
         language=$4, theme=$5, welcome_message=$6, use_docker_sandbox=$7, \
         top_p=$8, response_style=$9, max_iterations=$10, thinking_level=$11, \
         working_directory=$12, updated_at=NOW() WHERE id=1 \
         RETURNING temperature, max_tokens, default_model, language, theme, welcome_message, \
         use_docker_sandbox, top_p, response_style, max_iterations, thinking_level, working_directory",
    )
    .bind(temperature)
    .bind(max_tokens)
    .bind(&default_model)
    .bind(&language)
    .bind(&theme)
    .bind(&welcome_message)
    .bind(use_docker_sandbox)
    .bind(top_p)
    .bind(&response_style)
    .bind(max_iterations)
    .bind(&thinking_level)
    .bind(&working_directory)
    .fetch_one(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    crate::audit::log_audit(
        &state.db,
        "update_settings",
        serde_json::json!({
            "temperature": temperature,
            "max_tokens": max_tokens,
            "default_model": default_model,
            "language": language,
            "theme": theme,
            "top_p": top_p,
            "response_style": response_style,
            "max_iterations": max_iterations,
            "thinking_level": thinking_level,
            "working_directory": working_directory,
        }),
        Some(&addr.ip().to_string()),
    )
    .await;

    Ok(Json(row_to_settings(row)))
}

/// POST /api/settings/reset — restore defaults (picks best model from cache)
#[utoipa::path(post, path = "/api/settings/reset", tag = "settings",
    responses((status = 200, description = "Settings reset to defaults", body = AppSettings))
)]
pub async fn reset_settings(
    State(state): State<AppState>,
) -> Result<Json<AppSettings>, StatusCode> {
    let best_model = crate::model_registry::get_model_id(&state, "chat").await;

    let row = sqlx::query_as::<_, SettingsRow>(
        "UPDATE gh_settings SET temperature=1.0, max_tokens=65536, \
         default_model=$1, language='en', theme='dark', \
         welcome_message='', use_docker_sandbox=FALSE, \
         top_p=0.95, response_style='balanced', max_iterations=10, \
         thinking_level='medium', working_directory='', updated_at=NOW() WHERE id=1 \
         RETURNING temperature, max_tokens, default_model, language, theme, welcome_message, \
         use_docker_sandbox, top_p, response_style, max_iterations, thinking_level, working_directory",
    )
    .bind(&best_model)
    .fetch_one(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(row_to_settings(row)))
}

// ============================================================================
// Memory handlers
// ============================================================================

/// GET /api/memory/memories?agent=Geralt&topK=10
#[utoipa::path(get, path = "/api/memory/memories", tag = "memory",
    responses((status = 200, description = "Agent memories", body = Value))
)]
pub async fn list_memories(
    State(state): State<AppState>,
    Query(params): Query<MemoryQueryParams>,
) -> Result<Json<Value>, StatusCode> {
    let top_k = params.top_k.unwrap_or(10) as i64;

    let rows = match &params.agent {
        Some(agent) => {
            sqlx::query_as::<_, MemoryRow>(
                "SELECT id, agent, content, importance, created_at \
                 FROM gh_memories WHERE LOWER(agent) = LOWER($1) \
                 ORDER BY importance DESC LIMIT $2",
            )
            .bind(agent)
            .bind(top_k)
            .fetch_all(&state.db)
            .await
        }
        None => {
            sqlx::query_as::<_, MemoryRow>(
                "SELECT id, agent, content, importance, created_at \
                 FROM gh_memories ORDER BY importance DESC LIMIT $1",
            )
            .bind(top_k)
            .fetch_all(&state.db)
            .await
        }
    }
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let memories: Vec<MemoryEntry> = rows.into_iter().map(row_to_memory).collect();
    let count = memories.len();

    Ok(Json(json!({
        "memories": memories,
        "count": count,
    })))
}

/// POST /api/memory/memories
#[utoipa::path(post, path = "/api/memory/memories", tag = "memory",
    responses((status = 200, description = "Memory added", body = Value))
)]
pub async fn add_memory(
    State(state): State<AppState>,
    Json(body): Json<AddMemoryRequest>,
) -> Result<Json<Value>, StatusCode> {
    let importance = body.importance.clamp(0.0, 1.0);

    let row = sqlx::query_as::<_, MemoryRow>(
        "INSERT INTO gh_memories (agent, content, importance) VALUES ($1, $2, $3) \
         RETURNING id, agent, content, importance, created_at",
    )
    .bind(&body.agent)
    .bind(&body.content)
    .bind(importance)
    .fetch_one(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let entry = row_to_memory(row);
    Ok(Json(json!(entry)))
}

/// DELETE /api/memory/memories?agent=Geralt
#[utoipa::path(delete, path = "/api/memory/memories", tag = "memory",
    responses((status = 200, description = "Memories cleared", body = Value))
)]
pub async fn clear_memories(
    State(state): State<AppState>,
    Query(params): Query<ClearMemoryParams>,
) -> Result<Json<Value>, StatusCode> {
    match &params.agent {
        Some(agent) => {
            sqlx::query("DELETE FROM gh_memories WHERE LOWER(agent) = LOWER($1)")
                .bind(agent)
                .execute(&state.db)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            Ok(Json(json!({ "cleared": true, "agent": agent })))
        }
        None => {
            sqlx::query("DELETE FROM gh_memories")
                .execute(&state.db)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            Ok(Json(json!({ "cleared": true, "agent": null })))
        }
    }
}

// ============================================================================
// Knowledge-graph handlers
// ============================================================================

/// GET /api/memory/graph
#[utoipa::path(get, path = "/api/memory/graph", tag = "memory",
    responses((status = 200, description = "Knowledge graph nodes and edges", body = Value))
)]
pub async fn get_knowledge_graph(
    State(state): State<AppState>,
) -> Result<Json<Value>, StatusCode> {
    let node_rows = sqlx::query_as::<_, KnowledgeNodeRow>(
        "SELECT id, node_type, label FROM gh_knowledge_nodes",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let edge_rows = sqlx::query_as::<_, KnowledgeEdgeRow>(
        "SELECT source, target, label FROM gh_knowledge_edges",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let nodes: Vec<KnowledgeNode> = node_rows.into_iter().map(row_to_node).collect();
    let edges: Vec<KnowledgeEdge> = edge_rows.into_iter().map(row_to_edge).collect();

    Ok(Json(json!({
        "nodes": nodes,
        "edges": edges,
    })))
}

/// POST /api/memory/graph/nodes
#[utoipa::path(post, path = "/api/memory/graph/nodes", tag = "memory",
    responses((status = 200, description = "Knowledge node added/updated", body = Value))
)]
pub async fn add_knowledge_node(
    State(state): State<AppState>,
    Json(node): Json<KnowledgeNode>,
) -> Result<Json<Value>, StatusCode> {
    sqlx::query(
        "INSERT INTO gh_knowledge_nodes (id, node_type, label) VALUES ($1, $2, $3) \
         ON CONFLICT (id) DO UPDATE SET node_type = EXCLUDED.node_type, label = EXCLUDED.label",
    )
    .bind(&node.id)
    .bind(&node.node_type)
    .bind(&node.label)
    .execute(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(json!(node)))
}

/// POST /api/memory/graph/edges
#[utoipa::path(post, path = "/api/memory/graph/edges", tag = "memory",
    responses((status = 200, description = "Knowledge edge added", body = Value))
)]
pub async fn add_graph_edge(
    State(state): State<AppState>,
    Json(edge): Json<KnowledgeEdge>,
) -> Result<Json<Value>, StatusCode> {
    sqlx::query(
        "INSERT INTO gh_knowledge_edges (source, target, label) VALUES ($1, $2, $3) \
         ON CONFLICT DO NOTHING",
    )
    .bind(&edge.source)
    .bind(&edge.target)
    .bind(&edge.label)
    .execute(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(json!(edge)))
}

// ============================================================================
// Agent unlock & message rating
// ============================================================================

/// Unlock a session's locked agent so the next message gets reclassified.
pub async fn unlock_session_agent(
    State(state): State<AppState>,
    axum::extract::Path(session_id): axum::extract::Path<String>,
) -> Result<Json<Value>, (axum::http::StatusCode, Json<Value>)> {
    let sid: uuid::Uuid = session_id.parse().map_err(|_| (
        axum::http::StatusCode::BAD_REQUEST,
        Json(serde_json::json!({"error": "Invalid session ID"})),
    ))?;

    let prev = sqlx::query_as::<_, (Option<String>,)>(
        "SELECT agent_id FROM gh_sessions WHERE id = $1"
    )
    .bind(sid)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| (
        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({"error": format!("DB error: {}", e)})),
    ))?
    .and_then(|(a,)| a);

    sqlx::query("UPDATE gh_sessions SET agent_id = NULL WHERE id = $1")
        .bind(sid)
        .execute(&state.db)
        .await
        .map_err(|e| (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("DB error: {}", e)})),
        ))?;

    Ok(Json(serde_json::json!(UnlockAgentResponse {
        session_id: session_id,
        previous_agent: prev,
        unlocked: true,
    })))
}

/// Rate an AI message for quality feedback.
pub async fn rate_message(
    State(state): State<AppState>,
    Json(body): Json<RatingRequest>,
) -> Result<Json<Value>, (axum::http::StatusCode, Json<Value>)> {
    if body.rating < 1 || body.rating > 5 {
        return Err((
            axum::http::StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Rating must be between 1 and 5"})),
        ));
    }

    let mid: uuid::Uuid = body.message_id.parse().map_err(|_| (
        axum::http::StatusCode::BAD_REQUEST,
        Json(serde_json::json!({"error": "Invalid message ID"})),
    ))?;
    let sid: uuid::Uuid = body.session_id.parse().map_err(|_| (
        axum::http::StatusCode::BAD_REQUEST,
        Json(serde_json::json!({"error": "Invalid session ID"})),
    ))?;

    // Get agent/model from the message
    let msg_info = sqlx::query_as::<_, (Option<String>, Option<String>)>(
        "SELECT agent, model FROM gh_chat_messages WHERE id = $1"
    )
    .bind(mid)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| (
        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({"error": format!("DB error: {}", e)})),
    ))?;

    let (agent_id, model) = msg_info.unwrap_or((None, None));

    sqlx::query(
        "INSERT INTO gh_ratings (message_id, session_id, rating, feedback, agent_id, model) VALUES ($1, $2, $3, $4, $5, $6)"
    )
    .bind(mid)
    .bind(sid)
    .bind(body.rating)
    .bind(&body.feedback)
    .bind(&agent_id)
    .bind(&model)
    .execute(&state.db)
    .await
    .map_err(|e| (
        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({"error": format!("DB error: {}", e)})),
    ))?;

    Ok(Json(serde_json::json!(RatingResponse {
        success: true,
        message_id: body.message_id,
    })))
}

// ============================================================================
// Unit tests — pure functions only (no DB required)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{
        ChatMessageRow, KnowledgeEdgeRow, KnowledgeNodeRow, MemoryRow, SettingsRow,
    };
    use chrono::Utc;

    // ── row_to_message ──────────────────────────────────────────────────

    #[test]
    fn row_to_message_maps_all_fields() {
        let now = Utc::now();
        let row = ChatMessageRow {
            id: uuid::Uuid::nil(),
            role: "user".to_string(),
            content: "Hello".to_string(),
            model: Some("gemini-pro".to_string()),
            agent: Some("Geralt".to_string()),
            created_at: now,
        };
        let msg = row_to_message(row);
        assert_eq!(msg.id, uuid::Uuid::nil().to_string());
        assert_eq!(msg.role, "user");
        assert_eq!(msg.content, "Hello");
        assert_eq!(msg.model, Some("gemini-pro".to_string()));
        assert_eq!(msg.agent, Some("Geralt".to_string()));
        assert_eq!(msg.timestamp, now.to_rfc3339());
    }

    #[test]
    fn row_to_message_handles_none_optional_fields() {
        let row = ChatMessageRow {
            id: uuid::Uuid::new_v4(),
            role: "assistant".to_string(),
            content: "Hi there".to_string(),
            model: None,
            agent: None,
            created_at: Utc::now(),
        };
        let msg = row_to_message(row);
        assert!(msg.model.is_none());
        assert!(msg.agent.is_none());
    }

    // ── row_to_settings ─────────────────────────────────────────────────

    #[test]
    fn row_to_settings_maps_all_fields() {
        let row = SettingsRow {
            temperature: 0.7,
            max_tokens: 4096,
            default_model: "gemini-pro".to_string(),
            language: "pl".to_string(),
            theme: "light".to_string(),
            welcome_message: "Witaj!".to_string(),
            use_docker_sandbox: true,
            top_p: 0.9,
            response_style: "detailed".to_string(),
            max_iterations: 15,
            thinking_level: "high".to_string(),
            working_directory: "C:\\Users\\test".to_string(),
        };
        let settings = row_to_settings(row);
        assert!((settings.temperature - 0.7).abs() < f64::EPSILON);
        assert_eq!(settings.max_tokens, 4096);
        assert_eq!(settings.default_model, "gemini-pro");
        assert_eq!(settings.language, "pl");
        assert_eq!(settings.theme, "light");
        assert_eq!(settings.welcome_message, "Witaj!");
        assert!(settings.use_docker_sandbox);
        assert!((settings.top_p - 0.9).abs() < f64::EPSILON);
        assert_eq!(settings.response_style, "detailed");
        assert_eq!(settings.max_iterations, 15);
        assert_eq!(settings.thinking_level, "high");
    }

    // ── row_to_memory ───────────────────────────────────────────────────

    #[test]
    fn row_to_memory_maps_all_fields() {
        let now = Utc::now();
        let row = MemoryRow {
            id: uuid::Uuid::nil(),
            agent: "Yennefer".to_string(),
            content: "Important fact".to_string(),
            importance: 0.95,
            created_at: now,
        };
        let entry = row_to_memory(row);
        assert_eq!(entry.id, uuid::Uuid::nil().to_string());
        assert_eq!(entry.agent, "Yennefer");
        assert_eq!(entry.content, "Important fact");
        assert!((entry.importance - 0.95).abs() < f64::EPSILON);
        assert_eq!(entry.timestamp, now.to_rfc3339());
    }

    // ── row_to_node / row_to_edge ───────────────────────────────────────

    #[test]
    fn row_to_node_maps_all_fields() {
        let row = KnowledgeNodeRow {
            id: "n1".to_string(),
            node_type: "concept".to_string(),
            label: "Witcher Signs".to_string(),
        };
        let node = row_to_node(row);
        assert_eq!(node.id, "n1");
        assert_eq!(node.node_type, "concept");
        assert_eq!(node.label, "Witcher Signs");
    }

    #[test]
    fn row_to_edge_maps_all_fields() {
        let row = KnowledgeEdgeRow {
            source: "n1".to_string(),
            target: "n2".to_string(),
            label: "uses".to_string(),
        };
        let edge = row_to_edge(row);
        assert_eq!(edge.source, "n1");
        assert_eq!(edge.target, "n2");
        assert_eq!(edge.label, "uses");
    }

    // ── Serialization / Deserialization ─────────────────────────────────

    #[test]
    fn add_message_request_deserializes_with_defaults() {
        let json = r#"{"role":"user","content":"hello"}"#;
        let req: AddMessageRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.role, "user");
        assert_eq!(req.content, "hello");
        assert!(req.model.is_none());
        assert!(req.agent.is_none());
    }

    #[test]
    fn add_message_request_deserializes_with_all_fields() {
        let json = r#"{"role":"assistant","content":"hi","model":"gemini-pro","agent":"Geralt"}"#;
        let req: AddMessageRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.role, "assistant");
        assert_eq!(req.content, "hi");
        assert_eq!(req.model, Some("gemini-pro".to_string()));
        assert_eq!(req.agent, Some("Geralt".to_string()));
    }

    #[test]
    fn partial_settings_all_none_by_default() {
        let json = r#"{}"#;
        let patch: PartialSettings = serde_json::from_str(json).unwrap();
        assert!(patch.temperature.is_none());
        assert!(patch.max_tokens.is_none());
        assert!(patch.default_model.is_none());
        assert!(patch.language.is_none());
        assert!(patch.theme.is_none());
        assert!(patch.welcome_message.is_none());
        assert!(patch.use_docker_sandbox.is_none());
        assert!(patch.top_p.is_none());
        assert!(patch.response_style.is_none());
        assert!(patch.max_iterations.is_none());
    }

    #[test]
    fn partial_settings_picks_up_subset() {
        let json = r#"{"temperature":0.5,"theme":"light","top_p":0.8,"response_style":"concise"}"#;
        let patch: PartialSettings = serde_json::from_str(json).unwrap();
        assert!((patch.temperature.unwrap() - 0.5).abs() < f64::EPSILON);
        assert_eq!(patch.theme, Some("light".to_string()));
        assert!(patch.max_tokens.is_none());
        assert!((patch.top_p.unwrap() - 0.8).abs() < f64::EPSILON);
        assert_eq!(patch.response_style, Some("concise".to_string()));
    }

    #[test]
    fn knowledge_node_roundtrip() {
        let node = KnowledgeNode {
            id: "abc".to_string(),
            node_type: "entity".to_string(),
            label: "Test <special> & chars".to_string(),
        };
        let serialized = serde_json::to_string(&node).unwrap();
        let deserialized: KnowledgeNode = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.id, node.id);
        assert_eq!(deserialized.node_type, node.node_type);
        assert_eq!(deserialized.label, node.label);
    }

    #[test]
    fn knowledge_edge_roundtrip() {
        let edge = KnowledgeEdge {
            source: "src".to_string(),
            target: "tgt".to_string(),
            label: "edge with unicode: \u{1F5E1}".to_string(),
        };
        let serialized = serde_json::to_string(&edge).unwrap();
        let deserialized: KnowledgeEdge = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.source, edge.source);
        assert_eq!(deserialized.target, edge.target);
        assert_eq!(deserialized.label, edge.label);
    }

    #[test]
    fn memory_entry_serialization() {
        let entry = MemoryEntry {
            id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            agent: "Triss".to_string(),
            content: "Spell components list".to_string(),
            importance: 0.8,
            timestamp: "2026-01-15T10:30:00+00:00".to_string(),
        };
        let json = serde_json::to_value(&entry).unwrap();
        assert_eq!(json["agent"], "Triss");
        assert_eq!(json["importance"], 0.8);
    }

    #[test]
    fn pagination_params_defaults() {
        let json = r#"{}"#;
        let params: PaginationParams = serde_json::from_str(json).unwrap();
        assert!(params.limit.is_none());
        assert!(params.offset.is_none());
    }

    #[test]
    fn add_memory_request_importance_preserved() {
        let json = r#"{"agent":"Ciri","content":"Portal magic","importance":0.42}"#;
        let req: AddMemoryRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.agent, "Ciri");
        assert_eq!(req.content, "Portal magic");
        assert!((req.importance - 0.42).abs() < f64::EPSILON);
    }
}
