use serde_json::{json, Value};
use crate::models::{WitcherAgent, WsServerMessage};
use crate::core::agent::build_system_prompt;
use crate::files;
use axum::extract::ws::{Message as WsMessage, WebSocket};
use futures_util::{SinkExt, stream::SplitSink};
use std::time::Instant;
use uuid::Uuid;
use tokio_util::sync::CancellationToken;
use crate::state::AppState;
use crate::core::execution::sse::{SseParser, SseParsedEvent};
use crate::db::{resolve_session_agent, load_session_history, store_messages};
use futures_util::StreamExt;

pub mod sse;

pub fn build_tools() -> Value {
    json!([{
        "functionDeclarations": [
            { "name": "execute_command", "description": "Execute a shell command", "parameters": { "type": "object", "properties": { "command": { "type": "string" } }, "required": ["command"] } },
            { "name": "read_file", "description": "Read a file", "parameters": { "type": "object", "properties": { "path": { "type": "string" } }, "required": ["path"] } },
            { "name": "write_file", "description": "Write a file", "parameters": { "type": "object", "properties": { "path": { "type": "string" }, "content": { "type": "string" } }, "required": ["path", "content"] } },
            { "name": "list_directory", "description": "List a directory", "parameters": { "type": "object", "properties": { "path": { "type": "string" }, "show_hidden": { "type": "boolean" } }, "required": ["path"] } }
        ]
    }])
}

pub struct ExecuteContext {
    pub agent_id: String,
    pub confidence: f64,
    pub reasoning: String,
    pub model: String,
    pub api_key: String,
    pub system_prompt: String,
    pub final_user_prompt: String,
    pub files_loaded: Vec<String>,
    pub steps: Vec<String>,
}

pub async fn prepare_execution(
    state: &AppState,
    prompt: &str,
    model_override: Option<String>,
    agent_override: Option<(String, f64, String)>,
) -> ExecuteContext {
    let (agent_id, confidence, reasoning) = agent_override.unwrap_or_else(|| crate::core::agent::classify_prompt(prompt));

    let (def_model, lang) = sqlx::query_as::<_, (String, String)>(
        "SELECT default_model, language FROM gh_settings WHERE id = 1",
    )
    .fetch_one(&state.db)
    .await
    .unwrap_or_else(|_| ("gemini-3-flash-preview".to_string(), "en".to_string()));

    let model = model_override.unwrap_or(def_model);
    let language = match lang.as_str() { "pl" => "Polish", "en" => "English", other => other };

    let api_key = state.runtime.read().await.api_keys.get("google").cloned().unwrap_or_default();
    let system_prompt = build_system_prompt(&agent_id, &state.agents, language);

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

    ExecuteContext {
        agent_id,
        confidence,
        reasoning,
        model,
        api_key,
        system_prompt,
        final_user_prompt,
        files_loaded: if !file_context.is_empty() { detected_paths } else { Vec::new() },
        steps: vec![
            "classify prompt".into(),
            format!("route to agent (confidence {:.0}%)", confidence * 100.0),
            format!("call Gemini model {}", model),
        ],
    }
}

pub async fn ws_send(sender: &mut SplitSink<WebSocket, WsMessage>, msg: &WsServerMessage) -> bool {
    serde_json::to_string(msg).map(|j| sender.send(WsMessage::Text(j.into()))).is_ok()
}

pub async fn execute_streaming(
    sender: &mut SplitSink<WebSocket, WsMessage>,
    state: &AppState,
    prompt: &str,
    model_override: Option<String>,
    session_id: Option<String>,
    cancel: CancellationToken,
) {
    let start = Instant::now();
    let sid = session_id.as_deref().and_then(|s| Uuid::parse_str(s).ok());
    let agent_info = if let Some(s) = &sid { Some(resolve_session_agent(&state.db, s, prompt).await) } else { None };
    let ctx = prepare_execution(state, prompt, model_override, agent_info).await;
    let resp_id = Uuid::new_v4();

    if !ws_send(sender, &WsServerMessage::Start { id: resp_id.to_string(), agent: ctx.agent_id.clone(), model: ctx.model.clone(), files_loaded: ctx.files_loaded.clone() }).await { return; }
    let _ = ws_send(sender, &WsServerMessage::Plan { agent: ctx.agent_id.clone(), confidence: ctx.confidence, steps: ctx.steps.clone() }).await;

    if ctx.api_key.is_empty() {
        let _ = ws_send(sender, &WsServerMessage::Error { message: "Missing Google API Key".into(), code: Some("NO_API_KEY".into()) }).await;
        return;
    }

    let url = format!("https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent?alt=sse&key={}", ctx.model, ctx.api_key);
    let tools = build_tools();
    let mut contents = if let Some(s) = &sid { load_session_history(&state.db, s).await } else { Vec::new() };
    contents.push(json!({ "role": "user", "parts": [{ "text": ctx.final_user_prompt }] }));

    let mut full_text = String::new();
    for iter in 0..10 {
        let body = json!({ "systemInstruction": { "parts": [{ "text": ctx.system_prompt }] }, "contents": contents, "tools": tools });
        let resp = match state.client.post(&url).json(&body).send().await {
            Ok(r) if r.status().is_success() => r,
            Ok(r) => {
                let _ = ws_send(sender, &WsServerMessage::Error { message: format!("API Error: {}", r.status()), code: Some("GEMINI_ERROR".into()) }).await;
                return;
            }
            Err(e) => {
                let _ = ws_send(sender, &WsServerMessage::Error { message: e.to_string(), code: Some("REQUEST_FAILED".into()) }).await;
                return;
            }
        };

        let (text, fcs, aborted) = consume_gemini_stream(resp, sender, &cancel).await;
        full_text.push_str(&text);
        if aborted || fcs.is_empty() { break; }

        let mut model_parts: Vec<Value> = text.split_whitespace().next().map(|_| json!({ "text": text })).into_iter().collect();
        for (_, _, raw) in &fcs { model_parts.push(raw.clone()); }
        contents.push(json!({ "role": "model", "parts": model_parts }));

        let mut res_parts = Vec::new();
        for (name, args, _) in fcs {
            let _ = ws_send(sender, &WsServerMessage::ToolCall { name: name.clone(), args: args.clone(), iteration: iter + 1 }).await;
            let header = format!("\n\n---\n**🔧 Tool:** `{}`\n", name);
            full_text.push_str(&header);
            let _ = ws_send(sender, &WsServerMessage::Token { content: header }).await;

            let output = crate::tools::execute_tool(&name, &args).await.unwrap_or_else(|e| e);
            let _ = ws_send(sender, &WsServerMessage::ToolResult { name: name.clone(), success: true, summary: output.chars().take(200).collect(), iteration: iter + 1 }).await;
            
            let res_md = format!("```\n{}\n```\n---\n\n", output);
            full_text.push_str(&res_md);
            let _ = ws_send(sender, &WsServerMessage::Token { content: res_md }).await;
            res_parts.push(json!({ "functionResponse": { "name": name, "response": { "result": output } } }));
        }
        contents.push(json!({ "role": "user", "parts": res_parts }));
    }

    store_messages(&state.db, sid, resp_id, prompt, &full_text, &ctx.agent_id, &ctx.model, &ctx.reasoning).await;
    let _ = ws_send(sender, &WsServerMessage::Complete { duration_ms: start.elapsed().as_millis() as u64 }).await;
}

pub async fn consume_gemini_stream(
    resp: reqwest::Response,
    sender: &mut SplitSink<WebSocket, WsMessage>,
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
