// Jaskier Shared Pattern — state
// GeminiHydra v15 - Application state

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use reqwest::Client;
use sqlx::PgPool;
use tokio::sync::RwLock;

use crate::model_registry::ModelCache;
use crate::models::WitcherAgent;

// ── Shared: SystemSnapshot ───────────────────────────────────────────────────
/// Cached system statistics snapshot, refreshed every 5s by background task.
#[derive(Clone)]
pub struct SystemSnapshot {
    pub cpu_usage_percent: f32,
    pub memory_used_mb: f64,
    pub memory_total_mb: f64,
    pub platform: String,
}

impl Default for SystemSnapshot {
    fn default() -> Self {
        Self {
            cpu_usage_percent: 0.0,
            memory_used_mb: 0.0,
            memory_total_mb: 0.0,
            platform: std::env::consts::OS.to_string(),
        }
    }
}

// ── Shared: RuntimeState ────────────────────────────────────────────────────
/// Mutable runtime state (not persisted — lost on restart).
pub struct RuntimeState {
    pub api_keys: HashMap<String, String>,
}

/// Temporary PKCE state for an in-progress OAuth flow.
pub struct OAuthPkceState {
    pub code_verifier: String,
    pub state: String,
}

// ── Shared: AppState (project-specific fields vary) ─────────────────────────
/// Central application state. Clone-friendly — PgPool and Arc are both Clone.
#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub agents: Arc<RwLock<Vec<WitcherAgent>>>,
    pub runtime: Arc<RwLock<RuntimeState>>,
    pub model_cache: Arc<RwLock<ModelCache>>,
    pub start_time: Instant,
    pub client: Client,
    pub oauth_pkce: Arc<RwLock<Option<OAuthPkceState>>>,
    /// Cached system stats (CPU, memory) refreshed every 5s by background task.
    pub system_monitor: Arc<RwLock<SystemSnapshot>>,
    /// `true` once startup_sync completes (or times out).
    pub ready: Arc<AtomicBool>,
    /// Optional auth secret from AUTH_SECRET env. None = dev mode (no auth).
    pub auth_secret: Option<String>,
}

// ── Shared: readiness helpers ───────────────────────────────────────────────
impl AppState {
    pub fn is_ready(&self) -> bool {
        self.ready.load(Ordering::Relaxed)
    }

    pub fn mark_ready(&self) {
        self.ready.store(true, Ordering::Relaxed);
        tracing::info!("Backend marked as READY");
    }
}

impl AppState {
    pub async fn new(db: PgPool) -> Self {
        // ── API keys from environment ──────────────────────────────────
        let mut api_keys = HashMap::new();

        if let Ok(key) = std::env::var("GOOGLE_API_KEY")
            .or_else(|_| std::env::var("GEMINI_API_KEY"))
        {
            api_keys.insert("google".to_string(), key);
        }

        if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
            api_keys.insert("anthropic".to_string(), key);
        }

        // ── Load agents from DB ────────────────────────────────────────
        let agents_vec = sqlx::query_as::<_, WitcherAgent>("SELECT * FROM gh_agents ORDER BY created_at ASC")
            .fetch_all(&db)
            .await
            .unwrap_or_else(|e| {
                tracing::error!("Failed to load agents from DB: {}", e);
                vec![]
            });

        let auth_secret = std::env::var("AUTH_SECRET").ok().filter(|s| !s.is_empty());
        if auth_secret.is_some() {
            tracing::info!("AUTH_SECRET configured — authentication enabled");
        } else {
            tracing::info!("AUTH_SECRET not set — authentication disabled (dev mode)");
        }

        tracing::info!(
            "AppState initialised — {} agents loaded, keys: {:?}",
            agents_vec.len(),
            api_keys.keys().collect::<Vec<_>>()
        );

        Self {
            db,
            agents: Arc::new(RwLock::new(agents_vec)),
            runtime: Arc::new(RwLock::new(RuntimeState { api_keys })),
            model_cache: Arc::new(RwLock::new(ModelCache::new())),
            start_time: Instant::now(),
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(120))
                .connect_timeout(std::time::Duration::from_secs(5))
                .build()
                .expect("Failed to build HTTP client"),
            oauth_pkce: Arc::new(RwLock::new(None)),
            system_monitor: Arc::new(RwLock::new(SystemSnapshot::default())),
            ready: Arc::new(AtomicBool::new(false)),
            auth_secret,
        }
    }

    /// Refresh agents cache from DB
    pub async fn refresh_agents(&self) {
        if let Ok(new_list) = sqlx::query_as::<_, WitcherAgent>("SELECT * FROM gh_agents ORDER BY created_at ASC")
            .fetch_all(&self.db)
            .await
        {
            let mut lock = self.agents.write().await;
            *lock = new_list;
        }
    }
}
