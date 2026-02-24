// GeminiHydra v15 — Dynamic Model Registry
//
// Fetches available models from Google and Anthropic APIs,
// caches them with a TTL, and selects the latest model for each use case.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use axum::extract::{Path, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::state::AppState;

// ── Cache TTL ────────────────────────────────────────────────────────────────

const CACHE_TTL: Duration = Duration::from_secs(3600); // 1 hour

// ── Model info ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub provider: String,
    pub display_name: Option<String>,
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResolvedModels {
    pub chat: Option<ModelInfo>,
    pub thinking: Option<ModelInfo>,
    pub image: Option<ModelInfo>,
}

// ── Pin request ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct PinModelRequest {
    pub use_case: String,
    pub model_id: String,
}

// ── Model cache ──────────────────────────────────────────────────────────────

pub struct ModelCache {
    pub models: HashMap<String, Vec<ModelInfo>>,
    pub fetched_at: Option<Instant>,
}

impl ModelCache {
    pub fn new() -> Self {
        Self {
            models: HashMap::new(),
            fetched_at: None,
        }
    }

    pub fn is_stale(&self) -> bool {
        match self.fetched_at {
            Some(t) => t.elapsed() > CACHE_TTL,
            None => true,
        }
    }
}

// ── Fetch models from providers ──────────────────────────────────────────────

async fn fetch_google_models(
    client: &reqwest::Client,
    api_key: &str,
) -> Result<Vec<ModelInfo>, String> {
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models?key={}",
        api_key
    );

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Google models request failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Google models API returned {}", resp.status()));
    }

    let body: Value = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse Google models: {}", e))?;

    let models_arr = body["models"].as_array().cloned().unwrap_or_default();

    let mut models = Vec::new();
    for m in models_arr {
        let name = m["name"].as_str().unwrap_or("").to_string();
        let id = name.trim_start_matches("models/").to_string();
        let display_name = m["displayName"].as_str().map(|s| s.to_string());

        let methods: Vec<String> = m["supportedGenerationMethods"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect();

        let mut caps = Vec::new();
        if methods.contains(&"generateContent".to_string()) {
            caps.push("text".to_string());
        }

        let desc = m["description"].as_str().unwrap_or("");
        if desc.to_lowercase().contains("image")
            || desc.to_lowercase().contains("multimodal")
            || desc.to_lowercase().contains("vision")
        {
            caps.push("vision".to_string());
        }
        if desc.to_lowercase().contains("image generation")
            || id.contains("image")
            || id.contains("imagen")
        {
            caps.push("image_generation".to_string());
        }
        if id.contains("thinking") {
            caps.push("thinking".to_string());
        }

        if methods.contains(&"generateContent".to_string()) && id.starts_with("gemini") {
            models.push(ModelInfo {
                id,
                provider: "google".to_string(),
                display_name,
                capabilities: caps,
            });
        }
    }

    Ok(models)
}

async fn fetch_anthropic_models(
    client: &reqwest::Client,
    api_key: &str,
) -> Result<Vec<ModelInfo>, String> {
    let resp = client
        .get("https://api.anthropic.com/v1/models")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .send()
        .await
        .map_err(|e| format!("Anthropic models request failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Anthropic models API returned {}", resp.status()));
    }

    let body: Value = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse Anthropic models: {}", e))?;

    let data = body["data"].as_array().cloned().unwrap_or_default();

    let mut models = Vec::new();
    for m in data {
        let id = m["id"].as_str().unwrap_or("").to_string();
        let display_name = m["display_name"]
            .as_str()
            .or_else(|| m["name"].as_str())
            .map(|s| s.to_string());

        let mut caps = vec!["text".to_string(), "vision".to_string()];
        if id.contains("opus") {
            caps.push("advanced_reasoning".to_string());
        }

        if !id.is_empty() {
            models.push(ModelInfo {
                id,
                provider: "anthropic".to_string(),
                display_name,
                capabilities: caps,
            });
        }
    }

    Ok(models)
}

// ── Refresh cache ────────────────────────────────────────────────────────────

pub async fn refresh_cache(state: &AppState) -> HashMap<String, Vec<ModelInfo>> {
    let rt = state.runtime.read().await;
    let mut all_models: HashMap<String, Vec<ModelInfo>> = HashMap::new();

    if let Some(key) = rt.api_keys.get("google") {
        match fetch_google_models(&state.client, key).await {
            Ok(models) => {
                tracing::info!("model_registry: fetched {} Google models", models.len());
                all_models.insert("google".to_string(), models);
            }
            Err(e) => tracing::warn!("model_registry: Google fetch failed: {}", e),
        }
    }

    if let Some(key) = rt.api_keys.get("anthropic") {
        match fetch_anthropic_models(&state.client, key).await {
            Ok(models) => {
                tracing::info!("model_registry: fetched {} Anthropic models", models.len());
                all_models.insert("anthropic".to_string(), models);
            }
            Err(e) => tracing::warn!("model_registry: Anthropic fetch failed: {}", e),
        }
    }

    drop(rt);

    let mut cache = state.model_cache.write().await;
    cache.models = all_models.clone();
    cache.fetched_at = Some(Instant::now());

    all_models
}

// ── Model selection ──────────────────────────────────────────────────────────

fn version_key(id: &str) -> (u64, String) {
    let mut version: u64 = 0;
    let mut date_suffix = String::new();

    for part in id.split('-') {
        if let Some((major_s, minor_s)) = part.split_once('.') {
            if let (Ok(major), Ok(minor)) = (major_s.parse::<u64>(), minor_s.parse::<u64>()) {
                let v = major * 1000 + minor;
                if v > version {
                    version = v;
                }
            }
        } else if let Ok(n) = part.parse::<u64>() {
            if n > 20000000 {
                date_suffix = part.to_string();
            } else if n < 100 {
                let v = n * 1000;
                if v > version {
                    version = v;
                }
            }
        }
    }

    (version, date_suffix)
}

fn select_best(
    models: &[ModelInfo],
    must_contain: &[&str],
    must_not_contain: &[&str],
) -> Option<ModelInfo> {
    let mut candidates: Vec<&ModelInfo> = models
        .iter()
        .filter(|m| must_contain.iter().all(|p| m.id.contains(p)))
        .filter(|m| must_not_contain.iter().all(|p| !m.id.contains(p)))
        .collect();

    candidates.sort_by(|a, b| {
        let (av, ad) = version_key(&a.id);
        let (bv, bd) = version_key(&b.id);
        bv.cmp(&av).then_with(|| bd.cmp(&ad))
    });

    candidates.first().map(|m| (*m).clone())
}

pub async fn resolve_models(state: &AppState) -> ResolvedModels {
    {
        let cache = state.model_cache.read().await;
        if cache.is_stale() {
            drop(cache);
            refresh_cache(state).await;
        }
    }

    let cache = state.model_cache.read().await;
    let google = cache.models.get("google").cloned().unwrap_or_default();

    // Chat: latest gemini pro (not image, not lite, not latest alias)
    let chat = select_best(
        &google,
        &["pro"],
        &["lite", "latest", "image", "tts", "computer", "robotics", "customtools", "thinking"],
    )
    .or_else(|| select_best(&google, &["pro"], &["lite", "latest", "thinking"]));

    // Thinking: highest version_key (Gemini 3+ has dynamic thinking built-in)
    let thinking = select_best(&google, &[], &["lite", "latest", "image", "tts", "computer", "robotics", "audio"])
        .or_else(|| chat.clone());

    // Image: gemini model with image generation capability
    let image = select_best(&google, &["image"], &["robotics", "computer"])
        .or_else(|| chat.clone());

    ResolvedModels {
        chat,
        thinking,
        image,
    }
}

/// Get the model ID for a given use case.
/// Priority: 1) DB pin  2) dynamic auto-selection  3) hardcoded fallback.
pub async fn get_model_id(state: &AppState, use_case: &str) -> String {
    // 1) Check for a pinned model in DB
    let pinned: Option<String> = sqlx::query_scalar(
        "SELECT model_id FROM gh_model_pins WHERE use_case = $1",
    )
    .bind(use_case)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    if let Some(ref pin) = pinned {
        tracing::info!("model_registry: use_case={} → model={} (pinned)", use_case, pin);
        return pin.clone();
    }

    // 2) Dynamic auto-selection
    let resolved = resolve_models(state).await;

    let (model, fallback) = match use_case {
        "chat" => (resolved.chat, "gemini-3.1-pro-preview"),
        "thinking" => (resolved.thinking, "gemini-3.1-pro-preview"),
        "image" => (resolved.image, "gemini-3-pro-image-preview"),
        _ => (resolved.chat, "gemini-3.1-pro-preview"),
    };

    let id = model.as_ref().map(|m| m.id.as_str()).unwrap_or(fallback);

    tracing::info!(
        "model_registry: use_case={} → model={}{}",
        use_case,
        id,
        if model.is_some() { " (auto)" } else { " (fallback)" }
    );

    id.to_string()
}

// ── HTTP handlers ────────────────────────────────────────────────────────────

async fn get_pins_map(state: &AppState) -> HashMap<String, String> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT use_case, model_id FROM gh_model_pins",
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    rows.into_iter().collect()
}

// ── Startup sync ─────────────────────────────────────────────────────────────

/// Called once at startup: fetch models from API, pick the best chat model,
/// and persist it as `default_model` in `gh_settings`.
pub async fn startup_sync(state: &AppState) {
    tracing::info!("model_registry: fetching models at startup…");

    let models = refresh_cache(state).await;
    let total: usize = models.values().map(|v| v.len()).sum();
    tracing::info!("model_registry: {} models cached from {} providers", total, models.len());

    let resolved = resolve_models(state).await;

    if let Some(ref best) = resolved.chat {
        tracing::info!("model_registry: best chat model → {}", best.id);

        // Persist into gh_settings so the UI and handlers pick it up immediately
        let res = sqlx::query(
            "UPDATE gh_settings SET default_model = $1, updated_at = NOW() WHERE id = 1",
        )
        .bind(&best.id)
        .execute(&state.db)
        .await;

        match res {
            Ok(_) => tracing::info!("model_registry: default_model updated to {}", best.id),
            Err(e) => tracing::warn!("model_registry: failed to update default_model: {}", e),
        }
    } else {
        tracing::warn!("model_registry: no chat model resolved — keeping DB default");
    }

    // Log all resolved use cases
    tracing::info!(
        "model_registry: resolved → chat={}, thinking={}, image={}",
        resolved.chat.as_ref().map(|m| m.id.as_str()).unwrap_or("(none)"),
        resolved.thinking.as_ref().map(|m| m.id.as_str()).unwrap_or("(none)"),
        resolved.image.as_ref().map(|m| m.id.as_str()).unwrap_or("(none)"),
    );
}

/// GET /api/models — Return all cached models + resolved selections + pins
pub async fn list_models(State(state): State<AppState>) -> Json<Value> {
    let resolved = resolve_models(&state).await;
    let pins = get_pins_map(&state).await;
    let cache = state.model_cache.read().await;

    let total: usize = cache.models.values().map(|v| v.len()).sum();
    let stale = cache.is_stale();
    let fetched_ago = cache.fetched_at.map(|t| t.elapsed().as_secs());

    Json(json!({
        "total_models": total,
        "cache_stale": stale,
        "cache_age_seconds": fetched_ago,
        "pins": pins,
        "selected": {
            "chat": resolved.chat,
            "thinking": resolved.thinking,
            "image": resolved.image,
        },
        "providers": {
            "google": cache.models.get("google").cloned().unwrap_or_default(),
            "anthropic": cache.models.get("anthropic").cloned().unwrap_or_default(),
        }
    }))
}

/// POST /api/models/refresh — Force refresh of model cache
pub async fn refresh_models(State(state): State<AppState>) -> Json<Value> {
    let models = refresh_cache(&state).await;
    let resolved = resolve_models(&state).await;
    let pins = get_pins_map(&state).await;

    let total: usize = models.values().map(|v| v.len()).sum();

    Json(json!({
        "refreshed": true,
        "total_models": total,
        "pins": pins,
        "selected": {
            "chat": resolved.chat,
            "thinking": resolved.thinking,
            "image": resolved.image,
        }
    }))
}

/// POST /api/models/pin — Pin a specific model to a use case
pub async fn pin_model(
    State(state): State<AppState>,
    Json(body): Json<PinModelRequest>,
) -> Json<Value> {
    let valid = ["chat", "thinking", "image"];

    if !valid.contains(&body.use_case.as_str()) {
        return Json(json!({ "error": format!("Invalid use_case '{}'. Valid: {:?}", body.use_case, valid) }));
    }

    let result = sqlx::query(
        "INSERT INTO gh_model_pins (use_case, model_id) \
         VALUES ($1, $2) \
         ON CONFLICT (use_case) DO UPDATE SET model_id = $2, pinned_at = now()",
    )
    .bind(&body.use_case)
    .bind(&body.model_id)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => {
            tracing::info!("model_registry: pinned use_case={} → model={}", body.use_case, body.model_id);
            Json(json!({ "pinned": true, "use_case": body.use_case, "model_id": body.model_id }))
        }
        Err(e) => Json(json!({ "error": format!("Failed to pin: {}", e) })),
    }
}

/// DELETE /api/models/pin/{use_case} — Unpin a use case
pub async fn unpin_model(
    State(state): State<AppState>,
    Path(use_case): Path<String>,
) -> Json<Value> {
    let result = sqlx::query("DELETE FROM gh_model_pins WHERE use_case = $1")
        .bind(&use_case)
        .execute(&state.db)
        .await;

    match result {
        Ok(r) => Json(json!({ "unpinned": r.rows_affected() > 0, "use_case": use_case })),
        Err(e) => Json(json!({ "error": format!("Failed to unpin: {}", e) })),
    }
}

/// GET /api/models/pins — List all active pins
pub async fn list_pins(State(state): State<AppState>) -> Json<Value> {
    let pins = get_pins_map(&state).await;
    Json(json!({ "pins": pins }))
}
