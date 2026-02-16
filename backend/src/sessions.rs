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

use crate::models::{
    AppSettings, ChatMessage, ChatMessageRow, CreateSessionRequest, KnowledgeEdgeRow,
    KnowledgeNodeRow, MemoryRow, Session, SessionRow, SessionSummary, SessionSummaryRow,
    SettingsRow, UpdateSessionRequest,
};
use crate::state::AppState;

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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KnowledgeNode {
    pub id: String,
    pub node_type: String,
    pub label: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
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

#[derive(Debug, Deserialize)]
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
    #[serde(default)]
    pub limit: Option<usize>,
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
#[derive(Debug, Deserialize)]
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
}

#[derive(Debug, Deserialize)]
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
        .route("/api/sessions/{id}/messages", post(add_session_message))
}

// ============================================================================
// History handlers
// ============================================================================

/// GET /api/history?limit=50
async fn get_history(
    State(state): State<AppState>,
    Query(params): Query<HistoryParams>,
) -> Result<Json<Value>, StatusCode> {
    let limit = params.limit.unwrap_or(50) as i64;

    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM gh_chat_messages")
        .fetch_one(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let rows = sqlx::query_as::<_, ChatMessageRow>(
        "SELECT * FROM (\
            SELECT id, role, content, model, agent, created_at \
            FROM gh_chat_messages ORDER BY created_at DESC LIMIT $1\
        ) sub ORDER BY created_at ASC",
    )
    .bind(limit)
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let messages: Vec<ChatMessage> = rows.into_iter().map(row_to_message).collect();
    let returned = messages.len();

    Ok(Json(json!({
        "messages": messages,
        "total": total,
        "returned": returned,
    })))
}

/// GET /api/history/search?q=...
async fn search_history(
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
async fn add_message(
    State(state): State<AppState>,
    Json(body): Json<AddMessageRequest>,
) -> Result<Json<Value>, StatusCode> {
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
async fn clear_history(
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

/// GET /api/sessions
async fn list_sessions(
    State(state): State<AppState>,
) -> Result<Json<Value>, StatusCode> {
    let rows = sqlx::query_as::<_, SessionSummaryRow>(
        "SELECT s.id, s.title, s.created_at, \
         (SELECT COUNT(*) FROM gh_chat_messages WHERE session_id = s.id) as message_count \
         FROM gh_sessions s ORDER BY s.updated_at DESC",
    )
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

    Ok(Json(serde_json::to_value(summaries).unwrap()))
}

/// POST /api/sessions
async fn create_session(
    State(state): State<AppState>,
    Json(req): Json<CreateSessionRequest>,
) -> Result<(StatusCode, Json<Value>), StatusCode> {
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

    Ok((StatusCode::CREATED, Json(serde_json::to_value(session).unwrap())))
}

/// GET /api/sessions/:id
async fn get_session(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    let session_id: uuid::Uuid = id.parse().map_err(|_| StatusCode::BAD_REQUEST)?;

    let session_row = sqlx::query_as::<_, SessionRow>(
        "SELECT id, title, created_at, updated_at FROM gh_sessions WHERE id = $1",
    )
    .bind(session_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    let message_rows = sqlx::query_as::<_, ChatMessageRow>(
        "SELECT id, role, content, model, agent, created_at \
         FROM gh_chat_messages WHERE session_id = $1 ORDER BY created_at ASC",
    )
    .bind(session_id)
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

    Ok(Json(serde_json::to_value(session).unwrap()))
}

/// PATCH /api/sessions/:id
async fn update_session(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateSessionRequest>,
) -> Result<Json<Value>, StatusCode> {
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

    Ok(Json(serde_json::to_value(summary).unwrap()))
}

/// DELETE /api/sessions/:id
async fn delete_session(
    State(state): State<AppState>,
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

    Ok(Json(json!({ "status": "deleted", "id": id })))
}

/// POST /api/sessions/:id/messages
async fn add_session_message(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<AddMessageRequest>,
) -> Result<(StatusCode, Json<Value>), StatusCode> {
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
// Settings handlers
// ============================================================================

/// GET /api/settings
async fn get_settings(
    State(state): State<AppState>,
) -> Result<Json<AppSettings>, StatusCode> {
    let row = sqlx::query_as::<_, SettingsRow>(
        "SELECT temperature, max_tokens, default_model, language, theme, welcome_message \
         FROM gh_settings WHERE id = 1",
    )
    .fetch_one(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(row_to_settings(row)))
}

/// PATCH /api/settings — partial update (read-modify-write)
async fn update_settings(
    State(state): State<AppState>,
    Json(patch): Json<PartialSettings>,
) -> Result<Json<AppSettings>, StatusCode> {
    let current = sqlx::query_as::<_, SettingsRow>(
        "SELECT temperature, max_tokens, default_model, language, theme, welcome_message \
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

    let row = sqlx::query_as::<_, SettingsRow>(
        "UPDATE gh_settings SET temperature=$1, max_tokens=$2, default_model=$3, \
         language=$4, theme=$5, welcome_message=$6, updated_at=NOW() WHERE id=1 \
         RETURNING temperature, max_tokens, default_model, language, theme, welcome_message",
    )
    .bind(temperature)
    .bind(max_tokens)
    .bind(&default_model)
    .bind(&language)
    .bind(&theme)
    .bind(&welcome_message)
    .fetch_one(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(row_to_settings(row)))
}

/// POST /api/settings/reset — restore defaults
async fn reset_settings(
    State(state): State<AppState>,
) -> Result<Json<AppSettings>, StatusCode> {
    let row = sqlx::query_as::<_, SettingsRow>(
        "UPDATE gh_settings SET temperature=1.0, max_tokens=8192, \
         default_model='gemini-3-flash-preview', language='en', theme='dark', \
         welcome_message='', updated_at=NOW() WHERE id=1 \
         RETURNING temperature, max_tokens, default_model, language, theme, welcome_message",
    )
    .fetch_one(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(row_to_settings(row)))
}

// ============================================================================
// Memory handlers
// ============================================================================

/// GET /api/memory/memories?agent=Geralt&topK=10
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
async fn clear_memories(
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
