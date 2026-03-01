//! Session, History, Settings & Memory endpoints.
//!
//! Split into sub-modules for maintainability (previously 1700+ lines).
//! This module owns the shared types, conversion helpers, and route builder.

mod crud;
mod history;
mod memory;
mod messages;
mod settings;

use axum::routing::{get, patch, post};
use axum::Router;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::models::{
    ChatMessage, ChatMessageRow, KnowledgeEdgeRow, KnowledgeNodeRow, MemoryRow, SettingsRow,
};
use crate::state::AppState;

// Re-export all public handler functions AND utoipa-generated __path_* types
// for lib.rs OpenAPI paths + route wiring.
pub use crud::*;
pub use history::*;
pub use memory::*;
pub use messages::*;
pub use settings::*;

// ── Input length limits — Jaskier Shared Pattern ────────────────────────────
pub(crate) const MAX_TITLE_LENGTH: usize = 200;
pub(crate) const MAX_MESSAGE_LENGTH: usize = 50_000; // 50KB

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
// Conversions — used by sub-modules
// ============================================================================

pub(crate) fn row_to_message(row: ChatMessageRow) -> ChatMessage {
    ChatMessage {
        id: row.id.to_string(),
        role: row.role,
        content: row.content,
        model: row.model,
        timestamp: row.created_at.to_rfc3339(),
        agent: row.agent,
    }
}

pub(crate) fn row_to_settings(row: SettingsRow) -> crate::models::AppSettings {
    crate::models::AppSettings {
        temperature: row.temperature,
        max_tokens: row.max_tokens as u32,
        default_model: row.default_model,
        language: row.language,
        theme: row.theme,
        welcome_message: row.welcome_message,
        use_docker_sandbox: row.use_docker_sandbox,
        top_p: if row.top_p == 0.0 { 0.95 } else { row.top_p },
        response_style: if row.response_style.is_empty() {
            "balanced".to_string()
        } else {
            row.response_style
        },
        max_iterations: if row.max_iterations == 0 {
            20
        } else {
            row.max_iterations
        },
        thinking_level: if row.thinking_level.is_empty() {
            "medium".to_string()
        } else {
            row.thinking_level
        },
        working_directory: row.working_directory,
    }
}

pub(crate) fn row_to_memory(row: MemoryRow) -> MemoryEntry {
    MemoryEntry {
        id: row.id.to_string(),
        agent: row.agent,
        content: row.content,
        importance: row.importance,
        timestamp: row.created_at.to_rfc3339(),
    }
}

pub(crate) fn row_to_node(row: KnowledgeNodeRow) -> KnowledgeNode {
    KnowledgeNode {
        id: row.id,
        node_type: row.node_type,
        label: row.label,
    }
}

pub(crate) fn row_to_edge(row: KnowledgeEdgeRow) -> KnowledgeEdge {
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
        .route(
            "/api/sessions/{id}/working-directory",
            patch(update_session_working_directory),
        )
        .route("/api/ratings", post(rate_message))
        // Prompt history
        .route(
            "/api/prompt-history",
            get(list_prompt_history)
                .post(add_prompt_history)
                .delete(clear_prompt_history),
        )
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
        let json =
            r#"{"role":"assistant","content":"hi","model":"gemini-pro","agent":"Geralt"}"#;
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
        let json =
            r#"{"temperature":0.5,"theme":"light","top_p":0.8,"response_style":"concise"}"#;
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
