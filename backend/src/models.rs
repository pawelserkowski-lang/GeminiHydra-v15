use serde::{Deserialize, Serialize};

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
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            temperature: 0.7,
            max_tokens: 8192,
            default_model: "gemini-2.0-flash".to_string(),
            language: "en".to_string(),
            theme: "dark".to_string(),
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
