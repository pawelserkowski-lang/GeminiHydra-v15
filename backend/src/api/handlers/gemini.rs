use axum::extract::State;
use axum::Json;
use serde_json::{json, Value};
use crate::models::{GeminiModelInfo, GeminiModelsResponse};
use crate::state::AppState;

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
