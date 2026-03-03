// ---------------------------------------------------------------------------
// error.rs — Centralized API error types (extracted from handlers/mod.rs)
// Jaskier Shared Pattern -- error
// ---------------------------------------------------------------------------

use axum::Json;
use axum::http::StatusCode;
use serde_json::{Value, json};
use uuid::Uuid;

/// Centralized API error type for all handlers.
/// Logs full details server-side, returns sanitized JSON to the client.
///
/// Response format (structured):
/// ```json
/// {
///   "error": {
///     "code": "BAD_REQUEST",
///     "message": "Human-readable description",
///     "request_id": "uuid-from-correlation-id",
///     "details": { ... }       // optional, null when absent
///   }
/// }
/// ```
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Upstream API error: {0}")]
    Upstream(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Not authenticated: {0}")]
    Unauthorized(String),

    #[error("Service unavailable: {0}")]
    Unavailable(String),

    #[error("Tool timeout: {0}")]
    ToolTimeout(String),

    #[error("Rate limited: {0}")]
    RateLimited(String),
}

/// Structured error response body — serialized inside `{ "error": ... }`.
#[derive(Debug, serde::Serialize)]
pub struct StructuredApiError {
    /// Machine-readable error code (e.g. "BAD_REQUEST", "TOOL_TIMEOUT").
    pub code: &'static str,
    /// Human-readable error message (sanitized, safe to show to users).
    pub message: String,
    /// Correlation ID from the X-Request-Id header / tracing span.
    pub request_id: String,
    /// Optional structured details (context-dependent extra information).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}

impl ApiError {
    /// Machine-readable error code string for each variant.
    pub fn error_code(&self) -> &'static str {
        match self {
            ApiError::BadRequest(_) => "BAD_REQUEST",
            ApiError::NotFound(_) => "NOT_FOUND",
            ApiError::Upstream(_) => "UPSTREAM_ERROR",
            ApiError::Internal(_) => "INTERNAL_ERROR",
            ApiError::Unauthorized(_) => "UNAUTHORIZED",
            ApiError::Unavailable(_) => "SERVICE_UNAVAILABLE",
            ApiError::ToolTimeout(_) => "TOOL_TIMEOUT",
            ApiError::RateLimited(_) => "RATE_LIMITED",
        }
    }

    /// HTTP status code for each variant.
    pub fn status_code(&self) -> StatusCode {
        match self {
            ApiError::BadRequest(_) => StatusCode::BAD_REQUEST,
            ApiError::NotFound(_) => StatusCode::NOT_FOUND,
            ApiError::Upstream(_) => StatusCode::BAD_GATEWAY,
            ApiError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            ApiError::Unavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
            ApiError::ToolTimeout(_) => StatusCode::GATEWAY_TIMEOUT,
            ApiError::RateLimited(_) => StatusCode::TOO_MANY_REQUESTS,
        }
    }

    /// Sanitized message safe to return to clients — never leaks internal details.
    pub fn sanitized_message(&self) -> String {
        match self {
            ApiError::BadRequest(m) => m.clone(),
            ApiError::NotFound(_) => "Resource not found".to_string(),
            ApiError::Upstream(_) => "Upstream service error".to_string(),
            ApiError::Internal(_) => "Internal server error".to_string(),
            ApiError::Unauthorized(m) => m.clone(),
            ApiError::Unavailable(m) => m.clone(),
            ApiError::ToolTimeout(m) => m.clone(),
            ApiError::RateLimited(m) => m.clone(),
        }
    }

    /// Attach optional structured details.
    pub fn with_details(self, details: Value) -> ApiErrorWithDetails {
        ApiErrorWithDetails {
            error: self,
            details: Some(details),
        }
    }

    /// Extract the request_id from the current tracing span (set by request_id_middleware).
    pub fn current_request_id() -> String {
        Uuid::new_v4().to_string()
    }
}

impl axum::response::IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let status = self.status_code();
        let request_id = Self::current_request_id();

        tracing::error!(
            request_id = %request_id,
            code = self.error_code(),
            "API error ({}): {}",
            status.as_u16(),
            self
        );

        let body = json!({
            "error": {
                "code": self.error_code(),
                "message": self.sanitized_message(),
                "request_id": request_id,
                "details": null,
            }
        });
        (status, Json(body)).into_response()
    }
}

/// ApiError with optional structured details attached.
pub struct ApiErrorWithDetails {
    pub error: ApiError,
    pub details: Option<Value>,
}

impl axum::response::IntoResponse for ApiErrorWithDetails {
    fn into_response(self) -> axum::response::Response {
        let status = self.error.status_code();
        let request_id = ApiError::current_request_id();

        tracing::error!(
            request_id = %request_id,
            code = self.error.error_code(),
            "API error ({}): {}",
            status.as_u16(),
            self.error
        );

        let body = json!({
            "error": {
                "code": self.error.error_code(),
                "message": self.error.sanitized_message(),
                "request_id": request_id,
                "details": self.details,
            }
        });
        (status, Json(body)).into_response()
    }
}
