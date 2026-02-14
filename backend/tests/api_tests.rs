use std::sync::Arc;
use tokio::sync::Mutex;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

use geminihydra_backend::state::AppState;

/// Helper: build a fresh app router with a clean AppState for each test.
fn app() -> axum::Router {
    let state = Arc::new(Mutex::new(AppState::new()));
    geminihydra_backend::create_router(state)
}

/// Helper: collect a response body into a serde_json::Value.
async fn body_json(response: axum::response::Response) -> Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

// ═══════════════════════════════════════════════════════════════════════════
//  GET /api/health
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn health_returns_200() {
    let response = app()
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
async fn health_has_correct_fields() {
    let response = app()
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
    assert_eq!(json["version"], "15.0.0");
    assert_eq!(json["app"], "GeminiHydra");
    assert!(json["uptime_seconds"].is_u64());
    assert!(json["providers"].is_array());
}

// ═══════════════════════════════════════════════════════════════════════════
//  GET /api/agents
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn agents_returns_200() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/api/agents")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn agents_returns_12_agents() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/api/agents")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let json = body_json(response).await;
    let agents = json["agents"].as_array().unwrap();
    assert_eq!(agents.len(), 12);
}

#[tokio::test]
async fn agents_have_required_fields() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/api/agents")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let json = body_json(response).await;
    let agents = json["agents"].as_array().unwrap();

    for agent in agents {
        assert!(agent["id"].is_string(), "agent missing id");
        assert!(agent["name"].is_string(), "agent missing name");
        assert!(agent["role"].is_string(), "agent missing role");
        assert!(agent["tier"].is_string(), "agent missing tier");
        assert!(agent["status"].is_string(), "agent missing status");
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  GET /api/settings
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn get_settings_returns_200() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/api/settings")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn get_settings_default_values() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/api/settings")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let json = body_json(response).await;
    assert_eq!(json["language"], "en");
    assert_eq!(json["theme"], "dark");
    assert_eq!(json["default_model"], "gemini-3-flash-preview");
    assert!(json["temperature"].is_f64());
    assert!(json["max_tokens"].is_u64());
}

// ═══════════════════════════════════════════════════════════════════════════
//  PATCH /api/settings
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn patch_settings_partial_update() {
    let state = Arc::new(Mutex::new(AppState::new()));
    let router = geminihydra_backend::create_router(state.clone());

    let body = serde_json::json!({
        "language": "pl",
        "theme": "light"
    });

    let response = router
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/settings")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = body_json(response).await;
    assert_eq!(json["language"], "pl");
    assert_eq!(json["theme"], "light");
    // Other fields should retain defaults
    assert_eq!(json["default_model"], "gemini-3-flash-preview");
}

#[tokio::test]
async fn patch_settings_persists_changes() {
    let state = Arc::new(Mutex::new(AppState::new()));
    let router = geminihydra_backend::create_router(state.clone());

    let body = serde_json::json!({
        "temperature": 0.9
    });

    let _response = router
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/settings")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // Verify via state
    let st = state.lock().await;
    assert!((st.settings.temperature - 0.9).abs() < f64::EPSILON);
}

// ═══════════════════════════════════════════════════════════════════════════
//  POST /api/settings/reset
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn reset_settings_returns_200() {
    let response = app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/settings/reset")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn reset_settings_restores_defaults() {
    let state = Arc::new(Mutex::new(AppState::new()));
    let router = geminihydra_backend::create_router(state.clone());

    // First change a setting
    {
        let mut st = state.lock().await;
        st.settings.language = "fr".to_string();
        st.settings.temperature = 1.5;
    }

    // Then reset
    let response = router
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/settings/reset")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = body_json(response).await;
    assert_eq!(json["language"], "en");
    assert_eq!(json["theme"], "dark");
    assert!((json["temperature"].as_f64().unwrap() - 0.7).abs() < f64::EPSILON);
}

// ═══════════════════════════════════════════════════════════════════════════
//  GET /api/history
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn history_returns_200() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/api/history")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = body_json(response).await;
    assert!(json["messages"].is_array());
    assert_eq!(json["total"], 0);
}

// ═══════════════════════════════════════════════════════════════════════════
//  DELETE /api/history
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn clear_history_returns_200() {
    let response = app()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/history")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = body_json(response).await;
    assert_eq!(json["cleared"], true);
}

// ═══════════════════════════════════════════════════════════════════════════
//  404 for unknown routes
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn unknown_route_returns_404() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/api/nonexistent")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
