//! Session, History, Settings & Memory endpoints (Agent 2).
//!
//! This module is kept separate from `handlers.rs` to avoid merge conflicts.
//! It owns the memory / knowledge-graph models and all related API handlers.

use axum::extract::{Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::handlers::SharedState;
use crate::models::{AppSettings, ChatMessage};

// ============================================================================
// Models (re-exported so state.rs can reference them)
// ============================================================================

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MemoryEntry {
    pub id: String,
    pub agent: String,
    pub content: String,
    pub importance: f64, // 0.0 – 1.0
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
    pub role: String, // "user" | "assistant" | "system"
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
}

#[derive(Debug, Deserialize)]
pub struct AddMemoryRequest {
    pub agent: String,
    pub content: String,
    pub importance: f64,
}

// ============================================================================
// Helpers
// ============================================================================

/// ISO 8601 timestamp via chrono.
fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339()
}

// ============================================================================
// Route builder — merge this into the main Router
// ============================================================================

/// Returns all session/history/settings/memory routes.
///
/// Usage in `main.rs`:
/// ```ignore
/// let app = Router::new()
///     .merge(session_routes())
///     // ... other routes ...
///     .with_state(shared_state);
/// ```
pub fn session_routes() -> Router<SharedState> {
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
}

// ============================================================================
// History handlers
// ============================================================================

/// GET /api/history?limit=50
async fn get_history(
    State(state): State<SharedState>,
    Query(params): Query<HistoryParams>,
) -> Json<Value> {
    let state = state.lock().await;
    let limit = params.limit.unwrap_or(50);
    let total = state.history.len();
    let start = total.saturating_sub(limit);
    let messages: Vec<&ChatMessage> = state.history[start..].iter().collect();

    Json(json!({
        "messages": messages,
        "total": total,
        "returned": messages.len(),
    }))
}

/// GET /api/history/search?q=...
async fn search_history(
    State(state): State<SharedState>,
    Query(params): Query<SearchQuery>,
) -> Json<Value> {
    let state = state.lock().await;
    let query_lower = params.q.to_lowercase();
    let matches: Vec<&ChatMessage> = state
        .history
        .iter()
        .filter(|m| m.content.to_lowercase().contains(&query_lower))
        .collect();

    Json(json!({
        "query": params.q,
        "results": matches,
        "count": matches.len(),
    }))
}

/// POST /api/history  — add a single message
async fn add_message(
    State(state): State<SharedState>,
    Json(body): Json<AddMessageRequest>,
) -> Json<Value> {
    let mut state = state.lock().await;
    let msg = ChatMessage {
        id: uuid::Uuid::new_v4().to_string(),
        role: body.role,
        content: body.content,
        model: body.model,
        timestamp: now_iso(),
        agent: body.agent,
    };
    state.history.push(msg.clone());

    Json(json!(msg))
}

/// DELETE /api/history  — clear all history
async fn clear_history(State(state): State<SharedState>) -> Json<Value> {
    let mut state = state.lock().await;
    state.history.clear();

    Json(json!({ "cleared": true }))
}

// ============================================================================
// Settings handlers
// ============================================================================

/// GET /api/settings
async fn get_settings(State(state): State<SharedState>) -> Json<AppSettings> {
    let state = state.lock().await;
    Json(state.settings.clone())
}

/// PATCH /api/settings  — partial update
async fn update_settings(
    State(state): State<SharedState>,
    Json(patch): Json<PartialSettings>,
) -> Json<AppSettings> {
    let mut state = state.lock().await;

    if let Some(v) = patch.temperature {
        state.settings.temperature = v;
    }
    if let Some(v) = patch.max_tokens {
        state.settings.max_tokens = v;
    }
    if let Some(v) = patch.default_model {
        state.settings.default_model = v;
    }
    if let Some(v) = patch.language {
        state.settings.language = v;
    }
    if let Some(v) = patch.theme {
        state.settings.theme = v;
    }

    Json(state.settings.clone())
}

/// POST /api/settings/reset  — restore defaults
async fn reset_settings(State(state): State<SharedState>) -> Json<AppSettings> {
    let mut state = state.lock().await;
    state.settings = AppSettings::default();

    Json(state.settings.clone())
}

// ============================================================================
// Memory handlers
// ============================================================================

/// GET /api/memory/memories?agent=Geralt&topK=10
///
/// Also exported as a standalone handler for `main.rs` route registration.
pub async fn list_memories(
    State(state): State<SharedState>,
    Query(params): Query<MemoryQueryParams>,
) -> Json<Value> {
    let state = state.lock().await;
    let top_k = params.top_k.unwrap_or(10);

    let mut filtered: Vec<&MemoryEntry> = match &params.agent {
        Some(agent) => state
            .memories
            .iter()
            .filter(|m| m.agent.eq_ignore_ascii_case(agent))
            .collect(),
        None => state.memories.iter().collect(),
    };

    // Sort descending by importance, take top K
    filtered.sort_by(|a, b| {
        b.importance
            .partial_cmp(&a.importance)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    filtered.truncate(top_k);

    Json(json!({
        "memories": filtered,
        "count": filtered.len(),
    }))
}

/// POST /api/memory/memories
pub async fn add_memory(
    State(state): State<SharedState>,
    Json(body): Json<AddMemoryRequest>,
) -> Json<Value> {
    let mut state = state.lock().await;
    let entry = MemoryEntry {
        id: uuid::Uuid::new_v4().to_string(),
        agent: body.agent,
        content: body.content,
        importance: body.importance.clamp(0.0, 1.0),
        timestamp: now_iso(),
    };
    state.memories.push(entry.clone());

    Json(json!(entry))
}

/// DELETE /api/memory/memories?agent=Geralt
async fn clear_memories(
    State(state): State<SharedState>,
    Query(params): Query<ClearMemoryParams>,
) -> Json<Value> {
    let mut state = state.lock().await;

    match &params.agent {
        Some(agent) => {
            let agent_lower = agent.to_lowercase();
            state
                .memories
                .retain(|m| m.agent.to_lowercase() != agent_lower);
            Json(json!({ "cleared": true, "agent": agent }))
        }
        None => {
            state.memories.clear();
            Json(json!({ "cleared": true, "agent": null }))
        }
    }
}

// ============================================================================
// Knowledge-graph handlers
// ============================================================================

/// GET /api/memory/graph
pub async fn get_knowledge_graph(State(state): State<SharedState>) -> Json<Value> {
    let state = state.lock().await;

    Json(json!({
        "nodes": state.graph_nodes,
        "edges": state.graph_edges,
    }))
}

/// POST /api/memory/graph/nodes
pub async fn add_knowledge_node(
    State(state): State<SharedState>,
    Json(node): Json<KnowledgeNode>,
) -> Json<Value> {
    let mut state = state.lock().await;
    state.graph_nodes.push(node.clone());

    Json(json!(node))
}

/// POST /api/memory/graph/edges
pub async fn add_graph_edge(
    State(state): State<SharedState>,
    Json(edge): Json<KnowledgeEdge>,
) -> Json<Value> {
    let mut state = state.lock().await;
    state.graph_edges.push(edge.clone());

    Json(json!(edge))
}
