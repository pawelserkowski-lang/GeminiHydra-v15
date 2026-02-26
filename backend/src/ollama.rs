// Jaskier Shared Pattern — ollama
//! Ollama local LLM integration — model discovery and registry population.
//!
//! Auto-discovers available Ollama models at `http://localhost:11434/api/tags`
//! (or a custom URL from `gh_settings.ollama_url`) and merges them into the
//! shared model registry so they appear in the UI alongside Gemini/Anthropic.

use serde::Deserialize;

use crate::model_registry::ModelInfo;
use crate::state::AppState;

// ── Ollama /api/tags response types ─────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct OllamaTagsResponse {
    models: Option<Vec<OllamaModel>>,
}

#[derive(Debug, Deserialize)]
struct OllamaModel {
    name: String,
    #[serde(default)]
    details: Option<OllamaModelDetails>,
}

#[derive(Debug, Deserialize)]
struct OllamaModelDetails {
    parameter_size: Option<String>,
    family: Option<String>,
}

// ── Public API ──────────────────────────────────────────────────────────────

/// Discover locally available Ollama models and return them as `ModelInfo` entries.
///
/// Returns an empty `Vec` (never errors) when Ollama is unreachable — it is
/// treated as an optional provider.
pub async fn discover_models(state: &AppState) -> Vec<ModelInfo> {
    let ollama_url = get_ollama_url(state).await;
    let tags_url = format!("{}/api/tags", ollama_url.trim_end_matches('/'));

    let resp = match state
        .client
        .get(&tags_url)
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
    {
        Ok(r) if r.status().is_success() => r,
        Ok(r) => {
            tracing::debug!("ollama: /api/tags returned {}", r.status());
            return Vec::new();
        }
        Err(e) => {
            tracing::debug!("ollama: not reachable at {}: {}", tags_url, e);
            return Vec::new();
        }
    };

    let body: OllamaTagsResponse = match resp.json().await {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!("ollama: failed to parse /api/tags response: {}", e);
            return Vec::new();
        }
    };

    let raw_models = body.models.unwrap_or_default();
    let models: Vec<ModelInfo> = raw_models
        .into_iter()
        .map(|m| {
            let display_name = build_display_name(&m);
            let capabilities = infer_capabilities(&m.name);

            ModelInfo {
                // Prefix with "ollama:" so the execute dispatcher knows the provider
                id: format!("ollama:{}", m.name),
                provider: "ollama".to_string(),
                display_name: Some(display_name),
                capabilities,
            }
        })
        .collect();

    if !models.is_empty() {
        tracing::info!("ollama: discovered {} local models", models.len());
    }

    models
}

/// Check whether Ollama is reachable (HEAD request to /api/tags, 3s timeout).
pub async fn is_available(state: &AppState) -> bool {
    let ollama_url = get_ollama_url(state).await;
    let tags_url = format!("{}/api/tags", ollama_url.trim_end_matches('/'));

    state
        .client
        .head(&tags_url)
        .timeout(std::time::Duration::from_secs(3))
        .send()
        .await
        .is_ok_and(|r| r.status().is_success())
}

// ── Internal helpers ────────────────────────────────────────────────────────

/// Read the Ollama base URL from `gh_settings`, falling back to localhost.
async fn get_ollama_url(state: &AppState) -> String {
    sqlx::query_scalar::<_, String>("SELECT ollama_url FROM gh_settings WHERE id = 1")
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| "http://localhost:11434".to_string())
}

/// Build a human-friendly display name from the Ollama model metadata.
fn build_display_name(m: &OllamaModel) -> String {
    let base = m.name.clone();
    if let Some(ref details) = m.details {
        let mut parts = vec![base];
        if let Some(ref size) = details.parameter_size {
            parts.push(format!("({})", size));
        }
        if let Some(ref family) = details.family {
            parts.push(format!("[{}]", family));
        }
        parts.join(" ")
    } else {
        base
    }
}

/// Infer capabilities from the model name (heuristic).
fn infer_capabilities(name: &str) -> Vec<String> {
    let lower = name.to_lowercase();
    let mut caps = vec!["text".to_string()];

    if lower.contains("vision") || lower.contains("llava") {
        caps.push("vision".to_string());
    }
    if lower.contains("code") || lower.contains("codellama") || lower.contains("deepseek-coder") {
        caps.push("code".to_string());
    }

    caps
}
