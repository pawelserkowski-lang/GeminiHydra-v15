pub mod a2a;
pub mod analysis;
pub mod audit;
pub mod auth;
pub mod files;
pub mod handlers;
pub mod logs;
pub mod model_registry;
pub mod models;
pub mod oauth;
pub mod ocr;
pub mod sessions;
pub mod state;
pub mod system_monitor;
pub mod tools;
pub mod watchdog;

use axum::extract::State;
use axum::http::HeaderValue;
use axum::middleware;
use axum::routing::{delete, get, post};
use axum::Router;
use tower_governor::{governor::GovernorConfigBuilder, GovernorLayer};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use state::AppState;

// ---------------------------------------------------------------------------
// Jaskier Shared Pattern -- request_id middleware
// ---------------------------------------------------------------------------

/// Middleware that assigns a UUID correlation ID to every request.
/// - Adds the ID to the current tracing span for structured logging.
/// - Returns it as `X-Request-Id` response header for client-side correlation.
pub async fn request_id_middleware(
    request: axum::http::Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let request_id = uuid::Uuid::new_v4().to_string();

    // Record in the current tracing span so all log lines include it.
    tracing::Span::current().record("request_id", &tracing::field::display(&request_id));
    tracing::debug!(request_id = %request_id, "assigned correlation ID");

    let mut response = next.run(request).await;

    // Attach as response header — infallible for valid UUID strings.
    if let Ok(val) = HeaderValue::from_str(&request_id) {
        response.headers_mut().insert("x-request-id", val);
    }

    response
}

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
        sessions::get_session_messages,
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
        // Prompt history
        sessions::list_prompt_history,
        sessions::add_prompt_history,
        sessions::clear_prompt_history,
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
        // Prompt history
        models::AddPromptRequest,
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
    // ── Per-endpoint rate limiting ──────────────────────────────────
    // Jaskier Shared Pattern -- rate_limit (per-endpoint)
    //
    // WebSocket /ws/execute: 10 connections per minute (1 per 6s, burst 10)
    // /api/execute: 30 requests per minute (1 per 2s, burst 30)
    // Other routes: 120 requests per minute (2 per second, burst 120)

    let ws_governor = GovernorConfigBuilder::default()
        .per_second(6)
        .burst_size(10)
        .use_headers()
        .finish()
        .expect("WS rate-limit config is valid");

    let execute_governor = GovernorConfigBuilder::default()
        .per_second(2)
        .burst_size(30)
        .use_headers()
        .finish()
        .expect("Execute rate-limit config is valid");

    let default_governor = GovernorConfigBuilder::default()
        .per_second(2)
        .burst_size(120)
        .use_headers()
        .finish()
        .expect("Default rate-limit config is valid");

    // ── Public routes (no auth) ──────────────────────────────────────
    let public_health = Router::new()
        .route("/api/health", get(handlers::health))
        .route("/api/health/ready", get(handlers::readiness))
        .route("/api/health/detailed", get(handlers::health_detailed))
        .route("/api/auth/status", get(oauth::auth_status))
        .route("/api/auth/login", post(oauth::auth_login))
        .route("/api/auth/google/redirect", get(oauth::google_redirect))
        .route("/api/auth/logout", post(oauth::auth_logout))
        .route("/api/auth/apikey", post(oauth::save_api_key).delete(oauth::delete_api_key))
        .route("/api/auth/mode", get(handlers::auth_mode))
        // A2A v0.3 — Agent Card discovery (public, no auth)
        .route("/.well-known/agent-card.json", get(a2a::agent_card))
        // ADK sidecar internal tool bridge (localhost only, no auth)
        .route("/api/internal/tool", post(handlers::internal_tool_execute));

    // WebSocket with its own stricter rate limit (10 per minute)
    let ws_routes = Router::new()
        .route("/ws/execute", get(handlers::ws_execute))
        .layer(GovernorLayer::new(ws_governor));

    // Execute endpoint with medium rate limit (30 per minute)
    let execute_routes = Router::new()
        .route("/api/execute", post(handlers::execute))
        .route_layer(middleware::from_fn_with_state(state.clone(), auth::require_auth))
        .layer(GovernorLayer::new(execute_governor));

    // ── Protected routes (require auth when AUTH_SECRET is set) ──────
    let protected = Router::new()
        .route("/api/agents", get(handlers::list_agents).post(handlers::create_agent))
        .route("/api/agents/classify", post(handlers::classify_agent))
        .route("/api/agents/{id}", post(handlers::update_agent).delete(handlers::delete_agent))
        .route("/api/gemini/models", get(handlers::gemini_models))
        .route("/api/models", get(model_registry::list_models))
        .route("/api/models/refresh", post(model_registry::refresh_models))
        .route("/api/models/pin", post(model_registry::pin_model))
        .route("/api/models/pin/{use_case}", delete(model_registry::unpin_model))
        .route("/api/models/pins", get(model_registry::list_pins))
        .route("/api/files/read", post(handlers::read_file))
        .route("/api/files/list", post(handlers::list_files))
        .route("/api/files/browse", post(handlers::browse_directory))
        .route("/api/system/stats", get(handlers::system_stats))
        // Logs — centralized log endpoints for LogsView
        .route("/api/logs/backend", get(logs::backend_logs))
        .route("/api/logs/audit", get(logs::audit_logs))
        .route("/api/logs/flyio", get(logs::flyio_logs))
        .route("/api/logs/activity", get(logs::activity_logs))
        .route("/api/logs/usage", get(logs::usage_logs))
        .route("/api/logs/leaderboard", get(logs::leaderboard))
        // OCR — text extraction from images and PDFs
        .route("/api/ocr", post(ocr::ocr))
        .route("/api/ocr/stream", post(ocr::ocr_stream))
        .route("/api/ocr/batch/stream", post(ocr::ocr_batch_stream))
        .route("/api/ocr/history", get(ocr::ocr_history))
        .route("/api/ocr/history/{id}", get(ocr::ocr_history_item).delete(ocr::ocr_history_delete))
        // Admin — hot-reload API keys
        .route("/api/admin/rotate-key", post(handlers::rotate_key))
        // A2A v0.3 — Agent-to-Agent protocol endpoints
        .route("/a2a/message/send", post(a2a::message_send))
        .route("/a2a/message/stream", post(a2a::message_stream))
        .route("/a2a/tasks/{id}", get(a2a::tasks_get))
        .route("/a2a/tasks/{id}/cancel", post(a2a::tasks_cancel))
        .route_layer(middleware::from_fn_with_state(state.clone(), auth::require_auth));

    // ── Metrics endpoint (public, no auth) ─────────────────────────
    let metrics = Router::new().route("/api/metrics", get(metrics_handler));

    // ── API v1 prefix alias (mirrors /api routes for forward compat) ─
    let v1_public = Router::new()
        .route("/api/v1/health", get(handlers::health))
        .route("/api/v1/health/ready", get(handlers::readiness))
        .route("/api/v1/auth/mode", get(handlers::auth_mode));

    // Combine with default rate limit (120 per minute) for most routes
    public_health
        .merge(ws_routes)
        .merge(execute_routes)
        .merge(protected)
        .merge(metrics)
        .merge(v1_public)
        // Sessions routes merged separately — they need auth too
        .merge(
            sessions::session_routes()
                .route_layer(middleware::from_fn_with_state(state.clone(), auth::require_auth)),
        )
        // Swagger UI — no auth required
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .layer(GovernorLayer::new(default_governor))
        .with_state(state)
}

// ── Prometheus-compatible metrics endpoint ───────────────────────────────────

async fn metrics_handler(State(state): State<AppState>) -> String {
    let snapshot = state.system_monitor.read().await;
    let uptime = state.start_time.elapsed().as_secs();
    format!(
        "# HELP cpu_usage_percent CPU usage percentage\n\
         # TYPE cpu_usage_percent gauge\n\
         cpu_usage_percent {:.1}\n\
         # HELP memory_used_bytes Memory used in bytes\n\
         # TYPE memory_used_bytes gauge\n\
         memory_used_bytes {}\n\
         # HELP memory_total_bytes Total memory in bytes\n\
         # TYPE memory_total_bytes gauge\n\
         memory_total_bytes {}\n\
         # HELP uptime_seconds Backend uptime in seconds\n\
         # TYPE uptime_seconds counter\n\
         uptime_seconds {}\n",
        snapshot.cpu_usage_percent,
        (snapshot.memory_used_mb * 1024.0 * 1024.0) as u64,
        (snapshot.memory_total_mb * 1024.0 * 1024.0) as u64,
        uptime,
    )
}
