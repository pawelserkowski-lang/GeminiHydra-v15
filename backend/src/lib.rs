pub mod analysis;
pub mod auth;
pub mod files;
pub mod handlers;
pub mod model_registry;
pub mod models;
pub mod oauth;
pub mod sessions;
pub mod state;
pub mod system_monitor;
pub mod tools;
pub mod watchdog;

use axum::middleware;
use axum::routing::{delete, get, post};
use axum::Router;

use state::AppState;

/// Build the application router with the given state.
/// Extracted from `main()` so integration tests can construct the app
/// without binding to a network port.
pub fn create_router(state: AppState) -> Router {
    // ── Public routes (no auth) ──────────────────────────────────────
    let public = Router::new()
        .route("/api/health", get(handlers::health))
        .route("/api/health/ready", get(handlers::readiness))
        .route("/api/health/detailed", get(handlers::health_detailed))
        .route("/api/auth/status", get(oauth::auth_status))
        .route("/api/auth/login", post(oauth::auth_login))
        .route("/api/auth/callback", post(oauth::auth_callback))
        .route("/api/auth/logout", post(oauth::auth_logout))
        .route("/api/auth/mode", get(handlers::auth_mode))
        // WebSocket has its own auth via query param
        .route("/ws/execute", get(handlers::ws_execute));

    // ── Protected routes (require auth when AUTH_SECRET is set) ──────
    let protected = Router::new()
        .route("/api/agents", get(handlers::list_agents).post(handlers::create_agent))
        .route("/api/agents/classify", post(handlers::classify_agent))
        .route("/api/agents/{id}", post(handlers::update_agent).delete(handlers::delete_agent))
        .route("/api/execute", post(handlers::execute))
        .route("/api/gemini/models", get(handlers::gemini_models))
        .route("/api/models", get(model_registry::list_models))
        .route("/api/models/refresh", post(model_registry::refresh_models))
        .route("/api/models/pin", post(model_registry::pin_model))
        .route("/api/models/pin/{use_case}", delete(model_registry::unpin_model))
        .route("/api/models/pins", get(model_registry::list_pins))
        .route("/api/files/read", post(handlers::read_file))
        .route("/api/files/list", post(handlers::list_files))
        .route("/api/system/stats", get(handlers::system_stats))
        .route_layer(middleware::from_fn_with_state(state.clone(), auth::require_auth));

    public
        .merge(protected)
        // Sessions routes merged separately — they need auth too
        .merge(
            sessions::session_routes()
                .route_layer(middleware::from_fn_with_state(state.clone(), auth::require_auth)),
        )
        .with_state(state)
}
