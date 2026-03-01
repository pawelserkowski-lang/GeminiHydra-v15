// ---------------------------------------------------------------------------
// handlers/execute.rs — Legacy HTTP execute + internal tool bridge
// ---------------------------------------------------------------------------

use std::time::Instant;

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::models::{ExecutePlan, ExecuteRequest, ExecuteResponse};
use crate::state::AppState;

use super::{build_thinking_config, gemini_diagnose, prepare_execution, ApiError};

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

// ---------------------------------------------------------------------------
// HTTP Execute (Legacy)
// ---------------------------------------------------------------------------

/// Gemini retry helper — reuses the same backoff logic as streaming.
/// This is a simplified version for the non-streaming execute endpoint.
async fn gemini_request_simple(
    client: &reqwest::Client,
    url: &reqwest::Url,
    api_key: &str,
    is_oauth: bool,
    body: &Value,
) -> Result<reqwest::Response, String> {
    let result = crate::oauth::apply_google_auth(
            client.post(url.clone()), api_key, is_oauth,
        )
        .json(body)
        .timeout(std::time::Duration::from_secs(300))
        .send()
        .await;

    match result {
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
    }
}

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
    let ctx = prepare_execution(&state, &body.prompt, body.model.clone(), mode_override, "").await;
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
    let text = match gemini_request_simple(&state.client, &parsed_url, &ctx.api_key, ctx.is_oauth, &gem_body).await {
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
                match gemini_request_simple(&state.client, &parsed_url, &ctx.api_key, ctx.is_oauth, &retry_body).await {
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
