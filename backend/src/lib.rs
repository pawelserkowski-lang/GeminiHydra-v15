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

// ── Windows-native CPU monitoring via GetSystemTimes ─────────────────
#[cfg(windows)]
fn filetime_to_u64(ft: &windows::Win32::Foundation::FILETIME) -> u64 {
    ((ft.dwHighDateTime as u64) << 32) | ft.dwLowDateTime as u64
}

#[cfg(windows)]
fn get_cpu_times() -> (u64, u64, u64) {
    use windows::Win32::Foundation::FILETIME;
    use windows::Win32::System::Threading::GetSystemTimes;
    let mut idle = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    unsafe {
        GetSystemTimes(Some(&mut idle), Some(&mut kernel), Some(&mut user)).unwrap();
    }
    (filetime_to_u64(&idle), filetime_to_u64(&kernel), filetime_to_u64(&user))
}

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
            let mut sys = sysinfo::System::new_all();

            // CPU: Windows-native GetSystemTimes (sysinfo returns 100% on Win11 26200)
            #[cfg(windows)]
            let (mut prev_idle, mut prev_kernel, mut prev_user) = get_cpu_times();

            // CPU: sysinfo fallback for non-Windows platforms
            #[cfg(not(windows))]
            {
                sys.refresh_cpu_all();
                tokio::time::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL).await;
                sys.refresh_cpu_all();
            }

            loop {
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;

                // CPU via GetSystemTimes on Windows
                #[cfg(windows)]
                let cpu = {
                    let (idle, kernel, user) = get_cpu_times();
                    let idle_diff = idle - prev_idle;
                    let kernel_diff = kernel - prev_kernel;
                    let user_diff = user - prev_user;
                    let total = kernel_diff + user_diff;
                    let c = if total > 0 {
                        ((total - idle_diff) as f32 / total as f32) * 100.0
                    } else {
                        0.0
                    };
                    prev_idle = idle;
                    prev_kernel = kernel;
                    prev_user = user;
                    c
                };

                // CPU via sysinfo on non-Windows
                #[cfg(not(windows))]
                let cpu = {
                    sys.refresh_cpu_all();
                    if sys.cpus().is_empty() {
                        0.0
                    } else {
                        sys.cpus().iter().map(|c| c.cpu_usage()).sum::<f32>()
                            / sys.cpus().len() as f32
                    }
                };

                // Memory via sysinfo (works correctly on all platforms)
                sys.refresh_memory();

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
