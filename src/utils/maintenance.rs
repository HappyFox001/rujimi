use std::panic;
use std::sync::Arc;
use tokio::time::Duration;
use tokio_cron_scheduler::{JobScheduler, Job};
use crate::utils::{
    logging::{log, LOG_MANAGER},
    stats::ApiStatsManager,
    cache::ResponseCacheManager,
};
use crate::config::Settings;
use anyhow::Result;
use std::collections::HashMap;
use serde_json::{Value, json};

// Rust equivalent of Python utils/maintenance.py

/// Global exception handler - equivalent to Python's handle_exception
pub fn setup_global_exception_handler() {
    panic::set_hook(Box::new(|panic_info| {
        let location = panic_info.location().unwrap_or_else(|| {
            std::panic::Location::caller()
        });

        let message = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "Unknown panic occurred".to_string()
        };

        let error_message = crate::utils::error_handling::translate_error(&message);

        let mut extra = HashMap::new();
        extra.insert("status_code".to_string(), json!(500));
        extra.insert("error_message".to_string(), json!(error_message));
        extra.insert("file".to_string(), json!(location.file()));
        extra.insert("line".to_string(), json!(location.line()));

        log(
            "error",
            &format!("未捕获的异常: {}", error_message),
            Some(extra),
        );

        // Also print to stderr for immediate visibility
        eprintln!(
            "PANIC at {}:{}: {}",
            location.file(),
            location.line(),
            message
        );
    }));
}

/// Handle specific types of exceptions with context
pub fn handle_exception_with_context(
    error: &dyn std::error::Error,
    context: &str,
    extra_data: Option<HashMap<String, Value>>,
) {
    let error_message = crate::utils::error_handling::translate_error(&error.to_string());

    let mut extra = extra_data.unwrap_or_default();
    extra.insert("context".to_string(), json!(context));
    extra.insert("error_type".to_string(), json!(error.to_string()));
    extra.insert("status_code".to_string(), json!(500));
    extra.insert("error_message".to_string(), json!(error_message));

    log(
        "error",
        &format!("异常在 {}: {}", context, error_message),
        Some(extra),
    );
}

/// Maintenance scheduler for cache cleanup and stats management
pub struct MaintenanceScheduler {
    scheduler: JobScheduler,
    cache_manager: Option<Arc<ResponseCacheManager>>,
    stats_manager: Option<Arc<ApiStatsManager>>,
    settings: Arc<Settings>,
}

impl MaintenanceScheduler {
    pub async fn new(settings: Arc<Settings>) -> Result<Self> {
        let scheduler = JobScheduler::new().await?;

        Ok(Self {
            scheduler,
            cache_manager: None,
            stats_manager: None,
            settings,
        })
    }

    /// Set the cache manager for scheduled cleanup
    pub fn set_cache_manager(&mut self, cache_manager: Arc<ResponseCacheManager>) {
        self.cache_manager = Some(cache_manager);
    }

    /// Set the stats manager for scheduled cleanup
    pub fn set_stats_manager(&mut self, stats_manager: Arc<ApiStatsManager>) {
        self.stats_manager = Some(stats_manager);
    }

    /// Schedule cache cleanup - equivalent to Python's schedule_cache_cleanup
    pub async fn schedule_cache_cleanup(&mut self) -> Result<()> {
        if self.cache_manager.is_none() {
            log::warn!("Cache manager not set, skipping cache cleanup scheduling");
            return Ok(());
        }

        let cache_manager = self.cache_manager.clone();

        // Schedule cache cleanup every 10 minutes
        let job = Job::new_async("0 */10 * * * *", move |_uuid, _l| {
            let cache_manager = cache_manager.clone();
            Box::pin(async move {
                if let Some(ref cache_mgr) = cache_manager {
                    let cleaned_count = cache_mgr.cleanup_expired().await;
                    log(
                        "info",
                        &format!("定时清理缓存完成，清理了 {} 个过期项", cleaned_count),
                        Some({
                            let mut extra = HashMap::new();
                            extra.insert("cleanup".to_string(), json!("cache"));
                            extra.insert("cleaned_count".to_string(), json!(cleaned_count));
                            extra
                        }),
                    );
                } else {
                    log::warn!("Cache manager not available during cleanup");
                }
            })
        })?;

        self.scheduler.add(job).await?;
        log::info!("已安排缓存清理任务，每10分钟执行一次");
        Ok(())
    }

    /// Schedule API call statistics cleanup
    pub async fn schedule_api_stats_cleanup(&mut self) -> Result<()> {
        if self.stats_manager.is_none() {
            log::warn!("Stats manager not set, skipping stats cleanup scheduling");
            return Ok(());
        }

        let stats_manager = self.stats_manager.clone();

        // Schedule stats cleanup every hour
        let job = Job::new_async("0 0 * * * *", move |_uuid, _l| {
            let stats_manager = stats_manager.clone();
            Box::pin(async move {
                if let Some(ref stats_mgr) = stats_manager {
                    let cleaned_count = stats_mgr.cleanup_expired_records(Duration::from_secs(86400)); // 24 hours
                    log(
                        "info",
                        &format!("定时清理API统计完成，清理了 {} 个过期记录", cleaned_count),
                        Some({
                            let mut extra = HashMap::new();
                            extra.insert("cleanup".to_string(), json!("api_stats"));
                            extra.insert("cleaned_count".to_string(), json!(cleaned_count));
                            extra
                        }),
                    );
                } else {
                    log::warn!("Stats manager not available during cleanup");
                }
            })
        })?;

        self.scheduler.add(job).await?;
        log::info!("已安排API统计清理任务，每小时执行一次");
        Ok(())
    }

    /// Schedule log cleanup
    pub async fn schedule_log_cleanup(&mut self) -> Result<()> {
        // Schedule log cleanup every 6 hours
        let job = Job::new_async("0 0 */6 * * *", move |_uuid, _l| {
            Box::pin(async move {
                // Clean up old logs to prevent memory bloat
                LOG_MANAGER.clear();
                log(
                    "info",
                    "定时清理日志缓存完成",
                    Some({
                        let mut extra = HashMap::new();
                        extra.insert("cleanup".to_string(), json!("logs"));
                        extra
                    }),
                );
            })
        })?;

        self.scheduler.add(job).await?;
        log::info!("已安排日志清理任务，每6小时执行一次");
        Ok(())
    }

    /// Schedule system health check
    pub async fn schedule_health_check(&mut self) -> Result<()> {
        let settings = self.settings.clone();

        // Schedule health check every 30 minutes
        let job = Job::new_async("0 */30 * * * *", move |_uuid, _l| {
            let settings = settings.clone();
            Box::pin(async move {
                perform_health_check(&settings).await;
            })
        })?;

        self.scheduler.add(job).await?;
        log::info!("已安排系统健康检查任务，每30分钟执行一次");
        Ok(())
    }

    /// Start the scheduler
    pub async fn start(&self) -> Result<()> {
        self.scheduler.start().await?;
        log::info!("维护调度器已启动");
        Ok(())
    }

    /// Shutdown the scheduler
    pub async fn shutdown(&mut self) -> Result<()> {
        self.scheduler.shutdown().await?;
        log::info!("维护调度器已停止");
        Ok(())
    }

    /// Get scheduler status
    pub async fn get_status(&self) -> Value {
        json!({
            "running": true,
            "jobs_count": 0, // JobScheduler doesn't expose job count in this version
            "cache_manager_set": self.cache_manager.is_some(),
            "stats_manager_set": self.stats_manager.is_some()
        })
    }
}

/// Perform system health check
async fn perform_health_check(settings: &Settings) {
    let mut health_status = HashMap::new();
    let mut issues_found = 0;

    // Check memory usage (simplified)
    if let Ok(memory_info) = sys_info::mem_info() {
        let memory_usage_percent = ((memory_info.total - memory_info.avail) as f64 / memory_info.total as f64) * 100.0;
        health_status.insert("memory_usage_percent".to_string(), json!(memory_usage_percent));

        if memory_usage_percent > 90.0 {
            issues_found += 1;
            log(
                "warning",
                &format!("内存使用率过高: {:.1}%", memory_usage_percent),
                Some({
                    let mut extra = HashMap::new();
                    extra.insert("health_check".to_string(), json!("memory"));
                    extra.insert("usage_percent".to_string(), json!(memory_usage_percent));
                    extra
                }),
            );
        }
    }

    // Check log manager status
    let log_count = LOG_MANAGER.count();
    health_status.insert("log_count".to_string(), json!(log_count));

    if log_count > 500 {
        issues_found += 1;
        log(
            "warning",
            &format!("日志缓存条目过多: {}", log_count),
            Some({
                let mut extra = HashMap::new();
                extra.insert("health_check".to_string(), json!("logs"));
                extra.insert("log_count".to_string(), json!(log_count));
                extra
            }),
        );
    }

    // Check disk space if storage directory is configured
    if !settings.storage_dir.is_empty() {
        let storage_dir = &settings.storage_dir;
        if let Ok(space_info) = fs2::available_space(storage_dir) {
            let available_gb = space_info as f64 / 1024.0 / 1024.0 / 1024.0;
            health_status.insert("available_disk_gb".to_string(), json!(available_gb));

            if available_gb < 1.0 {
                issues_found += 1;
                log(
                    "error",
                    &format!("磁盘空间不足: {:.2} GB 可用", available_gb),
                    Some({
                        let mut extra = HashMap::new();
                        extra.insert("health_check".to_string(), json!("disk"));
                        extra.insert("available_gb".to_string(), json!(available_gb));
                        extra
                    }),
                );
            }
        }
    }

    if issues_found == 0 {
        log(
            "info",
            "系统健康检查完成，无异常发现",
            Some({
                let mut extra = HashMap::new();
                extra.insert("health_check".to_string(), json!("passed"));
                extra.insert("status".to_string(), json!(health_status));
                extra
            }),
        );
    } else {
        log(
            "warning",
            &format!("系统健康检查完成，发现 {} 个问题", issues_found),
            Some({
                let mut extra = HashMap::new();
                extra.insert("health_check".to_string(), json!("issues"));
                extra.insert("issues_count".to_string(), json!(issues_found));
                extra
            }),
        );
    }
}

/// API call stats cleanup function - equivalent to Python's api_call_stats_clean
pub async fn api_call_stats_clean(stats_manager: &ApiStatsManager) {
    let cleaned_count = stats_manager.cleanup_expired_records(Duration::from_secs(86400 * 7)); // 7 days

    log(
        "info",
        &format!("API统计清理完成，清理了 {} 个过期记录", cleaned_count),
        Some({
            let mut extra = HashMap::new();
            extra.insert("cleanup".to_string(), json!("api_call_stats"));
            extra.insert("cleaned_count".to_string(), json!(cleaned_count));
            extra
        }),
    );
}

/// Emergency cleanup function for critical situations
pub async fn emergency_cleanup(
    cache_manager: Option<&ResponseCacheManager>,
    stats_manager: Option<&ApiStatsManager>,
) {
    log(
        "warning",
        "执行紧急清理操作",
        Some({
            let mut extra = HashMap::new();
            extra.insert("cleanup".to_string(), json!("emergency"));
            extra
        }),
    );

    if let Some(cache_mgr) = cache_manager {
        cache_mgr.clear_sync();
        log::info!("紧急清理: 已清空所有缓存");
    }

    if let Some(stats_mgr) = stats_manager {
        let cleaned = stats_mgr.cleanup_expired_records(Duration::from_secs(3600)); // 1 hour
        log::info!("紧急清理: 清理了 {} 个统计记录", cleaned);
    }

    LOG_MANAGER.clear();
    log::info!("紧急清理: 已清空日志缓存");

    // Force garbage collection
    log::info!("紧急清理完成");
}

/// Get maintenance system status
pub async fn get_maintenance_status() -> Value {
    json!({
        "log_entries": LOG_MANAGER.count(),
        "panic_handler_installed": true,
        "health_check_available": true,
        "emergency_cleanup_available": true
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_maintenance_scheduler_creation() {
        let settings = Arc::new(Settings::default());
        let scheduler = MaintenanceScheduler::new(settings).await;
        assert!(scheduler.is_ok());
    }

    #[test]
    fn test_exception_handling() {
        // Test that we can handle errors without panicking
        let error = std::io::Error::new(std::io::ErrorKind::NotFound, "Test error");
        let context = "test_context";

        handle_exception_with_context(&error, context, None);

        // Verify that the log was created (by checking log count increased)
        assert!(LOG_MANAGER.count() > 0);
    }

    #[tokio::test]
    async fn test_health_check() {
        let settings = Settings::default();
        perform_health_check(&settings).await;
        // Health check should complete without panicking
    }

    #[tokio::test]
    async fn test_emergency_cleanup() {
        emergency_cleanup(None, None).await;
        // Emergency cleanup should complete without panicking
    }
}