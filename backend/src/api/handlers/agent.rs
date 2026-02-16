use axum::extract::State;
use axum::Json;
use serde_json::{json, Value};
use crate::models::{ClassifyRequest, ClassifyResponse};
use crate::state::AppState;
use crate::core::agent::classify_prompt;

pub async fn list_agents(State(state): State<AppState>) -> Json<Value> {
    Json(json!({ "agents": state.agents }))
}

pub async fn classify_agent(Json(body): Json<ClassifyRequest>) -> Json<ClassifyResponse> {
    let (agent_id, confidence, reasoning) = classify_prompt(&body.prompt);
    Json(ClassifyResponse { agent: agent_id, confidence, reasoning })
}
