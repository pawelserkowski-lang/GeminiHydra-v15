use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use axum::extract::ws::{Message as WsMessage, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use futures_util::{SinkExt, StreamExt};
use rand::Rng;
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::files;
use crate::models::{
    ClassifyRequest, ClassifyResponse, DetailedHealthResponse, ExecutePlan, ExecuteRequest,
    ExecuteResponse, FileEntryResponse, FileListRequest, FileListResponse, FileReadRequest,
    FileReadResponse, GeminiModelInfo, GeminiModelsResponse, HealthResponse, ProviderInfo,
    SystemStats, WitcherAgent, WsClientMessage, WsServerMessage,
};
use crate::state::AppState;

// ---------------------------------------------------------------------------
// Jaskier Shared Pattern -- error
// ---------------------------------------------------------------------------

/// Centralized API error type for all handlers.
/// Logs full details server-side, returns sanitized JSON to the client.
///
/// Response format (structured):
/// ```json
/// {
///   "error": {
///     "code": "BAD_REQUEST",
///     "message": "Human-readable description",
///     "request_id": "uuid-from-correlation-id",
///     "details": { ... }       // optional, null when absent
///   }
/// }
/// ```
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Upstream API error: {0}")]
    Upstream(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Not authenticated: {0}")]
    Unauthorized(String),

    #[error("Service unavailable: {0}")]
    Unavailable(String),

    #[error("Tool timeout: {0}")]
    ToolTimeout(String),

    #[error("Rate limited: {0}")]
    RateLimited(String),
}

/// Structured error response body — serialized inside `{ "error": ... }`.
#[derive(Debug, serde::Serialize)]
pub struct StructuredApiError {
    /// Machine-readable error code (e.g. "BAD_REQUEST", "TOOL_TIMEOUT").
    pub code: &'static str,
    /// Human-readable error message (sanitized, safe to show to users).
    pub message: String,
    /// Correlation ID from the X-Request-Id header / tracing span.
    pub request_id: String,
    /// Optional structured details (context-dependent extra information).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}

impl ApiError {
    /// Machine-readable error code string for each variant.
    fn error_code(&self) -> &'static str {
        match self {
            ApiError::BadRequest(_) => "BAD_REQUEST",
            ApiError::NotFound(_) => "NOT_FOUND",
            ApiError::Upstream(_) => "UPSTREAM_ERROR",
            ApiError::Internal(_) => "INTERNAL_ERROR",
            ApiError::Unauthorized(_) => "UNAUTHORIZED",
            ApiError::Unavailable(_) => "SERVICE_UNAVAILABLE",
            ApiError::ToolTimeout(_) => "TOOL_TIMEOUT",
            ApiError::RateLimited(_) => "RATE_LIMITED",
        }
    }

    /// Attach optional structured details. Returns a `(ApiError, Option<Value>)` tuple
    /// for use with the `with_details` constructor pattern.
    pub fn with_details(self, details: Value) -> ApiErrorWithDetails {
        ApiErrorWithDetails {
            error: self,
            details: Some(details),
        }
    }

    /// Extract the request_id from the current tracing span (set by request_id_middleware).
    fn current_request_id() -> String {
        // The request_id is recorded on the current span by the middleware.
        // We can read it via tracing's span visitor. If not available, generate a new one.
        // Since tracing doesn't provide easy read-back of recorded fields,
        // we use the Uuid approach — the middleware already set X-Request-Id on the response.
        Uuid::new_v4().to_string()
    }
}

impl axum::response::IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let status = match &self {
            ApiError::BadRequest(_) => axum::http::StatusCode::BAD_REQUEST,
            ApiError::NotFound(_) => axum::http::StatusCode::NOT_FOUND,
            ApiError::Upstream(_) => axum::http::StatusCode::BAD_GATEWAY,
            ApiError::Internal(_) => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::Unauthorized(_) => axum::http::StatusCode::UNAUTHORIZED,
            ApiError::Unavailable(_) => axum::http::StatusCode::SERVICE_UNAVAILABLE,
            ApiError::ToolTimeout(_) => axum::http::StatusCode::GATEWAY_TIMEOUT,
            ApiError::RateLimited(_) => axum::http::StatusCode::TOO_MANY_REQUESTS,
        };

        let request_id = Self::current_request_id();

        // Log full detail server-side (with request_id for correlation)
        tracing::error!(
            request_id = %request_id,
            code = self.error_code(),
            "API error ({}): {}",
            status.as_u16(),
            self
        );

        // Return sanitised message to client — never leak internal details
        let message = match &self {
            ApiError::BadRequest(m) => m.clone(),
            ApiError::NotFound(_) => "Resource not found".to_string(),
            ApiError::Upstream(_) => "Upstream service error".to_string(),
            ApiError::Internal(_) => "Internal server error".to_string(),
            ApiError::Unauthorized(m) => m.clone(),
            ApiError::Unavailable(m) => m.clone(),
            ApiError::ToolTimeout(m) => m.clone(),
            ApiError::RateLimited(m) => m.clone(),
        };

        let body = json!({
            "error": {
                "code": self.error_code(),
                "message": message,
                "request_id": request_id,
                "details": null,
            }
        });
        (status, Json(body)).into_response()
    }
}

/// ApiError with optional structured details attached.
/// Use `ApiError::BadRequest("msg".into()).with_details(json!({...}))` to construct.
pub struct ApiErrorWithDetails {
    pub error: ApiError,
    pub details: Option<Value>,
}

impl axum::response::IntoResponse for ApiErrorWithDetails {
    fn into_response(self) -> axum::response::Response {
        let status = match &self.error {
            ApiError::BadRequest(_) => axum::http::StatusCode::BAD_REQUEST,
            ApiError::NotFound(_) => axum::http::StatusCode::NOT_FOUND,
            ApiError::Upstream(_) => axum::http::StatusCode::BAD_GATEWAY,
            ApiError::Internal(_) => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::Unauthorized(_) => axum::http::StatusCode::UNAUTHORIZED,
            ApiError::Unavailable(_) => axum::http::StatusCode::SERVICE_UNAVAILABLE,
            ApiError::ToolTimeout(_) => axum::http::StatusCode::GATEWAY_TIMEOUT,
            ApiError::RateLimited(_) => axum::http::StatusCode::TOO_MANY_REQUESTS,
        };

        let request_id = ApiError::current_request_id();

        tracing::error!(
            request_id = %request_id,
            code = self.error.error_code(),
            "API error ({}): {}",
            status.as_u16(),
            self.error
        );

        let message = match &self.error {
            ApiError::BadRequest(m) => m.clone(),
            ApiError::NotFound(_) => "Resource not found".to_string(),
            ApiError::Upstream(_) => "Upstream service error".to_string(),
            ApiError::Internal(_) => "Internal server error".to_string(),
            ApiError::Unauthorized(m) => m.clone(),
            ApiError::Unavailable(m) => m.clone(),
            ApiError::ToolTimeout(m) => m.clone(),
            ApiError::RateLimited(m) => m.clone(),
        };

        let body = json!({
            "error": {
                "code": self.error.error_code(),
                "message": message,
                "request_id": request_id,
                "details": self.details,
            }
        });
        (status, Json(body)).into_response()
    }
}

// ---------------------------------------------------------------------------
// Helpers & Routing Logic

/// Extract diagnostic info from a Gemini API response that's missing expected parts.
pub(crate) fn gemini_diagnose(resp_json: &Value) -> String {
    let mut diag = Vec::new();

    if let Some(feedback) = resp_json.get("promptFeedback") {
        if let Some(reason) = feedback.get("blockReason").and_then(|v| v.as_str()) {
            diag.push(format!("promptFeedback.blockReason={}", reason));
        }
        if let Some(ratings) = feedback.get("safetyRatings").and_then(|v| v.as_array()) {
            for r in ratings {
                if let (Some(cat), Some(prob)) = (
                    r.get("category").and_then(|v| v.as_str()),
                    r.get("probability").and_then(|v| v.as_str()),
                ) {
                    if prob != "NEGLIGIBLE" && prob != "LOW" {
                        diag.push(format!("safety: {}={}", cat, prob));
                    }
                }
            }
        }
    }

    if let Some(candidates) = resp_json.get("candidates").and_then(|v| v.as_array()) {
        if candidates.is_empty() {
            diag.push("candidates array is empty".to_string());
        } else if let Some(c0) = candidates.first() {
            if let Some(reason) = c0.get("finishReason").and_then(|v| v.as_str()) {
                diag.push(format!("finishReason={}", reason));
            }
            if c0.get("content").is_none() {
                diag.push("candidate has no 'content' field".to_string());
            }
        }
    } else {
        diag.push("no 'candidates' field in response".to_string());
    }

    if diag.is_empty() {
        "unknown (response structure unrecognized)".to_string()
    } else {
        diag.join(", ")
    }
}
// ---------------------------------------------------------------------------

fn build_providers(api_keys: &HashMap<String, String>, cached_google: &[crate::model_registry::ModelInfo]) -> Vec<ProviderInfo> {
    let google_available = api_keys.get("google").is_some_and(|k| !k.is_empty());

    let mut providers = Vec::new();

    // Dynamic list from model cache (populated at startup)
    for m in cached_google {
        providers.push(ProviderInfo {
            name: format!("Google {}", m.display_name.as_deref().unwrap_or(&m.id)),
            available: google_available,
            model: Some(m.id.clone()),
        });
    }

    providers.push(ProviderInfo {
        name: "Anthropic Claude".to_string(),
        available: api_keys.get("anthropic").is_some_and(|k| !k.is_empty()),
        model: Some("claude-sonnet-4-6".to_string()),
    });

    providers
}

fn strip_diacritics(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'ą' => 'a', 'ć' => 'c', 'ę' => 'e', 'ł' => 'l',
            'ń' => 'n', 'ó' => 'o', 'ś' => 's', 'ź' | 'ż' => 'z',
            _ => c,
        })
        .collect()
}

fn keyword_match(text: &str, keyword: &str) -> bool {
    if keyword.len() >= 4 {
        text.contains(keyword)
    } else {
        text.split(|c: char| !c.is_alphanumeric())
            .any(|word| word == keyword)
    }
}

/// Compute the raw keyword confidence score for a single agent against a prompt.
fn classify_agent_score(lower_prompt: &str, agent: &WitcherAgent) -> f64 {
    let mut score = 0.0_f64;
    for keyword in &agent.keywords {
        if keyword_match(lower_prompt, keyword) {
            let weight = if keyword.len() >= 8 { 2.0 }
                else if keyword.len() >= 5 { 1.5 }
                else { 1.0 };
            score += weight;
        }
    }
    if score > 0.0 {
        (0.6 + (score / 8.0).min(0.35)).min(0.95)
    } else {
        0.0
    }
}

/// Expert agent classification based on prompt analysis and agent keywords.
fn classify_prompt(prompt: &str, agents: &[WitcherAgent]) -> (String, f64, String) {
    let lower = strip_diacritics(&prompt.to_lowercase());
    let mut best: Option<(String, f64, f64, String)> = None;

    for agent in agents {
        let mut score = 0.0_f64;
        let mut matched: Vec<&str> = Vec::new();
        for keyword in &agent.keywords {
            if keyword_match(&lower, keyword) {
                let weight = if keyword.len() >= 8 { 2.0 }
                    else if keyword.len() >= 5 { 1.5 }
                    else { 1.0 };
                score += weight;
                matched.push(keyword);
            }
        }
        if score > 0.0 {
            let confidence = (0.6 + (score / 8.0).min(0.35)).min(0.95);
            let reasoning = format!(
                "Matched [{}] for {} (score: {:.1})",
                matched.join(", "), agent.name, score
            );
            if best.as_ref().map_or(true, |b| score > b.2) {
                best = Some((agent.id.clone(), confidence, score, reasoning));
            }
        }
    }

    best.map(|(id, conf, _, reason)| (id, conf, reason))
        .unwrap_or_else(|| ("eskel".to_string(), 0.4, "Defaulting to Backend & APIs".to_string()))
}

/// Semantic classification fallback via Gemini Flash.
/// Called when keyword-based classification gives low confidence (<0.65).
async fn classify_with_gemini(
    client: &reqwest::Client,
    api_key: &str,
    is_oauth: bool,
    prompt: &str,
    agents: &[WitcherAgent],
) -> Option<(String, f64, String)> {
    let agent_list: String = agents.iter()
        .map(|a| format!("- {} ({}): {} [keywords: {}]", a.id, a.role, a.description, a.keywords.join(", ")))
        .collect::<Vec<_>>()
        .join("\n");

    // Safe UTF-8 truncation to 500 chars
    let truncated_prompt: String = prompt.char_indices()
        .take_while(|(i, _)| *i < 500)
        .map(|(_, c)| c)
        .collect();

    let classification_prompt = format!(
        "Given this user prompt:\n\"{}\"\n\nWhich agent should handle it? Choose from:\n{}\n\nRespond with ONLY the agent id (lowercase, one word).",
        truncated_prompt,
        agent_list
    );

    let url = "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash:generateContent";
    let body = serde_json::json!({
        "contents": [{"parts": [{"text": classification_prompt}]}],
        "generationConfig": {"temperature": 1.0, "maxOutputTokens": 256}
    });

    let resp = crate::oauth::apply_google_auth(client.post(url), api_key, is_oauth)
        .json(&body)
        .timeout(std::time::Duration::from_secs(5))
        .send().await.ok()?;

    let j: serde_json::Value = resp.json().await.ok()?;
    let text = j.get("candidates")?.get(0)?.get("content")?.get("parts")?.get(0)?.get("text")?.as_str()?;
    let agent_id = text.trim().to_lowercase();

    if agents.iter().any(|a| a.id == agent_id) {
        Some((agent_id.clone(), 0.80, format!("Gemini Flash classified as '{}'", agent_id)))
    } else {
        tracing::debug!("classify_with_gemini: Gemini returned unknown agent '{}'", agent_id);
        None
    }
}

// ---------------------------------------------------------------------------
// System Prompt Factory
// ---------------------------------------------------------------------------

pub(crate) fn build_system_prompt(agent_id: &str, agents: &[WitcherAgent], language: &str, model: &str, working_directory: &str) -> String {
    let agent = agents.iter().find(|a| a.id == agent_id).unwrap_or(&agents[0]);

    let roster: String = agents
        .iter()
        .map(|a| {
            let kw = if a.keywords.is_empty() { String::new() }
                else { format!(" [{}]", a.keywords.iter().take(5).cloned().collect::<Vec<_>>().join(", ")) };
            format!("  - {} ({}) — {}{}", a.name, a.role, a.description, kw)
        })
        .collect::<Vec<_>>()
        .join("\n");

    let custom = agent.system_prompt.as_deref().unwrap_or("");
    let base_prompt = format!(
        r#"## Identity
**{name}** | {role} | {tier} | `{model}` | GeminiHydra v15

## Rules
- Write ALL text in **{language}** (except code/paths/identifiers).
- You run on a LOCAL Windows machine with FULL filesystem access. NEVER say you can't access files.
- **ACT IMMEDIATELY — NEVER DESCRIBE, NEVER ASK.** When a task requires reading files, listing directories, or searching code, call the tools RIGHT NOW. Do NOT write sentences like "I would read the file..." or "Let me check..." or "First, I'll..." — just call the tool. Never output a numbered plan of steps — execute them.
- **FIX IT, DON'T JUST PROPOSE.** When you find a bug, error, or problem — USE `edit_file` TO APPLY THE FIX IMMEDIATELY. For small changes prefer `edit_file` (replaces targeted section), for new files or full rewrites use `write_file`. Do NOT just show code snippets and say "you should change X to Y". Actually edit the file. The workflow is: read → diagnose → FIX (edit_file) → report what you changed. Only propose without applying if the fix would be destructive (deleting data, dropping tables) or if you're genuinely unsure which of multiple approaches is correct.
- **NEVER ASK THE USER FOR CONFIRMATION OR CLARIFICATION.** Do NOT ask "Do you want me to...?", "Should I...?", "Which file should I...?". Instead, use your tools to gather the information you need, make decisions, and deliver results.
- Use dedicated tools (list_directory, read_file, search_files, get_code_structure) — NEVER execute_command for file ops.
- Call `get_code_structure` BEFORE `read_file` on source files to identify what to read.
- Request MULTIPLE tool calls in PARALLEL when independent.
- **ALWAYS ANSWER WITH TEXT.** After calling tools and applying a fix with edit_file, you MUST write a structured report explaining: what the bug was, what you changed (before/after snippets), and why. Include file paths and line numbers. If you only analyzed (no fix needed), write conclusions with headers, tables, and code refs. NEVER end with only tool outputs — always write at least a paragraph of explanation.
- Use `call_agent` to delegate subtasks to specialized agents (e.g., code analysis → Eskel, debugging → Lambert).

## execute_command Rules
- ALWAYS set `working_directory` to the project root when running cargo/npm/git commands.
- Do NOT use `cd` inside the command — use `working_directory` parameter instead.
- Example: `{{"command": "cargo check", "working_directory": "C:\\Users\\BIURODOM\\Desktop\\GeminiHydra-v15\\backend"}}`
- Do NOT quote paths in `--manifest-path` or similar flags — pass them unquoted.

## Swarm
{roster}"#,
        name = agent.name,
        role = agent.role,
        tier = agent.tier,
        model = model,
        language = language,
        roster = roster
    );

    // Inject working directory section if set
    let wd_section = if !working_directory.is_empty() {
        format!(
            "\n\n## Working Directory\n\
             **Current working directory**: `{wd}`\n\
             - All relative file paths in tool calls resolve against this directory.\n\
             - For `list_directory`, `read_file`, `search_files`, `find_file`, `get_code_structure`, `read_file_section`, `diff_files`: you can use relative paths (e.g., `src/main.rs` instead of `{wd}\\src\\main.rs`).\n\
             - For `execute_command`: if no `working_directory` parameter is set, it defaults to `{wd}`.\n\
             - Absolute paths still work as before.",
            wd = working_directory
        )
    } else {
        String::new()
    };

    let prompt = format!("{}{}", base_prompt, wd_section);

    if !custom.is_empty() {
        format!("{}\n\n## Agent-Specific Instructions\n{}", prompt, custom)
    } else {
        prompt
    }
}

// ---------------------------------------------------------------------------
// REST Handlers
// ---------------------------------------------------------------------------

#[utoipa::path(get, path = "/api/health", tag = "health",
    responses((status = 200, description = "Health check with provider status", body = HealthResponse))
)]
pub async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    let rt = state.runtime.read().await;
    let cache = state.model_cache.read().await;
    let google = cache.models.get("google").cloned().unwrap_or_default();
    drop(cache);
    Json(HealthResponse {
        status: if state.is_ready() { "ok" } else { "starting" }.to_string(),
        version: "15.0.0".to_string(),
        app: "GeminiHydra".to_string(),
        uptime_seconds: state.start_time.elapsed().as_secs(),
        providers: build_providers(&rt.api_keys, &google),
    })
}

/// GET /api/health/ready — lightweight readiness probe (no locks, no DB).
#[utoipa::path(get, path = "/api/health/ready", tag = "health",
    responses(
        (status = 200, description = "Service ready", body = Value),
        (status = 503, description = "Service not ready", body = Value)
    )
)]
pub async fn readiness(State(state): State<AppState>) -> axum::response::Response {
    use axum::http::StatusCode;

    let ready = state.is_ready();
    let uptime = state.start_time.elapsed().as_secs();
    let body = json!({ "ready": ready, "uptime_seconds": uptime });

    if ready {
        (StatusCode::OK, Json(body)).into_response()
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, Json(body)).into_response()
    }
}

/// GET /api/auth/mode — returns whether auth is required (public endpoint).
#[utoipa::path(get, path = "/api/auth/mode", tag = "auth",
    responses((status = 200, description = "Auth mode info", body = Value))
)]
pub async fn auth_mode(State(state): State<AppState>) -> Json<Value> {
    Json(json!({
        "auth_required": state.auth_secret.is_some()
    }))
}

#[utoipa::path(get, path = "/api/health/detailed", tag = "health",
    responses((status = 200, description = "Detailed health with system metrics", body = DetailedHealthResponse))
)]
pub async fn health_detailed(State(state): State<AppState>) -> Json<DetailedHealthResponse> {
    let rt = state.runtime.read().await;
    let cache = state.model_cache.read().await;
    let google = cache.models.get("google").cloned().unwrap_or_default();
    drop(cache);
    let snap = state.system_monitor.read().await;

    Json(DetailedHealthResponse {
        status: "ok".to_string(),
        version: "15.0.0".to_string(),
        app: "GeminiHydra".to_string(),
        uptime_seconds: state.start_time.elapsed().as_secs(),
        providers: build_providers(&rt.api_keys, &google),
        memory_usage_mb: snap.memory_used_mb,
        cpu_usage_percent: snap.cpu_usage_percent,
        platform: snap.platform.clone(),
    })
}

#[utoipa::path(get, path = "/api/agents", tag = "agents",
    responses((status = 200, description = "List of configured agents", body = Value))
)]
pub async fn list_agents(State(state): State<AppState>) -> impl IntoResponse {
    let agents = state.agents.read().await;
    // #6 — Cache agent list for 60 seconds
    (
        [(axum::http::header::CACHE_CONTROL, "public, max-age=60")],
        Json(json!({ "agents": *agents })),
    )
}

#[utoipa::path(post, path = "/api/agents/classify", tag = "agents",
    request_body = ClassifyRequest,
    responses((status = 200, description = "Agent classification result", body = ClassifyResponse))
)]
pub async fn classify_agent(
    State(state): State<AppState>,
    Json(body): Json<ClassifyRequest>,
) -> Json<ClassifyResponse> {
    let agents = state.agents.read().await;
    let (agent_id, confidence, reasoning) = classify_prompt(&body.prompt, &agents);
    Json(ClassifyResponse { agent: agent_id, confidence, reasoning })
}

// ── Agent CRUD ─────────────────────────────────────────────────────────────

#[utoipa::path(post, path = "/api/agents", tag = "agents",
    request_body = WitcherAgent,
    responses((status = 200, description = "Agent created", body = Value))
)]
pub async fn create_agent(
    State(state): State<AppState>,
    Json(agent): Json<WitcherAgent>,
) -> Json<Value> {
    let _ = sqlx::query(
        "INSERT INTO gh_agents (id, name, role, tier, status, description, system_prompt, keywords, temperature) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)"
    )
    .bind(&agent.id)
    .bind(&agent.name)
    .bind(&agent.role)
    .bind(&agent.tier)
    .bind(&agent.status)
    .bind(&agent.description)
    .bind(&agent.system_prompt)
    .bind(&agent.keywords)
    .bind(agent.temperature)
    .execute(&state.db)
    .await;

    state.refresh_agents().await;
    Json(json!({ "success": true }))
}

#[utoipa::path(post, path = "/api/agents/{id}", tag = "agents",
    params(("id" = String, Path, description = "Agent ID")),
    request_body = WitcherAgent,
    responses((status = 200, description = "Agent updated", body = Value))
)]
pub async fn update_agent(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
    Json(agent): Json<WitcherAgent>,
) -> Json<Value> {
    let _ = sqlx::query(
        "UPDATE gh_agents SET name=$1, role=$2, tier=$3, status=$4, description=$5, system_prompt=$6, keywords=$7, temperature=$8, updated_at=NOW() \
         WHERE id=$9"
    )
    .bind(&agent.name)
    .bind(&agent.role)
    .bind(&agent.tier)
    .bind(&agent.status)
    .bind(&agent.description)
    .bind(&agent.system_prompt)
    .bind(&agent.keywords)
    .bind(agent.temperature)
    .bind(id)
    .execute(&state.db)
    .await;

    state.refresh_agents().await;
    Json(json!({ "success": true }))
}

#[utoipa::path(delete, path = "/api/agents/{id}", tag = "agents",
    params(("id" = String, Path, description = "Agent ID")),
    responses((status = 200, description = "Agent deleted", body = Value))
)]
pub async fn delete_agent(
    State(state): State<AppState>,
    axum::extract::ConnectInfo(addr): axum::extract::ConnectInfo<std::net::SocketAddr>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Json<Value> {
    let _ = sqlx::query("DELETE FROM gh_agents WHERE id=$1").bind(&id).execute(&state.db).await;
    state.refresh_agents().await;

    crate::audit::log_audit(
        &state.db,
        "delete_agent",
        json!({ "agent_id": id }),
        Some(&addr.ip().to_string()),
    )
    .await;

    Json(json!({ "success": true }))
}

// ---------------------------------------------------------------------------
// Execution Context & Helpers
// ---------------------------------------------------------------------------

pub(crate) struct ExecuteContext {
    pub(crate) agent_id: String,
    pub(crate) confidence: f64,
    pub(crate) reasoning: String,
    pub(crate) model: String,
    pub(crate) api_key: String,
    /// When true, api_key is an OAuth Bearer token; when false, it's a Google API key.
    pub(crate) is_oauth: bool,
    pub(crate) system_prompt: String,
    pub(crate) final_user_prompt: String,
    pub(crate) files_loaded: Vec<String>,
    pub(crate) steps: Vec<String>,
    pub(crate) temperature: f64,
    pub(crate) max_tokens: i32,
    /// #46 — topP for Gemini generationConfig
    pub(crate) top_p: f64,
    /// #47 — Response style (stored for logging/audit; hint already appended to prompt)
    #[allow(dead_code)]
    pub(crate) response_style: String,
    /// #49 — Max tool call iterations per request
    pub(crate) max_iterations: i32,
    /// Gemini 3 thinking level: 'none', 'minimal', 'low', 'medium', 'high'
    pub(crate) thinking_level: String,
    /// A2A — current agent call depth (0 = user-initiated, max 3)
    pub(crate) call_depth: u32,
    /// Working directory for filesystem tools (empty = absolute paths only)
    pub(crate) working_directory: String,
}

pub(crate) async fn prepare_execution(
    state: &AppState,
    prompt: &str,
    model_override: Option<String>,
    agent_override: Option<(String, f64, String)>,
) -> ExecuteContext {
    let agents_lock = state.agents.read().await;

    // #32 — Parse @agent prefix from prompt before classification
    let (prompt_clean, agent_override_from_prefix) = if prompt.starts_with('@') {
        if let Some(space_idx) = prompt.find(' ') {
            let agent_name = prompt[1..space_idx].to_lowercase();
            if let Some(matched_agent) = agents_lock.iter()
                .find(|a| a.id == agent_name || a.name.to_lowercase() == agent_name)
            {
                let aid = matched_agent.id.clone();
                (prompt[space_idx + 1..].trim().to_string(), Some((aid, 0.99, "User explicitly selected agent via @prefix".to_string())))
            } else {
                (prompt.to_string(), None)
            }
        } else {
            (prompt.to_string(), None)
        }
    } else {
        (prompt.to_string(), None)
    };

    // Determine classification: explicit override > @prefix > keyword + optional Gemini fallback
    let (agent_id, confidence, reasoning) = if let Some(ov) = agent_override {
        ov
    } else if let Some(prefix_ov) = agent_override_from_prefix {
        prefix_ov
    } else {
        let (kw_agent, kw_conf, kw_reason) = classify_prompt(&prompt_clean, &agents_lock);
        // #28 — If keyword confidence is low, try Gemini Flash as fallback
        if kw_conf < 0.65 {
            let classify_cred = crate::oauth::get_google_credential(&state).await;
            if let Some((classify_key, classify_is_oauth)) = classify_cred {
                if let Some(gemini_result) = classify_with_gemini(&state.client, &classify_key, classify_is_oauth, &prompt_clean, &agents_lock).await {
                    tracing::info!("classify: Gemini Flash override — {} (keyword was {} @ {:.0}%)", gemini_result.0, kw_agent, kw_conf * 100.0);
                    gemini_result
                } else {
                    (kw_agent, kw_conf, kw_reason)
                }
            } else {
                (kw_agent, kw_conf, kw_reason)
            }
        } else {
            (kw_agent, kw_conf, kw_reason)
        }
    };

    // #30 — Multi-agent collaboration hint
    let lower_prompt = strip_diacritics(&prompt_clean.to_lowercase());
    let mut top_agents: Vec<_> = agents_lock.iter()
        .map(|a| {
            let score = classify_agent_score(&lower_prompt, a);
            (a.id.clone(), a.name.clone(), score)
        })
        .filter(|(id, _, s)| *s > 0.65 && *id != agent_id)
        .collect();
    top_agents.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
    let collab_hint = if let Some(secondary) = top_agents.first() {
        format!("\n[SYSTEM: This task also relates to {} ({:.0}% match). Consider their perspective in your analysis.]\n",
            secondary.1, secondary.2 * 100.0)
    } else {
        String::new()
    };

    let (def_model, lang, temperature, max_tokens, top_p, response_style, max_iterations, thinking_level, working_directory) =
        sqlx::query_as::<_, (String, String, f64, i32, f64, String, i32, String, String)>(
            "SELECT default_model, language, temperature, max_tokens, top_p, response_style, max_iterations, thinking_level, working_directory \
             FROM gh_settings WHERE id = 1",
        )
        .fetch_one(&state.db)
        .await
        .unwrap_or_else(|_| (
            "gemini-3.1-pro-preview-customtools".to_string(), "en".to_string(), 1.0, 65536, 0.95, "balanced".to_string(), 10, "medium".to_string(), String::new()
        ));

    // #48 — Per-agent temperature override
    let agent_temp = agents_lock.iter()
        .find(|a| a.id == agent_id)
        .and_then(|a| a.temperature);
    let effective_temperature = agent_temp.unwrap_or(temperature);

    // Model priority: 1) user request override → 2) per-agent DB override → 3) global default
    let agent_model = agents_lock.iter()
        .find(|a| a.id == agent_id)
        .and_then(|a| a.model_override.clone());
    let model = model_override.unwrap_or_else(|| agent_model.unwrap_or(def_model));
    let language = match lang.as_str() { "pl" => "Polish", "en" => "English", other => other };

    let (api_key, is_oauth) = crate::oauth::get_google_credential(state)
        .await
        .unwrap_or_default();

    // Cached system prompt — byte-identical across requests enables Gemini implicit caching
    let prompt_cache_key = format!("{}:{}:{}:{}", agent_id, language, model, working_directory);
    let system_prompt = {
        let cache = state.prompt_cache.read().await;
        cache.get(&prompt_cache_key).cloned()
    }.unwrap_or_else(|| {
        let prompt = build_system_prompt(&agent_id, &agents_lock, language, &model, &working_directory);
        let cache_clone = prompt.clone();
        let state_clone = state.prompt_cache.clone();
        let key_clone = prompt_cache_key.clone();
        tokio::spawn(async move {
            state_clone.write().await.insert(key_clone, cache_clone);
        });
        prompt
    });

    let detected_paths = files::extract_file_paths(&prompt_clean);

    // #25 — Sort detected paths by priority: config files first, then source, then docs
    let mut sorted_paths = detected_paths.clone();
    sorted_paths.sort_by_key(|p| {
        let name = std::path::Path::new(p)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");
        match name {
            "Cargo.toml" => 0,
            "package.json" => 1,
            "tsconfig.json" => 2,
            "go.mod" => 3,
            "pyproject.toml" => 4,
            "vite.config.ts" | "vite.config.js" => 5,
            "docker-compose.yml" | "docker-compose.yaml" => 6,
            "Makefile" => 7,
            "CLAUDE.md" => 8,
            "README.md" => 90,
            "LICENSE" | "LICENSE.md" => 91,
            _ => 50, // source files in the middle
        }
    });

    // #21 — Capture file context errors instead of discarding them
    let (file_context, context_errors) = if !sorted_paths.is_empty() {
        files::build_file_context(&sorted_paths).await
    } else {
        (String::new(), Vec::new())
    };

    let skip_warning = if !context_errors.is_empty() {
        format!("\n[SYSTEM: {} file(s) could not be auto-loaded (size/quota exceeded). Use read_file or read_file_section to inspect them manually.]\n", context_errors.len())
    } else {
        String::new()
    };

    let files_loaded = if !file_context.is_empty() { sorted_paths } else { Vec::new() };

    // #24 — Add file context summary
    let context_summary = if !files_loaded.is_empty() {
        let total_size = file_context.len();
        format!("\n[AUTO-LOADED: {} file(s), ~{}KB total: {}]\n",
            files_loaded.len(),
            total_size / 1024,
            files_loaded.join(", "))
    } else {
        String::new()
    };

    let dir_hint = detected_paths.iter()
        .filter(|p| std::path::Path::new(p).is_dir())
        .map(|p| format!("\"{}\"", p))
        .collect::<Vec<_>>();

    let dir_hint_str = if !dir_hint.is_empty() {
        format!("\n[SYSTEM HINT: Directory paths detected: {}. Use list_directory to explore them IMMEDIATELY.]\n", dir_hint.join(", "))
    } else {
        String::new()
    };

    // #47 — Response style hint
    let style_hint = match response_style.as_str() {
        "concise" => "\n[STYLE: Be extremely concise. Max 500 words. Tables over paragraphs. No filler or repetition.]\n",
        "detailed" => "\n[STYLE: Provide thorough analysis with examples, code snippets, and detailed explanations.]\n",
        "technical" => "\n[STYLE: Assume expert reader. Skip basics. Focus on implementation details, edge cases, and architecture.]\n",
        _ => "", // "balanced" = default, no override
    };

    // #50 — Rating-based quality warning (fire-and-forget, don't block on failure)
    let rating_warning = match sqlx::query_as::<_, (f64, i64)>(
        "SELECT COALESCE(avg_rating, 5.0)::FLOAT8, COALESCE(total_ratings, 0)::BIGINT \
         FROM gh_agent_rating_stats WHERE agent_id = $1"
    )
    .bind(&agent_id)
    .fetch_optional(&state.db)
    .await
    {
        Ok(Some((avg, total))) if avg < 3.0 && total >= 5 => {
            format!(
                "\n[QUALITY ALERT: Your recent responses received low ratings (avg {:.1}/5). \
                 Focus on: being concise, using tables, providing actionable insights instead of generic commentary.]\n",
                avg
            )
        }
        _ => String::new(),
    };

    let final_user_prompt = format!(
        "{}{}{}{}{}{}{}{}",
        file_context, context_summary, prompt_clean, dir_hint_str, skip_warning, style_hint, rating_warning, collab_hint
    );

    let steps = vec![
        "classify prompt".into(),
        format!("route to agent (confidence {:.0}%)", confidence * 100.0),
        format!("call Gemini model {}", model),
    ];

    ExecuteContext {
        agent_id,
        confidence,
        reasoning,
        model,
        api_key,
        is_oauth,
        system_prompt,
        final_user_prompt,
        files_loaded,
        steps,
        temperature: effective_temperature,
        max_tokens,
        top_p,
        response_style,
        max_iterations,
        thinking_level,
        call_depth: 0,
        working_directory,
    }
}

// ---------------------------------------------------------------------------
// Gemini 3 Thinking Config Helper
// ---------------------------------------------------------------------------

/// Build the thinkingConfig JSON for Gemini generationConfig.
/// - Gemini 3+ models: use `thinkingLevel` (string enum: minimal/low/medium/high)
/// - Gemini 2.5 models: use `thinkingBudget` (integer) mapped from thinking_level
/// - "none" disables thinking entirely (omit thinkingConfig)
pub(crate) fn build_thinking_config(model: &str, thinking_level: &str) -> Option<Value> {
    if thinking_level == "none" {
        return None;
    }

    let is_thinking_capable = model.contains("pro") || model.contains("flash");
    if !is_thinking_capable {
        return None;
    }

    if model.contains("gemini-3") {
        // Gemini 3+: thinkingLevel string enum
        Some(json!({ "thinkingLevel": thinking_level }))
    } else {
        // Gemini 2.5 and earlier: thinkingBudget integer mapped from level
        let budget = match thinking_level {
            "minimal" => 1024,
            "low" => 2048,
            "medium" => 4096,
            "high" => 8192,
            _ => 4096,
        };
        Some(json!({ "thinkingBudget": budget }))
    }
}

// ---------------------------------------------------------------------------
// SSE / WebSocket Streaming Refactored
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
enum SseParsedEvent {
    TextToken(String),
    FunctionCall { name: String, args: Value, raw_part: Value },
    /// Gemini returned MALFORMED_FUNCTION_CALL — tool schema issue, retry without tools
    MalformedFunctionCall,
}

struct SseParser { buffer: String }

impl SseParser {
    fn new() -> Self { Self { buffer: String::new() } }

    fn parse_parts(json_val: &Value) -> Vec<SseParsedEvent> {
        let mut events = Vec::new();
        if let Some(parts) = json_val
            .get("candidates")
            .and_then(|c| c.get(0))
            .and_then(|c0| c0.get("content"))
            .and_then(|ct| ct.get("parts"))
            .and_then(|p| p.as_array())
        {
            for part in parts {
                if let Some(text) = part["text"].as_str().filter(|t| !t.is_empty()) {
                    events.push(SseParsedEvent::TextToken(text.to_string()));
                }
                if let Some(name) = part.get("functionCall").and_then(|fc| fc["name"].as_str()) {
                    events.push(SseParsedEvent::FunctionCall {
                        name: name.to_string(),
                        args: part["functionCall"]["args"].clone(),
                        raw_part: part.clone(),
                    });
                }
            }
        } else {
            // Log diagnostic info for chunks that might indicate safety blocks or errors
            if let Some(reason) = json_val.get("promptFeedback")
                .and_then(|f| f.get("blockReason"))
                .and_then(|v| v.as_str())
            {
                tracing::warn!("stream: Gemini blocked request (blockReason={})", reason);
            }
            if let Some(reason) = json_val.get("candidates")
                .and_then(|c| c.get(0))
                .and_then(|c0| c0.get("finishReason"))
                .and_then(|v| v.as_str())
            {
                if reason == "MALFORMED_FUNCTION_CALL" {
                    tracing::warn!("stream: MALFORMED_FUNCTION_CALL — will retry without tools");
                    events.push(SseParsedEvent::MalformedFunctionCall);
                } else if reason != "STOP" {
                    tracing::warn!("stream: Gemini chunk has no 'parts' (finishReason={})", reason);
                }
            }
        }
        events
    }

    fn feed(&mut self, chunk: &str) -> Vec<SseParsedEvent> {
        self.buffer.push_str(chunk);
        let mut events = Vec::new();
        while let Some(pos) = self.buffer.find("\n\n") {
            let block = self.buffer[..pos].to_string();
            self.buffer = self.buffer[pos + 2..].to_string();
            for line in block.lines() {
                if let Some(jv) = line.strip_prefix("data: ")
                    .filter(|d| *d != "[DONE]" && !d.is_empty())
                    .and_then(|data| serde_json::from_str::<Value>(data).ok())
                {
                    events.extend(Self::parse_parts(&jv));
                }
            }
        }
        events
    }

    fn flush(&mut self) -> Vec<SseParsedEvent> {
        let mut events = Vec::new();
        for line in self.buffer.lines() {
            if let Some(jv) = line.strip_prefix("data: ")
                .filter(|d| *d != "[DONE]" && !d.is_empty())
                .and_then(|data| serde_json::from_str::<Value>(data).ok())
            {
                events.extend(Self::parse_parts(&jv));
            }
        }
        self.buffer.clear();
        events
    }
}

/// Truncate tool output for Gemini context to prevent context window overflow.
/// Full output is still streamed to the user via WebSocket — this only affects
/// what gets sent back to Gemini as functionResponse for the next iteration.
/// Default limit for truncate_for_context (used as fallback; dynamic limits in the loop override this).
#[allow(dead_code)]
const MAX_TOOL_RESULT_FOR_CONTEXT: usize = 25000;

/// Per-tool execution timeout — prevents individual tool calls from hanging forever.
const TOOL_TIMEOUT: Duration = Duration::from_secs(30);

// ── Retry with exponential backoff constants ────────────────────────────────
/// Maximum number of retry attempts for transient Gemini API errors (429, 503, timeout).
const GEMINI_MAX_RETRIES: u32 = 3;
/// Base delay for exponential backoff (doubles each attempt: 1s, 2s, 4s).
const GEMINI_BACKOFF_BASE: Duration = Duration::from_secs(1);
/// Maximum random jitter added to each backoff delay.
const GEMINI_BACKOFF_JITTER_MS: u64 = 500;

/// Wrapper using the default limit (kept for potential external usage).
#[allow(dead_code)]
fn truncate_for_context(output: &str) -> String {
    truncate_for_context_with_limit(output, MAX_TOOL_RESULT_FOR_CONTEXT)
}

fn truncate_for_context_with_limit(output: &str, limit: usize) -> String {
    if output.len() <= limit {
        return output.to_string();
    }
    let boundary = output.char_indices()
        .take_while(|(i, _)| *i < limit)
        .last()
        .map(|(i, c)| i + c.len_utf8())
        .unwrap_or(0);
    format!(
        "{}\n\n[Output truncated from {} to {} chars. Ask for specific sections if needed. ANALYZE what you see instead of reading more.]",
        &output[..boundary],
        output.len(),
        boundary,
    )
}

// ---------------------------------------------------------------------------
// ADK Internal Tool Bridge
// ---------------------------------------------------------------------------

/// POST /api/internal/tool — Internal tool execution bridge for ADK sidecar.
/// Only reachable from localhost (ADK sidecar). Exposes tools::execute_tool via HTTP.
pub async fn internal_tool_execute(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let name = body["name"]
        .as_str()
        .ok_or_else(|| ApiError::BadRequest("missing 'name' field".into()))?;
    let args = body.get("args").cloned().unwrap_or(json!({}));

    // Read working_directory from settings for tool path resolution
    let wd: String = sqlx::query_scalar("SELECT working_directory FROM gh_settings WHERE id = 1")
        .fetch_one(&state.db)
        .await
        .unwrap_or_default();

    match crate::tools::execute_tool(name, &args, &state, &wd).await {
        Ok(output) => Ok(Json(json!({
            "status": "success",
            "result": output.text
        }))),
        Err(e) => Ok(Json(json!({
            "status": "error",
            "result": e
        }))),
    }
}

async fn ws_send(sender: &mut futures_util::stream::SplitSink<WebSocket, WsMessage>, msg: &WsServerMessage) -> bool {
    if let Ok(json) = serde_json::to_string(msg) {
        sender.send(WsMessage::Text(json.into())).await.is_ok()
    } else {
        false
    }
}

// ---------------------------------------------------------------------------
// WebSocket Handler
// ---------------------------------------------------------------------------

pub async fn ws_execute(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
) -> impl IntoResponse {
    // Validate auth for WebSocket connections via query param
    let query_str = params
        .iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect::<Vec<_>>()
        .join("&");

    if !crate::auth::validate_ws_token(&query_str, state.auth_secret.as_deref()) {
        return (axum::http::StatusCode::UNAUTHORIZED, "Unauthorized").into_response();
    }

    ws.on_upgrade(move |socket| handle_ws(socket, state))
        .into_response()
}

async fn handle_ws(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();
    let cancel = CancellationToken::new();

    loop {
        tokio::select! {
            msg = receiver.next() => {
                match msg {
                    Some(Ok(WsMessage::Text(text))) => {
                        let client_msg: WsClientMessage = match serde_json::from_str(&text) {
                            Ok(m) => m,
                            Err(e) => {
                                let _ = ws_send(&mut sender, &WsServerMessage::Error { message: e.to_string(), code: Some("PARSE_ERROR".into()) }).await;
                                continue;
                            }
                        };
                        match client_msg {
                            WsClientMessage::Ping => { let _ = ws_send(&mut sender, &WsServerMessage::Pong).await; }
                            WsClientMessage::Cancel => { cancel.cancel(); }
                            WsClientMessage::Execute { prompt, mode, model, session_id } => {
                                execute_streaming(&mut sender, &state, &prompt, mode, model, session_id, cancel.child_token()).await;
                            }
                            WsClientMessage::Orchestrate { prompt, pattern, agents, session_id } => {
                                execute_orchestrated(&mut sender, &state, &prompt, &pattern, agents.as_deref(), session_id, cancel.child_token()).await;
                            }
                        }
                    }
                    Some(Ok(WsMessage::Ping(data))) => {
                        let _ = sender.send(WsMessage::Pong(data)).await;
                    }
                    Some(Ok(_)) => {} // ignore binary, close frames etc.
                    _ => break, // connection closed or error
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// ADK Orchestrated Execution (proxy to Python sidecar)
// ---------------------------------------------------------------------------

/// Proxy orchestration requests to the ADK Python sidecar's /run_sse endpoint.
/// Translates SSE events from ADK into WsServerMessage variants and forwards to the WS client.
async fn execute_orchestrated(
    sender: &mut futures_util::stream::SplitSink<WebSocket, WsMessage>,
    state: &AppState,
    prompt: &str,
    pattern: &str,
    agents: Option<&[String]>,
    session_id: Option<String>,
    cancel: CancellationToken,
) {
    let start = Instant::now();
    let adk_url = std::env::var("ADK_SIDECAR_URL")
        .unwrap_or_else(|_| "http://localhost:8000".into());

    // Announce orchestration start
    let agent_list = agents.map(|a| a.to_vec()).unwrap_or_default();
    let _ = ws_send(sender, &WsServerMessage::OrchestrationStart {
        pattern: pattern.to_string(),
        agents: agent_list,
    }).await;

    // Build ADK request
    let sid = session_id.unwrap_or_else(|| Uuid::new_v4().to_string());
    let adk_body = json!({
        "appName": "geminihydra",
        "userId": "default",
        "sessionId": sid,
        "newMessage": {
            "role": "user",
            "parts": [{ "text": prompt }]
        },
        "streaming": true,
        "config": {
            "pattern": pattern,
        }
    });

    // Stream SSE from ADK sidecar
    let resp = match state.client
        .post(format!("{}/run_sse", adk_url))
        .json(&adk_body)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("ADK sidecar unreachable ({}), falling back to direct execution", e);
            let _ = ws_send(sender, &WsServerMessage::Error {
                message: format!("ADK sidecar unavailable: {}. Use direct mode.", e),
                code: Some("ADK_UNAVAILABLE".into()),
            }).await;
            return;
        }
    };

    if !resp.status().is_success() {
        let status = resp.status();
        let body_text = resp.text().await.unwrap_or_default();
        let _ = ws_send(sender, &WsServerMessage::Error {
            message: format!("ADK returned {}: {}", status, &body_text[..body_text.len().min(200)]),
            code: Some("ADK_ERROR".into()),
        }).await;
        return;
    }

    // Parse SSE stream and translate to WS messages
    let mut stream = resp.bytes_stream();
    let mut buffer = String::new();
    let mut last_author = String::new();
    let mut step_count: u32 = 0;
    let mut heartbeat = tokio::time::interval(Duration::from_secs(15));

    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                tracing::info!("Orchestration cancelled by client");
                break;
            }
            _ = heartbeat.tick() => {
                let _ = ws_send(sender, &WsServerMessage::Heartbeat).await;
            }
            chunk = stream.next() => {
                match chunk {
                    Some(Ok(bytes)) => {
                        buffer.push_str(&String::from_utf8_lossy(&bytes));

                        // Parse SSE events from buffer
                        while let Some(pos) = buffer.find("\n\n") {
                            let event_text = buffer[..pos].to_string();
                            buffer = buffer[pos + 2..].to_string();

                            // Extract data: lines
                            for line in event_text.lines() {
                                if let Some(data) = line.strip_prefix("data: ") {
                                    if let Ok(event) = serde_json::from_str::<Value>(data) {
                                        translate_adk_event(sender, &event, &mut last_author, &mut step_count).await;
                                    }
                                }
                            }
                        }
                    }
                    Some(Err(e)) => {
                        tracing::error!("ADK SSE stream error: {}", e);
                        break;
                    }
                    None => break, // stream ended
                }
            }
        }
    }

    let _ = ws_send(sender, &WsServerMessage::Complete {
        duration_ms: start.elapsed().as_millis() as u64,
    }).await;
}

/// Translate a single ADK SSE event into WsServerMessage(s).
async fn translate_adk_event(
    sender: &mut futures_util::stream::SplitSink<WebSocket, WsMessage>,
    event: &Value,
    last_author: &mut String,
    step_count: &mut u32,
) {
    let author = event["author"].as_str().unwrap_or("unknown");

    // Detect agent change (delegation)
    if author != last_author.as_str() && !last_author.is_empty() {
        let reason = event.get("transfer_to_agent")
            .and_then(|v| v.as_str())
            .unwrap_or("task delegation")
            .to_string();

        let _ = ws_send(sender, &WsServerMessage::AgentDelegation {
            from_agent: last_author.clone(),
            to_agent: author.to_string(),
            reason,
        }).await;

        *step_count += 1;
    }
    *last_author = author.to_string();

    // Function call event
    if let Some(fc) = event.get("function_call") {
        let name = fc["name"].as_str().unwrap_or("unknown").to_string();
        let args = fc.get("args").cloned().unwrap_or(json!({}));
        let _ = ws_send(sender, &WsServerMessage::ToolCall {
            name,
            args,
            iteration: *step_count,
        }).await;
        return;
    }

    // Function response event
    if event.get("function_response").is_some() {
        let name = event["function_response"]["name"].as_str().unwrap_or("unknown").to_string();
        let _ = ws_send(sender, &WsServerMessage::ToolResult {
            name,
            success: true,
            summary: "Tool completed".to_string(),
            iteration: *step_count,
        }).await;
        return;
    }

    // Text content from agent
    if let Some(text) = event.get("text").and_then(|t| t.as_str()) {
        if !text.is_empty() {
            let _ = ws_send(sender, &WsServerMessage::AgentOutput {
                agent: author.to_string(),
                content: text.to_string(),
                is_final: false,
            }).await;
            // Also send as Token for backward compat with existing chat UI
            let _ = ws_send(sender, &WsServerMessage::Token {
                content: text.to_string(),
            }).await;
        }
    }

    // Escalation (loop exit)
    if event.get("escalate").and_then(|v| v.as_bool()).unwrap_or(false) {
        let _ = ws_send(sender, &WsServerMessage::AgentOutput {
            agent: author.to_string(),
            content: "Pipeline stage completed (escalated)".to_string(),
            is_final: true,
        }).await;
    }
}

// ---------------------------------------------------------------------------
// Streaming Execution Engine
// ---------------------------------------------------------------------------

async fn execute_streaming(
    sender: &mut futures_util::stream::SplitSink<WebSocket, WsMessage>,
    state: &AppState,
    prompt: &str,
    mode: String,
    model_override: Option<String>,
    session_id: Option<String>,
    cancel: CancellationToken,
) {
    let start = Instant::now();
    let sid = session_id.as_deref().and_then(|s| Uuid::parse_str(s).ok());

    // Resolve agent: explicit mode > session lock > classify
    let agent_info = if !mode.is_empty() && mode != "auto" {
        let agents = state.agents.read().await;
        agents.iter()
            .find(|a| a.id == mode || a.name.to_lowercase() == mode.to_lowercase())
            .map(|a| (a.id.clone(), 0.99_f64, "User explicitly selected agent via mode field".to_string()))
    } else if let Some(s) = &sid {
        Some(resolve_session_agent(state, s, prompt).await)
    } else {
        None
    };

    let mut ctx = prepare_execution(state, prompt, model_override, agent_info).await;
    let resp_id = Uuid::new_v4();

    // Session WD override (takes priority over global setting)
    if let Some(ref s) = sid {
        let session_wd: String = sqlx::query_scalar(
            "SELECT working_directory FROM gh_sessions WHERE id = $1",
        )
        .bind(s)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten()
        .unwrap_or_default();
        if !session_wd.is_empty() {
            ctx.working_directory = session_wd.clone();
            // Rebuild system prompt with session WD
            let agents_lock = state.agents.read().await;
            let (_, lang, ..) = sqlx::query_as::<_, (String, String,)>(
                "SELECT default_model, language FROM gh_settings WHERE id = 1",
            )
            .fetch_one(&state.db)
            .await
            .unwrap_or_else(|_| ("gemini-3.1-pro-preview".to_string(), "en".to_string()));
            let language = match lang.as_str() { "pl" => "Polish", "en" => "English", other => other };
            ctx.system_prompt = build_system_prompt(&ctx.agent_id, &agents_lock, language, &ctx.model, &session_wd);
        }
    }

    if !ws_send(sender, &WsServerMessage::Start { id: resp_id.to_string(), agent: ctx.agent_id.clone(), model: ctx.model.clone(), files_loaded: ctx.files_loaded.clone() }).await { return; }
    let _ = ws_send(sender, &WsServerMessage::Plan { agent: ctx.agent_id.clone(), confidence: ctx.confidence, steps: ctx.steps.clone(), reasoning: ctx.reasoning.clone() }).await;

    // Dispatch to Gemini streaming
    let full_text = execute_streaming_gemini(sender, state, &ctx, sid, cancel).await;

    store_messages(&state.db, sid, resp_id, prompt, &full_text, &ctx).await;
    let _ = ws_send(sender, &WsServerMessage::Complete { duration_ms: start.elapsed().as_millis() as u64 }).await;
}

// ── Gemini retry with exponential backoff ───────────────────────────────────
// Jaskier Shared Pattern -- gemini_retry

/// Whether a reqwest error or HTTP status is a transient failure worth retrying.
fn is_retryable(result: &Result<reqwest::Response, reqwest::Error>) -> bool {
    match result {
        Ok(resp) => {
            let status = resp.status().as_u16();
            status == 429 || status == 503
        }
        Err(e) => e.is_timeout() || e.is_connect(),
    }
}

/// Send a streaming Gemini API request with retry + exponential backoff.
/// Returns the successful response, or the last error after all retries are exhausted.
async fn gemini_request_with_retry(
    client: &reqwest::Client,
    url: &reqwest::Url,
    api_key: &str,
    is_oauth: bool,
    body: &Value,
) -> Result<reqwest::Response, String> {
    let mut last_err = String::new();

    for attempt in 0..=GEMINI_MAX_RETRIES {
        if attempt > 0 {
            // Exponential backoff: base * 2^(attempt-1) + random jitter
            let backoff = GEMINI_BACKOFF_BASE * 2u32.saturating_pow(attempt - 1);
            let jitter = Duration::from_millis(rand::thread_rng().gen_range(0..=GEMINI_BACKOFF_JITTER_MS));
            let delay = backoff + jitter;
            tracing::warn!(
                "gemini_retry: attempt {}/{} after {:?} backoff",
                attempt + 1,
                GEMINI_MAX_RETRIES + 1,
                delay
            );
            tokio::time::sleep(delay).await;
        }

        let result = crate::oauth::apply_google_auth(
                client.post(url.clone()), api_key, is_oauth,
            )
            .json(body)
            .timeout(Duration::from_secs(300))
            .send()
            .await;

        if !is_retryable(&result) {
            // Non-retryable outcome — return immediately.
            return match result {
                Ok(resp) if resp.status().is_success() => Ok(resp),
                Ok(resp) => {
                    let status = resp.status();
                    let err_body = resp.text().await.unwrap_or_default();
                    let safe_len = err_body
                        .char_indices()
                        .take_while(|(i, _)| *i < 500)
                        .last()
                        .map(|(i, c)| i + c.len_utf8())
                        .unwrap_or(0);
                    Err(format!("Gemini API error ({}): {}", status, &err_body[..safe_len]))
                }
                Err(e) => Err(format!("Gemini API request failed: {:?}", e)),
            };
        }

        // Retryable — log and loop.
        last_err = match &result {
            Ok(resp) => format!("HTTP {}", resp.status()),
            Err(e) => format!("{:?}", e),
        };
        tracing::warn!(
            "gemini_retry: transient error on attempt {}: {}",
            attempt + 1,
            last_err
        );
    }

    Err(format!(
        "Gemini API failed after {} attempts — last error: {}",
        GEMINI_MAX_RETRIES + 1,
        last_err
    ))
}

// ── Gemini Implementation ──────────────────────────────────────────────────

async fn execute_streaming_gemini(
    sender: &mut futures_util::stream::SplitSink<WebSocket, WsMessage>,
    state: &AppState,
    ctx: &ExecuteContext,
    sid: Option<Uuid>,
    cancel: CancellationToken,
) -> String {
    if ctx.api_key.is_empty() {
        let _ = ws_send(sender, &WsServerMessage::Error { message: "Missing Google API Key".into(), code: Some("NO_API_KEY".into()) }).await;
        return String::new();
    }

    // Circuit breaker — fail fast if the Gemini provider is tripped.
    if let Err(msg) = state.gemini_circuit.check().await {
        tracing::warn!("execute_streaming_gemini: {}", msg);
        let _ = ws_send(sender, &WsServerMessage::Error { message: msg, code: Some("CIRCUIT_OPEN".into()) }).await;
        return String::new();
    }

    let url = format!("https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent?alt=sse", ctx.model);
    let parsed_url = match reqwest::Url::parse(&url) {
        Ok(u) if u.scheme() == "https" => u,
        _ => {
            let _ = ws_send(sender, &WsServerMessage::Error { message: "API credentials require HTTPS".into(), code: Some("SECURITY".into()) }).await;
            return String::new();
        }
    };
    let tools = build_tools();
    let mut contents = if let Some(s) = &sid { load_session_history(&state.db, s).await } else { Vec::new() };
    contents.push(json!({ "role": "user", "parts": [{ "text": ctx.final_user_prompt }] }));

    // #36 — Dynamic max iterations based on prompt complexity, capped by #49 user setting
    let prompt_len = ctx.final_user_prompt.len();
    let file_count = ctx.files_loaded.len();
    let dynamic_max: usize = if prompt_len < 200 && file_count <= 1 {
        5  // Simple prompt
    } else if prompt_len < 1000 && file_count <= 3 {
        10 // Medium complexity
    } else {
        15 // Complex multi-file analysis
    };
    let max_iterations: usize = dynamic_max.min(ctx.max_iterations.max(1) as usize);

    let mut full_text = String::new();
    let mut has_written_file = false;
    let mut agent_text_len: usize = 0;

    // #39 — Global execution timeout: 3 minutes
    let execution_start = Instant::now();
    let execution_timeout = Duration::from_secs(180);

    for iter in 0..max_iterations {
        // #39 — Check elapsed time at the start of each iteration
        if execution_start.elapsed() >= execution_timeout {
            tracing::warn!("execute_streaming_gemini: global timeout after {}s at iteration {}", execution_start.elapsed().as_secs(), iter);
            let _ = ws_send(sender, &WsServerMessage::Error {
                message: "Execution timed out after 3 minutes".to_string(),
                code: Some("TIMEOUT".to_string()),
            }).await;
            break;
        }

        // #35 — Send iteration counter to frontend
        let _ = ws_send(sender, &WsServerMessage::Iteration { number: iter as u32 + 1, max: max_iterations as u32 }).await;

        let mut gen_config = json!({
            "temperature": ctx.temperature,
            "topP": ctx.top_p,
            "maxOutputTokens": ctx.max_tokens
        });
        if let Some(tc) = build_thinking_config(&ctx.model, &ctx.thinking_level) {
            gen_config["thinkingConfig"] = tc;
        }
        let body = json!({
            "systemInstruction": { "parts": [{ "text": ctx.system_prompt }] },
            "contents": contents,
            "tools": tools,
            "generationConfig": gen_config
        });

        // Use retry-with-backoff helper; circuit breaker is updated on success/failure.
        let resp = match gemini_request_with_retry(&state.client, &parsed_url, &ctx.api_key, ctx.is_oauth, &body).await {
            Ok(r) => {
                state.gemini_circuit.record_success().await;
                r
            }
            Err(e) => {
                state.gemini_circuit.record_failure().await;
                tracing::error!("{}", e);
                let _ = ws_send(sender, &WsServerMessage::Error { message: "AI service error".into(), code: Some("GEMINI_ERROR".into()) }).await;

                // #38 — Model fallback chain: try gemini-2.5-flash if primary model failed
                if full_text.is_empty() && ctx.model != "gemini-2.5-flash" {
                    tracing::warn!("Primary model {} failed, trying gemini-2.5-flash fallback", ctx.model);
                    let fallback_url_str = "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash:streamGenerateContent?alt=sse";
                    if let Ok(fallback_url) = reqwest::Url::parse(fallback_url_str) {
                        if let Ok(fallback_resp) = gemini_request_with_retry(&state.client, &fallback_url, &ctx.api_key, ctx.is_oauth, &body).await {
                            state.gemini_circuit.record_success().await;
                            let (fallback_text, _, _, _) = consume_gemini_stream(fallback_resp, sender, &cancel).await;
                            full_text.push_str(&fallback_text);
                        }
                    }
                }

                return full_text;
            }
        };

        let (text, fcs, aborted, malformed) = consume_gemini_stream(resp, sender, &cancel).await;
        full_text.push_str(&text);
        agent_text_len += text.trim().len();

        // Retry without tools if Gemini generated a malformed function call
        if malformed && full_text.trim().is_empty() {
            tracing::warn!("MALFORMED_FUNCTION_CALL on iter {}, retrying without tools", iter);
            let mut gen_config_retry = json!({
                "temperature": ctx.temperature,
                "topP": ctx.top_p,
                "maxOutputTokens": ctx.max_tokens
            });
            if let Some(tc) = build_thinking_config(&ctx.model, &ctx.thinking_level) {
                gen_config_retry["thinkingConfig"] = tc;
            }
            let retry_body = json!({
                "systemInstruction": { "parts": [{ "text": format!("{}\n\nIMPORTANT: Answer this question directly using your knowledge. Do NOT attempt to call any tools or functions.", ctx.system_prompt) }] },
                "contents": contents,
                "generationConfig": gen_config_retry
            });
            if let Ok(retry_resp) = gemini_request_with_retry(&state.client, &parsed_url, &ctx.api_key, ctx.is_oauth, &retry_body).await {
                let (retry_text, _, _, _) = consume_gemini_stream(retry_resp, sender, &cancel).await;
                full_text.push_str(&retry_text);
            }
            break;
        }
        if aborted || fcs.is_empty() { break; }

        // #37 — Early termination if agent text (excluding tool output) is too small after many iterations
        if iter >= 3 && text.trim().is_empty() {
            // Count how many iterations produced no agent text at all
            let meaningful_text: String = full_text.lines()
                .filter(|l| !l.starts_with("---") && !l.starts_with("```") && !l.starts_with("**🔧 Tool:**"))
                .collect::<Vec<_>>()
                .join("");
            if meaningful_text.trim().len() < 50 {
                let _ = ws_send(sender, &WsServerMessage::Token {
                    content: "\n\n[Agent produced no meaningful response after multiple tool calls. Please rephrase your question.]".to_string()
                }).await;
                break;
            }
        }

        let mut model_parts: Vec<Value> = if !text.is_empty() { vec![json!({ "text": text })] } else { vec![] };
        for (_, _, raw) in &fcs { model_parts.push(raw.clone()); }
        contents.push(json!({ "role": "model", "parts": model_parts }));

        // Announce all tool calls first (so the frontend can show them as "in progress")
        let tool_count = fcs.len();
        for (name, args, _) in &fcs {
            let _ = ws_send(sender, &WsServerMessage::ToolCall { name: name.clone(), args: args.clone(), iteration: iter as u32 + 1 }).await;
        }

        // #40 — Send parallel tool header as ToolProgress instead of Token
        if tool_count > 1 {
            let _ = ws_send(sender, &WsServerMessage::ToolProgress {
                iteration: iter as u32,
                tools_completed: 0,
                tools_total: tool_count as u32,
            }).await;
        }

        // Execute all tool calls concurrently using tokio::join_all.
        // Each call is wrapped in a per-tool timeout so one hanging tool
        // doesn't block the entire iteration.
        // Heartbeat messages are sent every 15s to prevent proxy/LB timeouts.
        let call_depth = ctx.call_depth;
        let wd = ctx.working_directory.clone();
        let tool_futures: Vec<_> = fcs.iter().map(|(name, args, _)| {
            let name = name.clone();
            let args = args.clone();
            let state = state.clone();
            let wd = wd.clone();
            async move {
                if name == "call_agent" {
                    // A2A agent delegation — longer timeout (120s), depth tracking
                    match tokio::time::timeout(
                        Duration::from_secs(120),
                        crate::a2a::execute_agent_call(&state, &args, call_depth),
                    ).await {
                        Ok(Ok(text)) => (name, crate::tools::ToolOutput::text(text)),
                        Ok(Err(e)) => (name, crate::tools::ToolOutput::text(format!("AGENT_CALL_ERROR: {}", e))),
                        Err(_) => (name, crate::tools::ToolOutput::text("AGENT_CALL_ERROR: timed out after 120s".to_string())),
                    }
                } else {
                    match tokio::time::timeout(TOOL_TIMEOUT, crate::tools::execute_tool(&name, &args, &state, &wd)).await {
                        Ok(Ok(output)) => (name, output),
                        Ok(Err(e)) => (name, crate::tools::ToolOutput::text(format!("TOOL_ERROR: {}", e))),
                        Err(_) => {
                            tracing::warn!("tool '{}' timed out after {}s", name, TOOL_TIMEOUT.as_secs());
                            (name, crate::tools::ToolOutput::text(format!("TOOL_ERROR: timed out after {}s", TOOL_TIMEOUT.as_secs())))
                        }
                    }
                }
            }
        }).collect();

        // Run tools in a spawned task so we can send heartbeats concurrently.
        // Heartbeat keeps the WS alive during long tool executions (prevents proxy timeouts).
        let mut tools_handle = tokio::spawn(async move {
            futures_util::future::join_all(tool_futures).await
        });
        let mut heartbeat_interval = tokio::time::interval(Duration::from_secs(15));
        heartbeat_interval.tick().await; // consume immediate first tick
        let tool_results = loop {
            tokio::select! {
                result = &mut tools_handle => {
                    break result.unwrap_or_default();
                }
                _ = heartbeat_interval.tick() => {
                    let _ = ws_send(sender, &WsServerMessage::Heartbeat).await;
                }
            }
        };

        // Gemini 3 Thought Signatures: build name→signature map from function call parts.
        // raw_part (from SseParsedEvent::FunctionCall) is part.clone() which captures
        // thoughtSignature if present. Must echo back on functionResponse parts (400 if missing).
        let sig_map: std::collections::HashMap<&str, &Value> = fcs.iter()
            .filter_map(|(name, _, raw)| {
                raw.get("thoughtSignature").map(|sig| (name.as_str(), sig))
            })
            .collect();

        // Track file-modifying tool usage (write_file or edit_file)
        for (name, _) in &tool_results {
            if name == "write_file" || name == "edit_file" { has_written_file = true; }
        }

        // Stream results to frontend + build Gemini context
        let mut res_parts = Vec::new();
        for (name, output) in &tool_results {
            let success = !output.text.starts_with("TOOL_ERROR:");
            let _ = ws_send(sender, &WsServerMessage::ToolResult { name: name.clone(), success, summary: output.text.chars().take(200).collect(), iteration: iter as u32 + 1 }).await;

            let header = format!("\n\n---\n**🔧 Tool:** `{}`\n", name);
            full_text.push_str(&header);
            let _ = ws_send(sender, &WsServerMessage::Token { content: header }).await;

            let res_md = format!("```\n{}\n```\n---\n\n", output.text);
            full_text.push_str(&res_md);
            let _ = ws_send(sender, &WsServerMessage::Token { content: res_md }).await;

            // #26 — Dynamic context limit based on iteration (earlier = more generous)
            let context_limit = if iter < 3 { 25000 } else if iter < 6 { 15000 } else { 8000 };
            let context_output = truncate_for_context_with_limit(&output.text, context_limit);
            let mut fn_response = json!({ "functionResponse": { "name": name, "response": { "result": context_output } } });
            // Gemini 3 multimodal function response: attach inline data if tool returned binary
            if let Some(ref data) = output.inline_data {
                fn_response["functionResponse"]["response"]["inline_data"] = json!({
                    "mimeType": data.mime_type,
                    "data": data.data
                });
            }
            // Attach thought signature from corresponding function call (Gemini 3 requirement)
            if let Some(sig) = sig_map.get(name.as_str()) {
                fn_response["thoughtSignature"] = (*sig).clone();
            }
            res_parts.push(fn_response);
        }

        // #27 — Approximate context usage metadata
        let approx_context_bytes: usize = contents.iter()
            .map(|c| serde_json::to_string(c).map(|s| s.len()).unwrap_or(0))
            .sum();
        let context_hint = format!("[CONTEXT: ~{}KB used across {} messages, iteration {}/{}]",
            approx_context_bytes / 1024, contents.len(), iter + 1, max_iterations);

        // #34 — Iteration reminders with edit_file tracking
        if iter >= 1 {
            let write_nudge = if has_written_file {
                "You already applied a fix with edit_file/write_file. Now write your report explaining what you changed."
            } else {
                "You have NOT called edit_file or write_file yet. If you found a problem, call edit_file NOW to apply the fix. Do NOT just describe the fix — actually edit the file."
            };
            let urgency = if iter >= 4 {
                format!("[SYSTEM CRITICAL: STOP ALL TOOL CALLS. {} iterations used. {} {} Write your FINAL report NOW.]", iter + 1, context_hint, write_nudge)
            } else if iter >= 2 {
                format!("[SYSTEM: {} iterations used. {} IMPORTANT: {} If you need to fix code, your NEXT tool call MUST be edit_file or write_file.]", iter + 1, context_hint, write_nudge)
            } else {
                format!("[SYSTEM: {} {} You may read 1-2 more files if critical.]", context_hint, write_nudge)
            };
            res_parts.push(json!({ "text": urgency }));
        }
        contents.push(json!({ "role": "user", "parts": res_parts }));
    }

    // #34a — Write-phase enforcement: if agent described a fix but never called edit_file/write_file,
    // give it ONE more Gemini call with ONLY edit/write tools.
    if !has_written_file && !full_text.is_empty() && agent_text_len > 50 {
        let fix_keywords = ["fix", "napraw", "zmian", "popraw", "zastosow", "write_file", "edit_file", "key={`", "prefix"];
        let lower_text = full_text.to_lowercase();
        let looks_like_fix = fix_keywords.iter().any(|kw| lower_text.contains(kw));
        if looks_like_fix {
            tracing::info!("execute_streaming_gemini: agent described a fix but never applied it — forcing edit phase");
            contents.push(json!({
                "role": "user",
                "parts": [{
                    "text": "[SYSTEM: CRITICAL — You described a fix above but you did NOT actually apply it. The file on disk is UNCHANGED. You MUST call edit_file RIGHT NOW to apply your fix. Use the file path, the exact old_text to find, and the new_text replacement. This is your LAST chance to apply the fix.]"
                }]
            }));
            let mut gen_config = json!({
                "temperature": ctx.temperature,
                "topP": ctx.top_p,
                "maxOutputTokens": ctx.max_tokens
            });
            if let Some(tc) = build_thinking_config(&ctx.model, &ctx.thinking_level) {
                gen_config["thinkingConfig"] = tc;
            }
            let edit_only_tools = json!([{
                "function_declarations": [{
                    "name": "edit_file",
                    "description": "Edit an existing file by replacing a specific text section.",
                    "parameters": { "type": "object", "properties": {
                        "path": { "type": "string", "description": "Absolute path to the file to edit" },
                        "old_text": { "type": "string", "description": "Exact text to find and replace" },
                        "new_text": { "type": "string", "description": "Replacement text" }
                    }, "required": ["path", "old_text", "new_text"] }
                }, {
                    "name": "write_file",
                    "description": "Write full file content. Use only if edit_file is not suitable.",
                    "parameters": { "type": "object", "properties": {
                        "path": { "type": "string", "description": "Absolute path to the file to write" },
                        "content": { "type": "string", "description": "Full file content to write" }
                    }, "required": ["path", "content"] }
                }]
            }]);
            let body = json!({
                "systemInstruction": { "parts": [{ "text": &ctx.system_prompt }] },
                "contents": contents,
                "tools": edit_only_tools,
                "generationConfig": gen_config
            });
            if let Ok(resp) = gemini_request_with_retry(&state.client, &parsed_url, &ctx.api_key, ctx.is_oauth, &body).await {
                state.gemini_circuit.record_success().await;
                let (write_text, write_fcs, _, _) = consume_gemini_stream(resp, sender, &cancel).await;
                full_text.push_str(&write_text);
                agent_text_len += write_text.trim().len();
                for (name, args, _) in &write_fcs {
                    if name == "write_file" || name == "edit_file" {
                        tracing::info!("execute_streaming_gemini: edit-phase enforcement — executing {}", name);
                        match tokio::time::timeout(TOOL_TIMEOUT, crate::tools::execute_tool(name, args, &state, &ctx.working_directory)).await {
                            Ok(Ok(output)) => {
                                has_written_file = true;
                                let header = format!("\n\n---\n**Tool:** `{}`\n", name);
                                full_text.push_str(&header);
                                let _ = ws_send(sender, &WsServerMessage::Token { content: header }).await;
                                let res_md = format!("```\n{}\n```\n---\n\n", output.text);
                                full_text.push_str(&res_md);
                                let _ = ws_send(sender, &WsServerMessage::Token { content: res_md }).await;
                            }
                            Ok(Err(e)) => {
                                tracing::warn!("edit-phase {} failed: {}", name, e);
                            }
                            Err(_) => {
                                tracing::warn!("edit-phase {} timed out", name);
                            }
                        }
                    }
                }
            }
        }
    }

    // #34b — Forced synthesis: if agent produced only tool output and no text analysis,
    // do one final Gemini call WITHOUT tools to force a text response.
    if agent_text_len < 100 && !full_text.is_empty() {
        tracing::info!("execute_streaming_gemini: no synthesis text detected (agent_text_len={}) — forcing final synthesis call", agent_text_len);
        contents.push(json!({
            "role": "user",
            "parts": [{
                "text": "[SYSTEM: You called tools and gathered data but did NOT write any text response. Write your comprehensive structured report NOW. If you applied a fix with edit_file or write_file, explain: what the bug was, what you changed (before/after), and file paths with line numbers. If you did NOT apply a fix, explain what you found and what needs to be changed. Use headers (##), bullet points, tables, and code refs.]"
            }]
        }));
        let mut gen_config = json!({
            "temperature": ctx.temperature,
            "topP": ctx.top_p,
            "maxOutputTokens": ctx.max_tokens
        });
        if let Some(tc) = build_thinking_config(&ctx.model, &ctx.thinking_level) {
            gen_config["thinkingConfig"] = tc;
        }
        let body = json!({
            "systemInstruction": { "parts": [{ "text": &ctx.system_prompt }] },
            "contents": contents,
            "generationConfig": gen_config
        });
        match gemini_request_with_retry(&state.client, &parsed_url, &ctx.api_key, ctx.is_oauth, &body).await {
            Ok(resp) => {
                state.gemini_circuit.record_success().await;
                let (synth_text, synth_fcs, _, _) = consume_gemini_stream(resp, sender, &cancel).await;
                tracing::info!("execute_streaming_gemini: synthesis call returned {} chars text, {} function_calls", synth_text.len(), synth_fcs.len());
                full_text.push_str(&synth_text);
            }
            Err(e) => {
                tracing::warn!("execute_streaming_gemini: synthesis Gemini call failed: {}", e);
            }
        }
    }

    full_text
}

/// Returns (text, function_calls, aborted, malformed_tool_call)
async fn consume_gemini_stream(
    resp: reqwest::Response,
    sender: &mut futures_util::stream::SplitSink<WebSocket, WsMessage>,
    cancel: &CancellationToken,
) -> (String, Vec<(String, Value, Value)>, bool, bool) {
    let mut parser = SseParser::new();
    let mut stream = resp.bytes_stream();
    let mut full_text = String::new();
    let mut fcs = Vec::new();
    let mut malformed = false;

    loop {
        tokio::select! {
            _ = cancel.cancelled() => return (full_text, fcs, true, malformed),
            chunk = stream.next() => {
                match chunk {
                    Some(Ok(b)) => {
                        for ev in parser.feed(&String::from_utf8_lossy(&b)) {
                            match ev {
                                SseParsedEvent::TextToken(t) => {
                                    full_text.push_str(&t);
                                    let _ = ws_send(sender, &WsServerMessage::Token { content: t }).await;
                                }
                                SseParsedEvent::FunctionCall { name, args, raw_part } => fcs.push((name, args, raw_part)),
                                SseParsedEvent::MalformedFunctionCall => malformed = true,
                            }
                        }
                    }
                    _ => {
                        for ev in parser.flush() {
                            match ev {
                                SseParsedEvent::TextToken(t) => {
                                    full_text.push_str(&t);
                                    let _ = ws_send(sender, &WsServerMessage::Token { content: t }).await;
                                }
                                SseParsedEvent::FunctionCall { name, args, raw_part } => fcs.push((name, args, raw_part)),
                                SseParsedEvent::MalformedFunctionCall => malformed = true,
                            }
                        }
                        break;
                    }
                }
            }
        }
    }
    (full_text, fcs, false, malformed)
}

// ---------------------------------------------------------------------------
// DB Helpers
// ---------------------------------------------------------------------------

async fn resolve_session_agent(state: &AppState, sid: &Uuid, prompt: &str) -> (String, f64, String) {
    if let Some(aid) = sqlx::query_as::<_, (Option<String>,)>("SELECT agent_id FROM gh_sessions WHERE id = $1")
        .bind(sid)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten()
        .and_then(|(a,)| a)
        .filter(|s| !s.is_empty()) 
    {
        return (aid, 0.95, "Locked".into());
    }
    
    let agents = state.agents.read().await;
    let (aid, conf, reas) = classify_prompt(prompt, &agents);
    
    let _ = sqlx::query("UPDATE gh_sessions SET agent_id = $1 WHERE id = $2").bind(&aid).bind(sid).execute(&state.db).await;
    (aid, conf, reas)
}

async fn load_session_history(db: &sqlx::PgPool, sid: &Uuid) -> Vec<Value> {
    // #22 — Reduced from 50 to 20 to save context window budget
    let mut messages: Vec<Value> = sqlx::query_as::<_, (String, String)>(
        "SELECT role, content FROM gh_chat_messages WHERE session_id = $1 ORDER BY created_at DESC LIMIT 20"
    )
        .bind(sid).fetch_all(db).await.unwrap_or_default().into_iter().rev()
        .map(|(r, c)| json!({ "role": if r == "assistant" { "model" } else { "user" }, "parts": [{ "text": c }] }))
        .collect();

    // #23 — Compress old messages: truncate everything except the last 6 messages
    for i in 0..messages.len() {
        if i < messages.len().saturating_sub(6) {
            if let Some(text) = messages[i].get_mut("parts")
                .and_then(|p| p.get_mut(0))
                .and_then(|p0| p0.get_mut("text"))
            {
                if let Some(s) = text.as_str().map(|s| s.to_string()) {
                    if s.len() > 500 {
                        let boundary = s.char_indices()
                            .take_while(|(idx, _)| *idx < 500)
                            .last()
                            .map(|(idx, c)| idx + c.len_utf8())
                            .unwrap_or(500.min(s.len()));
                        *text = json!(format!("{}... [message truncated for context efficiency]", &s[..boundary]));
                    }
                }
            }
        }
    }

    messages
}

async fn store_messages(db: &sqlx::PgPool, sid: Option<Uuid>, rid: Uuid, prompt: &str, result: &str, ctx: &ExecuteContext) {
    let _ = sqlx::query("INSERT INTO gh_chat_messages (id, role, content, model, agent, session_id) VALUES ($1, 'user', $2, $3, $4, $5)")
        .bind(rid).bind(prompt).bind(Some(&ctx.model)).bind(Some(&ctx.agent_id)).bind(sid).execute(db).await;
    if !result.is_empty() {
        let _ = sqlx::query("INSERT INTO gh_chat_messages (id, role, content, model, agent, session_id) VALUES ($1, 'assistant', $2, $3, $4, $5)")
            .bind(Uuid::new_v4()).bind(result).bind(Some(&ctx.model)).bind(Some(&ctx.reasoning)).bind(sid).execute(db).await;
    }
}

// ---------------------------------------------------------------------------
// Other Handlers (Proxy, Stats, Files)
// ---------------------------------------------------------------------------

#[utoipa::path(get, path = "/api/gemini/models", tag = "models",
    responses((status = 200, description = "Available Gemini models", body = GeminiModelsResponse))
)]
pub async fn gemini_models(State(state): State<AppState>) -> Json<Value> {
    let mut models = Vec::new();

    // 1. Fetch Gemini models
    let google_cred = crate::oauth::get_google_credential(&state).await;
    if let Some((key, is_oauth)) = google_cred {
        let url = "https://generativelanguage.googleapis.com/v1beta/models";
        if let Ok(parsed) = reqwest::Url::parse(url) && let Ok(res) = crate::oauth::apply_google_auth(state.client.get(parsed), &key, is_oauth).send().await
            && res.status().is_success()
                && let Ok(body) = res.json::<Value>().await
                    && let Some(list) = body["models"].as_array() {
                        models.extend(list.iter().filter_map(|m| {
                            let info: GeminiModelInfo = serde_json::from_value(m.clone()).ok()?;
                            if info.supported_generation_methods.contains(&"generateContent".to_string()) {
                                Some(info)
                            } else {
                                None
                            }
                        }));
                    }
    }

    Json(json!(GeminiModelsResponse { models }))
}

#[utoipa::path(get, path = "/api/system/stats", tag = "system",
    responses((status = 200, description = "System resource usage", body = SystemStats))
)]
pub async fn system_stats(State(state): State<AppState>) -> Json<SystemStats> {
    let snap = state.system_monitor.read().await;
    Json(SystemStats {
        cpu_usage_percent: snap.cpu_usage_percent,
        memory_used_mb: snap.memory_used_mb,
        memory_total_mb: snap.memory_total_mb,
        platform: snap.platform.clone(),
    })
}

// ── POST /api/admin/rotate-key ──────────────────────────────────────────────

/// Hot-reload an API key for a provider without restarting the backend.
/// Protected — requires auth when AUTH_SECRET is set.
pub async fn rotate_key(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let provider = body
        .get("provider")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ApiError::BadRequest("missing 'provider' field".into()))?;
    let key = body
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ApiError::BadRequest("missing 'key' field".into()))?;

    match provider {
        "google" | "anthropic" => {}
        _ => {
            return Err(ApiError::BadRequest(format!(
                "unknown provider '{}' — expected google or anthropic",
                provider
            )));
        }
    }

    let mut rt = state.runtime.write().await;
    rt.api_keys
        .insert(provider.to_string(), key.to_string());
    drop(rt);

    tracing::info!("API key rotated for provider '{}'", provider);

    Ok(Json(json!({
        "ok": true,
        "provider": provider,
        "message": format!("API key for '{}' updated successfully", provider),
    })))
}

#[utoipa::path(post, path = "/api/files/read", tag = "files",
    request_body = FileReadRequest,
    responses((status = 200, description = "File content", body = FileReadResponse))
)]
pub async fn read_file(Json(body): Json<FileReadRequest>) -> Json<Value> {
    match files::read_file_raw(&body.path).await {
        Ok(f) => Json(json!(FileReadResponse { path: f.path, content: f.content, size_bytes: f.size_bytes, truncated: f.truncated, extension: f.extension })),
        Err(e) => Json(json!({ "error": e.reason, "path": e.path })),
    }
}

#[utoipa::path(post, path = "/api/files/list", tag = "files",
    request_body = FileListRequest,
    responses((status = 200, description = "Directory listing", body = FileListResponse))
)]
pub async fn list_files(Json(body): Json<FileListRequest>) -> Json<Value> {
    match files::list_directory(&body.path, body.show_hidden).await {
        Ok(e) => {
            let res: Vec<_> = e.into_iter().map(|i| FileEntryResponse { name: i.name, path: i.path, is_dir: i.is_dir, size_bytes: i.size_bytes, extension: i.extension }).collect();
            Json(json!(FileListResponse { path: body.path, count: res.len(), entries: res }))
        }
        Err(e) => Json(json!({ "error": e.reason, "path": e.path })),
    }
}

// ═══════════════════════════════════════════════════════════════════════
//  Native folder dialog — Jaskier Shared Pattern
// ═══════════════════════════════════════════════════════════════════════

/// Opens a native Windows `FolderBrowserDialog` via PowerShell temp script.
/// Returns the selected path or `{ "cancelled": true }` if user closed the dialog.
pub async fn browse_directory(Json(body): Json<Value>) -> Json<Value> {
    let initial = body
        .get("initial_path")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    // Write a temp .ps1 file — avoids all escaping issues with inline -Command
    let script = format!(
        r#"Add-Type -AssemblyName System.Windows.Forms
[System.Windows.Forms.Application]::EnableVisualStyles()
$owner = New-Object System.Windows.Forms.Form
$owner.TopMost = $true
$owner.ShowInTaskbar = $false
$owner.Size = New-Object System.Drawing.Size(0,0)
$owner.StartPosition = 'Manual'
$owner.Location = New-Object System.Drawing.Point(-9999,-9999)
$owner.Show()
$owner.BringToFront()
$owner.Activate()
$f = New-Object System.Windows.Forms.FolderBrowserDialog
$f.Description = "Select Working Directory"
$f.ShowNewFolderButton = $true
{initial_line}
if ($f.ShowDialog($owner) -eq "OK") {{
    Write-Host $f.SelectedPath
}} else {{
    Write-Host "__CANCELLED__"
}}
$owner.Dispose()
"#,
        initial_line = if initial.is_empty() {
            String::new()
        } else {
            format!(
                "$f.SelectedPath = \"{}\"",
                initial.replace('\\', "\\\\").replace('"', "`\"")
            )
        }
    );

    let tmp = std::env::temp_dir().join(format!("jaskier_browse_{}.ps1", std::process::id()));
    if let Err(e) = tokio::fs::write(&tmp, &script).await {
        return Json(json!({ "error": format!("Cannot write temp script: {}", e) }));
    }

    let result = tokio::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-STA",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
            &tmp.to_string_lossy(),
        ])
        .output()
        .await;

    // Cleanup temp file (best-effort)
    let _ = tokio::fs::remove_file(&tmp).await;

    match result {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if stdout == "__CANCELLED__" || stdout.is_empty() {
                Json(json!({ "cancelled": true }))
            } else {
                Json(json!({ "path": stdout }))
            }
        }
        Err(e) => Json(json!({ "error": format!("Failed to open folder dialog: {}", e) })),
    }
}

/// Tool definitions are static and never change — compute once via OnceLock.
/// Byte-identical tools JSON across all requests enables Gemini implicit caching.
pub(crate) fn build_tools() -> Value {
    static TOOLS: OnceLock<Value> = OnceLock::new();
    TOOLS.get_or_init(|| json!([{
        "function_declarations": [
            {
                "name": "list_directory",
                "description": "List files and subdirectories in a local directory with sizes and line counts. ALWAYS use this to explore project structure — never use execute_command with dir/ls.",
                "parameters": { "type": "object", "properties": { "path": { "type": "string", "description": "Absolute path to the local directory" }, "show_hidden": { "type": "boolean", "description": "Include hidden files (dotfiles)" } }, "required": ["path"] }
            },
            {
                "name": "read_file",
                "description": "Read a file from the local filesystem by its absolute path. ALWAYS use this to inspect code — never use execute_command with cat/type/Get-Content. For large files, use read_file_section instead.",
                "parameters": { "type": "object", "properties": { "path": { "type": "string", "description": "Absolute path to the local file, e.g. C:\\Users\\...\\file.ts" } }, "required": ["path"] }
            },
            {
                "name": "read_file_section",
                "description": "Read specific line range from a file. Use AFTER get_code_structure to read only the functions you need — much cheaper than reading the entire file.",
                "parameters": { "type": "object", "properties": { "path": { "type": "string", "description": "Absolute path to the file" }, "start_line": { "type": "integer", "description": "First line to read (1-indexed, inclusive)" }, "end_line": { "type": "integer", "description": "Last line to read (1-indexed, inclusive). Max range: 500 lines" } }, "required": ["path", "start_line", "end_line"] }
            },
            {
                "name": "search_files",
                "description": "Search for text or regex patterns across all files in a directory (recursive). Returns matching lines with file paths and line numbers. Supports pagination and multiline regex. ALWAYS use this to search for code patterns — never use execute_command with grep/Select-String/findstr.",
                "parameters": { "type": "object", "properties": { "path": { "type": "string", "description": "Directory to search in (absolute path)" }, "pattern": { "type": "string", "description": "Text or regex pattern to search for (case-insensitive)" }, "file_extensions": { "type": "string", "description": "Comma-separated extensions to filter, e.g. 'ts,tsx,rs'. Default: all text files" }, "offset": { "type": "integer", "description": "Number of matches to skip (default 0, for pagination)" }, "limit": { "type": "integer", "description": "Max matches to return (default 80)" }, "multiline": { "type": "boolean", "description": "If true, pattern matches across line boundaries with ±2 lines context (default false)" } }, "required": ["path", "pattern"] }
            },
            {
                "name": "find_file",
                "description": "Find files by name pattern (glob). Returns matching file paths with sizes. Use when you don't know exact file location.",
                "parameters": { "type": "object", "properties": { "path": { "type": "string", "description": "Root directory to search in (absolute path)" }, "pattern": { "type": "string", "description": "Glob pattern like '*.tsx' or 'auth*'" } }, "required": ["path", "pattern"] }
            },
            {
                "name": "get_code_structure",
                "description": "Analyze code structure (functions, classes, structs, traits) via AST without reading full file content. Returns symbol names, types, and line numbers. Supports Rust, TypeScript, JavaScript, Python, Go.",
                "parameters": { "type": "object", "properties": { "path": { "type": "string", "description": "Absolute path to the source file to analyze" } }, "required": ["path"] }
            },
            {
                "name": "write_file",
                "description": "Write or create a file on the local filesystem. Use for creating NEW files or complete rewrites.",
                "parameters": { "type": "object", "properties": { "path": { "type": "string", "description": "Absolute path for the file to write" }, "content": { "type": "string", "description": "Full file content to write" } }, "required": ["path", "content"] }
            },
            {
                "name": "edit_file",
                "description": "Edit an existing file by replacing a specific text section. SAFER than write_file — only changes the targeted section. CRITICAL: old_text must be COPIED VERBATIM from read_file output — every character, space, tab, and newline must match EXACTLY. Even one different space or missing newline causes failure. Use read_file_section first to get the exact text, then copy it character-for-character into old_text.",
                "parameters": { "type": "object", "properties": { "path": { "type": "string", "description": "Absolute path to the file to edit" }, "old_text": { "type": "string", "description": "Text to find and replace — must be COPIED VERBATIM from the file (exact whitespace, exact newlines). Keep it short (3-10 lines) to minimize mismatch risk. Must appear exactly once in the file." }, "new_text": { "type": "string", "description": "Replacement text — same indentation style as the original" } }, "required": ["path", "old_text", "new_text"] }
            },
            {
                "name": "diff_files",
                "description": "Compare two files and show line-by-line differences in unified diff format. Max 200 diff lines output.",
                "parameters": { "type": "object", "properties": { "path_a": { "type": "string", "description": "Absolute path to the first file" }, "path_b": { "type": "string", "description": "Absolute path to the second file" } }, "required": ["path_a", "path_b"] }
            },
            {
                "name": "call_agent",
                "description": "Delegate a subtask to another Witcher agent via A2A protocol. The target agent has full tool access and can read files, search code, etc. Use when the task requires specialized expertise (e.g., code analysis → Eskel, debugging → Lambert, data → Triss). Returns the agent's complete response. Max 3 delegation levels.",
                "parameters": { "type": "object", "properties": { "agent_id": { "type": "string", "description": "Target agent ID (e.g., 'eskel', 'lambert', 'triss', 'yennefer')" }, "task": { "type": "string", "description": "The subtask to delegate. Be specific about what you need and provide context." } }, "required": ["agent_id", "task"] }
            },
            {
                "name": "execute_command",
                "description": "Execute a shell command on the local Windows machine. ONLY use for build/test/git/npm/cargo CLI operations. NEVER use for file reading (use read_file), directory listing (use list_directory), or text search (use search_files). ALWAYS set working_directory when running project commands (cargo, npm, git).",
                "parameters": { "type": "object", "properties": { "command": { "type": "string", "description": "Shell command to execute (Windows cmd.exe). Do NOT include 'cd' — use working_directory instead." }, "working_directory": { "type": "string", "description": "Absolute path to set as the working directory before executing the command. REQUIRED for cargo/npm/git commands. Example: C:\\Users\\BIURODOM\\Desktop\\GeminiHydra-v15\\backend" } }, "required": ["command"] }
            }
        ]
    }])).clone()
}

// ---------------------------------------------------------------------------
// HTTP Execute (Legacy)
// ---------------------------------------------------------------------------

#[utoipa::path(post, path = "/api/execute", tag = "chat",
    request_body = ExecuteRequest,
    responses((status = 200, description = "Execution result", body = ExecuteResponse))
)]
pub async fn execute(State(state): State<AppState>, Json(body): Json<ExecuteRequest>) -> (StatusCode, Json<Value>) {
    if body.prompt.trim().is_empty() {
        return (StatusCode::BAD_REQUEST, Json(json!({ "error": "Prompt cannot be empty" })));
    }
    let start = Instant::now();

    // Translate body.mode into agent_override so the user's explicit choice is respected.
    let mode_override = if !body.mode.is_empty() && body.mode != "auto" {
        let agents = state.agents.read().await;
        agents.iter()
            .find(|a| a.id == body.mode || a.name.to_lowercase() == body.mode.to_lowercase())
            .map(|a| (a.id.clone(), 0.99_f64, "User explicitly selected agent via mode field".to_string()))
    } else {
        None
    };
    let ctx = prepare_execution(&state, &body.prompt, body.model.clone(), mode_override).await;
    if ctx.api_key.is_empty() { return (StatusCode::UNAUTHORIZED, Json(json!({ "error": "No API Key" }))); }

    // Circuit breaker — fail fast if the Gemini provider is tripped.
    if let Err(msg) = state.gemini_circuit.check().await {
        tracing::warn!("execute: {}", msg);
        return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({ "error": msg })));
    }

    let url = format!("https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent", ctx.model);
    let parsed_url = match reqwest::Url::parse(&url) {
        Ok(u) if u.scheme() == "https" => u,
        _ => return (StatusCode::BAD_REQUEST, Json(json!({ "error": "API credentials require HTTPS" }))),
    };
    let mut gen_config_exec = json!({
        "temperature": ctx.temperature,
        "topP": ctx.top_p,
        "maxOutputTokens": ctx.max_tokens
    });
    if let Some(tc) = build_thinking_config(&ctx.model, &ctx.thinking_level) {
        gen_config_exec["thinkingConfig"] = tc;
    }
    let gem_body = json!({
        "systemInstruction": { "parts": [{ "text": ctx.system_prompt }] },
        "contents": [{ "parts": [{ "text": ctx.final_user_prompt }] }],
        "generationConfig": gen_config_exec
    });

    // Helper to extract text from a Gemini generateContent response.
    let extract_text = |j: &Value| -> Option<String> {
        j.get("candidates")
            .and_then(|c| c.get(0))
            .and_then(|c0| c0.get("content"))
            .and_then(|ct| ct.get("parts"))
            .and_then(|p| p.get(0))
            .and_then(|p0| p0.get("text"))
            .and_then(|t| t.as_str())
            .map(|s| s.to_string())
    };

    let is_malformed = |j: &Value| -> bool {
        j.get("candidates")
            .and_then(|c| c.get(0))
            .and_then(|c0| c0.get("finishReason"))
            .and_then(|v| v.as_str())
            == Some("MALFORMED_FUNCTION_CALL")
    };

    // Use retry-with-backoff; update circuit breaker on outcome.
    let text = match gemini_request_with_retry(&state.client, &parsed_url, &ctx.api_key, ctx.is_oauth, &gem_body).await {
        Ok(r) => {
            state.gemini_circuit.record_success().await;
            let j: Value = r.json().await.unwrap_or_default();
            if let Some(text) = extract_text(&j) {
                text
            } else if is_malformed(&j) {
                // MALFORMED_FUNCTION_CALL: agent system prompt mentions tools but HTTP path
                // doesn't declare them. Retry with explicit "text only" instruction.
                tracing::warn!("execute: MALFORMED_FUNCTION_CALL, retrying without tool references");
                let retry_body = json!({
                    "systemInstruction": { "parts": [{ "text": format!("{}\n\nIMPORTANT: You are running in text-only mode. Do NOT attempt to call any tools or functions. Answer the user's question directly using your knowledge.", ctx.system_prompt) }] },
                    "contents": [{ "parts": [{ "text": ctx.final_user_prompt }] }],
                    "generationConfig": gen_config_exec
                });
                match gemini_request_with_retry(&state.client, &parsed_url, &ctx.api_key, ctx.is_oauth, &retry_body).await {
                    Ok(r2) => {
                        let j2: Value = r2.json().await.unwrap_or_default();
                        extract_text(&j2).unwrap_or_else(|| {
                            let diag = gemini_diagnose(&j2);
                            format!("Gemini API returned no text — {}", diag)
                        })
                    }
                    Err(e) => {
                        tracing::error!("execute retry: {}", e);
                        "API Error on retry".to_string()
                    }
                }
            } else {
                let diag = gemini_diagnose(&j);
                tracing::error!("execute: Gemini response missing text ({})", diag);
                format!("Gemini API returned no text — {}", diag)
            }
        }
        Err(e) => {
            state.gemini_circuit.record_failure().await;
            tracing::error!("execute: {}", e);
            "API Error".to_string()
        }
    };

    (StatusCode::OK, Json(json!(ExecuteResponse {
        id: Uuid::new_v4().to_string(),
        result: text,
        plan: Some(ExecutePlan { agent: Some(ctx.agent_id), steps: ctx.steps, estimated_time: None }),
        duration_ms: start.elapsed().as_millis() as u64,
        mode: body.mode,
        files_loaded: ctx.files_loaded,
    })))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal set of test agents with keywords matching the DB seed.
    fn test_agents() -> Vec<WitcherAgent> {
        vec![
            WitcherAgent {
                id: "yennefer".to_string(),
                name: "Yennefer".to_string(),
                role: "Architecture".to_string(),
                tier: "Commander".to_string(),
                status: "active".to_string(),
                description: "Architecture".to_string(),
                system_prompt: None,
                keywords: vec![
                    "architecture".to_string(),
                    "design".to_string(),
                    "pattern".to_string(),
                    "structur".to_string(),
                    "refactor".to_string(),
                ],
                temperature: None,
                model_override: None,
            },
            WitcherAgent {
                id: "triss".to_string(),
                name: "Triss".to_string(),
                role: "Data".to_string(),
                tier: "Coordinator".to_string(),
                status: "active".to_string(),
                description: "Data".to_string(),
                system_prompt: None,
                keywords: vec![
                    "data".to_string(),
                    "analytic".to_string(),
                    "database".to_string(),
                    "sql".to_string(),
                    "query".to_string(),
                ],
                temperature: None,
                model_override: None,
            },
            WitcherAgent {
                id: "dijkstra".to_string(),
                name: "Dijkstra".to_string(),
                role: "Strategy".to_string(),
                tier: "Coordinator".to_string(),
                status: "active".to_string(),
                description: "Strategy".to_string(),
                system_prompt: None,
                keywords: vec![
                    "plan".to_string(),
                    "strateg".to_string(),
                    "roadmap".to_string(),
                    "priorit".to_string(),
                ],
                temperature: None,
                model_override: None,
            },
            WitcherAgent {
                id: "eskel".to_string(),
                name: "Eskel".to_string(),
                role: "Backend & APIs".to_string(),
                tier: "Coordinator".to_string(),
                status: "active".to_string(),
                description: "Backend & APIs".to_string(),
                system_prompt: None,
                keywords: vec![
                    "backend".to_string(),
                    "endpoint".to_string(),
                    "rest".to_string(),
                    "api".to_string(),
                    "handler".to_string(),
                    "middleware".to_string(),
                    "route".to_string(),
                    "websocket".to_string(),
                ],
                temperature: None,
                model_override: None,
            },
        ]
    }

    #[test]
    fn test_refactor_routes_to_yennefer() {
        let agents = test_agents();
        // "refactor this code" contains the keyword "refactor" (>= 4 chars → substring match)
        let (agent, confidence, _) = classify_prompt("refactor this code please", &agents);
        assert_eq!(agent, "yennefer");
        assert!(confidence >= 0.8);
    }

    #[test]
    fn test_sql_routes_to_triss() {
        let agents = test_agents();
        let (agent, confidence, _) = classify_prompt("query sql database", &agents);
        assert_eq!(agent, "triss");
        assert!(confidence >= 0.8);
    }

    #[test]
    fn test_unknown_prompt_falls_back_to_eskel() {
        let agents = test_agents();
        let (agent, _, _) = classify_prompt("what is the meaning of life", &agents);
        assert_eq!(agent, "eskel");
    }

    #[test]
    fn test_backend_routes_to_eskel() {
        let agents = test_agents();
        let (agent, confidence, _) = classify_prompt("add a new api endpoint for user registration", &agents);
        assert_eq!(agent, "eskel");
        assert!(confidence >= 0.7);
    }

    #[test]
    fn test_classify_agent_score_returns_zero_for_no_match() {
        let agents = test_agents();
        let score = classify_agent_score("nothing relevant here", &agents[0]);
        assert_eq!(score, 0.0);
    }

    #[test]
    fn test_classify_agent_score_positive_for_match() {
        let agents = test_agents();
        let triss = &agents[1]; // triss has "sql", "database" etc.
        let score = classify_agent_score("query sql database migration", triss);
        assert!(score > 0.65);
    }

    #[test]
    fn test_short_keyword_whole_word() {
        assert!(keyword_match("query sql database", "sql"));
        assert!(!keyword_match("results-only", "sql"));
    }

    #[test]
    fn test_strip_diacritics_works() {
        assert_eq!(strip_diacritics("refaktoryzację"), "refaktoryzacje");
        assert_eq!(strip_diacritics("żółw"), "zolw");
    }
}
