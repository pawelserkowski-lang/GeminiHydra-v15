use std::sync::Arc;
use tokio::sync::Mutex;
use tower_http::cors::{AllowHeaders, AllowMethods, AllowOrigin, CorsLayer};
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::trace::TraceLayer;

use geminihydra_backend::state::AppState;

fn build_app() -> axum::Router {
    let shared_state = Arc::new(Mutex::new(AppState::new()));

    // CORS — allow Vite dev server + Vercel production
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::list([
            "http://localhost:5176".parse().unwrap(),
            "http://localhost:5173".parse().unwrap(),
            "http://localhost:3000".parse().unwrap(),
            "https://geminihydra-v15.vercel.app".parse().unwrap(),
            "https://geminihydra-v15-pawelserkowskis-projects.vercel.app".parse().unwrap(),
        ]))
        .allow_methods(AllowMethods::any())
        .allow_headers(AllowHeaders::any());

    geminihydra_backend::create_router(shared_state)
        .layer(RequestBodyLimitLayer::new(10 * 1024 * 1024))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
}

// ── Shuttle deployment entry point ──────────────────────────────────
#[cfg(feature = "shuttle")]
#[shuttle_runtime::main]
async fn main() -> shuttle_axum::ShuttleAxum {
    Ok(build_app().into())
}

// ── Local development entry point ───────────────────────────────────
#[cfg(not(feature = "shuttle"))]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    use tracing_subscriber::EnvFilter;

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    dotenvy::dotenv().ok();

    let app = build_app();

    let port: u16 = std::env::var("PORT")
        .unwrap_or_else(|_| "8081".to_string())
        .parse()?;
    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));

    tracing::info!("GeminiHydra v15 backend listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
