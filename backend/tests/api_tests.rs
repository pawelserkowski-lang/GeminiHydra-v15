use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::Value;
use sqlx::PgPool;
use tower::ServiceExt;

use geminihydra_backend::state::AppState;

/// Helper: build a fresh AppState backed by a test Postgres database.
/// Requires DATABASE_URL env var to be set.
async fn test_state() -> AppState {
    dotenvy::dotenv().ok();
    let database_url =
        std::env::var("DATABASE_URL").expect("DATABASE_URL required for integration tests");
    let pool = PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to test database");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");
    AppState::new(pool).await
}

/// Helper: build a router from a test state.
fn app(state: AppState) -> axum::Router {
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
    let state = test_state().await;
    let response = app(state)
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
    let state = test_state().await;
    let response = app(state)
        .oneshot(
            Request::builder()
                .uri("/api/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let json = body_json(response).await;

    // test_state() does not call mark_ready(), so status is "starting"
    assert_eq!(json["status"], "starting");
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
    let state = test_state().await;
    let response = app(state)
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
    let state = test_state().await;
    let response = app(state)
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
    let state = test_state().await;
    let response = app(state)
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
    let state = test_state().await;
    let response = app(state)
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
async fn get_settings_has_expected_fields() {
    let state = test_state().await;
    let response = app(state)
        .oneshot(
            Request::builder()
                .uri("/api/settings")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let json = body_json(response).await;
    assert!(json["language"].is_string());
    assert!(json["theme"].is_string());
    assert!(json["default_model"].is_string());
    assert!(json["temperature"].is_f64());
    assert!(json["max_tokens"].is_u64());
}

// ═══════════════════════════════════════════════════════════════════════════
//  PATCH /api/settings
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn patch_settings_partial_update() {
    let state = test_state().await;

    let body = serde_json::json!({
        "language": "pl",
        "theme": "light"
    });

    let response = app(state)
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
    // default_model is whatever was in DB — just check it exists
    assert!(json["default_model"].is_string());
}

#[tokio::test]
async fn patch_settings_persists_changes() {
    let state = test_state().await;

    // PATCH temperature
    let body = serde_json::json!({ "temperature": 0.9 });
    let patch_resp = app(state.clone())
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
    assert_eq!(patch_resp.status(), StatusCode::OK);

    // GET settings to verify persistence
    let get_resp = app(state)
        .oneshot(
            Request::builder()
                .uri("/api/settings")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let json = body_json(get_resp).await;
    assert!((json["temperature"].as_f64().unwrap() - 0.9).abs() < f64::EPSILON);
}

// ═══════════════════════════════════════════════════════════════════════════
//  POST /api/settings/reset
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn reset_settings_returns_200() {
    let state = test_state().await;
    let response = app(state)
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
    let state = test_state().await;

    // First change a setting via PATCH
    let body = serde_json::json!({ "language": "fr", "temperature": 1.5 });
    let _patch = app(state.clone())
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

    // Then reset
    let reset_resp = app(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/settings/reset")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(reset_resp.status(), StatusCode::OK);

    let json = body_json(reset_resp).await;
    assert_eq!(json["language"], "en");
    assert_eq!(json["theme"], "dark");
    assert!((json["temperature"].as_f64().unwrap() - 1.0).abs() < f64::EPSILON);
}

// ═══════════════════════════════════════════════════════════════════════════
//  GET /api/history
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn history_returns_200() {
    let state = test_state().await;
    let response = app(state)
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
}

// ═══════════════════════════════════════════════════════════════════════════
//  DELETE /api/history
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn clear_history_returns_200() {
    let state = test_state().await;
    let response = app(state)
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
    let state = test_state().await;
    let response = app(state)
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
