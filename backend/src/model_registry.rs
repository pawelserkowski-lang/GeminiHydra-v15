// Jaskier Shared Pattern — model_registry
//
// GeminiHydra v15 — Dynamic Model Registry
// Fetches available models from Google and Anthropic APIs,
// caches them with a TTL, and selects the latest model for each use case.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use axum::extract::{Path, State};
use axum::http::header;
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use utoipa::ToSchema;

use crate::state::AppState;

// --- Jaskier Shared Core Types ---

// ── Cache TTL ────────────────────────────────────────────────────────────────

const CACHE_TTL: Duration = Duration::from_secs(3600); // 1 hour

// ── Model info ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ModelInfo {
    pub id: String,
    pub provider: String,
    pub display_name: Option<String>,
    pub capabilities: Vec<String>,
}

// --- Project-Specific Types ---

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ResolvedModels {
    pub chat: Option<ModelInfo>,
    pub thinking: Option<ModelInfo>,
    pub image: Option<ModelInfo>,
}

// ── Pin request ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct PinModelRequest {
    pub use_case: String,
    pub model_id: String,
}

// ── Model cache ──────────────────────────────────────────────────────────────

pub struct ModelCache {
    pub models: HashMap<String, Vec<ModelInfo>>,
    pub fetched_at: Option<Instant>,
}

impl Default for ModelCache {
    fn default() -> Self {
        Self::new()
    }
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
    is_oauth: bool,
) -> Result<Vec<ModelInfo>, String> {
    let url = "https://generativelanguage.googleapis.com/v1beta/models";

    let parsed_url = reqwest::Url::parse(url)
        .map_err(|e| format!("Invalid URL: {}", e))?;

    let resp = crate::oauth::apply_google_auth(client.get(parsed_url), api_key, is_oauth)
        .send()
        .await
        .map_err(|e| format!("Google models request failed: {:?}", e))?;

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
    let mut all_models: HashMap<String, Vec<ModelInfo>> = HashMap::new();

    if let Some((key, is_oauth)) = crate::oauth::get_google_credential(state).await {
        match fetch_google_models(&state.client, &key, is_oauth).await {
            Ok(models) => {
                tracing::info!("model_registry: fetched {} Google models", models.len());
                all_models.insert("google".to_string(), models);
            }
            Err(e) => tracing::warn!("model_registry: Google fetch failed: {}", e),
        }
    }

    {
        let rt = state.runtime.read().await;
        if let Some(key) = rt.api_keys.get("anthropic") {
            match fetch_anthropic_models(&state.client, key).await {
                Ok(models) => {
                    tracing::info!("model_registry: fetched {} Anthropic models", models.len());
                    all_models.insert("anthropic".to_string(), models);
                }
                Err(e) => tracing::warn!("model_registry: Anthropic fetch failed: {}", e),
            }
        }
    }

    let mut cache = state.model_cache.write().await;
    cache.models = all_models.clone();
    cache.fetched_at = Some(Instant::now());

    all_models
}

// ── Model selection ──────────────────────────────────────────────────────────

/// Extract a sortable version key from a model ID.
/// Handles patterns like "gemini-2.5-flash", "gemini-3.1-pro", "claude-sonnet-4-6".
/// Returns (major * 1000 + minor, date_suffix) for proper ordering.
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

/// Select the best model from a list using include/exclude filters.
/// Sorts by extracted version key (highest = newest).
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

/// Resolve the best model for each use case from the cached models.
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

    // Chat: prefer customtools variant (optimized for agent tool-calling workloads)
    let chat = select_best(
        &google,
        &["pro", "customtools"],
        &["lite", "latest", "image", "tts", "computer", "robotics", "thinking"],
    )
    .or_else(|| select_best(&google, &["pro"], &["lite", "latest", "image", "tts", "computer", "robotics", "thinking"]));

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
        "chat" => (resolved.chat, "gemini-3.1-pro-preview-customtools"),
        "thinking" => (resolved.thinking, "gemini-3.1-pro-preview-customtools"),
        "image" => (resolved.image, "gemini-3-pro-image-preview"),
        _ => (resolved.chat, "gemini-3.1-pro-preview-customtools"),
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

/// Read all pins from DB as a HashMap.
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

// --- Shared Handlers ---

/// GET /api/models — Return all cached models + resolved selections + pins
#[utoipa::path(get, path = "/api/models", tag = "models",
    responses((status = 200, description = "Cached models, resolved selections, and pins", body = Value))
)]
pub async fn list_models(State(state): State<AppState>) -> impl IntoResponse {
    let resolved = resolve_models(&state).await;
    let pins = get_pins_map(&state).await;
    let cache = state.model_cache.read().await;

    let total: usize = cache.models.values().map(|v| v.len()).sum();
    let stale = cache.is_stale();
    let fetched_ago = cache.fetched_at.map(|t| t.elapsed().as_secs());

    let body = Json(json!({
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
    }));

    // #6 — Cache static model list for 60 seconds
    ([(header::CACHE_CONTROL, "public, max-age=60")], body)
}

/// POST /api/models/refresh — Force refresh of model cache
#[utoipa::path(post, path = "/api/models/refresh", tag = "models",
    responses((status = 200, description = "Refreshed model cache", body = Value))
)]
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
#[utoipa::path(post, path = "/api/models/pin", tag = "models",
    request_body = PinModelRequest,
    responses((status = 200, description = "Model pinned", body = Value))
)]
pub async fn pin_model(
    State(state): State<AppState>,
    axum::extract::ConnectInfo(addr): axum::extract::ConnectInfo<std::net::SocketAddr>,
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

            crate::audit::log_audit(
                &state.db,
                "pin_model",
                json!({ "use_case": body.use_case, "model_id": body.model_id }),
                Some(&addr.ip().to_string()),
            )
            .await;

            Json(json!({ "pinned": true, "use_case": body.use_case, "model_id": body.model_id }))
        }
        Err(e) => Json(json!({ "error": format!("Failed to pin: {}", e) })),
    }
}

/// DELETE /api/models/pin/{use_case} — Unpin a use case
#[utoipa::path(delete, path = "/api/models/pin/{use_case}", tag = "models",
    params(("use_case" = String, Path, description = "Use case to unpin")),
    responses((status = 200, description = "Model unpinned", body = Value))
)]
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
#[utoipa::path(get, path = "/api/models/pins", tag = "models",
    responses((status = 200, description = "All active model pins", body = Value))
)]
pub async fn list_pins(State(state): State<AppState>) -> Json<Value> {
    let pins = get_pins_map(&state).await;
    Json(json!({ "pins": pins }))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Helper: make a ModelInfo ──────────────────────────────────────────

    fn model(id: &str, provider: &str) -> ModelInfo {
        ModelInfo {
            id: id.to_string(),
            provider: provider.to_string(),
            display_name: None,
            capabilities: vec!["text".to_string()],
        }
    }

    // ── version_key ──────────────────────────────────────────────────────

    #[test]
    fn version_key_gemini_2_5_flash() {
        // "2.5" → major=2, minor=5 → 2*1000 + 5 = 2005
        let (v, d) = version_key("gemini-2.5-flash");
        assert_eq!(v, 2005);
        assert!(d.is_empty());
    }

    #[test]
    fn version_key_gemini_3_1_pro_preview() {
        let (v, _) = version_key("gemini-3.1-pro-preview");
        assert_eq!(v, 3001);
    }

    #[test]
    fn version_key_gemini_3_pro_image() {
        let (v, _) = version_key("gemini-3-pro-image-preview");
        assert_eq!(v, 3000);
    }

    #[test]
    fn version_key_claude_opus_4_6() {
        let (v, _) = version_key("claude-opus-4-6");
        assert_eq!(v, 6000); // 4 -> 4000, then 6 -> 6000 (takes max)
    }

    #[test]
    fn version_key_claude_sonnet_4_6() {
        let (v, _) = version_key("claude-sonnet-4-6");
        assert_eq!(v, 6000);
    }

    #[test]
    fn version_key_claude_haiku_with_date() {
        let (v, d) = version_key("claude-haiku-4-5-20251001");
        assert_eq!(v, 5000); // 4 -> 4000, 5 -> 5000
        assert_eq!(d, "20251001");
    }

    #[test]
    fn version_key_no_version_info() {
        let (v, d) = version_key("some-model-name");
        assert_eq!(v, 0);
        assert!(d.is_empty());
    }

    #[test]
    fn version_key_higher_version_wins() {
        let (v3, _) = version_key("gemini-3.1-pro-preview");
        let (v2, _) = version_key("gemini-2.5-pro");
        assert!(v3 > v2, "3.1 ({}) should be > 2.5 ({})", v3, v2);
    }

    // ── select_best ──────────────────────────────────────────────────────

    #[test]
    fn select_best_picks_highest_version() {
        let models = vec![
            model("gemini-2.5-flash", "google"),
            model("gemini-3.1-pro-preview", "google"),
            model("gemini-2.0-flash", "google"),
        ];

        let best = select_best(&models, &[], &[]);
        assert_eq!(best.unwrap().id, "gemini-3.1-pro-preview");
    }

    #[test]
    fn select_best_must_contain_filter() {
        let models = vec![
            model("gemini-3.1-pro-preview", "google"),
            model("gemini-3-flash-preview", "google"),
            model("gemini-2.5-flash", "google"),
        ];

        let best = select_best(&models, &["flash"], &[]);
        assert_eq!(best.unwrap().id, "gemini-3-flash-preview");
    }

    #[test]
    fn select_best_must_not_contain_filter() {
        let models = vec![
            model("gemini-3.1-pro-preview", "google"),
            model("gemini-3-pro-image-preview", "google"),
            model("gemini-2.5-pro", "google"),
        ];

        let best = select_best(&models, &["pro"], &["image"]);
        assert_eq!(best.unwrap().id, "gemini-3.1-pro-preview");
    }

    #[test]
    fn select_best_no_match_returns_none() {
        let models = vec![
            model("gemini-3.1-pro-preview", "google"),
        ];

        let best = select_best(&models, &["nonexistent"], &[]);
        assert!(best.is_none());
    }

    #[test]
    fn select_best_empty_list_returns_none() {
        let best = select_best(&[], &[], &[]);
        assert!(best.is_none());
    }

    #[test]
    fn select_best_excludes_lite_and_latest() {
        let models = vec![
            model("gemini-3.1-pro-latest", "google"),
            model("gemini-3-pro-lite", "google"),
            model("gemini-2.5-pro", "google"),
        ];

        let best = select_best(&models, &["pro"], &["lite", "latest"]);
        assert_eq!(best.unwrap().id, "gemini-2.5-pro");
    }

    #[test]
    fn select_best_image_model() {
        let models = vec![
            model("gemini-3-pro-image-preview", "google"),
            model("gemini-2.5-flash-image", "google"),
        ];

        let best = select_best(&models, &["image"], &[]);
        assert_eq!(best.unwrap().id, "gemini-3-pro-image-preview");
    }

    // ── ModelCache ───────────────────────────────────────────────────────

    #[test]
    fn model_cache_new_is_stale() {
        let cache = ModelCache::new();
        assert!(cache.is_stale());
    }

    #[test]
    fn model_cache_default_is_stale() {
        let cache = ModelCache::default();
        assert!(cache.is_stale());
    }

    #[test]
    fn model_cache_fresh_after_set() {
        let mut cache = ModelCache::new();
        cache.fetched_at = Some(std::time::Instant::now());
        assert!(!cache.is_stale());
    }

    #[test]
    fn model_cache_empty_models_by_default() {
        let cache = ModelCache::new();
        assert!(cache.models.is_empty());
    }

    // ── customtools model ───────────────────────────────────────────────

    #[test]
    fn version_key_gemini_3_1_pro_customtools() {
        let (v, _) = version_key("gemini-3.1-pro-preview-customtools");
        assert_eq!(v, 3001);
    }

    #[test]
    fn select_best_prefers_customtools() {
        let models = vec![
            model("gemini-3.1-pro-preview", "google"),
            model("gemini-3.1-pro-preview-customtools", "google"),
        ];

        let best = select_best(&models, &["pro", "customtools"], &[]);
        assert_eq!(best.unwrap().id, "gemini-3.1-pro-preview-customtools");
    }

    #[test]
    fn select_best_falls_back_to_standard_pro_when_no_customtools() {
        let models = vec![
            model("gemini-3.1-pro-preview", "google"),
            model("gemini-3-flash-preview", "google"),
        ];

        // customtools filter matches nothing
        let best = select_best(&models, &["pro", "customtools"], &[]);
        assert!(best.is_none());

        // fallback to any pro
        let fallback = select_best(&models, &["pro"], &["lite", "latest", "image", "tts", "computer", "robotics", "thinking"]);
        assert_eq!(fallback.unwrap().id, "gemini-3.1-pro-preview");
    }
}
