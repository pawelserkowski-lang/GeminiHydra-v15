use std::collections::HashMap;
use std::time::Instant;

use axum::extract::State;
use axum::Json;
use serde_json::{json, Value};
use sysinfo::System;
use uuid::Uuid;

use crate::files;
use crate::models::{
    ClassifyRequest, ClassifyResponse, DetailedHealthResponse, ExecutePlan, ExecuteRequest,
    ExecuteResponse, FileEntryResponse, FileListRequest, FileListResponse, FileReadRequest,
    FileReadResponse, GeminiModelInfo, GeminiModelsResponse, HealthResponse, ProviderInfo,
    SystemStats, WitcherAgent,
};
use crate::state::AppState;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn build_providers(api_keys: &HashMap<String, String>) -> Vec<ProviderInfo> {
    let google_key = api_keys.get("google");
    let anthropic_key = api_keys.get("anthropic");
    let google_available = google_key.is_some() && !google_key.unwrap().is_empty();

    let mut providers = Vec::new();

    // Gemini 3 models
    for (model_id, display_name) in crate::models::GEMINI_MODELS {
        providers.push(ProviderInfo {
            name: format!("Google {}", display_name),
            available: google_available,
            model: Some(model_id.to_string()),
        });
    }

    // Anthropic Claude
    providers.push(ProviderInfo {
        name: "Anthropic Claude".to_string(),
        available: anthropic_key.is_some() && !anthropic_key.unwrap().is_empty(),
        model: Some("claude-sonnet-4-20250514".to_string()),
    });

    providers
}

/// Strip Polish diacritics so keywords work regardless of user input style.
fn strip_diacritics(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'ą' => 'a',
            'ć' => 'c',
            'ę' => 'e',
            'ł' => 'l',
            'ń' => 'n',
            'ó' => 'o',
            'ś' => 's',
            'ź' | 'ż' => 'z',
            _ => c,
        })
        .collect()
}

/// Check if `text` contains `keyword` with appropriate matching.
/// Short keywords (< 4 chars) require whole-word matching to prevent false positives
/// like "logike" (PL: logic) matching "log", or "cd" inside random words.
/// Longer keywords use substring matching for prefix support (e.g. "optim" → "optimization").
fn keyword_match(text: &str, keyword: &str) -> bool {
    if keyword.len() >= 4 {
        text.contains(keyword)
    } else {
        text.split(|c: char| !c.is_alphanumeric())
            .any(|word| word == keyword)
    }
}

/// Keyword-based agent classification with EN + PL support.
fn classify_prompt(prompt: &str) -> (String, f64, String) {
    let lower = strip_diacritics(&prompt.to_lowercase());

    // Order matters — first match wins. More specific patterns first.
    // Keywords include both EN and PL variants (diacritics already stripped).
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

    // Default to Dijkstra (strategy) when nothing matches.
    (
        "dijkstra".to_string(),
        0.4,
        "No strong keyword match — defaulting to Strategy & Planning".to_string(),
    )
}

// ---------------------------------------------------------------------------
// System prompt builder (hidden from main chat)
// ---------------------------------------------------------------------------

/// Build a system instruction for the Gemini API call.
/// This is sent as `systemInstruction` and never displayed in the chat UI.
fn build_system_prompt(agent_id: &str, agents: &[WitcherAgent]) -> String {
    let agent = agents.iter().find(|a| a.id == agent_id);

    let (name, role, description, tier) = match agent {
        Some(a) => (a.name.as_str(), a.role.as_str(), a.description.as_str(), a.tier.as_str()),
        None => ("Dijkstra", "Strategy & Planning", "The Spymaster — plans project strategy.", "Coordinator"),
    };

    let roster: String = agents
        .iter()
        .map(|a| format!("  - {} ({}) — {}", a.name, a.role, a.description))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"You are **{name}**, a specialized AI agent in the **GeminiHydra v15 Wolf Swarm** — a multi-agent system themed after The Witcher universe.

## Your Identity
- **Name:** {name}
- **Role:** {role}
- **Tier:** {tier}
- **Description:** {description}

## GeminiHydra Pipeline
1. User sends a prompt through the GeminiHydra chat interface.
2. The backend classifies the prompt using keyword analysis (EN + PL) and routes it to the best-matching agent.
3. You (the selected agent) receive the prompt with this system context.
4. You respond with expertise in your domain ({role}).
5. The response is stored in chat history and displayed to the user.

## Wolf Swarm — All 12 Agents
{roster}

## Guidelines
- Stay in character as {name}. Reference your Witcher persona when natural, but prioritize being helpful.
- Answer in the **same language** as the user's prompt (Polish or English).
- You specialize in **{role}** — leverage this expertise, but help with any topic if asked.
- Be concise and actionable. Use markdown formatting for code, lists, and structure.
- If a question falls outside your expertise, acknowledge it and suggest which swarm agent would be better suited.
- You can reference other agents by name when collaborating would help the user.
- When file contents are provided as context (prefixed with `--- FILE CONTEXT ---`), analyze them thoroughly. Reference specific lines and code. You have real access to the user's local files.
- You can suggest the user reference additional files by mentioning their full paths."#
    )
}

// ---------------------------------------------------------------------------
// GET /api/health
// ---------------------------------------------------------------------------

pub async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    let rt = state.runtime.read().await;
    let uptime = state.start_time.elapsed().as_secs();

    Json(HealthResponse {
        status: "ok".to_string(),
        version: "15.0.0".to_string(),
        app: "GeminiHydra".to_string(),
        uptime_seconds: uptime,
        providers: build_providers(&rt.api_keys),
    })
}

// ---------------------------------------------------------------------------
// GET /api/health/detailed
// ---------------------------------------------------------------------------

pub async fn health_detailed(State(state): State<AppState>) -> Json<DetailedHealthResponse> {
    let rt = state.runtime.read().await;
    let uptime = state.start_time.elapsed().as_secs();

    let mut sys = System::new();
    sys.refresh_memory();
    sys.refresh_cpu_all();

    let cpu_usage: f32 = sys.cpus().iter().map(|c| c.cpu_usage()).sum::<f32>()
        / sys.cpus().len().max(1) as f32;
    let memory_used_mb = sys.used_memory() as f64 / 1_048_576.0;

    Json(DetailedHealthResponse {
        status: "ok".to_string(),
        version: "15.0.0".to_string(),
        app: "GeminiHydra".to_string(),
        uptime_seconds: uptime,
        providers: build_providers(&rt.api_keys),
        memory_usage_mb: memory_used_mb,
        cpu_usage_percent: cpu_usage,
        platform: std::env::consts::OS.to_string(),
    })
}

// ---------------------------------------------------------------------------
// GET /api/agents
// ---------------------------------------------------------------------------

pub async fn list_agents(State(state): State<AppState>) -> Json<Value> {
    Json(json!({ "agents": state.agents }))
}

// ---------------------------------------------------------------------------
// POST /api/agents/classify
// ---------------------------------------------------------------------------

pub async fn classify_agent(
    Json(body): Json<ClassifyRequest>,
) -> Json<ClassifyResponse> {
    let (agent_id, confidence, reasoning) = classify_prompt(&body.prompt);

    Json(ClassifyResponse {
        agent: agent_id,
        confidence,
        reasoning,
    })
}

// ---------------------------------------------------------------------------
// POST /api/execute
// ---------------------------------------------------------------------------

pub async fn execute(
    State(state): State<AppState>,
    Json(body): Json<ExecuteRequest>,
) -> Json<Value> {
    let start = Instant::now();

    let (agent_id, confidence, reasoning) = classify_prompt(&body.prompt);

    // Read default_model from DB settings (fallback if DB is unavailable).
    let default_model = sqlx::query_scalar::<_, String>(
        "SELECT default_model FROM gh_settings WHERE id = 1",
    )
    .fetch_one(&state.db)
    .await
    .unwrap_or_else(|_| "gemini-3-flash-preview".to_string());

    let model = body
        .model
        .clone()
        .unwrap_or(default_model);

    // Read API key from runtime state.
    let api_key = {
        let rt = state.runtime.read().await;
        rt.api_keys.get("google").cloned().unwrap_or_default()
    };

    let system_prompt = build_system_prompt(&agent_id, &state.agents);

    // Detect file paths in prompt and load their contents as context.
    let detected_paths = files::extract_file_paths(&body.prompt);
    let (file_context, _file_errors) = if !detected_paths.is_empty() {
        files::build_file_context(&detected_paths).await
    } else {
        (String::new(), Vec::new())
    };

    let files_loaded: Vec<String> = if !file_context.is_empty() {
        detected_paths.clone()
    } else {
        Vec::new()
    };

    // Build final user prompt with file context prepended.
    let final_user_prompt = if file_context.is_empty() {
        body.prompt.clone()
    } else {
        format!("{}{}", file_context, body.prompt)
    };

    // If no API key, return a graceful error-like response.
    if api_key.is_empty() {
        let duration = start.elapsed().as_millis() as u64;
        return Json(json!(ExecuteResponse {
            id: Uuid::new_v4().to_string(),
            result: "Error: No Google/Gemini API key configured. Set GOOGLE_API_KEY or GEMINI_API_KEY in your environment.".to_string(),
            plan: Some(ExecutePlan {
                agent: Some(agent_id),
                steps: vec!["classify prompt".into(), "call Gemini API".into(), "return result".into()],
                estimated_time: None,
            }),
            duration_ms: duration,
            mode: body.mode.clone(),
            files_loaded: Vec::new(),
        }));
    }

    // Build Gemini generateContent request.
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
        model, api_key
    );

    let gemini_body = json!({
        "systemInstruction": {
            "parts": [{ "text": system_prompt }]
        },
        "contents": [{
            "parts": [{ "text": final_user_prompt }]
        }]
    });

    let gemini_result = state.client.post(&url).json(&gemini_body).send().await;

    let result_text = match gemini_result {
        Ok(resp) => {
            if resp.status().is_success() {
                let json_resp: Value = resp.json().await.unwrap_or(json!({}));
                json_resp["candidates"][0]["content"]["parts"][0]["text"]
                    .as_str()
                    .unwrap_or("No content in response")
                    .to_string()
            } else {
                let status = resp.status();
                let error_body = resp.text().await.unwrap_or_default();
                format!("Gemini API error ({}): {}", status, error_body)
            }
        }
        Err(e) => format!("Request failed: {}", e),
    };

    let duration = start.elapsed().as_millis() as u64;
    let response_id = Uuid::new_v4();

    // Build execution steps (include file loading info if applicable).
    let mut steps = vec![
        "classify prompt".into(),
        format!("route to agent (confidence {:.0}%)", confidence * 100.0),
    ];
    if !files_loaded.is_empty() {
        steps.push(format!("loaded {} file(s) as context", files_loaded.len()));
    }
    steps.push(format!("call Gemini model {}", model));
    steps.push("return result".into());

    let response = ExecuteResponse {
        id: response_id.to_string(),
        result: result_text,
        plan: Some(ExecutePlan {
            agent: Some(agent_id.clone()),
            steps,
            estimated_time: Some(format!("{}ms", duration)),
        }),
        duration_ms: duration,
        mode: body.mode.clone(),
        files_loaded,
    };

    // Store in DB (non-fatal on error).
    if let Err(e) = sqlx::query(
        "INSERT INTO gh_chat_messages (id, role, content, model, agent) VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(response_id)
    .bind("user")
    .bind(&body.prompt)
    .bind(Some(&model))
    .bind(Some(&agent_id))
    .execute(&state.db)
    .await
    {
        tracing::warn!("Failed to store user message: {}", e);
    }

    if let Err(e) = sqlx::query(
        "INSERT INTO gh_chat_messages (id, role, content, model, agent) VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(Uuid::new_v4())
    .bind("assistant")
    .bind(&response.result)
    .bind(Some(&model))
    .bind(Some(&reasoning))
    .execute(&state.db)
    .await
    {
        tracing::warn!("Failed to store assistant message: {}", e);
    }

    Json(json!(response))
}

// ---------------------------------------------------------------------------
// GET /api/gemini/models
// ---------------------------------------------------------------------------

pub async fn gemini_models(State(state): State<AppState>) -> Json<Value> {
    let api_key = {
        let rt = state.runtime.read().await;
        rt.api_keys.get("google").cloned().unwrap_or_default()
    };

    if api_key.is_empty() {
        return Json(json!({ "models": [], "error": "No Google/Gemini API key configured" }));
    }

    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models?key={}",
        api_key
    );

    let resp = state.client.get(&url).send().await;

    match resp {
        Ok(r) if r.status().is_success() => {
            let body: Value = r.json().await.unwrap_or(json!({}));
            let raw_models = body["models"].as_array();

            let models: Vec<GeminiModelInfo> = raw_models
                .map(|arr| {
                    arr.iter()
                        .filter_map(|m| serde_json::from_value::<GeminiModelInfo>(m.clone()).ok())
                        .filter(|m| {
                            m.supported_generation_methods
                                .iter()
                                .any(|method| method == "generateContent")
                        })
                        .collect()
                })
                .unwrap_or_default();

            Json(json!(GeminiModelsResponse { models }))
        }
        Ok(r) => {
            let status = r.status().to_string();
            let text = r.text().await.unwrap_or_default();
            Json(json!({ "models": [], "error": format!("Gemini API error ({}): {}", status, text) }))
        }
        Err(e) => Json(json!({ "models": [], "error": format!("Request failed: {}", e) })),
    }
}

// ---------------------------------------------------------------------------
// GET /api/system/stats
// ---------------------------------------------------------------------------

pub async fn system_stats() -> Json<SystemStats> {
    let mut sys = System::new_all();
    sys.refresh_all();

    let cpu_usage: f32 = sys.cpus().iter().map(|c| c.cpu_usage()).sum::<f32>()
        / sys.cpus().len().max(1) as f32;

    let memory_used_mb = sys.used_memory() as f64 / 1_048_576.0;
    let memory_total_mb = sys.total_memory() as f64 / 1_048_576.0;

    Json(SystemStats {
        cpu_usage_percent: cpu_usage,
        memory_used_mb,
        memory_total_mb,
        platform: std::env::consts::OS.to_string(),
    })
}

// ---------------------------------------------------------------------------
// POST /api/files/read
// ---------------------------------------------------------------------------

pub async fn read_file(Json(body): Json<FileReadRequest>) -> Json<Value> {
    match files::read_file_raw(&body.path).await {
        Ok(fc) => Json(json!(FileReadResponse {
            path: fc.path,
            content: fc.content,
            size_bytes: fc.size_bytes,
            truncated: fc.truncated,
            extension: fc.extension,
        })),
        Err(e) => Json(json!({
            "error": e.reason,
            "path": e.path,
        })),
    }
}

// ---------------------------------------------------------------------------
// POST /api/files/list
// ---------------------------------------------------------------------------

pub async fn list_files(Json(body): Json<FileListRequest>) -> Json<Value> {
    match files::list_directory(&body.path, body.show_hidden).await {
        Ok(entries) => {
            let count = entries.len();
            let response_entries: Vec<FileEntryResponse> = entries
                .into_iter()
                .map(|e| FileEntryResponse {
                    name: e.name,
                    path: e.path,
                    is_dir: e.is_dir,
                    size_bytes: e.size_bytes,
                    extension: e.extension,
                })
                .collect();
            Json(json!(FileListResponse {
                path: body.path,
                entries: response_entries,
                count,
            }))
        }
        Err(e) => Json(json!({
            "error": e.reason,
            "path": e.path,
        })),
    }
}
