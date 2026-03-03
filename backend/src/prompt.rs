// ---------------------------------------------------------------------------
// prompt.rs — System prompt building & knowledge context (extracted from handlers/mod.rs)
// ---------------------------------------------------------------------------

use serde_json::{Value, json};

use crate::models::WitcherAgent;
use crate::state::AppState;

// ---------------------------------------------------------------------------
// Jaskier Knowledge API — optional project context enrichment
// ---------------------------------------------------------------------------

/// Fetch project knowledge from the Jaskier Knowledge API.
/// Returns a formatted section to append to the system prompt, or an empty
/// string if the API is unavailable, not configured, or returns an error.
pub async fn fetch_knowledge_context(state: &AppState, project_id: &str) -> String {
    let base_url = match &state.knowledge_api_url {
        Some(url) => url,
        None => return String::new(),
    };

    if project_id.is_empty() {
        return String::new();
    }

    let url = format!(
        "{}/api/knowledge/projects/{}",
        base_url.trim_end_matches('/'),
        project_id
    );

    let mut req = state
        .client
        .get(&url)
        .timeout(std::time::Duration::from_secs(3));
    if let Some(secret) = &state.knowledge_auth_secret {
        req = req.header("Authorization", format!("Bearer {}", secret));
    }

    let resp = match req.send().await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("Knowledge API request failed: {}", e);
            return String::new();
        }
    };

    if !resp.status().is_success() {
        tracing::warn!("Knowledge API returned status {}", resp.status());
        return String::new();
    }

    let body: serde_json::Value = match resp.json().await {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!("Knowledge API response parse failed: {}", e);
            return String::new();
        }
    };

    // Build a concise summary from the response
    let mut parts = Vec::new();

    if let Some(name) = body.get("name").and_then(|v| v.as_str()) {
        parts.push(format!("**Project**: {}", name));
    }
    if let Some(count) = body.get("components_count").and_then(|v| v.as_u64()) {
        parts.push(format!("**Components**: {}", count));
    }
    if let Some(count) = body.get("dependencies_count").and_then(|v| v.as_u64()) {
        parts.push(format!("**Dependencies**: {}", count));
    }
    if let Some(views) = body.get("views").and_then(|v| v.as_array()) {
        let names: Vec<&str> = views.iter().filter_map(|v| v.as_str()).collect();
        if !names.is_empty() {
            parts.push(format!("**Views**: {}", names.join(", ")));
        }
    }
    if let Some(hooks) = body.get("hooks").and_then(|v| v.as_array()) {
        let names: Vec<&str> = hooks.iter().filter_map(|v| v.as_str()).collect();
        if !names.is_empty() {
            parts.push(format!("**Hooks**: {}", names.join(", ")));
        }
    }

    if parts.is_empty() {
        return String::new();
    }

    format!(
        "\n\n## Project Knowledge (from Jaskier Knowledge Base)\n{}",
        parts.join("\n")
    )
}

// ---------------------------------------------------------------------------
// System Prompt Factory
// ---------------------------------------------------------------------------

pub fn build_system_prompt(
    agent_id: &str,
    agents: &[WitcherAgent],
    language: &str,
    model: &str,
    working_directory: &str,
) -> String {
    let agent = agents
        .iter()
        .find(|a| a.id == agent_id)
        .unwrap_or(&agents[0]);

    let roster: String = agents
        .iter()
        .map(|a| {
            let kw = if a.keywords.is_empty() {
                String::new()
            } else {
                format!(
                    " [{}]",
                    a.keywords
                        .iter()
                        .take(5)
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            };
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
- **FIX IT, DON'T JUST PROPOSE.** When the user EXPLICITLY asks you to fix, change, refactor, or improve code — USE `edit_file` TO APPLY THE FIX IMMEDIATELY. For small changes prefer `edit_file` (replaces targeted section), for new files or full rewrites use `write_file`. Do NOT just show code snippets and say "you should change X to Y". Actually edit the file. The workflow is: read → diagnose → FIX (edit_file) → report what you changed. Only propose without applying if the fix would be destructive (deleting data, dropping tables) or if you're genuinely unsure which of multiple approaches is correct.
- **ANALYSIS vs EDITING.** If the user asks to "analyze", "describe", "explain", "check", "review", "list", "show", or "summarize" — DO NOT use `edit_file` or `write_file`. Only read and report. Use editing tools ONLY when the user's intent is clearly to CHANGE code (keywords: "fix", "change", "update", "refactor", "add", "remove", "implement", "napraw", "zmień", "popraw", "dodaj", "usuń", "zaimplementuj").
- **NEVER ASK THE USER FOR CONFIRMATION OR CLARIFICATION.** Do NOT ask "Do you want me to...?", "Should I...?", "Which file should I...?". Instead, use your tools to gather the information you need, make decisions, and deliver results.
- Use dedicated tools (list_directory, read_file, search_files, get_code_structure) — NEVER execute_command for file ops.
- Call `get_code_structure` BEFORE `read_file` on source files to identify what to read.
- Request MULTIPLE tool calls in PARALLEL when independent.
- **BUDGET YOUR ITERATIONS.** You have a limited number of tool calls (~15-25). For multi-step tasks, DO NOT spend all iterations on data gathering. Plan ahead: gather essential data in the first 60-70% of iterations, then STOP calling tools and write your report. If the system tells you iteration 8+, start writing your analysis with what you have. An incomplete report is better than no report.
- **ALWAYS ANSWER WITH TEXT.** After calling tools and applying a fix with edit_file, you MUST write a structured report explaining: what the bug was, what you changed (before/after snippets), and why. Include file paths and line numbers. If you only analyzed (no fix needed), write conclusions with headers, tables, and code refs. NEVER end with only tool outputs — always write at least a paragraph of explanation.
- **PROPOSE NEXT TASKS.** At the END of every completed task, add a markdown heading **Co dalej?** with exactly 5 numbered follow-up tasks the user could ask you to do next. Make them specific, actionable, and relevant to the work just completed. Example: if you fixed a bug, suggest writing tests, checking similar patterns, refactoring related code, updating docs, or running a full audit. Format each as a one-line imperative sentence.
- **VERIFY RUST EDITS.** After editing any `.rs` file, call `execute_command` with `cargo check --manifest-path <project>/backend/Cargo.toml` to verify compilation. If check fails — fix ALL errors before continuing. Never leave the project in a broken state.
- **RUST MODULE SYSTEM.** When creating a module directory (e.g., `files/mod.rs`), you MUST delete the old flat file (`files.rs`) using `delete_file`. In Rust, having both `files.rs` and `files/mod.rs` causes fatal E0761 error. Pattern: create `foo/mod.rs` → immediately `delete_file` `foo.rs`.
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
// Gemini 3 Thinking Config Helper
// ---------------------------------------------------------------------------

/// Build the thinkingConfig JSON for Gemini generationConfig.
/// - Gemini 3+ models: use `thinkingLevel` (string enum: minimal/low/medium/high)
/// - Gemini 2.5 models: use `thinkingBudget` (integer) mapped from thinking_level
/// - "none" disables thinking entirely (omit thinkingConfig)
pub fn build_thinking_config(model: &str, thinking_level: &str) -> Option<Value> {
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
