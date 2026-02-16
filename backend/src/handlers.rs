use std::collections::HashMap;
use std::time::Instant;

use axum::extract::ws::{Message as WsMessage, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use sysinfo::System;
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
// Helpers & Routing Logic
// ---------------------------------------------------------------------------

fn build_providers(api_keys: &HashMap<String, String>) -> Vec<ProviderInfo> {
    let google_key = api_keys.get("google");
    let anthropic_key = api_keys.get("anthropic");
    let google_available = google_key.is_some() && !google_key.unwrap().is_empty();

    let mut providers = Vec::new();

    for (model_id, display_name) in crate::models::GEMINI_MODELS {
        providers.push(ProviderInfo {
            name: format!("Google {}", display_name),
            available: google_available,
            model: Some(model_id.to_string()),
        });
    }

    providers.push(ProviderInfo {
        name: "Anthropic Claude".to_string(),
        available: anthropic_key.is_some() && !anthropic_key.unwrap().is_empty(),
        model: Some("claude-sonnet-4-20250514".to_string()),
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

/// Expert agent classification based on prompt analysis.
fn classify_prompt(prompt: &str) -> (String, f64, String) {
    let lower = strip_diacritics(&prompt.to_lowercase());

    let rules: &[(&[&str], &str, &str)] = &[
        (&["architecture", "design", "pattern", "structur", "architektur", "wzorzec", "refaktor"],
         "yennefer", "Prompt relates to architecture and design"),
        (&["test", "quality", "assert", "coverage", "testy", "jakosc", "pokrycie"],
         "vesemir", "Prompt relates to testing and quality assurance"),
        (&["security", "protect", "auth", "encrypt", "threat", "vulnerability",
           "bezpieczenst", "zabezpiecz", "szyfrowa", "zagrozeni", "injection", "cors", "xss"],
         "geralt", "Prompt relates to security and protection"),
        (&["monitor", "audit", "incident", "alert", "logging",
           "monitorowa", "audyt", "incydent"],
         "philippa", "Prompt relates to security monitoring"),
        (&["data", "analytic", "database", "sql", "query",
           "dane", "baza danych", "zapytani"],
         "triss", "Prompt relates to data and analytics"),
        (&["document", "readme", "comment", "communication",
           "dokumentacj", "komentarz", "komunikacj"],
         "jaskier", "Prompt relates to documentation"),
        (&["perf", "optim", "speed", "latency", "benchmark",
           "wydajnosc", "szybkosc", "opoznieni"],
         "ciri", "Prompt relates to performance and optimization"),
        (&["plan", "strateg", "roadmap", "priorit",
           "planowa", "priorytet"],
         "dijkstra", "Prompt relates to strategy and planning"),
        (&["devops", "deploy", "docker", "infra", "pipeline", "cicd", "kubernetes",
           "wdrozeni", "kontener"],
         "lambert", "Prompt relates to DevOps and infrastructure"),
        (&["backend", "endpoint", "rest", "serwer", "api"],
         "eskel", "Prompt relates to backend and APIs"),
        (&["research", "knowledge", "learn", "study", "paper",
           "badani", "wiedza", "nauka"],
         "regis", "Prompt relates to research and knowledge"),
        (&["frontend", "ui", "ux", "component", "react", "hook",
           "komponent", "interfejs", "css"],
         "zoltan", "Prompt relates to frontend and UI"),
    ];

    for (keywords, agent_id, reasoning) in rules {
        if keywords.iter().any(|kw| keyword_match(&lower, kw)) {
            return (agent_id.to_string(), 0.85, reasoning.to_string());
        }
    }

    ("dijkstra".to_string(), 0.4, "Defaulting to Strategy & Planning".to_string())
}

// ---------------------------------------------------------------------------
// System Prompt Factory
// ---------------------------------------------------------------------------

fn build_system_prompt(agent_id: &str, agents: &[WitcherAgent], language: &str) -> String {
    let agent = agents.iter().find(|a| a.id == agent_id).unwrap_or(&agents[0]);

    let roster: String = agents
        .iter()
        .map(|a| format!("  - {} ({}) — {}", a.name, a.role, a.description))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"## CRITICAL: Action-First Protocol
1. **NEVER suggest commands** — EXECUTE them with `execute_command`.
2. **NEVER ask the user to paste code** — use `read_file`.
3. **Directory detected?** — Call `list_directory` IMMEDIATELY.
4. **Refactoring**: `list_directory` → `read_file` → analyze → `write_file` → verify.
5. **Act first, explain after.**
6. **Chain up to 10 tool calls.**

## Your Identity
- **Name:** {name} | **Role:** {role} | **Tier:** {tier}
- {description}
- Part of **GeminiHydra v15 Wolf Swarm**.
- Speak as {name}, but tool usage is priority.

## Language
- Respond in **{language}** unless the user writes in a different language.

## Tools
- `execute_command`, `read_file`, `write_file`, `list_directory`

## Swarm Roster
{roster}"#,
        name = agent.name,
        role = agent.role,
        tier = agent.tier,
        description = agent.description,
        language = language,
        roster = roster
    )
}

// ---------------------------------------------------------------------------
// REST Handlers
// ---------------------------------------------------------------------------

pub async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    let rt = state.runtime.read().await;
    Json(HealthResponse {
        status: "ok".to_string(),
        version: "15.0.0".to_string(),
        app: "GeminiHydra".to_string(),
        uptime_seconds: state.start_time.elapsed().as_secs(),
        providers: build_providers(&rt.api_keys),
    })
}

pub async fn health_detailed(State(state): State<AppState>) -> Json<DetailedHealthResponse> {
    let rt = state.runtime.read().await;
    let mut sys = System::new_all();
    sys.refresh_all();

    let cpu_usage: f32 = sys.cpus().iter().map(|c| c.cpu_usage()).sum::<f32>() / sys.cpus().len().max(1) as f32;

    Json(DetailedHealthResponse {
        status: "ok".to_string(),
        version: "15.0.0".to_string(),
        app: "GeminiHydra".to_string(),
        uptime_seconds: state.start_time.elapsed().as_secs(),
        providers: build_providers(&rt.api_keys),
        memory_usage_mb: sys.used_memory() as f64 / 1_048_576.0,
        cpu_usage_percent: cpu_usage,
        platform: std::env::consts::OS.to_string(),
    })
}

pub async fn list_agents(State(state): State<AppState>) -> Json<Value> {
    Json(json!({ "agents": state.agents }))
}

pub async fn classify_agent(Json(body): Json<ClassifyRequest>) -> Json<ClassifyResponse> {
    let (agent_id, confidence, reasoning) = classify_prompt(&body.prompt);
    Json(ClassifyResponse { agent: agent_id, confidence, reasoning })
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
    let (agent_id, confidence, reasoning) = agent_override.unwrap_or_else(|| classify_prompt(prompt));

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
                if let Some(text) = part["text"].as_str() {
                    if !text.is_empty() { events.push(SseParsedEvent::TextToken(text.to_string())); }
                }
                if let Some(fc) = part.get("functionCall") {
                    if let Some(name) = fc["name"].as_str() {
                        events.push(SseParsedEvent::FunctionCall {
                            name: name.to_string(),
                            args: fc["args"].clone(),
                            raw_part: part.clone(),
                        });
                    }
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
                if let Some(data) = line.strip_prefix("data: ") {
                    if data != "[DONE]" && !data.is_empty() {
                        if let Ok(jv) = serde_json::from_str::<Value>(data) {
                            events.extend(Self::parse_parts(&jv));
                        }
                    }
                }
            }
        }
        events
    }

    fn flush(&mut self) -> Vec<SseParsedEvent> {
        let mut events = Vec::new();
        for line in self.buffer.lines() {
            if let Some(data) = line.strip_prefix("data: ") {
                if data != "[DONE]" && !data.is_empty() {
                    if let Ok(jv) = serde_json::from_str::<Value>(data) {
                        events.extend(Self::parse_parts(&jv));
                    }
                }
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

pub async fn ws_execute(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws(socket, state))
}

async fn handle_ws(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();
    let cancel = CancellationToken::new();

    while let Some(Ok(WsMessage::Text(text))) = receiver.next().await {
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

        let mut model_parts: Vec<Value> = if !text.is_empty() { vec![json!({ "text": text })] } else { vec![] };
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

    store_messages(&state.db, sid, resp_id, prompt, &full_text, &ctx).await;
    let _ = ws_send(sender, &WsServerMessage::Complete { duration_ms: start.elapsed().as_millis() as u64 }).await;
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

async fn resolve_session_agent(db: &sqlx::PgPool, sid: &Uuid, prompt: &str) -> (String, f64, String) {
    if let Ok(Some((Some(aid),))) = sqlx::query_as::<_, (Option<String>,)>("SELECT agent_id FROM gh_sessions WHERE id = $1").bind(sid).fetch_optional(db).await {
        if !aid.is_empty() { return (aid, 0.95, "Locked".into()); }
    }
    let (aid, conf, reas) = classify_prompt(prompt);
    let _ = sqlx::query("UPDATE gh_sessions SET agent_id = $1 WHERE id = $2").bind(&aid).bind(sid).execute(db).await;
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

pub async fn gemini_models(State(state): State<AppState>) -> Json<Value> {
    let key = state.runtime.read().await.api_keys.get("google").cloned().unwrap_or_default();
    if key.is_empty() { return Json(json!({ "models": [], "error": "No API key" })); }
    let url = format!("https://generativelanguage.googleapis.com/v1beta/models?key={}", key);
    match state.client.get(&url).send().await {
        Ok(r) if r.status().is_success() => {
            let body: Value = r.json().await.unwrap_or_default();
            let models: Vec<GeminiModelInfo> = body["models"].as_array().map(|a| a.iter().filter_map(|m| serde_json::from_value(m.clone()).ok()).filter(|m: &GeminiModelInfo| m.supported_generation_methods.contains(&"generateContent".to_string())).collect()).unwrap_or_default();
            Json(json!(GeminiModelsResponse { models }))
        }
        _ => Json(json!({ "models": [], "error": "API Error" })),
    }
}

pub async fn system_stats() -> Json<SystemStats> {
    let mut sys = System::new_all();
    sys.refresh_all();
    Json(SystemStats {
        cpu_usage_percent: sys.cpus().iter().map(|c| c.cpu_usage()).sum::<f32>() / sys.cpus().len().max(1) as f32,
        memory_used_mb: sys.used_memory() as f64 / 1_048_576.0,
        memory_total_mb: sys.total_memory() as f64 / 1_048_576.0,
        platform: std::env::consts::OS.to_string(),
    })
}

pub async fn read_file(Json(body): Json<FileReadRequest>) -> Json<Value> {
    match files::read_file_raw(&body.path).await {
        Ok(f) => Json(json!(FileReadResponse { path: f.path, content: f.content, size_bytes: f.size_bytes, truncated: f.truncated, extension: f.extension })),
        Err(e) => Json(json!({ "error": e.reason, "path": e.path })),
    }
}

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
        "functionDeclarations": [
            { "name": "execute_command", "description": "Execute a shell command", "parameters": { "type": "object", "properties": { "command": { "type": "string" } }, "required": ["command"] } },
            { "name": "read_file", "description": "Read a file", "parameters": { "type": "object", "properties": { "path": { "type": "string" } }, "required": ["path"] } },
            { "name": "write_file", "description": "Write a file", "parameters": { "type": "object", "properties": { "path": { "type": "string" }, "content": { "type": "string" } }, "required": ["path", "content"] } },
            { "name": "list_directory", "description": "List a directory", "parameters": { "type": "object", "properties": { "path": { "type": "string" }, "show_hidden": { "type": "boolean" } }, "required": ["path"] } }
        ]
    }])
}

// ---------------------------------------------------------------------------
// HTTP Execute (Legacy)
// ---------------------------------------------------------------------------

pub async fn execute(State(state): State<AppState>, Json(body): Json<ExecuteRequest>) -> Json<Value> {
    let start = Instant::now();
    let ctx = prepare_execution(&state, &body.prompt, body.model.clone(), None).await;
    if ctx.api_key.is_empty() { return Json(json!({ "error": "No API Key" })); }

    let url = format!("https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}", ctx.model, ctx.api_key);
    let gem_body = json!({ "systemInstruction": { "parts": [{ "text": ctx.system_prompt }] }, "contents": [{ "parts": [{ "text": ctx.final_user_prompt }] }] });

    let res = state.client.post(&url).json(&gem_body).send().await;
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

    #[test]
    fn test_refactor_routes_to_yennefer() {
        let (agent, confidence, _) = classify_prompt("refaktoruj ten kod proszę");
        assert_eq!(agent, "yennefer");
        assert!(confidence >= 0.8);
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
