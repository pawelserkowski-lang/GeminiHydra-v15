// ---------------------------------------------------------------------------
// context.rs — Execution context preparation (extracted from handlers/mod.rs)
// ---------------------------------------------------------------------------

use crate::classify::{
    classify_agent_score, classify_prompt, classify_with_gemini, strip_diacritics,
};
use crate::prompt::{build_system_prompt, fetch_knowledge_context};
use crate::state::AppState;

// ---------------------------------------------------------------------------
// Execution Context & Helpers
// ---------------------------------------------------------------------------

/// Context window token budget per model tier.
pub fn tier_token_budget(model: &str) -> i32 {
    let lower = model.to_lowercase();
    if lower.contains("flash") {
        8192
    } else if lower.contains("pro") {
        65536
    } else {
        32768
    }
}

/// Whether an HTTP status code is retryable (transient failure).
#[allow(dead_code)]
pub fn is_retryable_status(code: u16) -> bool {
    matches!(code, 429 | 502 | 503)
}

#[derive(Clone)]
pub struct ExecuteContext {
    pub agent_id: String,
    pub confidence: f64,
    pub reasoning: String,
    pub model: String,
    pub api_key: String,
    /// When true, api_key is an OAuth Bearer token; when false, it's a Google API key.
    pub is_oauth: bool,
    pub system_prompt: String,
    pub final_user_prompt: String,
    pub files_loaded: Vec<String>,
    pub steps: Vec<String>,
    pub temperature: f64,
    pub max_tokens: i32,
    /// #46 — topP for Gemini generationConfig
    pub top_p: f64,
    /// #47 — Response style (stored for logging/audit; hint already appended to prompt)
    #[allow(dead_code)]
    pub response_style: String,
    /// #49 — Max tool call iterations per request
    pub max_iterations: i32,
    /// Gemini 3 thinking level: 'none', 'minimal', 'low', 'medium', 'high'
    pub thinking_level: String,
    /// A2A — current agent call depth (0 = user-initiated, max 3)
    pub call_depth: u32,
    /// Working directory for filesystem tools (empty = absolute paths only)
    pub working_directory: String,
}

pub async fn prepare_execution(
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
            if let Some(matched_agent) = agents_lock
                .iter()
                .find(|a| a.id == agent_name || a.name.to_lowercase() == agent_name)
            {
                let aid = matched_agent.id.clone();
                (
                    prompt[space_idx + 1..].trim().to_string(),
                    Some((
                        aid,
                        0.99,
                        "User explicitly selected agent via @prefix".to_string(),
                    )),
                )
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
            let gemini_result = tokio::time::timeout(std::time::Duration::from_secs(8), async {
                let classify_cred = crate::oauth::get_google_credential(state).await;
                if let Some((classify_key, classify_is_oauth)) = classify_cred {
                    classify_with_gemini(
                        &state.client,
                        &classify_key,
                        classify_is_oauth,
                        &prompt_clean,
                        &agents_lock,
                    )
                    .await
                } else {
                    None
                }
            })
            .await;
            match gemini_result {
                Ok(Some(result)) => {
                    tracing::info!(
                        "classify: Gemini Flash override — {} (keyword was {} @ {:.0}%)",
                        result.0,
                        kw_agent,
                        kw_conf * 100.0
                    );
                    result
                }
                Ok(None) => (kw_agent, kw_conf, kw_reason),
                Err(_) => {
                    tracing::warn!(
                        "classify: Gemini Flash classification timed out after 8s, using keyword result"
                    );
                    (kw_agent, kw_conf, kw_reason)
                }
            }
        } else {
            (kw_agent, kw_conf, kw_reason)
        }
    };

    // #30 — Multi-agent collaboration hint
    let lower_prompt = strip_diacritics(&prompt_clean.to_lowercase());
    let mut top_agents: Vec<_> = agents_lock
        .iter()
        .map(|a| {
            let score = classify_agent_score(&lower_prompt, a);
            (a.id.clone(), a.name.clone(), score)
        })
        .filter(|(id, _, s)| *s > 0.65 && *id != agent_id)
        .collect();
    top_agents.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
    let collab_hint = if let Some(secondary) = top_agents.first() {
        format!(
            "\n[SYSTEM: This task also relates to {} ({:.0}% match). Consider their perspective in your analysis.]\n",
            secondary.1,
            secondary.2 * 100.0
        )
    } else {
        String::new()
    };

    let (force_model_setting, def_model, lang, temperature, max_tokens, top_p, response_style, max_iterations, thinking_level, settings_wd) =
        sqlx::query_as::<_, (Option<String>, String, String, f64, i32, f64, String, i32, String, String)>(
            "SELECT force_model, default_model, language, temperature, max_tokens, top_p, response_style, max_iterations, thinking_level, working_directory \
             FROM gh_settings WHERE id = 1",
        )
        .fetch_one(&state.db)
        .await
        .unwrap_or_else(|_| (
            None, "gemini-3.1-pro-preview-customtools".to_string(), "en".to_string(), 1.0, 65536, 0.95, "balanced".to_string(), 10, "medium".to_string(), String::new()
        ));

    // Session WD takes priority over global settings WD
    let working_directory = if !session_wd.is_empty() {
        session_wd.to_string()
    } else {
        settings_wd
    };

    // #48 — Per-agent temperature override
    let matched_agent = agents_lock.iter().find(|a| a.id == agent_id);
    let agent_temp = matched_agent.and_then(|a| a.temperature);
    let effective_temperature = agent_temp.unwrap_or(temperature);

    // Per-agent thinking level override (NULL = use global setting)
    let agent_thinking = matched_agent.and_then(|a| a.thinking_level.clone());
    let effective_thinking = agent_thinking.unwrap_or(thinking_level);

    // Model priority: 0) global force_model → 1) user request override → 2) per-agent DB override → 3) auto-tier → 4) global default
    let agent_model = matched_agent.and_then(|a| a.model_override.clone());
    let model = if let Some(fm) = force_model_setting {
        fm
    } else if let Some(ov) = model_override {
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
                tracing::info!(
                    "A/B test: agent {} using model_b={} (split={:.0}%)",
                    agent.id,
                    model_b,
                    split * 100.0
                );
                model_b.clone()
            } else {
                model
            }
        } else {
            model
        }
    } else {
        model
    };

    let language = match lang.as_str() {
        "pl" => "Polish",
        "en" => "English",
        other => other,
    };

    let (api_key, is_oauth) = crate::oauth::get_google_credential(state)
        .await
        .unwrap_or_default();

    // Cached system prompt — byte-identical across requests enables Gemini implicit caching
    let prompt_cache_key = format!("{}:{}:{}:{}", agent_id, language, model, working_directory);
    let system_prompt = {
        let cache = state.prompt_cache.read().await;
        cache.get(&prompt_cache_key).cloned()
    }
    .unwrap_or_else(|| {
        let prompt = build_system_prompt(
            &agent_id,
            &agents_lock,
            language,
            &model,
            &working_directory,
        );
        let cache_clone = prompt.clone();
        let state_clone = state.prompt_cache.clone();
        let key_clone = prompt_cache_key.clone();
        tokio::spawn(async move {
            state_clone.write().await.insert(key_clone, cache_clone);
        });
        prompt
    });

    // Jaskier Knowledge API — enrich system prompt with project context (optional, non-blocking)
    let system_prompt = if state.knowledge_api_url.is_some() {
        let project_id = if !working_directory.is_empty() {
            std::path::Path::new(&working_directory)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_lowercase()
        } else {
            "geminihydra-v15".to_string()
        };
        let knowledge_ctx = fetch_knowledge_context(state, &project_id).await;
        if knowledge_ctx.is_empty() {
            system_prompt
        } else {
            format!("{}{}", system_prompt, knowledge_ctx)
        }
    } else {
        system_prompt
    };

    // MCP tool awareness — inform agent about available MCP tools (PRIORITY)
    let system_prompt = {
        let mcp_tools = state.mcp_client.list_all_tools().await;
        if mcp_tools.is_empty() {
            system_prompt
        } else {
            let tool_list: Vec<&str> = mcp_tools.iter().map(|t| t.prefixed_name.as_str()).collect();
            format!(
                "{}\n\n## MCP Tools (PRIORITY)\n\
                You have access to **{} external MCP tools** from connected servers.\n\
                **ALWAYS prefer MCP tools over native equivalents when available.** \
                MCP tools provide richer functionality and are maintained by their respective servers.\n\
                Available: `{}`\n\
                MCP tools are prefixed with `mcp_` followed by server and tool name.\n\
                Use `list_mcp_tools` to see full descriptions, or call them directly by name.",
                system_prompt,
                mcp_tools.len(),
                tool_list.join("`, `")
            )
        }
    };

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
        format!(
            "\n[SYSTEM: {} file(s) could not be auto-loaded (size/quota exceeded). Use read_file or read_file_section to inspect them manually.]\n",
            context_errors.len()
        )
    } else {
        String::new()
    };

    let files_loaded = if !file_context.is_empty() {
        sorted_paths
    } else {
        Vec::new()
    };

    // #24 — Add file context summary
    let context_summary = if !files_loaded.is_empty() {
        let total_size = file_context.len();
        format!(
            "\n[AUTO-LOADED: {} file(s), ~{}KB total: {}]\n",
            files_loaded.len(),
            total_size / 1024,
            files_loaded.join(", ")
        )
    } else {
        String::new()
    };

    let dir_hint = detected_paths
        .iter()
        .filter(|p| std::path::Path::new(p).is_dir())
        .map(|p| format!("\"{}\"", p))
        .collect::<Vec<_>>();

    let dir_hint_str = if !dir_hint.is_empty() {
        format!(
            "\n[SYSTEM HINT: Directory paths detected: {}. Use list_directory to explore them IMMEDIATELY.]\n",
            dir_hint.join(", ")
        )
    } else {
        String::new()
    };

    // #47 — Response style hint
    let style_hint = match response_style.as_str() {
        "concise" => {
            "\n[STYLE: Be extremely concise. Max 500 words. Tables over paragraphs. No filler or repetition.]\n"
        }
        "detailed" => {
            "\n[STYLE: Provide thorough analysis with examples, code snippets, and detailed explanations.]\n"
        }
        "technical" => {
            "\n[STYLE: Assume expert reader. Skip basics. Focus on implementation details, edge cases, and architecture.]\n"
        }
        _ => "", // "balanced" = default, no override
    };

    // #50 — Rating-based quality warning (fire-and-forget, don't block on failure)
    let rating_warning = match sqlx::query_as::<_, (f64, i64)>(
        "SELECT COALESCE(avg_rating, 5.0)::FLOAT8, COALESCE(total_ratings, 0)::BIGINT \
         FROM gh_agent_rating_stats WHERE agent_id = $1",
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
        file_context,
        context_summary,
        prompt_clean,
        dir_hint_str,
        skip_warning,
        style_hint,
        rating_warning,
        collab_hint
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
