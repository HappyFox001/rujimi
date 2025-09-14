use std::sync::Arc;
use tokio::sync::RwLock;
use once_cell::sync::Lazy;

use super::{Settings, save_settings};
use anyhow::Result;

/// Global configuration manager - similar to hajimi's global settings module
static GLOBAL_CONFIG: Lazy<Arc<RwLock<Settings>>> = Lazy::new(|| {
    Arc::new(RwLock::new(Settings::default()))
});

/// Configuration manager that provides global access to settings
/// This mimics hajimi's behavior of having a global settings module
pub struct ConfigManager;

impl ConfigManager {
    /// Initialize the global configuration
    pub async fn initialize(settings: Settings) {
        let mut config = GLOBAL_CONFIG.write().await;
        *config = settings;
    }

    /// Get a read-only clone of current settings
    pub async fn get_settings() -> Settings {
        GLOBAL_CONFIG.read().await.clone()
    }

    /// Update a configuration value and save to disk
    /// This mimics hajimi's pattern: settings.PROPERTY = value; save_settings()
    pub async fn update_config(key: &str, value: serde_json::Value) -> Result<()> {
        let mut config = GLOBAL_CONFIG.write().await;

        // Apply the update - mimicking hajimi's direct property assignment
        match key {
            "fake_streaming" => {
                if let Some(val) = value.as_bool() {
                    config.fake_streaming = val;
                }
            }
            "fake_streaming_interval" => {
                if let Some(val) = value.as_f64() {
                    config.fake_streaming_interval = val;
                }
            }
            "fake_streaming_chunk_size" => {
                if let Some(val) = value.as_i64() {
                    config.fake_streaming_chunk_size = val as i32;
                }
            }
            "fake_streaming_delay_per_chunk" => {
                if let Some(val) = value.as_f64() {
                    config.fake_streaming_delay_per_chunk = val;
                }
            }
            "concurrent_requests" => {
                if let Some(val) = value.as_u64() {
                    config.concurrent_requests = val as usize;
                }
            }
            "increase_concurrent_on_failure" => {
                if let Some(val) = value.as_u64() {
                    config.increase_concurrent_on_failure = val as usize;
                }
            }
            "max_concurrent_requests" => {
                if let Some(val) = value.as_u64() {
                    config.max_concurrent_requests = val as usize;
                }
            }
            "cache_expiry_time" => {
                if let Some(val) = value.as_u64() {
                    config.cache_expiry_time = val;
                }
            }
            "max_cache_entries" => {
                if let Some(val) = value.as_u64() {
                    config.max_cache_entries = val as usize;
                }
            }
            "calculate_cache_entries" => {
                if let Some(val) = value.as_u64() {
                    config.calculate_cache_entries = val as usize;
                }
            }
            "precise_cache" => {
                if let Some(val) = value.as_bool() {
                    config.precise_cache = val;
                }
            }
            "enable_vertex" => {
                if let Some(val) = value.as_bool() {
                    config.enable_vertex = val;
                }
            }
            "google_credentials_json" => {
                if let Some(val) = value.as_str() {
                    config.google_credentials_json = val.to_string();
                }
            }
            "enable_vertex_express" => {
                if let Some(val) = value.as_bool() {
                    config.enable_vertex_express = val;
                }
            }
            "vertex_express_api_key" => {
                if let Some(val) = value.as_str() {
                    config.vertex_express_api_key = val.to_string();
                }
            }
            "search_mode" => {
                if let Some(val) = value.as_bool() {
                    config.search.search_mode = val;
                }
            }
            "search_prompt" => {
                if let Some(val) = value.as_str() {
                    config.search.search_prompt = val.to_string();
                }
            }
            "random_string" => {
                if let Some(val) = value.as_bool() {
                    config.random_string = val;
                }
            }
            "random_string_length" => {
                if let Some(val) = value.as_u64() {
                    config.random_string_length = val as usize;
                }
            }
            "max_empty_responses" => {
                if let Some(val) = value.as_u64() {
                    config.max_empty_responses = val as usize;
                }
            }
            "show_api_error_message" => {
                if let Some(val) = value.as_bool() {
                    config.show_api_error_message = val;
                }
            }
            "max_retry_num" => {
                if let Some(val) = value.as_u64() {
                    config.max_retry_num = val as usize;
                }
            }
            "max_requests_per_minute" => {
                if let Some(val) = value.as_u64() {
                    config.max_requests_per_minute = val as u32;
                }
            }
            "max_requests_per_day_per_ip" => {
                if let Some(val) = value.as_u64() {
                    config.max_requests_per_day_per_ip = val as u32;
                }
            }
            "api_key_daily_limit" => {
                if let Some(val) = value.as_u64() {
                    config.api_key_daily_limit = val as u32;
                }
            }
            "gemini_api_keys" => {
                if let Some(val) = value.as_str() {
                    config.gemini_api_keys = val
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                }
            }
            "public_mode" => {
                if let Some(val) = value.as_bool() {
                    config.public_mode = val;
                }
            }
            "dashboard_url" => {
                if let Some(val) = value.as_str() {
                    config.dashboard_url = val.to_string();
                }
            }
            "nonstream_keepalive_enabled" => {
                if let Some(val) = value.as_bool() {
                    config.nonstream_keepalive_enabled = val;
                }
            }
            "nonstream_keepalive_interval" => {
                if let Some(val) = value.as_f64() {
                    config.nonstream_keepalive_interval = val;
                }
            }
            _ => {
                return Err(anyhow::anyhow!("Unsupported configuration key: {}", key));
            }
        }

        // Save to disk - equivalent to hajimi's save_settings() call
        if let Err(e) = save_settings(&config, &config.storage_dir) {
            tracing::error!("Failed to save settings to disk: {}", e);
            return Err(e);
        }

        tracing::info!("Configuration {} updated and saved successfully", key);
        Ok(())
    }

    /// Get a specific configuration value
    pub async fn get_config_value(key: &str) -> Option<serde_json::Value> {
        let config = GLOBAL_CONFIG.read().await;

        match key {
            "fake_streaming" => Some(serde_json::Value::Bool(config.fake_streaming)),
            "fake_streaming_interval" => Some(serde_json::Value::Number(serde_json::Number::from_f64(config.fake_streaming_interval)?)),
            "concurrent_requests" => Some(serde_json::Value::Number(serde_json::Number::from(config.concurrent_requests as u64))),
            "enable_vertex" => Some(serde_json::Value::Bool(config.enable_vertex)),
            "search_mode" => Some(serde_json::Value::Bool(config.search.search_mode)),
            "show_api_error_message" => Some(serde_json::Value::Bool(config.show_api_error_message)),
            "max_requests_per_minute" => Some(serde_json::Value::Number(serde_json::Number::from(config.max_requests_per_minute as u64))),
            "max_requests_per_day_per_ip" => Some(serde_json::Value::Number(serde_json::Number::from(config.max_requests_per_day_per_ip as u64))),
            "gemini_api_keys" => Some(serde_json::Value::String(config.gemini_api_keys.join(","))),
            "google_credentials_json" => Some(serde_json::Value::String(config.google_credentials_json.clone())),
            "vertex_express_api_key" => Some(serde_json::Value::String(config.vertex_express_api_key.clone())),
            _ => None,
        }
    }

    /// Reload settings from disk - similar to hajimi's load_settings()
    pub async fn reload_from_disk() -> Result<()> {
        let current_config = GLOBAL_CONFIG.read().await;
        let storage_dir = current_config.storage_dir.clone();
        drop(current_config);

        if let Ok(loaded_settings) = super::load_settings(&storage_dir) {
            let mut config = GLOBAL_CONFIG.write().await;
            *config = loaded_settings;
            tracing::info!("Settings reloaded from disk");
            Ok(())
        } else {
            Err(anyhow::anyhow!("Failed to reload settings from disk"))
        }
    }
}