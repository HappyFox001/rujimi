use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiCallRecord {
    pub timestamp: SystemTime,
    pub model: String,
    pub tokens_used: u32,
    pub success: bool,
    pub response_time_ms: u64,
    pub ip_address: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiStats {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub total_tokens: u64,
    pub requests_last_minute: u32,
    pub requests_last_hour: u32,
    pub requests_last_day: u32,
    pub average_response_time: f64,
}

impl Default for ApiStats {
    fn default() -> Self {
        Self {
            total_requests: 0,
            successful_requests: 0,
            failed_requests: 0,
            total_tokens: 0,
            requests_last_minute: 0,
            requests_last_hour: 0,
            requests_last_day: 0,
            average_response_time: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelStats {
    pub model_name: String,
    pub request_count: u64,
    pub token_count: u64,
    pub success_rate: f64,
    pub average_response_time: f64,
}

#[derive(Debug, Clone)]
pub struct ApiStatsManager {
    call_records: Arc<RwLock<Vec<ApiCallRecord>>>,
    model_stats: Arc<DashMap<String, ModelStats>>,
    cached_stats: Arc<RwLock<ApiStats>>,
    last_cleanup: Arc<RwLock<SystemTime>>,
}

impl ApiStatsManager {
    pub fn new() -> Self {
        Self {
            call_records: Arc::new(RwLock::new(Vec::new())),
            model_stats: Arc::new(DashMap::new()),
            cached_stats: Arc::new(RwLock::new(ApiStats::default())),
            last_cleanup: Arc::new(RwLock::new(SystemTime::now())),
        }
    }

    pub async fn record_api_call(
        &self,
        model: String,
        tokens_used: u32,
        success: bool,
        response_time_ms: u64,
        ip_address: Option<String>,
    ) {
        let record = ApiCallRecord {
            timestamp: SystemTime::now(),
            model: model.clone(),
            tokens_used,
            success,
            response_time_ms,
            ip_address,
        };

        // Add to call records
        {
            let mut records = self.call_records.write().await;
            records.push(record);

            // Keep only recent records (last 7 days)
            let cutoff = SystemTime::now() - Duration::from_secs(7 * 24 * 3600);
            records.retain(|r| r.timestamp > cutoff);
        }

        // Update model-specific stats
        self.update_model_stats(&model, tokens_used, success, response_time_ms).await;

        // Update cached global stats
        self.update_cached_stats().await;
    }

    async fn update_model_stats(&self, model: &str, tokens: u32, success: bool, response_time: u64) {
        let mut stats = self.model_stats.entry(model.to_string()).or_insert_with(|| ModelStats {
            model_name: model.to_string(),
            request_count: 0,
            token_count: 0,
            success_rate: 100.0,
            average_response_time: 0.0,
        });

        let old_count = stats.request_count;
        let old_avg_time = stats.average_response_time;

        stats.request_count += 1;
        stats.token_count += tokens as u64;

        // Update success rate
        let successful_requests = if success {
            (stats.success_rate * old_count as f64 / 100.0) + 1.0
        } else {
            stats.success_rate * old_count as f64 / 100.0
        };
        stats.success_rate = (successful_requests / stats.request_count as f64) * 100.0;

        // Update average response time
        stats.average_response_time = (old_avg_time * old_count as f64 + response_time as f64) / stats.request_count as f64;
    }

    async fn update_cached_stats(&self) {
        let records = self.call_records.read().await;
        let now = SystemTime::now();

        let minute_ago = now - Duration::from_secs(60);
        let hour_ago = now - Duration::from_secs(3600);
        let day_ago = now - Duration::from_secs(86400);

        let mut stats = ApiStats::default();

        stats.total_requests = records.len() as u64;

        let mut total_response_time = 0u64;
        let mut response_count = 0u64;

        for record in records.iter() {
            // Count successful/failed requests
            if record.success {
                stats.successful_requests += 1;
            } else {
                stats.failed_requests += 1;
            }

            // Count tokens
            stats.total_tokens += record.tokens_used as u64;

            // Calculate average response time
            total_response_time += record.response_time_ms;
            response_count += 1;

            // Count requests in time windows
            if record.timestamp > minute_ago {
                stats.requests_last_minute += 1;
            }
            if record.timestamp > hour_ago {
                stats.requests_last_hour += 1;
            }
            if record.timestamp > day_ago {
                stats.requests_last_day += 1;
            }
        }

        if response_count > 0 {
            stats.average_response_time = total_response_time as f64 / response_count as f64;
        }

        let mut cached_stats = self.cached_stats.write().await;
        *cached_stats = stats;
    }

    pub async fn get_stats(&self) -> ApiStats {
        let cached_stats = self.cached_stats.read().await;
        cached_stats.clone()
    }

    pub async fn get_model_stats(&self) -> Vec<ModelStats> {
        self.model_stats
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
    }

    pub async fn get_recent_calls(&self, limit: usize) -> Vec<ApiCallRecord> {
        let records = self.call_records.read().await;
        records
            .iter()
            .rev()
            .take(limit)
            .cloned()
            .collect()
    }

    pub async fn clear_stats(&self) {
        {
            let mut records = self.call_records.write().await;
            records.clear();
        }

        self.model_stats.clear();

        {
            let mut cached_stats = self.cached_stats.write().await;
            *cached_stats = ApiStats::default();
        }

        info!("API statistics cleared");
    }

    pub async fn get_requests_per_ip_last_day(&self) -> std::collections::HashMap<String, u32> {
        let records = self.call_records.read().await;
        let day_ago = SystemTime::now() - Duration::from_secs(86400);

        let mut ip_counts = std::collections::HashMap::new();

        for record in records.iter() {
            if record.timestamp > day_ago {
                if let Some(ip) = &record.ip_address {
                    *ip_counts.entry(ip.clone()).or_insert(0) += 1;
                }
            }
        }

        ip_counts
    }

    pub async fn get_requests_for_ip_last_day(&self, ip: &str) -> u32 {
        let ip_counts = self.get_requests_per_ip_last_day().await;
        ip_counts.get(ip).copied().unwrap_or(0)
    }

    pub async fn start_cleanup_task(self: Arc<Self>) {
        let mut interval = tokio::time::interval(Duration::from_secs(3600)); // Clean up every hour

        loop {
            interval.tick().await;

            let now = SystemTime::now();
            let mut last_cleanup = self.last_cleanup.write().await;

            // Only clean up if it's been at least an hour since last cleanup
            if now.duration_since(*last_cleanup).unwrap_or(Duration::ZERO) > Duration::from_secs(3600) {
                self.cleanup_old_records().await;
                *last_cleanup = now;
            }
        }
    }

    async fn cleanup_old_records(&self) {
        let cutoff = SystemTime::now() - Duration::from_secs(7 * 24 * 3600); // Keep 7 days

        let mut records = self.call_records.write().await;
        let old_count = records.len();
        records.retain(|r| r.timestamp > cutoff);
        let new_count = records.len();

        if old_count != new_count {
            info!("Cleaned up {} old API call records", old_count - new_count);
            drop(records); // Release the lock before updating cached stats
            self.update_cached_stats().await;
        }
    }

    // Get time series data for charts (last 24 hours, hourly buckets)
    pub async fn get_hourly_stats(&self) -> Vec<(SystemTime, u32, u64)> {
        let records = self.call_records.read().await;
        let now = SystemTime::now();
        let mut hourly_data = Vec::new();

        for hour in (0..24).rev() {
            let hour_start = now - Duration::from_secs(hour * 3600);
            let hour_end = hour_start + Duration::from_secs(3600);

            let mut request_count = 0u32;
            let mut token_count = 0u64;

            for record in records.iter() {
                if record.timestamp >= hour_start && record.timestamp < hour_end {
                    request_count += 1;
                    token_count += record.tokens_used as u64;
                }
            }

            hourly_data.push((hour_start, request_count, token_count));
        }

        hourly_data
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_api_stats_manager() {
        let manager = ApiStatsManager::new();

        // Record some API calls
        manager.record_api_call(
            "gpt-4".to_string(),
            100,
            true,
            500,
            Some("127.0.0.1".to_string()),
        ).await;

        manager.record_api_call(
            "gpt-3.5".to_string(),
            50,
            false,
            1000,
            Some("127.0.0.1".to_string()),
        ).await;

        let stats = manager.get_stats().await;
        assert_eq!(stats.total_requests, 2);
        assert_eq!(stats.successful_requests, 1);
        assert_eq!(stats.failed_requests, 1);
        assert_eq!(stats.total_tokens, 150);

        let model_stats = manager.get_model_stats().await;
        assert_eq!(model_stats.len(), 2);
    }
}