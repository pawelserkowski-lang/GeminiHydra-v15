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
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use state::AppState;

// ── OpenAPI documentation ────────────────────────────────────────────────────

#[derive(OpenApi)]
#[openapi(
    info(
        title = "GeminiHydra v15 API",
        version = "15.0.0",
        description = "Multi-Agent AI Swarm — Backend API",
        license(name = "MIT")
    ),
    paths(
        // Health
        handlers::health,
        handlers::readiness,
        handlers::health_detailed,
        handlers::auth_mode,
        handlers::system_stats,
        // Agents
        handlers::list_agents,
        handlers::classify_agent,
        handlers::create_agent,
        handlers::update_agent,
        handlers::delete_agent,
        // Execute / Chat
        handlers::execute,
        handlers::gemini_models,
        // Files
        handlers::read_file,
        handlers::list_files,
        // Model registry
        model_registry::list_models,
        model_registry::refresh_models,
        model_registry::pin_model,
        model_registry::unpin_model,
        model_registry::list_pins,
        // Sessions
        sessions::list_sessions,
        sessions::create_session,
        sessions::get_session,
        sessions::update_session,
        sessions::delete_session,
        sessions::add_session_message,
        sessions::generate_session_title,
        // History
        sessions::get_history,
        sessions::search_history,
        sessions::add_message,
        sessions::clear_history,
        // Settings
        sessions::get_settings,
        sessions::update_settings,
        sessions::reset_settings,
        // Memory
        sessions::list_memories,
        sessions::add_memory,
        sessions::clear_memories,
        sessions::get_knowledge_graph,
        sessions::add_knowledge_node,
        sessions::add_graph_edge,
    ),
    components(schemas(
        // Core models
        models::HealthResponse,
        models::DetailedHealthResponse,
        models::ProviderInfo,
        models::SystemStats,
        // Agents
        models::WitcherAgent,
        models::ClassifyRequest,
        models::ClassifyResponse,
        // Execute
        models::ExecuteRequest,
        models::ExecuteResponse,
        models::ExecutePlan,
        // Gemini
        models::GeminiModelsResponse,
        models::GeminiModelInfo,
        models::GeminiStreamRequest,
        // Settings
        models::AppSettings,
        // Chat
        models::ChatMessage,
        // Files
        models::FileReadRequest,
        models::FileReadResponse,
        models::FileListRequest,
        models::FileListResponse,
        models::FileEntryResponse,
        // Sessions
        models::Session,
        models::SessionSummary,
        models::CreateSessionRequest,
        models::UpdateSessionRequest,
        // Model registry
        model_registry::ModelInfo,
        model_registry::ResolvedModels,
        model_registry::PinModelRequest,
    )),
    tags(
        (name = "health", description = "Health & readiness endpoints"),
        (name = "auth", description = "Authentication & API key management"),
        (name = "agents", description = "Witcher agent CRUD & classification"),
        (name = "chat", description = "Execute prompts & streaming"),
        (name = "models", description = "Dynamic model registry & pinning"),
        (name = "files", description = "Local filesystem access"),
        (name = "sessions", description = "Chat session management"),
        (name = "history", description = "Chat history"),
        (name = "settings", description = "Application settings"),
        (name = "memory", description = "Agent memory & knowledge graph"),
        (name = "system", description = "System monitoring"),
    )
)]
pub struct ApiDoc;

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
        // Swagger UI — no auth required
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .with_state(state)
}
