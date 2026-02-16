use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// DB row types
// ---------------------------------------------------------------------------

#[derive(sqlx::FromRow)]
pub struct SettingsRow {
    pub temperature: f64,
    pub max_tokens: i32,
    pub default_model: String,
    pub language: String,
    pub theme: String,
    pub welcome_message: String,
}

#[derive(sqlx::FromRow)]
pub struct ChatMessageRow {
    pub id: uuid::Uuid,
    pub role: String,
    pub content: String,
    pub model: Option<String>,
    pub agent: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(sqlx::FromRow)]
pub struct MemoryRow {
    pub id: uuid::Uuid,
    pub agent: String,
    pub content: String,
    pub importance: f64,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(sqlx::FromRow)]
pub struct KnowledgeNodeRow {
    pub id: String,
    pub node_type: String,
    pub label: String,
}

#[derive(sqlx::FromRow)]
pub struct KnowledgeEdgeRow {
    pub source: String,
    pub target: String,
    pub label: String,
}

// ---------------------------------------------------------------------------
// Witcher Agents
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WitcherAgent {
    pub id: String,
    pub name: String,
    pub role: String,
    pub tier: String,
    pub status: String,
    pub description: String,
}

// ---------------------------------------------------------------------------
// Health
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderInfo {
    pub name: String,
    pub available: bool,
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub app: String,
    pub uptime_seconds: u64,
    pub providers: Vec<ProviderInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetailedHealthResponse {
    pub status: String,
    pub version: String,
    pub app: String,
    pub uptime_seconds: u64,
    pub providers: Vec<ProviderInfo>,
    pub memory_usage_mb: f64,
    pub cpu_usage_percent: f32,
    pub platform: String,
}

// ---------------------------------------------------------------------------
// Execute (Chat / Swarm)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteRequest {
    pub prompt: String,
    pub mode: String,
    #[serde(default)]
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutePlan {
    pub agent: Option<String>,
    pub steps: Vec<String>,
    pub estimated_time: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteResponse {
    pub id: String,
    pub result: String,
    pub plan: Option<ExecutePlan>,
    pub duration_ms: u64,
    pub mode: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files_loaded: Vec<String>,
}

// ---------------------------------------------------------------------------
// Gemini Proxy
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiStreamRequest {
    pub prompt: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub temperature: Option<f64>,
    #[serde(default)]
    pub max_tokens: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiModelsResponse {
    pub models: Vec<GeminiModelInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiModelInfo {
    pub name: String,
    #[serde(default, alias = "displayName")]
    pub display_name: String,
    #[serde(default, alias = "supportedGenerationMethods")]
    pub supported_generation_methods: Vec<String>,
}

// ---------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub temperature: f64,
    pub max_tokens: u32,
    pub default_model: String,
    pub language: String,
    pub theme: String,
    pub welcome_message: String,
}

/// Available Gemini 3 model IDs.
pub const GEMINI_MODELS: &[(&str, &str)] = &[
    ("gemini-3-flash-preview", "Gemini 3 Flash"),
    ("gemini-3-pro-preview", "Gemini 3 Pro"),
    ("gemini-3-pro-image-preview", "Gemini 3 Pro Image"),
];

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            temperature: 1.0,
            max_tokens: 8192,
            default_model: "gemini-3-flash-preview".to_string(),
            language: "en".to_string(),
            theme: "dark".to_string(),
            welcome_message: String::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// System Stats
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemStats {
    pub cpu_usage_percent: f32,
    pub memory_used_mb: f64,
    pub memory_total_mb: f64,
    pub platform: String,
}

// ---------------------------------------------------------------------------
// Chat History
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: String,
    pub role: String,
    pub content: String,
    #[serde(default)]
    pub model: Option<String>,
    pub timestamp: String,
    #[serde(default)]
    pub agent: Option<String>,
}

// ---------------------------------------------------------------------------
// File Access
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileReadRequest {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileReadResponse {
    pub path: String,
    pub content: String,
    pub size_bytes: u64,
    pub truncated: bool,
    pub extension: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileListRequest {
    pub path: String,
    #[serde(default)]
    pub show_hidden: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileListResponse {
    pub path: String,
    pub entries: Vec<FileEntryResponse>,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntryResponse {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size_bytes: u64,
    pub extension: Option<String>,
}

// ---------------------------------------------------------------------------
// Classify
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassifyRequest {
    pub prompt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassifyResponse {
    pub agent: String,
    pub confidence: f64,
    pub reasoning: String,
}

// ---------------------------------------------------------------------------
// Sessions
// ---------------------------------------------------------------------------

#[derive(sqlx::FromRow)]
pub struct SessionRow {
    pub id: uuid::Uuid,
    pub title: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(sqlx::FromRow)]
pub struct SessionSummaryRow {
    pub id: uuid::Uuid,
    pub title: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub message_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub messages: Vec<ChatMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub message_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSessionRequest {
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateSessionRequest {
    pub title: String,
}

// ---------------------------------------------------------------------------
// WebSocket Protocol
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WsClientMessage {
    Execute {
        prompt: String,
        mode: String,
        #[serde(default)]
        model: Option<String>,
        #[serde(default)]
        session_id: Option<String>,
    },
    Cancel,
    Ping,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WsServerMessage {
    Start {
        id: String,
        agent: String,
        model: String,
        files_loaded: Vec<String>,
    },
    Token {
        content: String,
    },
    Plan {
        agent: String,
        confidence: f64,
        steps: Vec<String>,
    },
    Complete {
        duration_ms: u64,
    },
    ToolCall {
        name: String,
        args: serde_json::Value,
        iteration: u32,
    },
    ToolResult {
        name: String,
        success: bool,
        summary: String,
        iteration: u32,
    },
    Error {
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        code: Option<String>,
    },
    Pong,
}
