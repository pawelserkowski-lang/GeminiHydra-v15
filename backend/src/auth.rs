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
use subtle::ConstantTimeEq;

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
            if bool::from(token.as_bytes().ct_eq(secret.as_bytes())) {
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
        .any(|(key, value)| key == "token" && bool::from(value.as_bytes().ct_eq(secret.as_bytes())))
}

/// Pure function: extract and validate a Bearer token from an Authorization header value.
/// Returns true if the token matches the expected secret.
/// Used internally by `require_auth` middleware.
pub fn check_bearer_token(header_value: Option<&str>, expected_secret: &str) -> bool {
    match header_value {
        Some(header) if header.starts_with("Bearer ") => {
            bool::from(header[7..].as_bytes().ct_eq(expected_secret.as_bytes()))
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── validate_ws_token ────────────────────────────────────────────────

    #[test]
    fn ws_token_valid() {
        assert!(validate_ws_token("token=mysecret", Some("mysecret")));
    }

    #[test]
    fn ws_token_invalid() {
        assert!(!validate_ws_token("token=wrong", Some("mysecret")));
    }

    #[test]
    fn ws_token_none_allows_all() {
        assert!(validate_ws_token("", None));
    }

    #[test]
    fn ws_token_none_allows_any_query() {
        assert!(validate_ws_token("token=anything&foo=bar", None));
    }

    #[test]
    fn ws_token_missing_param() {
        assert!(!validate_ws_token("foo=bar&baz=qux", Some("mysecret")));
    }

    #[test]
    fn ws_token_empty_query_with_secret() {
        assert!(!validate_ws_token("", Some("mysecret")));
    }

    #[test]
    fn ws_token_multiple_params() {
        assert!(validate_ws_token("session=abc&token=s3cret&lang=en", Some("s3cret")));
    }

    #[test]
    fn ws_token_duplicate_token_first_wrong() {
        // If there are two "token" params, any match should succeed
        assert!(validate_ws_token("token=wrong&token=correct", Some("correct")));
    }

    #[test]
    fn ws_token_case_sensitive() {
        assert!(!validate_ws_token("token=MySecret", Some("mysecret")));
    }

    // ── check_bearer_token ───────────────────────────────────────────────

    #[test]
    fn bearer_valid_token() {
        assert!(check_bearer_token(Some("Bearer mysecret"), "mysecret"));
    }

    #[test]
    fn bearer_wrong_token() {
        assert!(!check_bearer_token(Some("Bearer wrong"), "mysecret"));
    }

    #[test]
    fn bearer_missing_header() {
        assert!(!check_bearer_token(None, "mysecret"));
    }

    #[test]
    fn bearer_malformed_no_prefix() {
        assert!(!check_bearer_token(Some("mysecret"), "mysecret"));
    }

    #[test]
    fn bearer_basic_auth_rejected() {
        assert!(!check_bearer_token(Some("Basic not-a-bearer-token"), "mysecret"));
    }

    #[test]
    fn bearer_empty_token() {
        assert!(!check_bearer_token(Some("Bearer "), "mysecret"));
    }

    #[test]
    fn bearer_extra_spaces_rejected() {
        // "Bearer  mysecret" (double space) — token starts with space
        assert!(!check_bearer_token(Some("Bearer  mysecret"), "mysecret"));
    }

    #[test]
    fn bearer_case_sensitive() {
        assert!(!check_bearer_token(Some("bearer mysecret"), "mysecret"));
    }
}
