pub mod files;
pub mod handlers;
pub mod models;
pub mod sessions;
pub mod state;

use axum::routing::{get, post};
use axum::Router;

use state::AppState;

/// Build the application router with the given state.
/// Extracted from `main()` so integration tests can construct the app
/// without binding to a network port.
pub fn create_router(state: AppState) -> Router {
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
        // Files
        .route("/api/files/read", post(handlers::read_file))
        .route("/api/files/list", post(handlers::list_files))
        // System
        .route("/api/system/stats", get(handlers::system_stats))
        // Sessions / History / Settings / Memory / Knowledge
        .merge(sessions::session_routes())
        // Shared state
        .with_state(state)
}
