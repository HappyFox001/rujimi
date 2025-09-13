use anyhow::{Context, Result};
use dashmap::DashMap;
use rand::seq::SliceRandom;
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::config::Settings;

#[derive(Debug, Clone)]
pub struct ApiKeyStats {
    pub daily_usage: u32,
    pub last_used: chrono::DateTime<chrono::Utc>,
    pub consecutive_failures: u32,
}

impl Default for ApiKeyStats {
    fn default() -> Self {
        Self {
            daily_usage: 0,
            last_used: chrono::Utc::now(),
            consecutive_failures: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ApiKeyManager {
    settings: Arc<Settings>,
    available_keys: Arc<RwLock<VecDeque<String>>>,
    key_stats: Arc<DashMap<String, ApiKeyStats>>,
    invalid_keys: Arc<RwLock<Vec<String>>>,
}

impl ApiKeyManager {
    pub fn new(settings: Arc<Settings>) -> Self {
        Self {
            settings,
            available_keys: Arc::new(RwLock::new(VecDeque::new())),
            key_stats: Arc::new(DashMap::new()),
            invalid_keys: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn initialize(&self) -> Result<()> {
        info!("Initializing API key manager...");

        let valid_keys = self.settings.get_valid_api_keys();

        if valid_keys.is_empty() {
            warn!("No valid API keys found in configuration");
            return Ok(());
        }

        info!("Found {} API keys to validate", valid_keys.len());

        // Test all keys in parallel and collect results
        let mut valid_tested_keys = Vec::new();
        let mut invalid_tested_keys = Vec::new();

        let futures = valid_keys.iter().map(|key| {
            let key = key.clone();
            async move {
                match self.test_api_key(&key).await {
                    Ok(true) => (key, true),
                    Ok(false) => (key, false),
                    Err(e) => {
                        warn!("Error testing API key {}: {}", &key[..8.min(key.len())], e);
                        (key, false)
                    }
                }
            }
        });

        let results = futures::future::join_all(futures).await;

        for (key, is_valid) in results {
            if is_valid {
                valid_tested_keys.push(key.clone());
                self.key_stats.insert(key, ApiKeyStats::default());
            } else {
                invalid_tested_keys.push(key);
            }
        }

        // Update available keys
        {
            let mut available_keys = self.available_keys.write().await;
            available_keys.clear();
            available_keys.extend(valid_tested_keys.iter().cloned());
            self.shuffle_keys(&mut available_keys).await;
        }

        // Update invalid keys
        {
            let mut invalid_keys = self.invalid_keys.write().await;
            invalid_keys.extend(invalid_tested_keys);
        }

        info!(
            "API key initialization complete: {} valid, {} invalid",
            valid_tested_keys.len(),
            self.invalid_keys.read().await.len()
        );

        Ok(())
    }

    pub async fn get_next_key(&self) -> Option<String> {
        let mut available_keys = self.available_keys.write().await;

        // Try to find a key that hasn't exceeded daily limit
        while let Some(key) = available_keys.pop_front() {
            if let Some(stats) = self.key_stats.get(&key) {
                if stats.daily_usage < self.settings.api_key_daily_limit {
                    // Key is still within daily limit, use it
                    available_keys.push_back(key.clone());
                    return Some(key);
                } else {
                    // Key has exceeded daily limit, put it at the back
                    available_keys.push_back(key);
                    continue;
                }
            } else {
                // No stats for this key, initialize and use it
                self.key_stats.insert(key.clone(), ApiKeyStats::default());
                available_keys.push_back(key.clone());
                return Some(key);
            }
        }

        // If we get here, all keys have exceeded daily limit
        if !available_keys.is_empty() {
            warn!("All API keys have exceeded daily limits, recycling oldest key");
            let key = available_keys.pop_front().unwrap();
            available_keys.push_back(key.clone());
            return Some(key);
        }

        None
    }

    pub async fn mark_key_used(&self, key: &str, success: bool) {
        if let Some(mut stats) = self.key_stats.get_mut(key) {
            stats.last_used = chrono::Utc::now();

            if success {
                stats.daily_usage += 1;
                stats.consecutive_failures = 0;
            } else {
                stats.consecutive_failures += 1;

                // If a key fails too many times consecutively, mark it as invalid
                if stats.consecutive_failures >= 5 {
                    warn!("Marking API key as invalid due to consecutive failures: {}...", &key[..8.min(key.len())]);
                    self.mark_key_invalid(key).await;
                }
            }
        }
    }

    pub async fn mark_key_invalid(&self, key: &str) {
        // Remove from available keys
        {
            let mut available_keys = self.available_keys.write().await;
            available_keys.retain(|k| k != key);
        }

        // Add to invalid keys
        {
            let mut invalid_keys = self.invalid_keys.write().await;
            if !invalid_keys.contains(&key.to_string()) {
                invalid_keys.push(key.to_string());
            }
        }

        // Remove stats
        self.key_stats.remove(key);

        warn!("API key marked as invalid: {}...", &key[..8.min(key.len())]);
    }

    pub async fn reset_daily_usage(&self) {
        info!("Resetting daily usage for all API keys");
        for mut entry in self.key_stats.iter_mut() {
            entry.daily_usage = 0;
        }
    }

    pub async fn available_keys_count(&self) -> usize {
        self.available_keys.read().await.len()
    }

    pub async fn get_key_stats(&self) -> Vec<(String, ApiKeyStats)> {
        self.key_stats
            .iter()
            .map(|entry| (entry.key().clone(), (*entry.value()).clone()))
            .collect()
    }

    pub async fn reset_key_stack(&self) {
        let mut available_keys = self.available_keys.write().await;
        self.shuffle_keys(&mut available_keys).await;
        info!("API key stack reset and shuffled");
    }

    async fn shuffle_keys(&self, keys: &mut VecDeque<String>) {
        let mut vec: Vec<String> = keys.drain(..).collect();
        let mut rng = rand::thread_rng();
        vec.shuffle(&mut rng);
        keys.extend(vec);
    }

    async fn test_api_key(&self, api_key: &str) -> Result<bool> {
        let client = reqwest::Client::new();
        let url = "https://generativelanguage.googleapis.com/v1beta/models";

        let response = client
            .get(url)
            .header("x-goog-api-key", api_key)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
            .context("Failed to send API key test request")?;

        let is_valid = response.status().is_success();

        if is_valid {
            info!("API key validation successful: {}...", &api_key[..8.min(api_key.len())]);
        } else {
            warn!(
                "API key validation failed: {}... (status: {})",
                &api_key[..8.min(api_key.len())],
                response.status()
            );
        }

        Ok(is_valid)
    }

    // Background task to clean up expired daily usage
    pub async fn start_daily_cleanup_task(self: Arc<Self>) {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600)); // Check every hour

        loop {
            interval.tick().await;

            let now = chrono::Utc::now();
            let mut reset_needed = false;

            // Check if we've crossed into a new day
            for entry in self.key_stats.iter() {
                let last_used = entry.value().last_used;
                if now.date_naive() > last_used.date_naive() {
                    reset_needed = true;
                    break;
                }
            }

            if reset_needed {
                self.reset_daily_usage().await;
            }
        }
    }
}