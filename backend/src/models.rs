use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

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
    #[sqlx(default)]
    pub use_docker_sandbox: bool,
    #[sqlx(default)]
    pub top_p: f64,
    #[sqlx(default)]
    pub response_style: String,
    #[sqlx(default)]
    pub max_iterations: i32,
    /// Gemini 3 thinking level: 'none', 'minimal', 'low', 'medium', 'high'
    #[sqlx(default)]
    pub thinking_level: String,
    /// Working directory for filesystem tools (empty = absolute paths only)
    #[sqlx(default)]
    pub working_directory: String,
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

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct WitcherAgent {
    pub id: String,
    pub name: String,
    pub role: String,
    pub tier: String,
    pub status: String,
    pub description: String,
    #[serde(default)]
    pub system_prompt: Option<String>,
    #[serde(default)]
    pub keywords: Vec<String>,
    /// #48 — Per-agent temperature override (NULL = use global setting)
    #[serde(default)]
    #[sqlx(default)]
    pub temperature: Option<f64>,
    /// Per-agent model override (NULL = use global default_model)
    #[serde(default)]
    #[sqlx(default)]
    pub model_override: Option<String>,
}

// ---------------------------------------------------------------------------
// Health
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ProviderInfo {
    pub name: String,
    pub available: bool,
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub app: String,
    pub uptime_seconds: u64,
    pub providers: Vec<ProviderInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
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

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ExecuteRequest {
    pub prompt: String,
    pub mode: String,
    #[serde(default)]
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ExecutePlan {
    pub agent: Option<String>,
    pub steps: Vec<String>,
    pub estimated_time: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
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

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct GeminiStreamRequest {
    pub prompt: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub temperature: Option<f64>,
    #[serde(default)]
    pub max_tokens: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct GeminiModelsResponse {
    pub models: Vec<GeminiModelInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
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

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AppSettings {
    pub temperature: f64,
    pub max_tokens: u32,
    pub default_model: String,
    pub language: String,
    pub theme: String,
    pub welcome_message: String,
    pub use_docker_sandbox: bool,
    /// #46 — topP for Gemini generationConfig
    pub top_p: f64,
    /// #47 — Response style: 'concise', 'balanced', 'detailed', 'technical'
    pub response_style: String,
    /// #49 — Max tool call iterations per request
    pub max_iterations: i32,
    /// Gemini 3 thinking level: 'none', 'minimal', 'low', 'medium', 'high'
    pub thinking_level: String,
    /// Working directory for filesystem tools (empty = absolute paths only)
    pub working_directory: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            temperature: 1.0,
            max_tokens: 65536,
            default_model: "gemini-3.1-pro-preview".to_string(),
            language: "en".to_string(),
            theme: "dark".to_string(),
            welcome_message: String::new(),
            use_docker_sandbox: false,
            top_p: 0.95,
            response_style: "balanced".into(),
            max_iterations: 10,
            thinking_level: "medium".into(),
            working_directory: String::new(),
        }
    }
}


// ---------------------------------------------------------------------------
// System Stats
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SystemStats {
    pub cpu_usage_percent: f32,
    pub memory_used_mb: f64,
    pub memory_total_mb: f64,
    pub platform: String,
}

// ---------------------------------------------------------------------------
// Chat History
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
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

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct FileReadRequest {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct FileReadResponse {
    pub path: String,
    pub content: String,
    pub size_bytes: u64,
    pub truncated: bool,
    pub extension: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct FileListRequest {
    pub path: String,
    #[serde(default)]
    pub show_hidden: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct FileListResponse {
    pub path: String,
    pub entries: Vec<FileEntryResponse>,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
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

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ClassifyRequest {
    pub prompt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ClassifyResponse {
    pub agent: String,
    pub confidence: f64,
    pub reasoning: String,
}

// ---------------------------------------------------------------------------
// Ratings
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RatingRequest {
    pub message_id: String,
    pub session_id: String,
    pub rating: i32,
    #[serde(default)]
    pub feedback: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RatingResponse {
    pub success: bool,
    pub message_id: String,
}

// ---------------------------------------------------------------------------
// Agent Unlock
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UnlockAgentResponse {
    pub session_id: String,
    pub previous_agent: Option<String>,
    pub unlocked: bool,
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

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Session {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub messages: Vec<ChatMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SessionSummary {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub message_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateSessionRequest {
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
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
    /// Orchestrated multi-agent execution via ADK sidecar.
    Orchestrate {
        prompt: String,
        /// Pattern: "sequential", "parallel", "loop", "hierarchical", "review", "security"
        pattern: String,
        #[serde(default)]
        agents: Option<Vec<String>>,
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
        reasoning: String,
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
    ToolProgress {
        iteration: u32,
        tools_completed: u32,
        tools_total: u32,
    },
    Iteration {
        number: u32,
        max: u32,
    },
    AgentSuggestion {
        agent: String,
        confidence: f64,
        reasoning: String,
    },
    Error {
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        code: Option<String>,
    },
    Pong,
    Heartbeat,
    // ── ADK Orchestration messages ──────────────────────────────────
    /// Orchestration pipeline started
    OrchestrationStart {
        pattern: String,
        agents: Vec<String>,
    },
    /// Agent delegated work to another agent (hierarchical)
    AgentDelegation {
        from_agent: String,
        to_agent: String,
        reason: String,
    },
    /// Output from an individual agent in the pipeline
    AgentOutput {
        agent: String,
        content: String,
        is_final: bool,
    },
    /// Progress through a sequential pipeline
    PipelineProgress {
        current_step: u32,
        total_steps: u32,
        current_agent: String,
        status: String,
    },
    /// Status of all agents in a parallel pipeline
    ParallelStatus {
        agents: Vec<ParallelAgentStatus>,
    },
}

/// Status of a single agent in a parallel orchestration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParallelAgentStatus {
    pub agent: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_preview: Option<String>,
}
