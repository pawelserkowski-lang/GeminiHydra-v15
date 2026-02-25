use axum::http::{header, HeaderValue, Method};
use sqlx::postgres::PgPoolOptions;
use tower_governor::{governor::GovernorConfigBuilder, GovernorLayer};
use tower_http::cors::CorsLayer;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::compression::CompressionLayer;
use tower_http::trace::TraceLayer;

use geminihydra_backend::model_registry;
use geminihydra_backend::state::AppState;
use geminihydra_backend::watchdog;

async fn build_app() -> (axum::Router, AppState) {
    dotenvy::dotenv().ok();

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL required");
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(std::time::Duration::from_secs(3))
        .idle_timeout(std::time::Duration::from_secs(600))
        .max_lifetime(std::time::Duration::from_secs(1800))
        .connect(&database_url)
        .await
        .expect("Failed to connect to Postgres");

    // Skip migrations if schema already exists (avoids checksum mismatch)
    if let Err(e) = sqlx::migrate!("./migrations").run(&pool).await {
        tracing::warn!("Migration skipped (schema likely exists): {}", e);
    }

    let state = AppState::new(pool).await;

    // ── Spawn system monitor (CPU/memory stats, refreshed every 5s) ──
    geminihydra_backend::system_monitor::spawn(state.system_monitor.clone());

    // CORS — explicit allowlist for Vite dev servers + Vercel production
    let cors = CorsLayer::new()
        .allow_origin([
            "http://localhost:5176".parse().unwrap(),
            "http://127.0.0.1:5176".parse().unwrap(),
            // ClaudeHydra frontend (partner app cross-session access)
            "http://localhost:5199".parse().unwrap(),
            "http://127.0.0.1:5199".parse().unwrap(),
            "https://geminihydra-v15.vercel.app".parse().unwrap(),
        ])
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION])
        .max_age(std::time::Duration::from_secs(86_400));

    // Security headers
    let nosniff: SetResponseHeaderLayer<HeaderValue> = SetResponseHeaderLayer::overriding(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    let frame_deny: SetResponseHeaderLayer<HeaderValue> = SetResponseHeaderLayer::overriding(
        header::X_FRAME_OPTIONS,
        HeaderValue::from_static("DENY"),
    );
    let referrer: SetResponseHeaderLayer<HeaderValue> = SetResponseHeaderLayer::overriding(
        header::REFERRER_POLICY,
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    );
    let csp: SetResponseHeaderLayer<HeaderValue> = SetResponseHeaderLayer::overriding(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; connect-src 'self' https://generativelanguage.googleapis.com https://api.anthropic.com https://api.openai.com; img-src 'self' data: blob:",
        ),
    );
    let hsts: SetResponseHeaderLayer<HeaderValue> = SetResponseHeaderLayer::overriding(
        header::STRICT_TRANSPORT_SECURITY,
        HeaderValue::from_static("max-age=63072000; includeSubDomains"),
    );

    // Rate limiting: 30 req burst, replenish 1 per 2 seconds, per IP
    // Jaskier Shared Pattern -- rate_limit
    let governor_conf = GovernorConfigBuilder::default()
        .per_second(2)
        .burst_size(30)
        .finish()
        .unwrap();

    let app = geminihydra_backend::create_router(state.clone())
        .layer(GovernorLayer::new(governor_conf))
        .layer(RequestBodyLimitLayer::new(10 * 1024 * 1024))
        .layer(cors)
        .layer(nosniff)
        .layer(frame_deny)
        .layer(referrer)
        .layer(csp)
        .layer(hsts)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(|request: &axum::http::Request<_>| {
                    tracing::info_span!(
                        "http_request",
                        method = %request.method(),
                        uri = %request.uri(),
                    )
                })
        )
        .layer(CompressionLayer::new());

    (app, state)
}

// ── Shuttle deployment entry point ──────────────────────────────────
#[cfg(feature = "shuttle")]
#[shuttle_runtime::main]
async fn main() -> shuttle_axum::ShuttleAxum {
    let (app, state) = build_app().await;
    model_registry::startup_sync(&state).await;
    state.mark_ready();
    Ok(app.into())
}

// ── Local / Fly.io entry point ──────────────────────────────────────
#[cfg(not(feature = "shuttle"))]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    use tracing_subscriber::EnvFilter;

    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into());
    if std::env::var("RUST_LOG_FORMAT").as_deref() == Ok("json") {
        tracing_subscriber::fmt()
            .with_env_filter(env_filter)
            .json()
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(env_filter)
            .init();
    }

    let (app, state) = build_app().await;

    // ── Non-blocking startup: model sync in background ──
    let startup_state = state.clone();
    tokio::spawn(async move {
        let sync_timeout = std::time::Duration::from_secs(90);
        match tokio::time::timeout(sync_timeout, model_registry::startup_sync(&startup_state)).await
        {
            Ok(()) => tracing::info!("startup: model registry sync complete"),
            Err(_) => tracing::error!(
                "startup: model registry sync timed out after {}s — using fallback models",
                sync_timeout.as_secs()
            ),
        }
        startup_state.mark_ready();
    });

    // ── Spawn background watchdog ──
    let _watchdog = watchdog::spawn(state.clone());

    let port: u16 = std::env::var("PORT")
        .unwrap_or_else(|_| "8081".to_string())
        .parse()?;
    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));

    tracing::info!("GeminiHydra v15 backend listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = tokio::signal::ctrl_c();
    #[cfg(unix)]
    {
        let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler");
        tokio::select! {
            _ = ctrl_c => {},
            _ = sigterm.recv() => {},
        }
    }
    #[cfg(not(unix))]
    {
        ctrl_c.await.ok();
    }
    tracing::info!("Shutdown signal received, starting graceful shutdown");
}
