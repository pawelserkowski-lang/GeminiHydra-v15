use std::collections::HashMap;
use std::time::Instant;

use axum::extract::ws::{Message as WsMessage, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;
use futures_util::{SinkExt, StreamExt};
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
        };

        // Log full detail server-side
        tracing::error!("API error ({}): {}", status.as_u16(), self);

        // Return sanitised message to client — never leak internal details
        let message = match &self {
            ApiError::BadRequest(m) => m.clone(),
            ApiError::NotFound(_) => "Resource not found".to_string(),
            ApiError::Upstream(_) => "Upstream service error".to_string(),
            ApiError::Internal(_) => "Internal server error".to_string(),
            ApiError::Unauthorized(m) => m.clone(),
            ApiError::Unavailable(m) => m.clone(),
        };

        let body = json!({ "error": message });
        (status, Json(body)).into_response()
    }
}

// ---------------------------------------------------------------------------
// Helpers & Routing Logic
// ---------------------------------------------------------------------------

fn build_providers(api_keys: &HashMap<String, String>, cached_google: &[crate::model_registry::ModelInfo]) -> Vec<ProviderInfo> {
    let google_key = api_keys.get("google");
    let anthropic_key = api_keys.get("anthropic");
    let google_available = google_key.is_some() && !google_key.unwrap().is_empty();

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
        available: anthropic_key.is_some() && !anthropic_key.unwrap().is_empty(),
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

/// Expert agent classification based on prompt analysis and agent keywords.
fn classify_prompt(prompt: &str, agents: &[WitcherAgent]) -> (String, f64, String) {
    let lower = strip_diacritics(&prompt.to_lowercase());

    for agent in agents {
        for keyword in &agent.keywords {
            if keyword_match(&lower, keyword) {
                return (
                    agent.id.clone(),
                    0.85,
                    format!("Prompt matches keyword '{}' for agent {}", keyword, agent.name),
                );
            }
        }
    }

    // Default fallback
    ("dijkstra".to_string(), 0.4, "Defaulting to Strategy & Planning".to_string())
}

// ---------------------------------------------------------------------------
// System Prompt Factory
// ---------------------------------------------------------------------------

fn build_system_prompt(agent_id: &str, agents: &[WitcherAgent], language: &str, model: &str) -> String {
    let agent = agents.iter().find(|a| a.id == agent_id).unwrap_or(&agents[0]);

    let roster: String = agents
        .iter()
        .map(|a| format!("  - {} ({}) — {}", a.name, a.role, a.description))
        .collect::<Vec<_>>()
        .join("\n");

    let custom = agent.system_prompt.as_deref().unwrap_or("");
    let base_prompt = format!(
        r#"## CRITICAL: Local Machine Access
You are running on the user's LOCAL machine with FULL filesystem access.
You CAN and MUST read, write, and browse local files directly using your tools.
NEVER say "I don't have access to your files" or "I can't read local files" — YOU CAN.
When the user provides a file path or directory path, USE your tools to access it immediately.

## CRITICAL: Action-First Protocol
1. **NEVER suggest commands** — EXECUTE them with `execute_command`.
2. **NEVER ask the user to paste code** — use `read_file` to read it yourself.
3. **Directory detected?** — Call `list_directory` IMMEDIATELY, then explore.
4. **Refactoring**: `list_directory` → `read_file` → analyze → `write_file` → verify.
5. **Act first, explain after.**
6. **Chain up to 10 tool calls per turn.**

## CRITICAL: Tool Selection Rules
- **To list files/directories** → ALWAYS use `list_directory`, NEVER `execute_command` with ls/dir.
- **To read a file** → ALWAYS use `read_file`, NEVER `execute_command` with cat/type.
- **To write a file** → ALWAYS use `write_file`, NEVER `execute_command` with echo/redirect.
- Use `execute_command` ONLY for: build, test, git, npm, cargo, pip, and other CLI tools.
- This is a **Windows** machine. If you must use `execute_command`, use Windows commands (dir, type, etc.), NOT Unix (ls, cat, cd ~).

## Your Identity
- **Name:** {name} | **Role:** {role} | **Tier:** {tier}
- **AI Model:** You are powered by `{model}`. NEVER claim to use a different model or version.
- {description}
- Part of **GeminiHydra v15 Wolf Swarm**.
- Speak as {name}, but tool usage is priority.

## Language
- Respond in **{language}** unless the user writes in a different language.

## Tools (all operate on the LOCAL filesystem)
- `execute_command` — run shell commands on this machine
- `read_file` — read any local file by absolute path
- `write_file` — write/create local files
- `list_directory` — browse local directories
- `get_code_structure` — analyze code AST without full read

## Swarm Roster
{roster}"#,
        name = agent.name,
        role = agent.role,
        tier = agent.tier,
        model = model,
        description = agent.description,
        language = language,
        roster = roster
    );

    if !custom.is_empty() {
        format!("{}\n\n## Agent-Specific Instructions\n{}", base_prompt, custom)
    } else {
        base_prompt
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
pub async fn list_agents(State(state): State<AppState>) -> Json<Value> {
    let agents = state.agents.read().await;
    Json(json!({ "agents": *agents }))
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
        "INSERT INTO gh_agents (id, name, role, tier, status, description, system_prompt, keywords) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"
    )
    .bind(&agent.id)
    .bind(&agent.name)
    .bind(&agent.role)
    .bind(&agent.tier)
    .bind(&agent.status)
    .bind(&agent.description)
    .bind(&agent.system_prompt)
    .bind(&agent.keywords)
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
        "UPDATE gh_agents SET name=$1, role=$2, tier=$3, status=$4, description=$5, system_prompt=$6, keywords=$7, updated_at=NOW() \
         WHERE id=$8"
    )
    .bind(&agent.name)
    .bind(&agent.role)
    .bind(&agent.tier)
    .bind(&agent.status)
    .bind(&agent.description)
    .bind(&agent.system_prompt)
    .bind(&agent.keywords)
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
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Json<Value> {
    let _ = sqlx::query("DELETE FROM gh_agents WHERE id=$1").bind(id).execute(&state.db).await;
    state.refresh_agents().await;
    Json(json!({ "success": true }))
}

// ---------------------------------------------------------------------------
// Execution Context & Helpers
// ---------------------------------------------------------------------------

struct ExecuteContext {
    agent_id: String,
    confidence: f64,
    reasoning: String,
    model: String,
    api_key: String,
    system_prompt: String,
    final_user_prompt: String,
    files_loaded: Vec<String>,
    steps: Vec<String>,
}

async fn prepare_execution(
    state: &AppState,
    prompt: &str,
    model_override: Option<String>,
    agent_override: Option<(String, f64, String)>,
) -> ExecuteContext {
    let agents_lock = state.agents.read().await;
    
    let (agent_id, confidence, reasoning) = agent_override.unwrap_or_else(|| classify_prompt(prompt, &agents_lock));

    let (def_model, lang) = sqlx::query_as::<_, (String, String)>(
        "SELECT default_model, language FROM gh_settings WHERE id = 1",
    )
    .fetch_one(&state.db)
    .await
    .unwrap_or_else(|_| ("gemini-3.1-pro-preview".to_string(), "en".to_string()));

    let model = model_override.unwrap_or(def_model);
    let language = match lang.as_str() { "pl" => "Polish", "en" => "English", other => other };

    let api_key = state.runtime.read().await.api_keys.get("google").cloned().unwrap_or_default();
    let system_prompt = build_system_prompt(&agent_id, &agents_lock, language, &model);

    let detected_paths = files::extract_file_paths(prompt);
    let (file_context, _) = if !detected_paths.is_empty() {
        files::build_file_context(&detected_paths).await
    } else {
        (String::new(), Vec::new())
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

    let final_user_prompt = format!("{}{}{}", file_context, prompt, dir_hint_str);
    let files_loaded = if !file_context.is_empty() { detected_paths } else { Vec::new() };
    
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
        system_prompt,
        final_user_prompt,
        files_loaded,
        steps,
    }
}

// ---------------------------------------------------------------------------
// SSE / WebSocket Streaming Refactored
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
enum SseParsedEvent {
    TextToken(String),
    FunctionCall { name: String, args: Value, raw_part: Value },
}

struct SseParser { buffer: String }

impl SseParser {
    fn new() -> Self { Self { buffer: String::new() } }

    fn parse_parts(json_val: &Value) -> Vec<SseParsedEvent> {
        let mut events = Vec::new();
        if let Some(parts) = json_val["candidates"][0]["content"]["parts"].as_array() {
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
                            WsClientMessage::Execute { prompt, model, session_id, .. } => {
                                execute_streaming(&mut sender, &state, &prompt, model, session_id, cancel.child_token()).await;
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
// Streaming Execution Engine
// ---------------------------------------------------------------------------

async fn execute_streaming(
    sender: &mut futures_util::stream::SplitSink<WebSocket, WsMessage>,
    state: &AppState,
    prompt: &str,
    model_override: Option<String>,
    session_id: Option<String>,
    cancel: CancellationToken,
) {
    let start = Instant::now();
    let sid = session_id.as_deref().and_then(|s| Uuid::parse_str(s).ok());
    
    // Resolve agent (check session lock or classify)
    let agent_info = if let Some(s) = &sid { 
        Some(resolve_session_agent(state, s, prompt).await) 
    } else { 
        None 
    };
    
    let ctx = prepare_execution(state, prompt, model_override, agent_info).await;
    let resp_id = Uuid::new_v4();

    if !ws_send(sender, &WsServerMessage::Start { id: resp_id.to_string(), agent: ctx.agent_id.clone(), model: ctx.model.clone(), files_loaded: ctx.files_loaded.clone() }).await { return; }
    let _ = ws_send(sender, &WsServerMessage::Plan { agent: ctx.agent_id.clone(), confidence: ctx.confidence, steps: ctx.steps.clone() }).await;

    // Dispatch based on model provider
    let full_text = if ctx.model.starts_with("ollama:") {
        execute_streaming_ollama(sender, state, &ctx, sid, cancel).await
    } else {
        execute_streaming_gemini(sender, state, &ctx, sid, cancel).await
    };

    store_messages(&state.db, sid, resp_id, prompt, &full_text, &ctx).await;
    let _ = ws_send(sender, &WsServerMessage::Complete { duration_ms: start.elapsed().as_millis() as u64 }).await;
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

    let url = format!("https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent?alt=sse&key={}", ctx.model, ctx.api_key);
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

    let mut full_text = String::new();
    for iter in 0..10 {
        let body = json!({ "systemInstruction": { "parts": [{ "text": ctx.system_prompt }] }, "contents": contents, "tools": tools });
        let resp = match state.client.post(parsed_url.clone()).json(&body).send().await {
            Ok(r) if r.status().is_success() => r,
            Ok(r) => {
                let err_body = r.text().await.unwrap_or_default();
                tracing::error!("Gemini API error: {}", &err_body[..err_body.len().min(500)]);
                let _ = ws_send(sender, &WsServerMessage::Error { message: "AI service error".into(), code: Some("GEMINI_ERROR".into()) }).await;
                return full_text;
            }
            Err(e) => {
                tracing::error!("Gemini API request failed: {}", e);
                let _ = ws_send(sender, &WsServerMessage::Error { message: "AI service error".into(), code: Some("REQUEST_FAILED".into()) }).await;
                return full_text;
            }
        };

        let (text, fcs, aborted) = consume_gemini_stream(resp, sender, &cancel).await;
        full_text.push_str(&text);
        if aborted || fcs.is_empty() { break; }

        let mut model_parts: Vec<Value> = if !text.is_empty() { vec![json!({ "text": text })] } else { vec![] };
        for (_, _, raw) in &fcs { model_parts.push(raw.clone()); }
        contents.push(json!({ "role": "model", "parts": model_parts }));

        let mut res_parts = Vec::new();
        for (name, args, _) in fcs {
            let _ = ws_send(sender, &WsServerMessage::ToolCall { name: name.clone(), args: args.clone(), iteration: iter + 1 }).await;
            let header = format!("\n\n---\n**🔧 Tool:** `{}`\n", name);
            full_text.push_str(&header);
            let _ = ws_send(sender, &WsServerMessage::Token { content: header }).await;

            let output = crate::tools::execute_tool(&name, &args, state).await.unwrap_or_else(|e| format!("TOOL_ERROR: {}", e));
            let _ = ws_send(sender, &WsServerMessage::ToolResult { name: name.clone(), success: true, summary: output.chars().take(200).collect(), iteration: iter + 1 }).await;

            let res_md = format!("```\n{}\n```\n---\n\n", output);
            full_text.push_str(&res_md);
            let _ = ws_send(sender, &WsServerMessage::Token { content: res_md }).await;
            res_parts.push(json!({ "functionResponse": { "name": name, "response": { "result": output } } }));
        }
        contents.push(json!({ "role": "user", "parts": res_parts }));
    }
    full_text
}

// ── Ollama Implementation ──────────────────────────────────────────────────

async fn execute_streaming_ollama(
    sender: &mut futures_util::stream::SplitSink<WebSocket, WsMessage>,
    state: &AppState,
    ctx: &ExecuteContext,
    sid: Option<Uuid>,
    cancel: CancellationToken,
) -> String {
    let settings = sqlx::query_as::<_, (String,)>("SELECT ollama_url FROM gh_settings WHERE id = 1")
        .fetch_one(&state.db)
        .await
        .unwrap_or(("http://localhost:11434".to_string(),));
    
    let ollama_base = settings.0.trim_end_matches('/');
    let model_name = ctx.model.strip_prefix("ollama:").unwrap_or(&ctx.model);
    let url = format!("{}/api/chat", ollama_base);

    // Load history (map to Ollama format)
    let mut messages = Vec::new();
    if let Some(s) = &sid {
        let history = load_session_history(&state.db, s).await;
        for msg in history {
            if let Some(parts) = msg["parts"].as_array()
                && let Some(text) = parts[0]["text"].as_str() {
                    let role = msg["role"].as_str().unwrap_or("user");
                    messages.push(json!({ "role": if role == "model" { "assistant" } else { "user" }, "content": text }));
                }
        }
    }
    
    // Add system prompt (Ollama supports system message)
    messages.insert(0, json!({ "role": "system", "content": ctx.system_prompt }));
    
    // Add current user prompt
    messages.push(json!({ "role": "user", "content": ctx.final_user_prompt }));

    // Ollama doesn't support tools natively in the same way yet (or experimental), 
    // so we just do a simple chat for now (no tool loops).
    // TODO: Add Ollama tool support if available (function calling).

    let body = json!({
        "model": model_name,
        "messages": messages,
        "stream": true,
    });

    let resp = match state.client.post(&url).json(&body).send().await {
        Ok(r) if r.status().is_success() => r,
        Ok(r) => {
            tracing::error!("Ollama API error: {}", r.status());
            let _ = ws_send(sender, &WsServerMessage::Error { message: "AI service error".into(), code: Some("OLLAMA_ERROR".into()) }).await;
            return String::new();
        }
        Err(e) => {
            tracing::error!("Ollama connection failed: {}", e);
            let _ = ws_send(sender, &WsServerMessage::Error { message: "AI service error".into(), code: Some("CONNECTION_ERROR".into()) }).await;
            return String::new();
        }
    };

    let mut stream = resp.bytes_stream();
    let mut full_text = String::new();

    loop {
        tokio::select! {
            _ = cancel.cancelled() => break,
            chunk = stream.next() => {
                match chunk {
                    Some(Ok(b)) => {
                        // Ollama sends multiple JSON objects in one chunk sometimes, or one per line
                        let s = String::from_utf8_lossy(&b);
                        for line in s.lines() {
                            if let Ok(val) = serde_json::from_str::<Value>(line) {
                                if let Some(content) = val["message"]["content"].as_str() {
                                    full_text.push_str(content);
                                    let _ = ws_send(sender, &WsServerMessage::Token { content: content.to_string() }).await;
                                }
                                if val["done"].as_bool().unwrap_or(false) {
                                    return full_text;
                                }
                            }
                        }
                    }
                    Some(Err(e)) => {
                        tracing::error!("Ollama stream error: {}", e);
                        let _ = ws_send(sender, &WsServerMessage::Error { message: "AI service error".into(), code: Some("STREAM_ERROR".into()) }).await;
                        break;
                    }
                    None => break,
                }
            }
        }
    }

    full_text
}

async fn consume_gemini_stream(
    resp: reqwest::Response,
    sender: &mut futures_util::stream::SplitSink<WebSocket, WsMessage>,
    cancel: &CancellationToken,
) -> (String, Vec<(String, Value, Value)>, bool) {
    let mut parser = SseParser::new();
    let mut stream = resp.bytes_stream();
    let mut full_text = String::new();
    let mut fcs = Vec::new();

    loop {
        tokio::select! {
            _ = cancel.cancelled() => return (full_text, fcs, true),
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
                            }
                        }
                        break;
                    }
                }
            }
        }
    }
    (full_text, fcs, false)
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
    sqlx::query_as::<_, (String, String)>("SELECT role, content FROM gh_chat_messages WHERE session_id = $1 ORDER BY created_at DESC LIMIT 20")
        .bind(sid).fetch_all(db).await.unwrap_or_default().into_iter().rev()
        .map(|(r, c)| json!({ "role": if r == "assistant" { "model" } else { "user" }, "parts": [{ "text": c }] }))
        .collect()
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
    responses((status = 200, description = "Available Gemini and Ollama models", body = GeminiModelsResponse))
)]
pub async fn gemini_models(State(state): State<AppState>) -> Json<Value> {
    let mut models = Vec::new();

    // 1. Fetch Gemini models
    let key = state.runtime.read().await.api_keys.get("google").cloned().unwrap_or_default();
    if !key.is_empty() {
        let url = format!("https://generativelanguage.googleapis.com/v1beta/models?key={}", key);
        if let Ok(parsed) = reqwest::Url::parse(&url) && parsed.scheme() == "https" && let Ok(res) = state.client.get(parsed).send().await
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

    // 2. Fetch Ollama models
    if let Ok((ollama_url,)) = sqlx::query_as::<_, (String,)>("SELECT ollama_url FROM gh_settings WHERE id = 1")
        .fetch_one(&state.db)
        .await
    {
        let url = format!("{}/api/tags", ollama_url.trim_end_matches('/'));
        // Use an async block for the request to simplify error handling logic?
        // Or just keep it flat.
        
        if let Ok(res) = state.client.get(&url).timeout(std::time::Duration::from_secs(2)).send().await
            && res.status().is_success()
                && let Ok(body) = res.json::<Value>().await
                    && let Some(list) = body["models"].as_array() {
                        for m in list {
                            if let Some(name) = m["name"].as_str() {
                                models.push(GeminiModelInfo {
                                    name: format!("ollama:{}", name),
                                    display_name: format!("Ollama: {}", name),
                                    supported_generation_methods: vec!["generateContent".to_string()],
                                });
                            }
                        }
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

fn build_tools() -> Value {
    json!([{
        "function_declarations": [
            { "name": "execute_command", "description": "Execute a shell command on the local machine. Use for git, npm, cargo, and other CLI operations.", "parameters": { "type": "object", "properties": { "command": { "type": "string", "description": "Shell command to execute" } }, "required": ["command"] } },
            { "name": "read_file", "description": "Read a file from the local filesystem by its absolute path. Use this to inspect code, configs, logs, etc.", "parameters": { "type": "object", "properties": { "path": { "type": "string", "description": "Absolute path to the local file, e.g. C:\\Users\\...\\file.ts" } }, "required": ["path"] } },
            { "name": "write_file", "description": "Write or create a file on the local filesystem. Use for code edits, config changes, etc.", "parameters": { "type": "object", "properties": { "path": { "type": "string", "description": "Absolute path for the file to write" }, "content": { "type": "string", "description": "Full file content to write" } }, "required": ["path", "content"] } },
            { "name": "list_directory", "description": "List files and subdirectories in a local directory. Use to explore project structure.", "parameters": { "type": "object", "properties": { "path": { "type": "string", "description": "Absolute path to the local directory" }, "show_hidden": { "type": "boolean", "description": "Include hidden files (dotfiles)" } }, "required": ["path"] } },
            { "name": "get_code_structure", "description": "Analyze code structure (functions, classes, imports) via AST without reading full file content. Supports Rust, TypeScript, JavaScript, Python, Go.", "parameters": { "type": "object", "properties": { "path": { "type": "string", "description": "Absolute path to the source file to analyze" } }, "required": ["path"] } }
        ]
    }])
}

// ---------------------------------------------------------------------------
// HTTP Execute (Legacy)
// ---------------------------------------------------------------------------

#[utoipa::path(post, path = "/api/execute", tag = "chat",
    request_body = ExecuteRequest,
    responses((status = 200, description = "Execution result", body = ExecuteResponse))
)]
pub async fn execute(State(state): State<AppState>, Json(body): Json<ExecuteRequest>) -> Json<Value> {
    let start = Instant::now();
    let ctx = prepare_execution(&state, &body.prompt, body.model.clone(), None).await;
    if ctx.api_key.is_empty() { return Json(json!({ "error": "No API Key" })); }

    let url = format!("https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}", ctx.model, ctx.api_key);
    let parsed_url = match reqwest::Url::parse(&url) {
        Ok(u) if u.scheme() == "https" => u,
        _ => return Json(json!({ "error": "API credentials require HTTPS" })),
    };
    let gem_body = json!({ "systemInstruction": { "parts": [{ "text": ctx.system_prompt }] }, "contents": [{ "parts": [{ "text": ctx.final_user_prompt }] }] });

    let res = state.client.post(parsed_url).json(&gem_body).send().await;
    let text = match res {
        Ok(r) if r.status().is_success() => {
            let j: Value = r.json().await.unwrap_or_default();
            j["candidates"][0]["content"]["parts"][0]["text"].as_str().unwrap_or("Error").to_string()
        }
        _ => "API Error".to_string(),
    };

    Json(json!(ExecuteResponse {
        id: Uuid::new_v4().to_string(),
        result: text,
        plan: Some(ExecutePlan { agent: Some(ctx.agent_id), steps: ctx.steps, estimated_time: None }),
        duration_ms: start.elapsed().as_millis() as u64,
        mode: body.mode,
        files_loaded: ctx.files_loaded,
    }))
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
    fn test_unknown_prompt_falls_back_to_dijkstra() {
        let agents = test_agents();
        let (agent, _, _) = classify_prompt("what is the meaning of life", &agents);
        assert_eq!(agent, "dijkstra");
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
