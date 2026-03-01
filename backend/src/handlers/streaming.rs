// ---------------------------------------------------------------------------
// handlers/streaming.rs — WebSocket streaming, Gemini SSE parsing, ADK proxy
// ---------------------------------------------------------------------------

use std::collections::HashMap;
use std::time::{Duration, Instant};

use axum::extract::ws::{Message as WsMessage, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::IntoResponse;
use futures_util::{SinkExt, StreamExt};
use rand::Rng;
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::models::{WsClientMessage, WsServerMessage};
use crate::state::AppState;

use super::{build_thinking_config, build_tools, prepare_execution, ExecuteContext};

// ---------------------------------------------------------------------------
// SSE Parser
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

// ---------------------------------------------------------------------------
// Truncation & Constants
// ---------------------------------------------------------------------------

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
// WebSocket Helper
// ---------------------------------------------------------------------------

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

    // Fetch session WD before prepare_execution so cache key includes correct WD
    let session_wd: String = if let Some(ref s) = sid {
        sqlx::query_scalar("SELECT working_directory FROM gh_sessions WHERE id = $1")
            .bind(s)
            .fetch_optional(&state.db)
            .await
            .ok()
            .flatten()
            .unwrap_or_default()
    } else {
        String::new()
    };

    let ctx = prepare_execution(state, prompt, model_override, agent_info, &session_wd).await;
    let resp_id = Uuid::new_v4();

    if !ws_send(sender, &WsServerMessage::Start { id: resp_id.to_string(), agent: ctx.agent_id.clone(), model: ctx.model.clone(), files_loaded: ctx.files_loaded.clone() }).await { return; }
    let _ = ws_send(sender, &WsServerMessage::Plan { agent: ctx.agent_id.clone(), confidence: ctx.confidence, steps: ctx.steps.clone(), reasoning: ctx.reasoning.clone() }).await;

    // Dispatch to Gemini streaming (with fallback to flash on failure)
    let full_text = execute_streaming_gemini(sender, state, &ctx, sid, cancel.clone()).await;
    let (full_text, used_model) = if full_text.is_empty() && !ctx.model.contains("flash") {
        let flash_model = crate::model_registry::get_model_id(state, "flash").await;
        tracing::warn!("Model fallback: {} failed, retrying with {}", ctx.model, flash_model);
        let mut fallback_ctx = ctx.clone();
        fallback_ctx.model = flash_model;
        let fb_text = execute_streaming_gemini(sender, state, &fallback_ctx, sid, cancel).await;
        (fb_text, fallback_ctx.model)
    } else {
        (full_text, ctx.model.clone())
    };

    store_messages(&state.db, sid, resp_id, prompt, &full_text, &ctx).await;

    // Token usage tracking — fire-and-forget INSERT
    let latency = start.elapsed().as_millis() as i32;
    let success = !full_text.is_empty();
    let input_est = (prompt.len() / 4) as i32;
    let output_est = (full_text.len() / 4) as i32;
    let db = state.db.clone();
    let agent_id = ctx.agent_id.clone();
    let model = used_model;
    tokio::spawn(async move {
        let _ = sqlx::query(
            "INSERT INTO gh_agent_usage (agent_id, model, input_tokens, output_tokens, total_tokens, latency_ms, success, tier) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
        )
        .bind(&agent_id)
        .bind(&model)
        .bind(input_est)
        .bind(output_est)
        .bind(input_est + output_est)
        .bind(latency)
        .bind(success)
        .bind(if model.contains("flash") { "flash" } else if model.contains("thinking") { "thinking" } else { "chat" })
        .execute(&db)
        .await;
    });

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
    let tools = build_tools(&state);
    let mut contents = if let Some(s) = &sid { load_session_history(&state.db, s).await } else { Vec::new() };
    contents.push(json!({ "role": "user", "parts": [{ "text": ctx.final_user_prompt }] }));

    // #36 — Dynamic max iterations based on prompt complexity, capped by #49 user setting
    // NOTE: Short prompt ≠ simple task. "refaktoruj cały kod" is 20 chars but extremely complex.
    // We use generous minimums and let the user setting be the real cap.
    let prompt_len = ctx.final_user_prompt.len();
    let file_count = ctx.files_loaded.len();
    let dynamic_max: usize = if prompt_len < 200 && file_count <= 1 {
        15 // Short prompt — still may be complex (e.g. "refactor all code")
    } else if prompt_len < 1000 && file_count <= 3 {
        20 // Medium complexity
    } else {
        25 // Complex multi-file analysis
    };
    let max_iterations: usize = dynamic_max.min(ctx.max_iterations.max(1) as usize);

    let mut full_text = String::new();
    let mut has_written_file = false;
    let mut agent_text_len: usize = 0;

    // #39 — Global execution timeout: 5 minutes (relaxed for complex multi-step tasks)
    let execution_start = Instant::now();
    let execution_timeout = Duration::from_secs(300);

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
        if iter >= 8 && text.trim().is_empty() {
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

        // #34 — Iteration reminders (relaxed: let agent work autonomously for longer)
        if iter >= 3 {
            let write_nudge = if has_written_file {
                "You have applied edits. Continue with remaining tasks or summarize when done."
            } else {
                "Reminder: if you found issues, use edit_file/write_file to apply fixes."
            };
            let urgency = if iter >= 12 {
                format!("[SYSTEM: Approaching iteration limit ({}/{}). {} {} Wrap up your remaining work.]", iter + 1, max_iterations, context_hint, write_nudge)
            } else if iter >= 8 {
                format!("[SYSTEM: {} {} Consider applying edits if you have enough information.]", context_hint, write_nudge)
            } else {
                format!("[SYSTEM: {} Continue working. {} iterations remaining.]", context_hint, max_iterations - iter - 1)
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
    let (aid, conf, reas) = super::classify_prompt(prompt, &agents);

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
