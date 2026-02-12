use std::sync::Arc;

use tokio::sync::Mutex;
use tower_http::cors::{AllowHeaders, AllowMethods, AllowOrigin, CorsLayer};
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

use geminihydra_backend::state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // ── Logging ────────────────────────────────────────────────────────
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    // ── Environment ────────────────────────────────────────────────────
    dotenvy::dotenv().ok();

    // ── Shared state ───────────────────────────────────────────────────
    let shared_state = Arc::new(Mutex::new(AppState::new()));

    // ── CORS — allow the frontend dev server ───────────────────────────
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::list([
            "http://localhost:5176".parse().unwrap(),
            "http://localhost:5173".parse().unwrap(),
            "http://localhost:3000".parse().unwrap(),
        ]))
        .allow_methods(AllowMethods::any())
        .allow_headers(AllowHeaders::any());

    // ── Router ─────────────────────────────────────────────────────────
    let app = geminihydra_backend::create_router(shared_state)
        // Layers (applied bottom-up)
        .layer(RequestBodyLimitLayer::new(10 * 1024 * 1024)) // 10 MB
        .layer(cors)
        .layer(TraceLayer::new_for_http());

    // ── Listen ─────────────────────────────────────────────────────────
    let port: u16 = std::env::var("PORT")
        .unwrap_or_else(|_| "8081".to_string())
        .parse()?;
    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));

    tracing::info!("GeminiHydra v15 backend listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
