use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tracing::{debug, info};
use xxhash_rust::xxh3::xxh3_64;

use crate::config::Settings;
use crate::models::schemas::ChatCompletionResponse;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    pub response: ChatCompletionResponse,
    pub created_at: SystemTime,
    pub access_count: usize,
}

impl CacheEntry {
    pub fn new(response: ChatCompletionResponse) -> Self {
        Self {
            response,
            created_at: SystemTime::now(),
            access_count: 0,
        }
    }

    pub fn is_expired(&self, ttl: Duration) -> bool {
        self.created_at.elapsed().unwrap_or(Duration::MAX) > ttl
    }

    pub fn access(&mut self) {
        self.access_count += 1;
    }
}

#[derive(Debug, Clone)]
pub struct ResponseCacheManager {
    settings: Arc<Settings>,
    cache: Arc<DashMap<String, VecDeque<CacheEntry>>>,
    access_times: Arc<DashMap<String, SystemTime>>,
}

impl ResponseCacheManager {
    pub fn new(settings: Arc<Settings>) -> Self {
        Self {
            settings,
            cache: Arc::new(DashMap::new()),
            access_times: Arc::new(DashMap::new()),
        }
    }

    pub async fn get(&self, cache_key: &str) -> Option<ChatCompletionResponse> {
        if let Some(mut entries) = self.cache.get_mut(cache_key) {
            if let Some(mut entry) = entries.pop_front() {
                // Check if entry is expired
                let ttl = Duration::from_secs(self.settings.cache_expiry_time);
                if entry.is_expired(ttl) {
                    debug!("Cache entry expired for key: {}", cache_key);
                    return None;
                }

                entry.access();
                let response = entry.response.clone();

                // Put back the entry if there are multiple cached responses
                entries.push_back(entry);

                // Update access time
                self.access_times.insert(cache_key.to_string(), SystemTime::now());

                debug!("Cache hit for key: {}", cache_key);
                return Some(response);
            }
        }

        debug!("Cache miss for key: {}", cache_key);
        None
    }

    pub async fn put(&self, cache_key: String, response: ChatCompletionResponse) {
        let entry = CacheEntry::new(response);

        // Get or create the entry queue for this cache key
        let mut entries = self.cache.entry(cache_key.clone()).or_insert_with(VecDeque::new);

        // Add the new entry
        entries.push_back(entry);

        // Limit the number of cached responses per key (e.g., 3)
        while entries.len() > 3 {
            entries.pop_front();
        }

        // Update access time
        self.access_times.insert(cache_key.clone(), SystemTime::now());

        debug!("Cached response for key: {}", cache_key);

        // Check if we need to evict old entries to stay within the global limit
        self.enforce_size_limit().await;
    }

    pub async fn size(&self) -> usize {
        self.cache.len()
    }

    pub async fn clear(&self) {
        self.cache.clear();
        self.access_times.clear();
        info!("Cache cleared");
    }

    pub async fn cleanup_expired(&self) {
        let ttl = Duration::from_secs(self.settings.cache_expiry_time);
        let mut removed_count = 0;
        let mut keys_to_remove = Vec::new();

        // Find expired entries and collect updates
        let mut updates = Vec::new();
        for entry in self.cache.iter() {
            let key = entry.key().clone();
            let entries = entry.value();

            // Remove expired entries from the queue
            let mut new_entries = VecDeque::new();
            for cache_entry in entries.iter() {
                if !cache_entry.is_expired(ttl) {
                    new_entries.push_back(cache_entry.clone());
                } else {
                    removed_count += 1;
                }
            }

            if new_entries.is_empty() {
                keys_to_remove.push(key);
            } else if new_entries.len() != entries.len() {
                updates.push((key, new_entries));
            }
        }

        // Apply updates
        for (key, new_entries) in updates {
            if let Some(mut entry) = self.cache.get_mut(&key) {
                *entry = new_entries;
            }
        }

        // Remove completely empty cache keys
        for key in keys_to_remove {
            self.cache.remove(&key);
            self.access_times.remove(&key);
        }

        if removed_count > 0 {
            info!("Cleaned up {} expired cache entries", removed_count);
        }
    }

    async fn enforce_size_limit(&self) {
        if self.cache.len() <= self.settings.max_cache_entries {
            return;
        }

        let excess_count = self.cache.len() - self.settings.max_cache_entries;
        let mut keys_by_access_time: Vec<(String, SystemTime)> = self
            .access_times
            .iter()
            .map(|entry| (entry.key().clone(), *entry.value()))
            .collect();

        // Sort by access time (oldest first)
        keys_by_access_time.sort_by_key(|(_, time)| *time);

        // Remove the oldest entries
        for (key, _) in keys_by_access_time.into_iter().take(excess_count) {
            self.cache.remove(&key);
            self.access_times.remove(&key);
        }

        info!("Evicted {} cache entries to enforce size limit", excess_count);
    }

    pub async fn start_cleanup_task(self: Arc<Self>) {
        let mut interval = tokio::time::interval(Duration::from_secs(300)); // Clean up every 5 minutes

        loop {
            interval.tick().await;
            self.cleanup_expired().await;
        }
    }

    pub async fn get_stats(&self) -> CacheStats {
        let mut total_entries = 0;
        let mut total_responses = 0;
        let expired_count = self.count_expired_entries().await;

        for entry in self.cache.iter() {
            total_entries += 1;
            total_responses += entry.value().len();
        }

        CacheStats {
            total_keys: total_entries,
            total_responses,
            expired_entries: expired_count,
            hit_ratio: 0.0, // This would require tracking hits/misses separately
        }
    }

    async fn count_expired_entries(&self) -> usize {
        let ttl = Duration::from_secs(self.settings.cache_expiry_time);
        let mut expired_count = 0;

        for entry in self.cache.iter() {
            for cache_entry in entry.value().iter() {
                if cache_entry.is_expired(ttl) {
                    expired_count += 1;
                }
            }
        }

        expired_count
    }
}

#[derive(Debug, Serialize)]
pub struct CacheStats {
    pub total_keys: usize,
    pub total_responses: usize,
    pub expired_entries: usize,
    pub hit_ratio: f64,
}

pub fn generate_cache_key(
    messages: &[serde_json::Value],
    model: &str,
    calculate_entries: usize,
    precise: bool,
) -> String {
    let messages_to_hash = if precise {
        messages
    } else {
        let start_idx = messages.len().saturating_sub(calculate_entries);
        &messages[start_idx..]
    };

    let content_for_hash = serde_json::json!({
        "messages": messages_to_hash,
        "model": model,
    });

    let content_string = serde_json::to_string(&content_for_hash).unwrap_or_default();
    let hash = xxh3_64(content_string.as_bytes());

    format!("{}_{:x}", model, hash)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_cache_key_generation() {
        let messages = vec![
            json!({"role": "user", "content": "Hello"}),
            json!({"role": "assistant", "content": "Hi there!"}),
            json!({"role": "user", "content": "How are you?"}),
        ];

        let key1 = generate_cache_key(&messages, "gpt-4", 2, false);
        let key2 = generate_cache_key(&messages, "gpt-4", 2, false);
        let key3 = generate_cache_key(&messages, "gpt-3.5", 2, false);

        assert_eq!(key1, key2);
        assert_ne!(key1, key3);
    }

    #[test]
    fn test_cache_entry_expiry() {
        let response = ChatCompletionResponse::default();
        let mut entry = CacheEntry::new(response);

        assert!(!entry.is_expired(Duration::from_secs(60)));

        // Simulate an old entry
        entry.created_at = SystemTime::now() - Duration::from_secs(120);
        assert!(entry.is_expired(Duration::from_secs(60)));
    }
}