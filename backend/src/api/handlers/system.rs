use axum::Json;
use sysinfo::System;
use crate::models::SystemStats;

pub async fn system_stats() -> Json<SystemStats> {
    let mut sys = System::new_all();
    sys.refresh_all();
    Json(SystemStats {
        cpu_usage_percent: sys.cpus().iter().map(|c| c.cpu_usage()).sum::<f32>() / sys.cpus().len().max(1) as f32,
        memory_used_mb: sys.used_memory() as f64 / 1_048_576.0,
        memory_total_mb: sys.total_memory() as f64 / 1_048_576.0,
        platform: std::env::consts::OS.to_string(),
    })
}
