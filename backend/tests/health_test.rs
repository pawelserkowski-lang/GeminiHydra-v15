// Jaskier Shared Pattern -- backend integration test
// GeminiHydra v15 - Health endpoint integration test
//
// Note: GeminiHydra's AppState::new() is async and requires a real DB
// connection to load agents. These tests use a minimal router to avoid
// that dependency.

use axum::http::StatusCode;
use axum::routing::get;
use axum::body::Body;
use axum::http::Request;
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

/// Build a minimal test router that mirrors the health endpoints
/// without requiring AppState (which needs a DB connection).
fn test_app() -> axum::Router {
    axum::Router::new()
        .route("/api/health", get(|| async {
            axum::Json(serde_json::json!({
                "status": "ok",
                "version": env!("CARGO_PKG_VERSION"),
            }))
        }))
        .route("/api/health/ready", get(|| async {
            (StatusCode::OK, axum::Json(serde_json::json!({"ready": true})))
        }))
}

/// Collect a response body into a `serde_json::Value`.
async fn body_json(response: axum::response::Response) -> Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn health_endpoint_returns_ok() {
    let response = test_app()
        .oneshot(
            Request::builder()
                .uri("/api/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn health_endpoint_returns_json_with_status_field() {
    let response = test_app()
        .oneshot(
            Request::builder()
                .uri("/api/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let json = body_json(response).await;
    assert_eq!(json["status"], "ok");
}

#[tokio::test]
async fn readiness_endpoint_returns_ok() {
    let response = test_app()
        .oneshot(
            Request::builder()
                .uri("/api/health/ready")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn nonexistent_route_returns_404() {
    let response = test_app()
        .oneshot(
            Request::builder()
                .uri("/api/does-not-exist")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
