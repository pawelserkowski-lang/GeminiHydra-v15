use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use reqwest::Client;
use sqlx::PgPool;
use tokio::sync::RwLock;

use crate::model_registry::ModelCache;
use crate::models::WitcherAgent;

/// Mutable runtime state (not persisted — lost on restart).
pub struct RuntimeState {
    pub api_keys: HashMap<String, String>,
}

/// Temporary PKCE state for an in-progress OAuth flow.
pub struct OAuthPkceState {
    pub code_verifier: String,
    pub state: String,
}

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
    /// `true` once startup_sync completes (or times out).
    pub ready: Arc<AtomicBool>,
}

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
            client: Client::new(),
            oauth_pkce: Arc::new(RwLock::new(None)),
            ready: Arc::new(AtomicBool::new(false)),
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
