pub mod analysis;
pub mod files;
pub mod handlers;
pub mod model_registry;
pub mod models;
pub mod oauth;
pub mod sessions;
pub mod state;
pub mod tools;
pub mod watchdog;

use axum::routing::{delete, get, post};
use axum::Router;

use state::AppState;

/// Build the application router with the given state.
/// Extracted from `main()` so integration tests can construct the app
/// without binding to a network port.
pub fn create_router(state: AppState) -> Router {
    Router::new()
        // Health
        .route("/api/health", get(handlers::health))
        .route("/api/health/ready", get(handlers::readiness))
        .route("/api/health/detailed", get(handlers::health_detailed))
        // Agents
        .route("/api/agents", get(handlers::list_agents).post(handlers::create_agent))
        .route("/api/agents/classify", post(handlers::classify_agent))
        .route("/api/agents/{id}", post(handlers::update_agent).delete(handlers::delete_agent))
        // Execute
        .route("/api/execute", post(handlers::execute))
        // WebSocket streaming
        .route("/ws/execute", get(handlers::ws_execute))
        // Gemini proxy
        .route("/api/gemini/models", get(handlers::gemini_models))
        // Model registry
        .route("/api/models", get(model_registry::list_models))
        .route("/api/models/refresh", post(model_registry::refresh_models))
        .route("/api/models/pin", post(model_registry::pin_model))
        .route("/api/models/pin/{use_case}", delete(model_registry::unpin_model))
        .route("/api/models/pins", get(model_registry::list_pins))
        // Files
        .route("/api/files/read", post(handlers::read_file))
        .route("/api/files/list", post(handlers::list_files))
        // System
        .route("/api/system/stats", get(handlers::system_stats))
        // OAuth authentication
        .route("/api/auth/status", get(oauth::auth_status))
        .route("/api/auth/login", post(oauth::auth_login))
        .route("/api/auth/callback", post(oauth::auth_callback))
        .route("/api/auth/logout", post(oauth::auth_logout))
        // Sessions / History / Settings / Memory / Knowledge
        .merge(sessions::session_routes())
        // Shared state
        .with_state(state)
}
