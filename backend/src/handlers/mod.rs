// ---------------------------------------------------------------------------
// handlers/ — split from monolithic handlers.rs
// Sub-modules for logical grouping; mod.rs re-exports all public items
// so that `crate::handlers::*` paths remain unchanged.
// ---------------------------------------------------------------------------

// Sub-modules are pub(crate) so utoipa __path_* types are accessible from lib.rs OpenApi derive.
pub(crate) mod agents;
pub(crate) mod execute;
pub(crate) mod files_handlers;
pub(crate) mod streaming;
pub(crate) mod system;
#[cfg(test)]
mod tests;

// ── Re-exports (backward-compatible — lib.rs routes unchanged) ───────────────

// System / health / models
pub use system::{
    auth_mode, gemini_models, health, health_detailed, readiness, rotate_key, system_stats,
};

// Agents CRUD + classification
pub use agents::{classify_agent, create_agent, delete_agent, list_agents, update_agent};

// Files
pub use files_handlers::{browse_directory, list_files, read_file};

// Execute (legacy HTTP + internal tool bridge)
pub use execute::{execute, internal_tool_execute};

// WebSocket
pub use streaming::ws_execute;

// ── utoipa __path_* re-exports ───────────────────────────────────────────────
// The #[utoipa::path] attribute macro generates private structs like __path_health.
// The OpenApi derive in lib.rs expects them at `handlers::__path_health`, so we
// re-export them here.
pub use system::{
    __path_auth_mode, __path_gemini_models, __path_health, __path_health_detailed,
    __path_readiness, __path_system_stats,
};
pub use agents::{
    __path_classify_agent, __path_create_agent, __path_delete_agent, __path_list_agents,
    __path_update_agent,
};
pub use files_handlers::{__path_list_files, __path_read_file};
pub use execute::__path_execute;

// ── Shared types ─────────────────────────────────────────────────────────────

use std::collections::HashMap;

use axum::http::StatusCode;
use axum::Json;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::models::{ProviderInfo, WitcherAgent};
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

    /// HTTP status code for each variant.
    fn status_code(&self) -> StatusCode {
        match self {
            ApiError::BadRequest(_) => StatusCode::BAD_REQUEST,
            ApiError::NotFound(_) => StatusCode::NOT_FOUND,
            ApiError::Upstream(_) => StatusCode::BAD_GATEWAY,
            ApiError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            ApiError::Unavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
            ApiError::ToolTimeout(_) => StatusCode::GATEWAY_TIMEOUT,
            ApiError::RateLimited(_) => StatusCode::TOO_MANY_REQUESTS,
        }
    }

    /// Sanitized message safe to return to clients — never leaks internal details.
    /// Some variants (NotFound, Upstream, Internal) return generic messages;
    /// others pass through the original message.
    fn sanitized_message(&self) -> String {
        match self {
            ApiError::BadRequest(m) => m.clone(),
            ApiError::NotFound(_) => "Resource not found".to_string(),
            ApiError::Upstream(_) => "Upstream service error".to_string(),
            ApiError::Internal(_) => "Internal server error".to_string(),
            ApiError::Unauthorized(m) => m.clone(),
            ApiError::Unavailable(m) => m.clone(),
            ApiError::ToolTimeout(m) => m.clone(),
            ApiError::RateLimited(m) => m.clone(),
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
        let status = self.status_code();
        let request_id = Self::current_request_id();

        // Log full detail server-side (with request_id for correlation)
        tracing::error!(
            request_id = %request_id,
            code = self.error_code(),
            "API error ({}): {}",
            status.as_u16(),
            self
        );

        let body = json!({
            "error": {
                "code": self.error_code(),
                "message": self.sanitized_message(),
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
        let status = self.error.status_code();
        let request_id = ApiError::current_request_id();

        tracing::error!(
            request_id = %request_id,
            code = self.error.error_code(),
            "API error ({}): {}",
            status.as_u16(),
            self.error
        );

        let body = json!({
            "error": {
                "code": self.error.error_code(),
                "message": self.error.sanitized_message(),
                "request_id": request_id,
                "details": self.details,
            }
        });
        (status, Json(body)).into_response()
    }
}

// ---------------------------------------------------------------------------
// Helpers & Routing Logic
// ---------------------------------------------------------------------------

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

pub(crate) fn build_providers(api_keys: &HashMap<String, String>, cached_google: &[crate::model_registry::ModelInfo]) -> Vec<ProviderInfo> {
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

pub(crate) fn strip_diacritics(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'ą' => 'a', 'ć' => 'c', 'ę' => 'e', 'ł' => 'l',
            'ń' => 'n', 'ó' => 'o', 'ś' => 's', 'ź' | 'ż' => 'z',
            _ => c,
        })
        .collect()
}

pub(crate) fn keyword_match(text: &str, keyword: &str) -> bool {
    if keyword.len() >= 4 {
        text.contains(keyword)
    } else {
        text.split(|c: char| !c.is_alphanumeric())
            .any(|word| word == keyword)
    }
}

/// Compute the raw keyword confidence score for a single agent against a prompt.
pub(crate) fn classify_agent_score(lower_prompt: &str, agent: &WitcherAgent) -> f64 {
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
pub(crate) fn classify_prompt(prompt: &str, agents: &[WitcherAgent]) -> (String, f64, String) {
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
pub(crate) async fn classify_with_gemini(
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
- **PROPOSE NEXT TASKS.** At the END of every completed task, add a markdown heading **Co dalej?** with exactly 5 numbered follow-up tasks the user could ask you to do next. Make them specific, actionable, and relevant to the work just completed. Example: if you fixed a bug, suggest writing tests, checking similar patterns, refactoring related code, updating docs, or running a full audit. Format each as a one-line imperative sentence.
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
// Execution Context & Helpers
// ---------------------------------------------------------------------------

/// Context window token budget per model tier.
pub(crate) fn tier_token_budget(model: &str) -> i32 {
    let lower = model.to_lowercase();
    if lower.contains("flash") { 8192 }
    else if lower.contains("pro") { 65536 }
    else { 32768 }
}

/// Whether an HTTP status code is retryable (transient failure).
#[allow(dead_code)]
pub(crate) fn is_retryable_status(code: u16) -> bool {
    matches!(code, 429 | 502 | 503)
}

#[derive(Clone)]
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
    session_wd: &str,
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
        // #28 — If keyword confidence is low, try Gemini Flash as fallback (with timeout)
        if kw_conf < 0.65 {
            let gemini_result = tokio::time::timeout(
                std::time::Duration::from_secs(8),
                async {
                    let classify_cred = crate::oauth::get_google_credential(&state).await;
                    if let Some((classify_key, classify_is_oauth)) = classify_cred {
                        classify_with_gemini(&state.client, &classify_key, classify_is_oauth, &prompt_clean, &agents_lock).await
                    } else {
                        None
                    }
                },
            )
            .await;
            match gemini_result {
                Ok(Some(result)) => {
                    tracing::info!("classify: Gemini Flash override — {} (keyword was {} @ {:.0}%)", result.0, kw_agent, kw_conf * 100.0);
                    result
                }
                Ok(None) => (kw_agent, kw_conf, kw_reason),
                Err(_) => {
                    tracing::warn!("classify: Gemini Flash classification timed out after 8s, using keyword result");
                    (kw_agent, kw_conf, kw_reason)
                }
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

    let (def_model, lang, temperature, max_tokens, top_p, response_style, max_iterations, thinking_level, settings_wd) =
        sqlx::query_as::<_, (String, String, f64, i32, f64, String, i32, String, String)>(
            "SELECT default_model, language, temperature, max_tokens, top_p, response_style, max_iterations, thinking_level, working_directory \
             FROM gh_settings WHERE id = 1",
        )
        .fetch_one(&state.db)
        .await
        .unwrap_or_else(|_| (
            "gemini-3.1-pro-preview-customtools".to_string(), "en".to_string(), 1.0, 65536, 0.95, "balanced".to_string(), 10, "medium".to_string(), String::new()
        ));

    // Session WD takes priority over global settings WD
    let working_directory = if !session_wd.is_empty() { session_wd.to_string() } else { settings_wd };

    // #48 — Per-agent temperature override
    let matched_agent = agents_lock.iter().find(|a| a.id == agent_id);
    let agent_temp = matched_agent.and_then(|a| a.temperature);
    let effective_temperature = agent_temp.unwrap_or(temperature);

    // Per-agent thinking level override (NULL = use global setting)
    let agent_thinking = matched_agent.and_then(|a| a.thinking_level.clone());
    let effective_thinking = agent_thinking.unwrap_or(thinking_level);

    // Model priority: 1) user request override → 2) per-agent DB override → 3) auto-tier → 4) global default
    let agent_model = matched_agent.and_then(|a| a.model_override.clone());
    let model = if let Some(ov) = model_override {
        ov
    } else if let Some(am) = agent_model {
        am
    } else {
        // Auto-tier routing based on prompt complexity
        let complexity = crate::model_registry::classify_complexity(prompt);
        match complexity {
            "simple" => crate::model_registry::get_model_id(state, "flash").await,
            "complex" => crate::model_registry::get_model_id(state, "thinking").await,
            _ => def_model,
        }
    };

    // A/B testing: per-agent model_b with ab_split probability
    let model = if let Some(agent) = matched_agent {
        if let (Some(model_b), Some(split)) = (&agent.model_b, agent.ab_split) {
            if rand::random::<f64>() < split {
                tracing::info!("A/B test: agent {} using model_b={} (split={:.0}%)", agent.id, model_b, split * 100.0);
                model_b.clone()
            } else { model }
        } else { model }
    } else { model };

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

    let detected_paths = crate::files::extract_file_paths(&prompt_clean);

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
        crate::files::build_file_context(&sorted_paths).await
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
        model: model.clone(),
        max_tokens: max_tokens.min(tier_token_budget(&model)),
        api_key,
        is_oauth,
        system_prompt,
        final_user_prompt,
        files_loaded,
        steps,
        temperature: effective_temperature,
        top_p,
        response_style,
        max_iterations,
        thinking_level: effective_thinking,
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
// Tool Definitions
// ---------------------------------------------------------------------------

/// Tool definitions are static and never change — compute once via AppState OnceLock.
/// Byte-identical tools JSON across all requests enables Gemini implicit caching.
pub(crate) fn build_tools(state: &crate::state::AppState) -> Value {
    state.tool_defs_cache.get_or_init(|| json!([{
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
                "name": "read_pdf",
                "description": "Extract text from a PDF file. Uses pdf-extract for embedded text; falls back to Gemini Vision OCR for scanned/image-based PDFs. Supports page range filtering.",
                "parameters": { "type": "object", "properties": { "path": { "type": "string", "description": "Absolute path to the PDF file" }, "page_range": { "type": "string", "description": "Optional page range like '1-5' or '3' (1-indexed)" } }, "required": ["path"] }
            },
            {
                "name": "analyze_image",
                "description": "Analyze an image file using Gemini Vision API. Describes contents, text, objects, colors, and notable features. Set extract_text=true to perform OCR (extract text from the image). Supports PNG, JPEG, WebP, GIF (max 10 MB).",
                "parameters": { "type": "object", "properties": { "path": { "type": "string", "description": "Absolute path to the image file" }, "prompt": { "type": "string", "description": "Optional custom analysis prompt" }, "extract_text": { "type": "boolean", "description": "When true, extract text (OCR) from the image instead of describing it" } }, "required": ["path"] }
            },
            {
                "name": "ocr_document",
                "description": "Extract text from an image or PDF using Gemini Vision OCR. Returns text with preserved formatting: tables as markdown (| pipes + --- separators), headers, lists, paragraphs. Ideal for invoices, reports, forms, tables, receipts, scanned documents. The extracted text can be copied with rich formatting (pastes as real tables in Word/Excel). Supports PNG, JPEG, WebP, GIF, PDF (max 22 MB).",
                "parameters": { "type": "object", "properties": { "path": { "type": "string", "description": "Absolute path to the image or PDF file" }, "prompt": { "type": "string", "description": "Optional custom OCR prompt (default extracts all text preserving tables and formatting)" } }, "required": ["path"] }
            },
            {
                "name": "fetch_webpage",
                "description": "Fetch a web page with SSRF protection, extract readable text (HTML tables→markdown, code→fenced blocks, inline links preserved), metadata (OpenGraph, JSON-LD, language), and categorized links (internal/external/resource). Supports retry with backoff, content deduplication, custom headers, and JSON output format.",
                "parameters": { "type": "object", "properties": {
                    "url": { "type": "string", "description": "Full URL to fetch (http/https). Private IPs and localhost are blocked." },
                    "extract_links": { "type": "boolean", "description": "Extract and categorize all links as internal/external/resource (default: true)" },
                    "extract_metadata": { "type": "boolean", "description": "Extract OpenGraph, JSON-LD, canonical URL, language (default: false)" },
                    "include_images": { "type": "boolean", "description": "Include image alt text as ![alt](src) in output (default: false)" },
                    "output_format": { "type": "string", "description": "Output format: 'text' (markdown) or 'json' (structured). Default: 'text'" },
                    "max_text_length": { "type": "integer", "description": "Max characters of page text to return. 0 = unlimited (default: 0)" },
                    "headers": { "type": "object", "description": "Custom HTTP headers as key-value pairs" }
                }, "required": ["url"] }
            },
            {
                "name": "crawl_website",
                "description": "Crawl a website with robots.txt compliance, optional sitemap seeding, concurrent requests, SSRF protection, and content deduplication. Extracts text from each page (tables→markdown, code→fenced) and builds categorized link index. Supports path prefix filtering, exclude patterns, and configurable rate limiting.",
                "parameters": { "type": "object", "properties": {
                    "url": { "type": "string", "description": "Starting URL to crawl (http/https)" },
                    "max_depth": { "type": "integer", "description": "Max link depth (default: 1, max: 5)" },
                    "max_pages": { "type": "integer", "description": "Max pages to fetch (default: 10, max: 50)" },
                    "same_domain_only": { "type": "boolean", "description": "Only follow same-domain links (default: true)" },
                    "path_prefix": { "type": "string", "description": "Only crawl URLs whose path starts with this prefix (e.g. '/docs/')" },
                    "exclude_patterns": { "type": "array", "items": { "type": "string" }, "description": "Skip URLs containing any of these substrings" },
                    "respect_robots_txt": { "type": "boolean", "description": "Fetch and respect robots.txt (default: true)" },
                    "use_sitemap": { "type": "boolean", "description": "Seed crawl queue from sitemap.xml (default: false)" },
                    "concurrent_requests": { "type": "integer", "description": "Concurrent fetches (default: 1, max: 5)" },
                    "delay_ms": { "type": "integer", "description": "Delay between requests in ms (default: 300)" },
                    "max_total_seconds": { "type": "integer", "description": "Max total crawl time in seconds (default: 180)" },
                    "output_format": { "type": "string", "description": "Output format: 'text' or 'json' (default: 'text')" },
                    "max_text_length": { "type": "integer", "description": "Max text chars per page excerpt (default: 2000)" },
                    "include_metadata": { "type": "boolean", "description": "Include OpenGraph/JSON-LD metadata per page (default: false)" },
                    "headers": { "type": "object", "description": "Custom HTTP headers as key-value pairs" }
                }, "required": ["url"] }
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
