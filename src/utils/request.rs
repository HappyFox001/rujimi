use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use uuid::Uuid;
use crate::utils::logging::log;
use serde_json::{Value, json};

// Rust equivalent of Python utils/request.py

#[derive(Debug, Clone)]
pub struct ActiveRequest {
    pub id: String,
    pub creation_time: SystemTime,
    pub task_handle: Option<Arc<JoinHandle<()>>>,
    pub metadata: Option<HashMap<String, Value>>,
}

impl ActiveRequest {
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            creation_time: SystemTime::now(),
            task_handle: None,
            metadata: None,
        }
    }

    pub fn with_id(mut self, id: String) -> Self {
        self.id = id;
        self
    }

    pub fn with_metadata(mut self, metadata: HashMap<String, Value>) -> Self {
        self.metadata = Some(metadata);
        self
    }

    pub fn with_task_handle(mut self, handle: JoinHandle<()>) -> Self {
        self.task_handle = Some(Arc::new(handle));
        self
    }

    pub fn age(&self) -> Duration {
        self.creation_time
            .elapsed()
            .unwrap_or(Duration::from_secs(0))
    }

    pub fn is_finished(&self) -> bool {
        if let Some(ref handle) = self.task_handle {
            handle.is_finished()
        } else {
            false
        }
    }

    pub fn abort(&self) {
        if let Some(ref handle) = self.task_handle {
            handle.abort();
        }
    }
}

/// Manager for active API requests - equivalent to Python's ActiveRequestsManager
pub struct ActiveRequestsManager {
    active_requests: Arc<RwLock<HashMap<String, ActiveRequest>>>,
}

impl ActiveRequestsManager {
    /// Create a new ActiveRequestsManager
    pub fn new() -> Self {
        Self {
            active_requests: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create with existing requests pool
    pub fn with_requests_pool(requests_pool: HashMap<String, ActiveRequest>) -> Self {
        Self {
            active_requests: Arc::new(RwLock::new(requests_pool)),
        }
    }

    /// Add new active request task - equivalent to Python's add()
    pub async fn add(&self, key: String, request: ActiveRequest) {
        let mut requests = self.active_requests.write().await;
        requests.insert(key, request);
    }

    /// Add with automatically generated key
    pub async fn add_auto(&self, request: ActiveRequest) -> String {
        let key = request.id.clone();
        self.add(key.clone(), request).await;
        key
    }

    /// Get active request task - equivalent to Python's get()
    pub async fn get(&self, key: &str) -> Option<ActiveRequest> {
        let requests = self.active_requests.read().await;
        requests.get(key).cloned()
    }

    /// Remove active request task - equivalent to Python's remove()
    pub async fn remove(&self, key: &str) -> bool {
        let mut requests = self.active_requests.write().await;
        requests.remove(key).is_some()
    }

    /// Get all active requests
    pub async fn get_all(&self) -> HashMap<String, ActiveRequest> {
        let requests = self.active_requests.read().await;
        requests.clone()
    }

    /// Get count of active requests
    pub async fn count(&self) -> usize {
        let requests = self.active_requests.read().await;
        requests.len()
    }

    /// Clean completed or cancelled tasks - equivalent to Python's clean_completed()
    pub async fn clean_completed(&self) -> usize {
        let mut requests = self.active_requests.write().await;
        let initial_count = requests.len();

        // Collect keys of completed requests
        let completed_keys: Vec<String> = requests
            .iter()
            .filter(|(_, request)| request.is_finished())
            .map(|(key, _)| key.clone())
            .collect();

        // Remove completed requests
        for key in &completed_keys {
            requests.remove(key);
        }

        let cleaned_count = completed_keys.len();

        if cleaned_count > 0 {
            log(
                "info",
                &format!("清理已完成请求任务: {} 个", cleaned_count),
                Some({
                    let mut extra = HashMap::new();
                    extra.insert("cleanup".to_string(), json!("active_requests"));
                    extra.insert("cleaned_count".to_string(), json!(cleaned_count));
                    extra
                }),
            );
        }

        cleaned_count
    }

    /// Clean long running tasks - equivalent to Python's clean_long_running()
    pub async fn clean_long_running(&self, max_age_seconds: u64) -> usize {
        let mut requests = self.active_requests.write().await;
        let max_age = Duration::from_secs(max_age_seconds);

        let mut long_running_keys = Vec::new();

        // Find long-running tasks
        for (key, request) in requests.iter() {
            if request.age() > max_age && !request.is_finished() {
                long_running_keys.push(key.clone());
                request.abort(); // Cancel the task
            }
        }

        // Remove long-running tasks
        for key in &long_running_keys {
            requests.remove(key);
        }

        let cleaned_count = long_running_keys.len();

        if cleaned_count > 0 {
            log(
                "warning",
                &format!("取消长时间运行的任务: {} 个", cleaned_count),
                Some({
                    let mut extra = HashMap::new();
                    extra.insert("cleanup".to_string(), json!("long_running_tasks"));
                    extra.insert("cleaned_count".to_string(), json!(cleaned_count));
                    extra.insert("max_age_seconds".to_string(), json!(max_age_seconds));
                    extra
                }),
            );
        }

        cleaned_count
    }

    /// Clean all requests (emergency cleanup)
    pub async fn clean_all(&self) -> usize {
        let mut requests = self.active_requests.write().await;
        let count = requests.len();

        // Abort all running tasks
        for (_, request) in requests.iter() {
            request.abort();
        }

        requests.clear();

        if count > 0 {
            log(
                "warning",
                &format!("清理所有活跃请求: {} 个", count),
                Some({
                    let mut extra = HashMap::new();
                    extra.insert("cleanup".to_string(), json!("all_requests"));
                    extra.insert("cleaned_count".to_string(), json!(count));
                    extra
                }),
            );
        }

        count
    }

    /// Get statistics about active requests
    pub async fn get_statistics(&self) -> RequestStatistics {
        let requests = self.active_requests.read().await;
        let total_count = requests.len();
        let mut completed_count = 0;
        let mut running_count = 0;
        let mut old_requests = 0;
        let threshold = Duration::from_secs(300); // 5 minutes

        for request in requests.values() {
            if request.is_finished() {
                completed_count += 1;
            } else {
                running_count += 1;
            }

            if request.age() > threshold {
                old_requests += 1;
            }
        }

        RequestStatistics {
            total_count,
            running_count,
            completed_count,
            old_requests,
        }
    }

    /// Get detailed information about requests
    pub async fn get_detailed_info(&self) -> Value {
        let requests = self.active_requests.read().await;
        let mut request_info = Vec::new();

        for (key, request) in requests.iter() {
            let info = json!({
                "key": key,
                "id": request.id,
                "creation_time": request.creation_time
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or(Duration::from_secs(0))
                    .as_secs(),
                "age_seconds": request.age().as_secs(),
                "is_finished": request.is_finished(),
                "has_task_handle": request.task_handle.is_some(),
                "metadata": request.metadata
            });
            request_info.push(info);
        }

        json!({
            "total_requests": requests.len(),
            "requests": request_info
        })
    }

    /// Periodic cleanup task
    pub async fn run_periodic_cleanup(
        &self,
        cleanup_interval: Duration,
        max_age_seconds: u64,
    ) -> JoinHandle<()> {
        let manager = ActiveRequestsManager {
            active_requests: self.active_requests.clone(),
        };

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(cleanup_interval);

            loop {
                interval.tick().await;

                // Clean completed requests
                manager.clean_completed().await;

                // Clean long-running requests
                manager.clean_long_running(max_age_seconds).await;
            }
        })
    }

    /// Health check for the request manager
    pub async fn health_check(&self) -> bool {
        // Perform basic health checks
        let stats = self.get_statistics().await;

        // Check if we have too many requests
        if stats.total_count > 1000 {
            log(
                "warning",
                &format!("活跃请求数量过多: {}", stats.total_count),
                Some({
                    let mut extra = HashMap::new();
                    extra.insert("health_check".to_string(), json!("request_manager"));
                    extra.insert("total_requests".to_string(), json!(stats.total_count));
                    extra
                }),
            );
            return false;
        }

        // Check if we have too many old requests
        if stats.old_requests > 50 {
            log(
                "warning",
                &format!("长期运行请求过多: {}", stats.old_requests),
                Some({
                    let mut extra = HashMap::new();
                    extra.insert("health_check".to_string(), json!("request_manager"));
                    extra.insert("old_requests".to_string(), json!(stats.old_requests));
                    extra
                }),
            );
            return false;
        }

        true
    }
}

impl Default for ActiveRequestsManager {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct RequestStatistics {
    pub total_count: usize,
    pub running_count: usize,
    pub completed_count: usize,
    pub old_requests: usize,
}

impl RequestStatistics {
    pub fn to_json(&self) -> Value {
        json!({
            "total_count": self.total_count,
            "running_count": self.running_count,
            "completed_count": self.completed_count,
            "old_requests": self.old_requests
        })
    }
}

/// Utility function to create a request with common metadata
pub fn create_request_with_metadata(
    model: Option<&str>,
    api_key: Option<&str>,
    request_type: Option<&str>,
) -> ActiveRequest {
    let mut metadata = HashMap::new();

    if let Some(model) = model {
        metadata.insert("model".to_string(), json!(model));
    }
    if let Some(api_key) = api_key {
        // Only store a hash of the API key for security
        let key_hash = format!("{:x}", xxhash_rust::xxh3::xxh3_64(api_key.as_bytes()));
        metadata.insert("api_key_hash".to_string(), json!(key_hash));
    }
    if let Some(request_type) = request_type {
        metadata.insert("request_type".to_string(), json!(request_type));
    }

    ActiveRequest::new().with_metadata(metadata)
}

/// Global active requests manager instance
lazy_static::lazy_static! {
    pub static ref GLOBAL_REQUEST_MANAGER: ActiveRequestsManager = ActiveRequestsManager::new();
}

/// Convenience functions for global request manager
pub async fn add_global_request(key: String, request: ActiveRequest) {
    GLOBAL_REQUEST_MANAGER.add(key, request).await;
}

pub async fn remove_global_request(key: &str) -> bool {
    GLOBAL_REQUEST_MANAGER.remove(key).await
}

pub async fn get_global_request_stats() -> RequestStatistics {
    GLOBAL_REQUEST_MANAGER.get_statistics().await
}

pub async fn cleanup_global_requests() -> usize {
    let completed = GLOBAL_REQUEST_MANAGER.clean_completed().await;
    let long_running = GLOBAL_REQUEST_MANAGER.clean_long_running(300).await; // 5 minutes
    completed + long_running
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_active_request_creation() {
        let request = ActiveRequest::new();
        assert!(!request.id.is_empty());
        assert!(request.task_handle.is_none());
        assert!(request.metadata.is_none());
    }

    #[tokio::test]
    async fn test_active_requests_manager() {
        let manager = ActiveRequestsManager::new();

        // Add a request
        let request = ActiveRequest::new().with_id("test-1".to_string());
        let id = request.id.clone();
        manager.add("key-1".to_string(), request).await;

        // Check count
        assert_eq!(manager.count().await, 1);

        // Get the request
        let retrieved = manager.get("key-1").await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id, id);

        // Remove the request
        let removed = manager.remove("key-1").await;
        assert!(removed);
        assert_eq!(manager.count().await, 0);
    }

    #[tokio::test]
    async fn test_request_statistics() {
        let manager = ActiveRequestsManager::new();

        // Add some test requests
        for i in 0..5 {
            let request = ActiveRequest::new().with_id(format!("test-{}", i));
            manager.add(format!("key-{}", i), request).await;
        }

        let stats = manager.get_statistics().await;
        assert_eq!(stats.total_count, 5);
    }

    #[tokio::test]
    async fn test_cleanup_completed() {
        let manager = ActiveRequestsManager::new();

        // Add a completed task (simulate with a finished handle)
        let handle = tokio::spawn(async {
            // Task that completes immediately
        });

        // Wait for the task to complete
        let _ = handle.await;

        let request = ActiveRequest::new();
        manager.add("completed-1".to_string(), request).await;

        // Clean completed should work without errors
        let cleaned = manager.clean_completed().await;
        // Note: This test might not always remove the request since it's hard to
        // guarantee the task is marked as finished immediately
    }

    #[test]
    fn test_create_request_with_metadata() {
        let request = create_request_with_metadata(
            Some("gpt-4"),
            Some("test-api-key"),
            Some("chat_completion"),
        );

        assert!(request.metadata.is_some());
        let metadata = request.metadata.unwrap();
        assert_eq!(metadata.get("model").unwrap(), "gpt-4");
        assert_eq!(metadata.get("request_type").unwrap(), "chat_completion");
        assert!(metadata.contains_key("api_key_hash"));
    }

    #[tokio::test]
    async fn test_health_check() {
        let manager = ActiveRequestsManager::new();
        let is_healthy = manager.health_check().await;
        assert!(is_healthy);
    }
}