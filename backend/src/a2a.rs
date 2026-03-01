// A2A v0.3 Protocol — Agent-to-Agent communication
// https://google.github.io/A2A/specification/

use std::convert::Infallible;
use std::time::Duration;

use axum::extract::{Path, State};
use axum::response::sse::{Event, Sse};
use axum::Json;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio_stream::wrappers::ReceiverStream;
use uuid::Uuid;

use crate::state::AppState;

// ---------------------------------------------------------------------------
// A2A v0.3 Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentCard {
    pub name: String,
    pub description: String,
    pub url: String,
    pub version: String,
    pub capabilities: AgentCapabilities,
    pub skills: Vec<AgentSkill>,
    pub default_input_modes: Vec<String>,
    pub default_output_modes: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentCapabilities {
    pub streaming: bool,
    pub push_notifications: bool,
    pub state_transition_history: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct AgentSkill {
    pub id: String,
    pub name: String,
    pub description: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct A2aTask {
    pub id: String,
    pub status: String,
    pub agent_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_task_id: Option<String>,
    pub messages: Vec<A2aMessage>,
    pub artifacts: Vec<A2aArtifact>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2aMessage {
    pub role: String,
    pub parts: Vec<Part>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2aArtifact {
    pub name: Option<String>,
    pub parts: Vec<Part>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Part {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "data")]
    Data { data: Value },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendMessageRequest {
    pub message: A2aMessage,
    #[serde(default)]
    pub agent_id: Option<String>,
}

// ---------------------------------------------------------------------------
// Handler: GET /.well-known/agent-card.json
// ---------------------------------------------------------------------------

pub async fn agent_card(State(state): State<AppState>) -> Json<AgentCard> {
    let agents = state.agents.read().await;
    let skills: Vec<AgentSkill> = agents
        .iter()
        .filter(|a| a.status == "active" || a.status == "online")
        .map(|a| AgentSkill {
            id: a.id.clone(),
            name: a.name.clone(),
            description: format!("{} — {}", a.role, a.description),
            tags: a.keywords.clone(),
        })
        .collect();

    Json(AgentCard {
        name: "GeminiHydra".to_string(),
        description: "Multi-Agent AI Swarm — 12 Witcher agents with filesystem tools, code analysis, and inter-agent delegation".to_string(),
        url: "http://localhost:8081".to_string(),
        version: "15.0.0".to_string(),
        capabilities: AgentCapabilities {
            streaming: true,
            push_notifications: false,
            state_transition_history: true,
        },
        skills,
        default_input_modes: vec!["text/plain".to_string()],
        default_output_modes: vec!["text/plain".to_string()],
    })
}

// ---------------------------------------------------------------------------
// Handler: POST /a2a/message/send
// ---------------------------------------------------------------------------

pub async fn message_send(
    State(state): State<AppState>,
    Json(body): Json<SendMessageRequest>,
) -> Json<Value> {
    let task_id = Uuid::new_v4().to_string();
    let prompt = extract_text_from_parts(&body.message.parts);

    if prompt.is_empty() {
        return Json(json!({ "error": "Message must contain at least one text part" }));
    }

    // Determine target agent
    let agent_override = body.agent_id.map(|id| (id, 0.99_f64, "A2A explicit agent_id".to_string()));

    // Record task as submitted
    if let Err(e) = sqlx::query(
        "INSERT INTO gh_a2a_tasks (id, agent_id, status, prompt) VALUES ($1, $2, 'submitted', $3)",
    )
    .bind(&task_id)
    .bind(agent_override.as_ref().map(|o| o.0.as_str()).unwrap_or("auto"))
    .bind(&prompt)
    .execute(&state.db)
    .await
    {
        tracing::error!("a2a: failed to create task: {}", e);
    }

    // Execute
    match execute_a2a_task(&state, &task_id, &prompt, agent_override, 0).await {
        Ok((agent_id, result)) => {
            // Save messages
            save_message(&state, &task_id, "user", &prompt, None).await;
            save_message(&state, &task_id, "agent", &result, Some(&agent_id)).await;

            let task = build_task_response(&state, &task_id).await;
            Json(json!({ "task": task }))
        }
        Err(e) => {
            let _ = sqlx::query(
                "UPDATE gh_a2a_tasks SET status = 'failed', error_message = $1, updated_at = NOW() WHERE id = $2",
            )
            .bind(&e)
            .bind(&task_id)
            .execute(&state.db)
            .await;

            Json(json!({ "error": e, "task_id": task_id }))
        }
    }
}

// ---------------------------------------------------------------------------
// Handler: POST /a2a/message/stream
// ---------------------------------------------------------------------------

pub async fn message_stream(
    State(state): State<AppState>,
    Json(body): Json<SendMessageRequest>,
) -> Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>> {
    let (tx, rx) = tokio::sync::mpsc::channel::<Event>(64);
    let prompt = extract_text_from_parts(&body.message.parts);
    let agent_override = body.agent_id.map(|id| (id, 0.99_f64, "A2A stream".to_string()));
    let task_id = Uuid::new_v4().to_string();
    let task_id_clone = task_id.clone();

    // Record task
    let _ = sqlx::query(
        "INSERT INTO gh_a2a_tasks (id, agent_id, status, prompt) VALUES ($1, $2, 'submitted', $3)",
    )
    .bind(&task_id)
    .bind(agent_override.as_ref().map(|o| o.0.as_str()).unwrap_or("auto"))
    .bind(&prompt)
    .execute(&state.db)
    .await;

    tokio::spawn(async move {
        // Status: working
        let _ = tx
            .send(
                Event::default()
                    .event("task_status_update")
                    .json_data(json!({ "task_id": task_id_clone, "status": "working" }))
                    .unwrap_or_default(),
            )
            .await;

        match execute_a2a_task(&state, &task_id_clone, &prompt, agent_override, 0).await {
            Ok((agent_id, result)) => {
                save_message(&state, &task_id_clone, "user", &prompt, None).await;
                save_message(&state, &task_id_clone, "agent", &result, Some(&agent_id)).await;

                // Send result as artifact
                let _ = tx
                    .send(
                        Event::default()
                            .event("task_artifact_update")
                            .json_data(json!({
                                "task_id": task_id_clone,
                                "artifact": { "name": "response", "parts": [{ "type": "text", "text": result }] }
                            }))
                            .unwrap_or_default(),
                    )
                    .await;

                // Status: completed
                let _ = tx
                    .send(
                        Event::default()
                            .event("task_status_update")
                            .json_data(json!({ "task_id": task_id_clone, "status": "completed" }))
                            .unwrap_or_default(),
                    )
                    .await;
            }
            Err(e) => {
                let _ = tx
                    .send(
                        Event::default()
                            .event("task_status_update")
                            .json_data(json!({ "task_id": task_id_clone, "status": "failed", "error": e }))
                            .unwrap_or_default(),
                    )
                    .await;
            }
        }
    });

    let stream = ReceiverStream::new(rx).map(|event| Ok::<_, Infallible>(event));
    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("heartbeat"),
    )
}

// ---------------------------------------------------------------------------
// Handler: GET /a2a/tasks/{id}
// ---------------------------------------------------------------------------

pub async fn tasks_get(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Json<Value> {
    match build_task_response(&state, &id).await {
        Some(task) => Json(json!({ "task": task })),
        None => Json(json!({ "error": "Task not found" })),
    }
}

// ---------------------------------------------------------------------------
// Handler: POST /a2a/tasks/{id}/cancel
// ---------------------------------------------------------------------------

pub async fn tasks_cancel(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Json<Value> {
    let result = sqlx::query_scalar::<_, String>(
        "UPDATE gh_a2a_tasks SET status = 'canceled', updated_at = NOW() \
         WHERE id = $1 AND status IN ('submitted', 'working') \
         RETURNING id",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await;

    match result {
        Ok(Some(_)) => {
            // Cancel via token if running
            let tokens = state.a2a_cancel_tokens.read().await;
            if let Some(token) = tokens.get(&id) {
                token.cancel();
            }
            Json(json!({ "task_id": id, "status": "canceled" }))
        }
        Ok(None) => Json(json!({ "error": "Task not found or not cancelable" })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

// ---------------------------------------------------------------------------
// Core: Execute A2A task (with tools, multi-turn)
// ---------------------------------------------------------------------------

async fn execute_a2a_task(
    state: &AppState,
    task_id: &str,
    prompt: &str,
    agent_override: Option<(String, f64, String)>,
    call_depth: u32,
) -> Result<(String, String), String> {
    // Update status to working
    let _ = sqlx::query("UPDATE gh_a2a_tasks SET status = 'working', updated_at = NOW() WHERE id = $1")
        .bind(task_id)
        .execute(&state.db)
        .await;

    let mut ctx = crate::handlers::prepare_execution(state, prompt, None, agent_override, "").await;
    ctx.call_depth = call_depth;
    let agent_id = ctx.agent_id.clone();

    // Update task with resolved agent
    let _ = sqlx::query("UPDATE gh_a2a_tasks SET agent_id = $1, updated_at = NOW() WHERE id = $2")
        .bind(&agent_id)
        .bind(task_id)
        .execute(&state.db)
        .await;

    if ctx.api_key.is_empty() {
        return Err("No API key configured".to_string());
    }

    state.gemini_circuit.check().await?;

    let tools = crate::handlers::build_tools(state);
    let mut gen_config = json!({
        "temperature": ctx.temperature,
        "topP": ctx.top_p,
        "maxOutputTokens": ctx.max_tokens
    });
    if let Some(tc) = crate::handlers::build_thinking_config(&ctx.model, &ctx.thinking_level) {
        gen_config["thinkingConfig"] = tc;
    }

    let mut contents = vec![json!({ "parts": [{ "text": ctx.final_user_prompt }] })];
    let max_iter = 5.min(ctx.max_iterations as usize);

    for _iter in 0..max_iter {
        let body = json!({
            "systemInstruction": { "parts": [{ "text": ctx.system_prompt }] },
            "contents": contents,
            "tools": tools,
            "generationConfig": gen_config
        });

        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent",
            ctx.model
        );

        let resp = crate::oauth::apply_google_auth(
                state.client.post(&url), &ctx.api_key, ctx.is_oauth,
            )
            .json(&body)
            .timeout(Duration::from_secs(120))
            .send()
            .await
            .map_err(|e| format!("Gemini request failed: {}", e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            state.gemini_circuit.record_failure().await;
            return Err(format!("Gemini API error {}: {}", status, &text[..text.len().min(500)]));
        }

        state.gemini_circuit.record_success().await;
        let json_resp: Value = resp.json().await.map_err(|e| format!("JSON parse error: {}", e))?;

        let parts = json_resp["candidates"][0]["content"]["parts"]
            .as_array()
            .ok_or_else(|| "No content parts in Gemini response".to_string())?;

        let mut text_result = String::new();
        let mut function_calls = Vec::new();

        for part in parts {
            if let Some(text) = part["text"].as_str() {
                text_result.push_str(text);
            }
            if part.get("functionCall").is_some() {
                function_calls.push(part["functionCall"].clone());
            }
        }

        // No tool calls — agent is done
        if function_calls.is_empty() {
            let _ = sqlx::query(
                "UPDATE gh_a2a_tasks SET status = 'completed', result = $1, updated_at = NOW() WHERE id = $2",
            )
            .bind(&text_result)
            .bind(task_id)
            .execute(&state.db)
            .await;
            return Ok((agent_id, text_result));
        }

        // Build model turn with function calls
        contents.push(json!({ "role": "model", "parts": parts }));

        // Execute tools
        let mut result_parts = Vec::new();
        for fc in &function_calls {
            let name = fc["name"].as_str().unwrap_or("");
            let args = &fc["args"];

            let output = if name == "call_agent" {
                match Box::pin(execute_agent_call(state, args, ctx.call_depth)).await {
                    Ok(text) => text,
                    Err(e) => format!("AGENT_CALL_ERROR: {}", e),
                }
            } else {
                match tokio::time::timeout(
                    Duration::from_secs(60),
                    crate::tools::execute_tool(name, args, state, &ctx.working_directory),
                )
                .await
                {
                    Ok(Ok(out)) => {
                        // Truncate tool output for context window
                        let text = out.text;
                        if text.len() > 15000 {
                            let truncated: String = text.chars().take(15000).collect();
                            format!("{}...\n[TRUNCATED: {} → 15000 chars]", truncated, text.len())
                        } else {
                            text
                        }
                    }
                    Ok(Err(e)) => format!("TOOL_ERROR: {}", e),
                    Err(_) => format!("TOOL_ERROR: {} timed out after 60s", name),
                }
            };

            result_parts.push(json!({
                "functionResponse": {
                    "name": name,
                    "response": { "result": output }
                }
            }));
        }
        contents.push(json!({ "role": "user", "parts": result_parts }));
    }

    // Reached max iterations
    let _ = sqlx::query(
        "UPDATE gh_a2a_tasks SET status = 'completed', result = 'Max iterations reached', updated_at = NOW() WHERE id = $1",
    )
    .bind(task_id)
    .execute(&state.db)
    .await;

    Ok((agent_id, "Agent reached maximum iterations. Partial results may be available in the task history.".to_string()))
}

// ---------------------------------------------------------------------------
// Inter-agent delegation (call_agent tool)
// ---------------------------------------------------------------------------

const MAX_CALL_DEPTH: u32 = 3;

/// Execute an agent-to-agent call (used by the `call_agent` tool).
/// Runs a full Gemini multi-turn execution with tools for the target agent.
pub(crate) async fn execute_agent_call(
    state: &AppState,
    args: &Value,
    parent_depth: u32,
) -> Result<String, String> {
    let depth = parent_depth + 1;
    if depth > MAX_CALL_DEPTH {
        return Err(format!(
            "Agent call depth limit ({}) reached — cannot delegate further",
            MAX_CALL_DEPTH
        ));
    }

    let agent_id = args["agent_id"]
        .as_str()
        .ok_or("Missing required argument: agent_id")?;
    let task_prompt = args["task"]
        .as_str()
        .ok_or("Missing required argument: task")?;

    // Validate agent exists
    {
        let agents = state.agents.read().await;
        if !agents.iter().any(|a| a.id == agent_id) {
            let available: Vec<_> = agents.iter().map(|a| a.id.as_str()).collect();
            return Err(format!(
                "Unknown agent '{}'. Available: {}",
                agent_id,
                available.join(", ")
            ));
        }
    }

    let task_id = Uuid::new_v4().to_string();

    // Record A2A task
    let _ = sqlx::query(
        "INSERT INTO gh_a2a_tasks (id, agent_id, caller_agent_id, status, prompt) \
         VALUES ($1, $2, $3, 'working', $4)",
    )
    .bind(&task_id)
    .bind(agent_id)
    .bind("parent") // caller context
    .bind(task_prompt)
    .execute(&state.db)
    .await;

    let override_tuple = Some((agent_id.to_string(), 0.99, "A2A call_agent delegation".to_string()));

    match execute_a2a_task(state, &task_id, task_prompt, override_tuple, depth).await {
        Ok((_agent, result)) => Ok(result),
        Err(e) => Err(e),
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn extract_text_from_parts(parts: &[Part]) -> String {
    parts
        .iter()
        .filter_map(|p| match p {
            Part::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

async fn save_message(state: &AppState, task_id: &str, role: &str, content: &str, agent_id: Option<&str>) {
    let id = Uuid::new_v4().to_string();
    let _ = sqlx::query(
        "INSERT INTO gh_a2a_messages (id, task_id, role, content, agent_id) VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(&id)
    .bind(task_id)
    .bind(role)
    .bind(content)
    .bind(agent_id)
    .execute(&state.db)
    .await;
}

async fn build_task_response(state: &AppState, task_id: &str) -> Option<A2aTask> {
    let row = sqlx::query_as::<_, (String, String, String, Option<String>, Option<String>, Option<String>, chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>)>(
        "SELECT id, status, agent_id, parent_task_id, result, error_message, created_at, updated_at \
         FROM gh_a2a_tasks WHERE id = $1",
    )
    .bind(task_id)
    .fetch_optional(&state.db)
    .await
    .ok()??;

    let messages: Vec<A2aMessage> = sqlx::query_as::<_, (String, String)>(
        "SELECT role, content FROM gh_a2a_messages WHERE task_id = $1 ORDER BY created_at ASC",
    )
    .bind(task_id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|(role, content)| A2aMessage {
        role,
        parts: vec![Part::Text { text: content }],
    })
    .collect();

    let mut artifacts = Vec::new();
    if let Some(ref result) = row.4 {
        artifacts.push(A2aArtifact {
            name: Some("response".to_string()),
            parts: vec![Part::Text { text: result.clone() }],
        });
    }

    Some(A2aTask {
        id: row.0,
        status: row.1,
        agent_id: row.2,
        parent_task_id: row.3,
        messages,
        artifacts,
        error_message: row.5,
        created_at: row.6.to_rfc3339(),
        updated_at: row.7.to_rfc3339(),
    })
}
