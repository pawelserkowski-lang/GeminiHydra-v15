pub mod handlers;
pub mod models;
pub mod sessions;
pub mod state;

use axum::routing::{get, post};
use axum::Router;

use handlers::SharedState;

/// Build the application router with the given shared state.
/// Extracted from `main()` so integration tests can construct the app
/// without binding to a network port.
pub fn create_router(shared_state: SharedState) -> Router {
    Router::new()
        // Health
        .route("/api/health", get(handlers::health))
        .route("/api/health/detailed", get(handlers::health_detailed))
        // Agents
        .route("/api/agents", get(handlers::list_agents))
        .route("/api/agents/classify", post(handlers::classify_agent))
        // Execute
        .route("/api/execute", post(handlers::execute))
        // Gemini proxy
        .route("/api/gemini/models", get(handlers::gemini_models))
        // System
        .route("/api/system/stats", get(handlers::system_stats))
        // Sessions / History / Settings / Memory / Knowledge (Agent 2)
        .merge(sessions::session_routes())
        // Shared state
        .with_state(shared_state)
}
