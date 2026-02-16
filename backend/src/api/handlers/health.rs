use axum::extract::State;
use axum::Json;
use sysinfo::System;
use std::collections::HashMap;

use crate::models::{HealthResponse, DetailedHealthResponse, ProviderInfo};
use crate::state::AppState;

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
