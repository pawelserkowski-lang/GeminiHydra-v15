// ---------------------------------------------------------------------------
// handlers/agents.rs — Agent CRUD + classification endpoints
// ---------------------------------------------------------------------------

use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;
use serde_json::{json, Value};

use crate::models::{ClassifyRequest, ClassifyResponse, WitcherAgent};
use crate::state::AppState;

use super::classify_prompt;

// ---------------------------------------------------------------------------
// Agent List & Classification
// ---------------------------------------------------------------------------

#[utoipa::path(get, path = "/api/agents", tag = "agents",
    responses((status = 200, description = "List of configured agents", body = Value))
)]
pub async fn list_agents(State(state): State<AppState>) -> impl IntoResponse {
    let agents = state.agents.read().await;
    // #6 — Cache agent list for 60 seconds
    (
        [(axum::http::header::CACHE_CONTROL, "public, max-age=60")],
        Json(json!({ "agents": *agents })),
    )
}

#[utoipa::path(post, path = "/api/agents/classify", tag = "agents",
    request_body = ClassifyRequest,
    responses((status = 200, description = "Agent classification result", body = ClassifyResponse))
)]
pub async fn classify_agent(
    State(state): State<AppState>,
    Json(body): Json<ClassifyRequest>,
) -> Json<ClassifyResponse> {
    let agents = state.agents.read().await;
    let (agent_id, confidence, reasoning) = classify_prompt(&body.prompt, &agents);
    Json(ClassifyResponse { agent: agent_id, confidence, reasoning })
}

// ---------------------------------------------------------------------------
// Agent CRUD
// ---------------------------------------------------------------------------

#[utoipa::path(post, path = "/api/agents", tag = "agents",
    request_body = WitcherAgent,
    responses((status = 200, description = "Agent created", body = Value))
)]
pub async fn create_agent(
    State(state): State<AppState>,
    Json(agent): Json<WitcherAgent>,
) -> Json<Value> {
    let _ = sqlx::query(
        "INSERT INTO gh_agents (id, name, role, tier, status, description, system_prompt, keywords, temperature) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)"
    )
    .bind(&agent.id)
    .bind(&agent.name)
    .bind(&agent.role)
    .bind(&agent.tier)
    .bind(&agent.status)
    .bind(&agent.description)
    .bind(&agent.system_prompt)
    .bind(&agent.keywords)
    .bind(agent.temperature)
    .execute(&state.db)
    .await;

    state.refresh_agents().await;
    Json(json!({ "success": true }))
}

#[utoipa::path(post, path = "/api/agents/{id}", tag = "agents",
    params(("id" = String, Path, description = "Agent ID")),
    request_body = WitcherAgent,
    responses((status = 200, description = "Agent updated", body = Value))
)]
pub async fn update_agent(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
    Json(agent): Json<WitcherAgent>,
) -> Json<Value> {
    let _ = sqlx::query(
        "UPDATE gh_agents SET name=$1, role=$2, tier=$3, status=$4, description=$5, system_prompt=$6, keywords=$7, temperature=$8, updated_at=NOW() \
         WHERE id=$9"
    )
    .bind(&agent.name)
    .bind(&agent.role)
    .bind(&agent.tier)
    .bind(&agent.status)
    .bind(&agent.description)
    .bind(&agent.system_prompt)
    .bind(&agent.keywords)
    .bind(agent.temperature)
    .bind(id)
    .execute(&state.db)
    .await;

    state.refresh_agents().await;
    Json(json!({ "success": true }))
}

#[utoipa::path(delete, path = "/api/agents/{id}", tag = "agents",
    params(("id" = String, Path, description = "Agent ID")),
    responses((status = 200, description = "Agent deleted", body = Value))
)]
pub async fn delete_agent(
    State(state): State<AppState>,
    axum::extract::ConnectInfo(addr): axum::extract::ConnectInfo<std::net::SocketAddr>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Json<Value> {
    let _ = sqlx::query("DELETE FROM gh_agents WHERE id=$1").bind(&id).execute(&state.db).await;
    state.refresh_agents().await;

    crate::audit::log_audit(
        &state.db,
        "delete_agent",
        json!({ "agent_id": id }),
        Some(&addr.ip().to_string()),
    )
    .await;

    Json(json!({ "success": true }))
}
