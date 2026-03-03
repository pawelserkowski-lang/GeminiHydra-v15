// ---------------------------------------------------------------------------
// handlers/ — HTTP request handlers
// Sub-modules for logical grouping; mod.rs re-exports all public items
// so that `crate::handlers::*` paths remain unchanged.
// ---------------------------------------------------------------------------

use crate::auth;
use crate::state::AppState;
use axum::{
    Router, middleware,
    routing::{get, post},
};

pub(crate) mod agents;
pub(crate) mod execute;
pub(crate) mod files_handlers;
pub(crate) mod streaming;
pub(crate) mod system;
#[cfg(test)]
mod tests;

// ── Router Factories ────────────────────────────────────────────────────────

pub fn agents_router(state: AppState) -> Router<AppState> {
    Router::new()
        .route(
            "/api/agents",
            get(agents::list_agents).post(agents::create_agent),
        )
        .route("/api/agents/classify", post(agents::classify_agent))
        .route(
            "/api/agents/{id}",
            post(agents::update_agent).delete(agents::delete_agent),
        )
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth::require_auth,
        ))
}

pub fn system_router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/api/system/stats", get(system::system_stats))
        .route("/api/admin/rotate-key", post(system::rotate_key))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth::require_auth,
        ))
}

pub fn files_router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/api/files/read", post(files_handlers::read_file))
        .route("/api/files/list", post(files_handlers::list_files))
        .route("/api/files/browse", post(files_handlers::browse_directory))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth::require_auth,
        ))
}

// ── Re-exports (backward-compatible) ─────────────────────────────────────────

pub use agents::{classify_agent, create_agent, delete_agent, list_agents, update_agent};
pub use execute::{execute, internal_tool_execute};
pub use files_handlers::{browse_directory, list_files, read_file};
pub use streaming::ws_execute;
pub use system::{
    auth_mode, gemini_models, health, health_detailed, readiness, rotate_key, system_stats,
};

// ── utoipa __path_* re-exports ───────────────────────────────────────────────
pub use agents::{
    __path_classify_agent, __path_create_agent, __path_delete_agent, __path_list_agents,
    __path_update_agent,
};
pub use execute::__path_execute;
pub use files_handlers::{__path_list_files, __path_read_file};
pub use system::{
    __path_auth_mode, __path_gemini_models, __path_health, __path_health_detailed,
    __path_readiness, __path_system_stats,
};

pub use crate::error::{ApiError, ApiErrorWithDetails, StructuredApiError};
use crate::models::ProviderInfo;
use serde_json::Value;
use std::collections::HashMap;

/// Extract diagnostic info from a Gemini API response that's missing expected parts.
pub(crate) fn gemini_diagnose(resp_json: &Value) -> String {
    let mut diag = Vec::new();

    if let Some(feedback) = resp_json.get("promptFeedback") {
        if let Some(reason) = feedback.get("blockReason").and_then(|v| v.as_str()) {
            diag.push(format!("promptFeedback.blockReason={}", reason));
        }
        if let Some(ratings) = feedback.get("safetyRatings").and_then(|v| v.as_array()) {
            for r in ratings {
                if let (Some(cat), Some(prob)) = (
                    r.get("category").and_then(|v| v.as_str()),
                    r.get("probability").and_then(|v| v.as_str()),
                ) && prob != "NEGLIGIBLE"
                    && prob != "LOW"
                {
                    diag.push(format!("safety: {}={}", cat, prob));
                }
            }
        }
    }

    if let Some(candidates) = resp_json.get("candidates").and_then(|v| v.as_array()) {
        if candidates.is_empty() {
            diag.push("candidates array is empty".to_string());
        } else if let Some(c0) = candidates.first() {
            if let Some(reason) = c0.get("finishReason").and_then(|v| v.as_str()) {
                diag.push(format!("finishReason={}", reason));
            }
            if c0.get("content").is_none() {
                diag.push("candidate has no 'content' field".to_string());
            }
        }
    } else {
        diag.push("no 'candidates' field in response".to_string());
    }

    if diag.is_empty() {
        "unknown (response structure unrecognized)".to_string()
    } else {
        diag.join(", ")
    }
}

pub(crate) fn build_providers(
    api_keys: &HashMap<String, String>,
    cached_google: &[crate::model_registry::ModelInfo],
) -> Vec<ProviderInfo> {
    let google_available = api_keys.get("google").is_some_and(|k| !k.is_empty());

    let mut providers = Vec::new();

    for m in cached_google {
        providers.push(ProviderInfo {
            name: format!("Google {}", m.display_name.as_deref().unwrap_or(&m.id)),
            available: google_available,
            model: Some(m.id.clone()),
        });
    }

    providers.push(ProviderInfo {
        name: "Anthropic Claude".to_string(),
        available: api_keys.get("anthropic").is_some_and(|k| !k.is_empty()),
        model: Some("claude-sonnet-4-6".to_string()),
    });

    providers
}
