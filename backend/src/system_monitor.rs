// Jaskier Shared Pattern -- system_monitor
//
// Windows-native CPU monitoring via GetSystemTimes + sysinfo memory stats.
// Spawns a background task that refreshes a `SystemSnapshot` every 5 seconds.

use std::sync::Arc;
use tokio::sync::RwLock;

use crate::state::SystemSnapshot;

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

/// Spawn a background task that refreshes system stats every 5 seconds.
///
/// On Windows, CPU usage is measured via the native `GetSystemTimes` API
/// (sysinfo returns incorrect values on Win11 26200).
/// On other platforms, sysinfo's per-core average is used instead.
pub fn spawn(system_monitor: Arc<RwLock<SystemSnapshot>>) {
    tokio::spawn(async move {
        let mut sys = sysinfo::System::new_all();

        // CPU: Windows-native GetSystemTimes
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

            let snap = SystemSnapshot {
                cpu_usage_percent: cpu,
                memory_used_mb: sys.used_memory() as f64 / 1_048_576.0,
                memory_total_mb: sys.total_memory() as f64 / 1_048_576.0,
                platform: std::env::consts::OS.to_string(),
            };

            *system_monitor.write().await = snap;
        }
    });
}
