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
use sysinfo::System;

use state::AppState;

/// Build the application router with the given state.
/// Extracted from `main()` so integration tests can construct the app
/// without binding to a network port.
pub fn create_router(state: AppState) -> Router {
    // ── Background system monitor (CPU / memory) ────────────────────────
    // Refreshes every 5 s so handlers always read a pre-computed snapshot
    // instead of creating a throw-away `System` (which always reports 0 / 100 %).
    {
        let monitor = state.system_monitor.clone();
        tokio::spawn(async move {
            let mut sys = System::new_all();
            // Baseline measurement for delta-based CPU %.
            sys.refresh_cpu_all();
            // Must wait at least MINIMUM_CPU_UPDATE_INTERVAL before second measurement.
            tokio::time::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL).await;
            sys.refresh_cpu_all();
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                // Only refresh CPU + memory, NOT refresh_all() which resets CPU baseline.
                sys.refresh_cpu_all();
                sys.refresh_memory();

                let cpu = if sys.cpus().is_empty() {
                    0.0
                } else {
                    sys.cpus().iter().map(|c| c.cpu_usage()).sum::<f32>()
                        / sys.cpus().len() as f32
                };

                let snap = state::SystemSnapshot {
                    cpu_usage_percent: cpu,
                    memory_used_mb: sys.used_memory() as f64 / 1_048_576.0,
                    memory_total_mb: sys.total_memory() as f64 / 1_048_576.0,
                    platform: std::env::consts::OS.to_string(),
                };

                *monitor.write().await = snap;
            }
        });
    }

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
