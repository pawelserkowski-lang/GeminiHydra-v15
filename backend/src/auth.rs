// Jaskier Shared Pattern -- auth
// Optional Bearer token authentication middleware.
// If AUTH_SECRET env is set, all protected routes require
// `Authorization: Bearer <secret>`. If not set, auth is disabled (dev mode).

use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};

use crate::state::AppState;

/// Middleware that enforces Bearer token auth when AUTH_SECRET is configured.
/// Public routes (health, readiness, auth/*) should NOT use this middleware.
pub async fn require_auth(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let secret = match state.auth_secret.as_deref() {
        Some(s) => s,
        None => return Ok(next.run(request).await), // Dev mode — no auth required
    };

    let auth_header = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok());

    match auth_header {
        Some(header) if header.starts_with("Bearer ") => {
            let token = &header[7..];
            if token == secret {
                Ok(next.run(request).await)
            } else {
                tracing::warn!("Auth failed: invalid token");
                Err(StatusCode::UNAUTHORIZED)
            }
        }
        _ => {
            tracing::warn!("Auth failed: missing or malformed Authorization header");
            Err(StatusCode::UNAUTHORIZED)
        }
    }
}

/// Validate auth for WebSocket upgrade requests.
/// Checks `?token=<secret>` query parameter since WebSocket doesn't support
/// custom headers during the upgrade handshake.
pub fn validate_ws_token(query: &str, auth_secret: Option<&str>) -> bool {
    let secret = match auth_secret {
        Some(s) => s,
        None => return true, // Dev mode — no auth
    };

    // Parse ?token=xxx from query string
    query
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .any(|(key, value)| key == "token" && value == secret)
}
