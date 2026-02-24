// GeminiHydra v15 — Background watchdog
//
// Periodically checks backend health and performs auto-recovery:
// - DB connectivity ping (SELECT 1)
// - Model cache staleness check + auto-refresh
// - Logs health status for external monitoring

use std::time::Duration;

use crate::model_registry;
use crate::state::AppState;

const CHECK_INTERVAL: Duration = Duration::from_secs(60);
const DB_PING_TIMEOUT: Duration = Duration::from_secs(5);

pub fn spawn(state: AppState) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        tracing::info!("watchdog: started (interval={}s)", CHECK_INTERVAL.as_secs());

        loop {
            tokio::time::sleep(CHECK_INTERVAL).await;

            let db_ok = check_db(&state).await;
            let cache_ok = check_and_refresh_cache(&state).await;

            if db_ok && cache_ok {
                tracing::debug!("watchdog: all checks passed");
            } else {
                tracing::warn!(
                    "watchdog: db={} cache={}",
                    if db_ok { "ok" } else { "FAIL" },
                    if cache_ok { "ok" } else { "REFRESHED" },
                );
            }
        }
    })
}

async fn check_db(state: &AppState) -> bool {
    let result = tokio::time::timeout(
        DB_PING_TIMEOUT,
        sqlx::query_scalar::<_, i32>("SELECT 1").fetch_one(&state.db),
    )
    .await;

    match result {
        Ok(Ok(_)) => true,
        Ok(Err(e)) => {
            tracing::error!("watchdog: DB ping failed: {}", e);
            false
        }
        Err(_) => {
            tracing::error!("watchdog: DB ping timed out after {}s", DB_PING_TIMEOUT.as_secs());
            false
        }
    }
}

async fn check_and_refresh_cache(state: &AppState) -> bool {
    let is_stale = {
        let lock_result = tokio::time::timeout(
            Duration::from_secs(5),
            state.model_cache.read(),
        )
        .await;

        match lock_result {
            Ok(cache) => cache.is_stale(),
            Err(_) => {
                tracing::error!("watchdog: model_cache read lock timed out — possible deadlock");
                return false;
            }
        }
    };

    if is_stale {
        tracing::info!("watchdog: model cache stale, triggering refresh");
        let refresh_result = tokio::time::timeout(
            Duration::from_secs(30),
            model_registry::refresh_cache(state),
        )
        .await;

        match refresh_result {
            Ok(models) => {
                let total: usize = models.values().map(|v| v.len()).sum();
                tracing::info!("watchdog: cache refreshed — {} models from {} providers", total, models.len());
            }
            Err(_) => {
                tracing::error!("watchdog: cache refresh timed out after 30s");
            }
        }
        false
    } else {
        true
    }
}
