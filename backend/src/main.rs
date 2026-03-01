use axum::http::{header, HeaderValue, Method};
use sqlx::postgres::PgPoolOptions;
use tower_http::cors::CorsLayer;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::compression::CompressionLayer;
use tower_http::trace::TraceLayer;

use geminihydra_backend::model_registry;
use geminihydra_backend::state::{AppState, LogEntry, LogRingBuffer};
use geminihydra_backend::watchdog;

async fn build_app(log_buffer: std::sync::Arc<LogRingBuffer>) -> (axum::Router, AppState) {
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

    let state = AppState::new(pool, log_buffer).await;

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
    let xss_protection: SetResponseHeaderLayer<HeaderValue> = SetResponseHeaderLayer::overriding(
        header::HeaderName::from_static("x-xss-protection"),
        HeaderValue::from_static("1; mode=block"),
    );
    let permissions_policy: SetResponseHeaderLayer<HeaderValue> = SetResponseHeaderLayer::overriding(
        header::HeaderName::from_static("permissions-policy"),
        HeaderValue::from_static("camera=(), microphone=(), geolocation=()"),
    );

    // Rate limiting is now per-endpoint inside create_router() — see lib.rs
    // WS: 10/min, /api/execute: 30/min, other: 120/min

    let app = geminihydra_backend::create_router(state.clone())
        .layer(RequestBodyLimitLayer::new(10 * 1024 * 1024))
        .layer(cors)
        .layer(nosniff)
        .layer(frame_deny)
        .layer(referrer)
        .layer(csp)
        .layer(hsts)
        .layer(xss_protection)
        .layer(permissions_policy)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(|request: &axum::http::Request<_>| {
                    tracing::info_span!(
                        "http_request",
                        method = %request.method(),
                        uri = %request.uri(),
                        request_id = tracing::field::Empty,
                    )
                })
        )
        // Correlation ID middleware — assigns UUID and returns X-Request-Id header
        .layer(axum::middleware::from_fn(geminihydra_backend::request_id_middleware))
        .layer(CompressionLayer::new());

    (app, state)
}

// ── Log buffer tracing layer ────────────────────────────────────────
struct LogBufferLayer {
    buffer: std::sync::Arc<LogRingBuffer>,
}

impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for LogBufferLayer {
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let meta = event.metadata();
        let mut visitor = MessageVisitor(String::new());
        event.record(&mut visitor);

        self.buffer.push(LogEntry {
            timestamp: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            level: meta.level().to_string(),
            target: meta.target().to_string(),
            message: visitor.0,
        });
    }
}

struct MessageVisitor(String);

impl tracing::field::Visit for MessageVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.0 = format!("{:?}", value);
        } else if self.0.is_empty() {
            self.0 = format!("{}={:?}", field.name(), value);
        }
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            self.0 = value.to_string();
        } else if self.0.is_empty() {
            self.0 = format!("{}={}", field.name(), value);
        }
    }
}

// ── Shuttle deployment entry point ──────────────────────────────────
#[cfg(feature = "shuttle")]
#[shuttle_runtime::main]
async fn main() -> shuttle_axum::ShuttleAxum {
    let log_buffer = std::sync::Arc::new(LogRingBuffer::new(1000));
    let (app, state) = build_app(log_buffer).await;
    model_registry::startup_sync(&state).await;
    state.mark_ready();
    Ok(app.into())
}

// ── Local / Fly.io entry point ──────────────────────────────────────
#[cfg(not(feature = "shuttle"))]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    use tracing_subscriber::prelude::*;
    use tracing_subscriber::EnvFilter;

    enable_ansi();

    // Create log ring buffer BEFORE subscriber so the Layer can capture events
    let log_buffer = std::sync::Arc::new(LogRingBuffer::new(1000));
    let buffer_layer = LogBufferLayer { buffer: log_buffer.clone() };

    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into());
    if std::env::var("RUST_LOG_FORMAT").as_deref() == Ok("json") {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(tracing_subscriber::fmt::layer().json())
            .with(buffer_layer)
            .init();
    } else {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(tracing_subscriber::fmt::layer().with_ansi(true))
            .with(buffer_layer)
            .init();
    }

    let (app, state) = build_app(log_buffer).await;

    // ── Non-blocking startup: model sync in background with retry ──
    let startup_state = state.clone();
    tokio::spawn(async move {
        // Retry up to 3 times with increasing delays: 5s, 15s, 30s
        const RETRY_DELAYS_SECS: &[u64] = &[5, 15, 30];
        const SYNC_TIMEOUT_PER_ATTEMPT: u64 = 90;

        let mut last_err = String::new();
        for (attempt, delay_secs) in RETRY_DELAYS_SECS.iter().enumerate() {
            let attempt_num = attempt + 1;
            tracing::info!(
                "startup: model registry sync attempt {}/{}",
                attempt_num,
                RETRY_DELAYS_SECS.len()
            );

            let timeout = std::time::Duration::from_secs(SYNC_TIMEOUT_PER_ATTEMPT);
            match tokio::time::timeout(timeout, model_registry::startup_sync(&startup_state)).await
            {
                Ok(()) => {
                    tracing::info!(
                        "startup: model registry sync complete (attempt {})",
                        attempt_num
                    );
                    startup_state.mark_ready();
                    return;
                }
                Err(_) => {
                    last_err = format!(
                        "timed out after {}s on attempt {}",
                        SYNC_TIMEOUT_PER_ATTEMPT, attempt_num
                    );
                    tracing::warn!(
                        "startup: model registry sync {} — retrying in {}s",
                        last_err,
                        delay_secs
                    );
                }
            }

            // Wait before next retry (unless this was the last attempt)
            if attempt_num < RETRY_DELAYS_SECS.len() {
                tokio::time::sleep(std::time::Duration::from_secs(*delay_secs)).await;
            }
        }

        tracing::error!(
            "startup: model registry sync failed after {} attempts ({}) — using fallback models",
            RETRY_DELAYS_SECS.len(),
            last_err
        );
        startup_state.mark_ready();
    });

    // ── Spawn background watchdog ──
    let _watchdog = watchdog::spawn(state.clone());

    let port: u16 = std::env::var("PORT")
        .unwrap_or_else(|_| "8081".to_string())
        .parse()?;
    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));

    print_banner(port);
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

// Jaskier Shared Pattern -- enable ANSI colors on Windows consoles
#[cfg(windows)]
fn enable_ansi() {
    use windows::Win32::System::Console::{
        GetConsoleMode, GetStdHandle, SetConsoleMode, ENABLE_VIRTUAL_TERMINAL_PROCESSING,
        STD_ERROR_HANDLE, STD_OUTPUT_HANDLE,
    };
    for std_handle in [STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
        unsafe {
            let Ok(handle) = GetStdHandle(std_handle) else {
                continue;
            };
            let mut mode = Default::default();
            if GetConsoleMode(handle, &mut mode).is_ok() {
                let _ = SetConsoleMode(handle, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING);
            }
        }
    }
}
#[cfg(not(windows))]
fn enable_ansi() {}

fn print_banner(port: u16) {
    // GeminiHydra: bold cyan (36)
    println!();
    println!("  \x1b[1;36m>>>  GEMINIHYDRA v15  <<<\x1b[0m");
    println!("  \x1b[36mMulti-Agent AI Swarm\x1b[0m");
    println!("  \x1b[1;32mhttp://localhost:{port}\x1b[0m");
    println!();
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
