// Jaskier Shared Pattern — state
// GeminiHydra v15 - Application state

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Instant;

use reqwest::Client;
use sqlx::PgPool;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

use crate::model_registry::ModelCache;
use crate::models::WitcherAgent;

// ── Log Ring Buffer — Jaskier Shared Pattern ────────────────────────────────
/// In-memory ring buffer for backend log entries (last N events).
/// Uses `std::sync::Mutex` because writes happen in the tracing Layer
/// (sync context — not inside a tokio runtime poll).

#[derive(Clone, serde::Serialize)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: String,
    pub target: String,
    pub message: String,
}

pub struct LogRingBuffer {
    entries: std::sync::Mutex<VecDeque<LogEntry>>,
    capacity: usize,
}

impl LogRingBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::sync::Mutex::new(VecDeque::with_capacity(capacity)),
            capacity,
        }
    }

    pub fn push(&self, entry: LogEntry) {
        let mut buf = self.entries.lock().unwrap_or_else(|p| p.into_inner());
        if buf.len() >= self.capacity {
            buf.pop_front();
        }
        buf.push_back(entry);
    }

    pub fn recent(&self, limit: usize, min_level: Option<&str>, search: Option<&str>) -> Vec<LogEntry> {
        let buf = self.entries.lock().unwrap_or_else(|p| p.into_inner());
        buf.iter()
            .rev()
            .filter(|e| {
                min_level.map_or(true, |lvl| level_ord(&e.level) >= level_ord(lvl))
            })
            .filter(|e| {
                search.map_or(true, |s| {
                    let s_lower = s.to_lowercase();
                    e.message.to_lowercase().contains(&s_lower)
                        || e.target.to_lowercase().contains(&s_lower)
                })
            })
            .take(limit)
            .cloned()
            .collect()
    }
}

fn level_ord(level: &str) -> u8 {
    match level.to_uppercase().as_str() {
        "ERROR" => 5,
        "WARN" => 4,
        "INFO" => 3,
        "DEBUG" => 2,
        "TRACE" => 1,
        _ => 0,
    }
}

// ── Circuit Breaker ─────────────────────────────────────────────────────────
// Jaskier Shared Pattern -- circuit_breaker
//
// Simple circuit breaker for external API providers.
// After `FAILURE_THRESHOLD` consecutive failures the circuit trips (OPEN) and
// all requests fail fast for `RECOVERY_TIMEOUT` seconds. Once that window
// elapses the breaker moves to HALF-OPEN: the next call is allowed through
// and either resets the breaker (on success) or trips it again.

const FAILURE_THRESHOLD: u32 = 3;
const RECOVERY_TIMEOUT_SECS: u64 = 60;

/// Circuit states (encoded as u32 for lock-free atomic access).
/// 0 = CLOSED (healthy), 1 = OPEN (tripped), 2 = HALF_OPEN (probing).
const STATE_CLOSED: u32 = 0;
const STATE_OPEN: u32 = 1;
const STATE_HALF_OPEN: u32 = 2;

#[derive(Debug)]
pub struct CircuitBreaker {
    /// Current state: CLOSED / OPEN / HALF_OPEN.
    state: AtomicU32,
    /// Consecutive failure count.
    consecutive_failures: AtomicU32,
    /// Instant when the circuit was last tripped (OPEN). Protected by RwLock
    /// because `Instant` is not atomic but writes are rare (only on state change).
    last_failure_time: RwLock<Option<Instant>>,
    /// Human-readable label for log messages.
    provider: String,
}

impl CircuitBreaker {
    pub fn new(provider: &str) -> Self {
        Self {
            state: AtomicU32::new(STATE_CLOSED),
            consecutive_failures: AtomicU32::new(0),
            last_failure_time: RwLock::new(None),
            provider: provider.to_string(),
        }
    }

    /// Check whether a request is allowed through.
    /// Returns `Ok(())` if the circuit is CLOSED or HALF_OPEN, or
    /// `Err(message)` if OPEN and the recovery window hasn't elapsed.
    pub async fn check(&self) -> Result<(), String> {
        let current = self.state.load(Ordering::Acquire);

        if current == STATE_CLOSED {
            return Ok(());
        }

        // OPEN — check if recovery timeout has elapsed.
        if current == STATE_OPEN {
            let lock = self.last_failure_time.read().await;
            if let Some(t) = *lock {
                if t.elapsed().as_secs() >= RECOVERY_TIMEOUT_SECS {
                    drop(lock);
                    // Transition to HALF_OPEN so the next request is a probe.
                    self.state.store(STATE_HALF_OPEN, Ordering::Release);
                    tracing::info!(
                        "circuit_breaker[{}]: OPEN -> HALF_OPEN (recovery window elapsed)",
                        self.provider
                    );
                    return Ok(());
                }
            }
            let remaining = lock
                .map(|t| RECOVERY_TIMEOUT_SECS.saturating_sub(t.elapsed().as_secs()))
                .unwrap_or(RECOVERY_TIMEOUT_SECS);
            return Err(format!(
                "Circuit breaker OPEN for provider '{}' — failing fast (retry in ~{}s)",
                self.provider, remaining
            ));
        }

        // HALF_OPEN — allow the probe request through.
        Ok(())
    }

    /// Record a successful call. Resets failures and closes the circuit.
    pub async fn record_success(&self) {
        let prev = self.state.swap(STATE_CLOSED, Ordering::Release);
        self.consecutive_failures.store(0, Ordering::Release);
        if prev != STATE_CLOSED {
            tracing::info!(
                "circuit_breaker[{}]: {} -> CLOSED (success)",
                self.provider,
                match prev { STATE_OPEN => "OPEN", STATE_HALF_OPEN => "HALF_OPEN", _ => "?" }
            );
        }
    }

    /// Record a failed call. If the threshold is breached, trip the circuit.
    pub async fn record_failure(&self) {
        let count = self.consecutive_failures.fetch_add(1, Ordering::AcqRel) + 1;

        if count >= FAILURE_THRESHOLD {
            let prev = self.state.swap(STATE_OPEN, Ordering::Release);
            *self.last_failure_time.write().await = Some(Instant::now());
            if prev != STATE_OPEN {
                tracing::warn!(
                    "circuit_breaker[{}]: TRIPPED after {} consecutive failures — \
                     failing fast for {}s",
                    self.provider, count, RECOVERY_TIMEOUT_SECS
                );
            }
        }
    }
}

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
    /// Circuit breaker for the Gemini API provider.
    pub gemini_circuit: Arc<CircuitBreaker>,
    /// Cached system prompts keyed by "agent_id:language:model".
    /// Cleared on agent refresh for byte-identical Gemini API requests.
    pub prompt_cache: Arc<RwLock<HashMap<String, String>>>,
    /// A2A — cancellation tokens for running tasks (task_id → token).
    pub a2a_cancel_tokens: Arc<RwLock<HashMap<String, CancellationToken>>>,
    /// `false` when OAuth token was rejected by Gemini API (401/403).
    /// Causes credential resolution to skip OAuth and use API key.
    /// Reset to `true` on new OAuth login.
    pub oauth_gemini_valid: Arc<AtomicBool>,
    /// In-memory ring buffer for backend log entries (last 1000).
    pub log_buffer: Arc<LogRingBuffer>,
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
    pub async fn new(db: PgPool, log_buffer: Arc<LogRingBuffer>) -> Self {
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

        // ── Load stored API key from DB (encrypted) ─────────────────────
        // Priority: DB key overrides env var (user explicitly saved it)
        if let Ok(Some(row)) = sqlx::query_as::<_, (String, String)>(
            "SELECT auth_method, api_key_encrypted FROM gh_google_auth WHERE id = 1",
        )
        .fetch_optional(&db)
        .await
        {
            let (method, encrypted_key) = row;
            if method == "api_key" && !encrypted_key.is_empty() {
                match crate::oauth::decrypt_token(&encrypted_key) {
                    Ok(decrypted) if !decrypted.is_empty() => {
                        api_keys.insert("google".to_string(), decrypted);
                        tracing::info!("Loaded Google API key from DB (encrypted)");
                    }
                    Ok(_) => {}
                    Err(e) => tracing::warn!("Failed to decrypt stored API key: {}", e),
                }
            }
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
                .pool_max_idle_per_host(10)
                .timeout(std::time::Duration::from_secs(120))
                .connect_timeout(std::time::Duration::from_secs(5))
                .build()
                .expect("Failed to build HTTP client"),
            oauth_pkce: Arc::new(RwLock::new(None)),
            system_monitor: Arc::new(RwLock::new(SystemSnapshot::default())),
            ready: Arc::new(AtomicBool::new(false)),
            auth_secret,
            gemini_circuit: Arc::new(CircuitBreaker::new("gemini")),
            prompt_cache: Arc::new(RwLock::new(HashMap::new())),
            a2a_cancel_tokens: Arc::new(RwLock::new(HashMap::new())),
            oauth_gemini_valid: Arc::new(AtomicBool::new(true)),
            log_buffer,
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
        // Invalidate system prompt cache — agent roster changed
        self.prompt_cache.write().await.clear();
    }
}
