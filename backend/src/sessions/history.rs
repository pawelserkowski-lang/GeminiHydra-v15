//! Prompt history handlers: list, add (dedup + cap), and clear.

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde_json::{json, Value};

use crate::models::{AddPromptRequest, PromptHistoryRow};
use crate::state::AppState;

// ============================================================================
// Prompt History handlers
// ============================================================================

const MAX_PROMPT_HISTORY: i64 = 200;

/// GET /api/prompt-history — list all prompts (oldest first).
#[utoipa::path(get, path = "/api/prompt-history", tag = "prompt-history",
    responses((status = 200, description = "List of prompt strings", body = Vec<String>))
)]
pub async fn list_prompt_history(
    State(state): State<AppState>,
) -> Result<Json<Value>, StatusCode> {
    let rows = sqlx::query_as::<_, PromptHistoryRow>(
        "SELECT id, content, created_at FROM gh_prompt_history ORDER BY created_at ASC LIMIT 500",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| {
        tracing::error!("Failed to list prompt history: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let prompts: Vec<String> = rows.into_iter().map(|r| r.content).collect();
    Ok(Json(json!(prompts)))
}

/// POST /api/prompt-history — add a prompt (dedup + cap).
#[utoipa::path(post, path = "/api/prompt-history", tag = "prompt-history",
    request_body = AddPromptRequest,
    responses((status = 201, description = "Prompt saved"))
)]
pub async fn add_prompt_history(
    State(state): State<AppState>,
    Json(body): Json<AddPromptRequest>,
) -> Result<StatusCode, StatusCode> {
    let trimmed = body.content.trim();
    if trimmed.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Deduplicate: skip if last entry is identical
    let last: Option<String> = sqlx::query_scalar(
        "SELECT content FROM gh_prompt_history ORDER BY created_at DESC LIMIT 1",
    )
    .fetch_optional(&state.db)
    .await
    .map_err(|e| {
        tracing::error!("Failed to check last prompt: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if let Some(ref last_content) = last {
        if last_content == trimmed {
            return Ok(StatusCode::OK);
        }
    }

    // Insert new prompt
    sqlx::query("INSERT INTO gh_prompt_history (content) VALUES ($1)")
        .bind(trimmed)
        .execute(&state.db)
        .await
        .map_err(|e| {
            tracing::error!("Failed to insert prompt: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Cap at MAX_PROMPT_HISTORY — delete oldest beyond limit
    sqlx::query(
        "DELETE FROM gh_prompt_history WHERE id NOT IN \
         (SELECT id FROM gh_prompt_history ORDER BY created_at DESC LIMIT $1)",
    )
    .bind(MAX_PROMPT_HISTORY)
    .execute(&state.db)
    .await
    .map_err(|e| {
        tracing::error!("Failed to cap prompt history: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(StatusCode::CREATED)
}

/// DELETE /api/prompt-history — clear all prompt history.
#[utoipa::path(delete, path = "/api/prompt-history", tag = "prompt-history",
    responses((status = 200, description = "Prompt history cleared"))
)]
pub async fn clear_prompt_history(
    State(state): State<AppState>,
) -> Result<Json<Value>, StatusCode> {
    sqlx::query("DELETE FROM gh_prompt_history")
        .execute(&state.db)
        .await
        .map_err(|e| {
            tracing::error!("Failed to clear prompt history: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(json!({ "cleared": true })))
}
