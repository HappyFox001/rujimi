use anyhow::Result;
use dashmap::DashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;
use tracing::{debug, warn};

#[derive(Debug, Clone)]
pub struct RateLimiter {
    // IP-based rate limiting
    ip_requests: Arc<DashMap<String, Vec<SystemTime>>>,
    // Global rate limiting
    global_requests: Arc<RwLock<Vec<SystemTime>>>,
    // Configuration
    max_requests_per_minute: u32,
    max_requests_per_day_per_ip: u32,
}

impl RateLimiter {
    pub fn new(max_requests_per_minute: u32, max_requests_per_day_per_ip: u32) -> Self {
        Self {
            ip_requests: Arc::new(DashMap::new()),
            global_requests: Arc::new(RwLock::new(Vec::new())),
            max_requests_per_minute,
            max_requests_per_day_per_ip,
        }
    }

    pub async fn check_rate_limit(&self, ip: Option<&str>) -> Result<(), RateLimitError> {
        let now = SystemTime::now();

        // Check global rate limit (per minute)
        if let Err(e) = self.check_global_rate_limit(now).await {
            return Err(e);
        }

        // Check IP-specific rate limit (per day)
        if let Some(ip_addr) = ip {
            if let Err(e) = self.check_ip_rate_limit(ip_addr, now).await {
                return Err(e);
            }
        }

        Ok(())
    }

    async fn check_global_rate_limit(&self, now: SystemTime) -> Result<(), RateLimitError> {
        let mut global_requests = self.global_requests.write().await;

        // Remove requests older than 1 minute
        let minute_ago = now - Duration::from_secs(60);
        global_requests.retain(|&time| time > minute_ago);

        // Check if we've exceeded the limit
        if global_requests.len() >= self.max_requests_per_minute as usize {
            warn!("Global rate limit exceeded: {} requests in the last minute", global_requests.len());
            return Err(RateLimitError::GlobalLimitExceeded {
                limit: self.max_requests_per_minute,
                current: global_requests.len() as u32,
            });
        }

        // Add current request
        global_requests.push(now);
        debug!("Global requests in last minute: {}", global_requests.len());

        Ok(())
    }

    async fn check_ip_rate_limit(&self, ip: &str, now: SystemTime) -> Result<(), RateLimitError> {
        let mut ip_requests = self.ip_requests.entry(ip.to_string()).or_insert_with(Vec::new);

        // Remove requests older than 24 hours
        let day_ago = now - Duration::from_secs(24 * 60 * 60);
        ip_requests.retain(|&time| time > day_ago);

        // Check if we've exceeded the limit
        if ip_requests.len() >= self.max_requests_per_day_per_ip as usize {
            warn!("IP rate limit exceeded for {}: {} requests in the last day", ip, ip_requests.len());
            return Err(RateLimitError::IpLimitExceeded {
                ip: ip.to_string(),
                limit: self.max_requests_per_day_per_ip,
                current: ip_requests.len() as u32,
            });
        }

        // Add current request
        ip_requests.push(now);
        debug!("Requests for IP {} in last day: {}", ip, ip_requests.len());

        Ok(())
    }

    pub async fn get_rate_limit_info(&self, ip: Option<&str>) -> RateLimitInfo {
        let now = SystemTime::now();

        // Get global info
        let global_requests = self.global_requests.read().await;
        let minute_ago = now - Duration::from_secs(60);
        let global_count = global_requests.iter().filter(|&&time| time > minute_ago).count();

        let mut ip_count = 0;
        if let Some(ip_addr) = ip {
            if let Some(ip_requests) = self.ip_requests.get(ip_addr) {
                let day_ago = now - Duration::from_secs(24 * 60 * 60);
                ip_count = ip_requests.iter().filter(|&&time| time > day_ago).count();
            }
        }

        RateLimitInfo {
            global_requests_per_minute: global_count as u32,
            global_limit_per_minute: self.max_requests_per_minute,
            ip_requests_per_day: ip_count as u32,
            ip_limit_per_day: self.max_requests_per_day_per_ip,
        }
    }

    // Background cleanup task
    pub async fn start_cleanup_task(self: Arc<Self>) {
        let mut interval = tokio::time::interval(Duration::from_secs(300)); // Clean up every 5 minutes

        loop {
            interval.tick().await;
            self.cleanup_old_entries().await;
        }
    }

    async fn cleanup_old_entries(&self) {
        let now = SystemTime::now();
        let day_ago = now - Duration::from_secs(24 * 60 * 60);
        let minute_ago = now - Duration::from_secs(60);

        // Clean up IP requests
        let mut removed_ips = Vec::new();
        for mut entry in self.ip_requests.iter_mut() {
            let ip = entry.key().clone();
            let requests = entry.value_mut();

            let old_len = requests.len();
            requests.retain(|&time| time > day_ago);

            if requests.is_empty() {
                removed_ips.push(ip);
            } else if requests.len() != old_len {
                debug!("Cleaned up {} old requests for IP {}", old_len - requests.len(), entry.key());
            }
        }

        // Remove empty IP entries
        for ip in removed_ips {
            self.ip_requests.remove(&ip);
        }

        // Clean up global requests
        {
            let mut global_requests = self.global_requests.write().await;
            let old_len = global_requests.len();
            global_requests.retain(|&time| time > minute_ago);
            if global_requests.len() != old_len {
                debug!("Cleaned up {} old global requests", old_len - global_requests.len());
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct RateLimitInfo {
    pub global_requests_per_minute: u32,
    pub global_limit_per_minute: u32,
    pub ip_requests_per_day: u32,
    pub ip_limit_per_day: u32,
}

#[derive(Debug, thiserror::Error)]
pub enum RateLimitError {
    #[error("Global rate limit exceeded: {current}/{limit} requests per minute")]
    GlobalLimitExceeded { limit: u32, current: u32 },

    #[error("IP rate limit exceeded for {ip}: {current}/{limit} requests per day")]
    IpLimitExceeded {
        ip: String,
        limit: u32,
        current: u32,
    },
}

impl RateLimitError {
    pub fn status_code(&self) -> u16 {
        429 // Too Many Requests
    }

    pub fn retry_after_seconds(&self) -> u64 {
        match self {
            RateLimitError::GlobalLimitExceeded { .. } => 60, // Retry after 1 minute
            RateLimitError::IpLimitExceeded { .. } => 3600,   // Retry after 1 hour
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    

    #[tokio::test]
    async fn test_global_rate_limiting() {
        let limiter = RateLimiter::new(2, 10); // 2 per minute, 10 per day per IP

        // First two requests should succeed
        assert!(limiter.check_rate_limit(Some("127.0.0.1")).await.is_ok());
        assert!(limiter.check_rate_limit(Some("127.0.0.1")).await.is_ok());

        // Third request should fail
        assert!(limiter.check_rate_limit(Some("127.0.0.1")).await.is_err());
    }

    #[tokio::test]
    async fn test_ip_rate_limiting() {
        let limiter = RateLimiter::new(100, 2); // 100 per minute, 2 per day per IP

        // First two requests should succeed
        assert!(limiter.check_rate_limit(Some("127.0.0.1")).await.is_ok());
        assert!(limiter.check_rate_limit(Some("127.0.0.1")).await.is_ok());

        // Third request should fail
        assert!(limiter.check_rate_limit(Some("127.0.0.1")).await.is_err());

        // Different IP should still work
        assert!(limiter.check_rate_limit(Some("192.168.1.1")).await.is_ok());
    }

    #[tokio::test]
    async fn test_rate_limit_info() {
        let limiter = RateLimiter::new(10, 100);

        let info = limiter.get_rate_limit_info(Some("127.0.0.1")).await;
        assert_eq!(info.global_requests_per_minute, 0);
        assert_eq!(info.ip_requests_per_day, 0);

        // Make a request
        let _ = limiter.check_rate_limit(Some("127.0.0.1")).await;

        let info = limiter.get_rate_limit_info(Some("127.0.0.1")).await;
        assert_eq!(info.global_requests_per_minute, 1);
        assert_eq!(info.ip_requests_per_day, 1);
    }
}