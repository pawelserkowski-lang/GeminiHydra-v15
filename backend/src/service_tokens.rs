// Jaskier Shared Pattern — Service Token Management
// Generic encrypted token storage for services like Fly.io.
// Reuses encrypt_token/decrypt_token from oauth.rs.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::oauth::{decrypt_token, encrypt_token};
use crate::state::AppState;

// ── DB row ───────────────────────────────────────────────────────────────

#[derive(sqlx::FromRow)]
struct ServiceTokenRow {
    service: String,
    encrypted_token: String,
}

// ═══════════════════════════════════════════════════════════════════════
//  Handlers (PROTECTED — behind auth middleware)
// ═══════════════════════════════════════════════════════════════════════

/// GET /api/tokens — list all stored service tokens (names only, not values)
pub async fn list_tokens(State(state): State<AppState>) -> Json<Value> {
    let rows = sqlx::query_as::<_, ServiceTokenRow>(concat!(
        "SELECT service, encrypted_token FROM ",
        "gh_service_tokens"
    ))
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let services: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "service": r.service,
                "configured": decrypt_token(&r.encrypted_token).is_ok(),
            })
        })
        .collect();

    Json(json!({ "tokens": services }))
}

#[derive(Deserialize)]
pub struct StoreTokenRequest {
    pub service: String,
    pub token: String,
}

/// POST /api/tokens — store or update a service token
pub async fn store_token(
    State(state): State<AppState>,
    Json(req): Json<StoreTokenRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    if req.service.is_empty() || req.token.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "service and token are required" })),
        ));
    }

    let encrypted = encrypt_token(&req.token);

    sqlx::query(concat!(
        "INSERT INTO ",
        "gh_service_tokens",
        " (service, encrypted_token, updated_at) ",
        "VALUES ($1, $2, NOW()) ",
        "ON CONFLICT (service) DO UPDATE SET ",
        "encrypted_token = $2, updated_at = NOW()"
    ))
    .bind(&req.service)
    .bind(&encrypted)
    .execute(&state.db)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to store token: {}", e) })),
        )
    })?;

    tracing::info!("Service token stored for: {}", req.service);

    Ok(Json(json!({
        "status": "ok",
        "service": req.service,
    })))
}

/// DELETE /api/tokens/{service} — delete a service token
pub async fn delete_token(
    State(state): State<AppState>,
    Path(service): Path<String>,
) -> Json<Value> {
    sqlx::query(concat!(
        "DELETE FROM ",
        "gh_service_tokens",
        " WHERE service = $1"
    ))
    .bind(&service)
    .execute(&state.db)
    .await
    .ok();

    tracing::info!("Service token deleted for: {}", service);
    Json(json!({ "status": "ok" }))
}

// ═══════════════════════════════════════════════════════════════════════
//  Token access (used by tools)
// ═══════════════════════════════════════════════════════════════════════

/// Get a decrypted service token by service name.
pub async fn get_service_token(state: &AppState, service: &str) -> Option<String> {
    let row = sqlx::query_as::<_, ServiceTokenRow>(concat!(
        "SELECT service, encrypted_token FROM ",
        "gh_service_tokens",
        " WHERE service = $1"
    ))
    .bind(service)
    .fetch_optional(&state.db)
    .await
    .ok()??;

    decrypt_token(&row.encrypted_token).ok()
}
