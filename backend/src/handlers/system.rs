// ---------------------------------------------------------------------------
// handlers/system.rs — Health, readiness, system stats, auth mode, models, admin
// ---------------------------------------------------------------------------

use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;
use serde_json::{json, Value};

use crate::models::{
    DetailedHealthResponse, GeminiModelInfo, GeminiModelsResponse, HealthResponse, SystemStats,
};
use crate::state::AppState;

use super::{build_providers, ApiError};

// ---------------------------------------------------------------------------
// Health Endpoints
// ---------------------------------------------------------------------------

#[utoipa::path(get, path = "/api/health", tag = "health",
    responses((status = 200, description = "Health check with provider status", body = HealthResponse))
)]
pub async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    let rt = state.runtime.read().await;
    let cache = state.model_cache.read().await;
    let google = cache.models.get("google").cloned().unwrap_or_default();
    drop(cache);
    Json(HealthResponse {
        status: if state.is_ready() { "ok" } else { "starting" }.to_string(),
        version: "15.0.0".to_string(),
        app: "GeminiHydra".to_string(),
        uptime_seconds: state.start_time.elapsed().as_secs(),
        providers: build_providers(&rt.api_keys, &google),
    })
}

/// GET /api/health/ready — lightweight readiness probe (no locks, no DB).
#[utoipa::path(get, path = "/api/health/ready", tag = "health",
    responses(
        (status = 200, description = "Service ready", body = Value),
        (status = 503, description = "Service not ready", body = Value)
    )
)]
pub async fn readiness(State(state): State<AppState>) -> axum::response::Response {
    use axum::http::StatusCode;

    let ready = state.is_ready();
    let uptime = state.start_time.elapsed().as_secs();
    let body = json!({ "ready": ready, "uptime_seconds": uptime });

    if ready {
        (StatusCode::OK, Json(body)).into_response()
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, Json(body)).into_response()
    }
}

/// GET /api/auth/mode — returns whether auth is required (public endpoint).
#[utoipa::path(get, path = "/api/auth/mode", tag = "auth",
    responses((status = 200, description = "Auth mode info", body = Value))
)]
pub async fn auth_mode(State(state): State<AppState>) -> Json<Value> {
    Json(json!({
        "auth_required": state.auth_secret.is_some()
    }))
}

#[utoipa::path(get, path = "/api/health/detailed", tag = "health",
    responses((status = 200, description = "Detailed health with system metrics", body = DetailedHealthResponse))
)]
pub async fn health_detailed(State(state): State<AppState>) -> Json<DetailedHealthResponse> {
    let rt = state.runtime.read().await;
    let cache = state.model_cache.read().await;
    let google = cache.models.get("google").cloned().unwrap_or_default();
    drop(cache);
    let snap = state.system_monitor.read().await;

    Json(DetailedHealthResponse {
        status: "ok".to_string(),
        version: "15.0.0".to_string(),
        app: "GeminiHydra".to_string(),
        uptime_seconds: state.start_time.elapsed().as_secs(),
        providers: build_providers(&rt.api_keys, &google),
        memory_usage_mb: snap.memory_used_mb,
        cpu_usage_percent: snap.cpu_usage_percent,
        platform: snap.platform.clone(),
    })
}

#[utoipa::path(get, path = "/api/system/stats", tag = "system",
    responses((status = 200, description = "System resource usage", body = SystemStats))
)]
pub async fn system_stats(State(state): State<AppState>) -> Json<SystemStats> {
    let snap = state.system_monitor.read().await;
    Json(SystemStats {
        cpu_usage_percent: snap.cpu_usage_percent,
        memory_used_mb: snap.memory_used_mb,
        memory_total_mb: snap.memory_total_mb,
        platform: snap.platform.clone(),
    })
}

// ---------------------------------------------------------------------------
// Gemini Models
// ---------------------------------------------------------------------------

#[utoipa::path(get, path = "/api/gemini/models", tag = "models",
    responses((status = 200, description = "Available Gemini models", body = GeminiModelsResponse))
)]
pub async fn gemini_models(State(state): State<AppState>) -> Json<Value> {
    let mut models = Vec::new();

    // 1. Fetch Gemini models
    let google_cred = crate::oauth::get_google_credential(&state).await;
    if let Some((key, is_oauth)) = google_cred {
        let url = "https://generativelanguage.googleapis.com/v1beta/models";
        if let Ok(parsed) = reqwest::Url::parse(url) && let Ok(res) = crate::oauth::apply_google_auth(state.client.get(parsed), &key, is_oauth).send().await
            && res.status().is_success()
                && let Ok(body) = res.json::<Value>().await
                    && let Some(list) = body["models"].as_array() {
                        models.extend(list.iter().filter_map(|m| {
                            let info: GeminiModelInfo = serde_json::from_value(m.clone()).ok()?;
                            if info.supported_generation_methods.contains(&"generateContent".to_string()) {
                                Some(info)
                            } else {
                                None
                            }
                        }));
                    }
    }

    Json(json!(GeminiModelsResponse { models }))
}

// ---------------------------------------------------------------------------
// Admin — Key Rotation
// ---------------------------------------------------------------------------

/// Hot-reload an API key for a provider without restarting the backend.
/// Protected — requires auth when AUTH_SECRET is set.
pub async fn rotate_key(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let provider = body
        .get("provider")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ApiError::BadRequest("missing 'provider' field".into()))?;
    let key = body
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ApiError::BadRequest("missing 'key' field".into()))?;

    match provider {
        "google" | "anthropic" => {}
        _ => {
            return Err(ApiError::BadRequest(format!(
                "unknown provider '{}' — expected google or anthropic",
                provider
            )));
        }
    }

    let mut rt = state.runtime.write().await;
    rt.api_keys
        .insert(provider.to_string(), key.to_string());
    drop(rt);

    tracing::info!("API key rotated for provider '{}'", provider);

    Ok(Json(json!({
        "ok": true,
        "provider": provider,
        "message": format!("API key for '{}' updated successfully", provider),
    })))
}
