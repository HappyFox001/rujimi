use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use serde_json::Value;
use anyhow::{Result, anyhow};
use reqwest;
use crate::config::Settings;

// Rust equivalent of Python vertex/model_loader.py

lazy_static::lazy_static! {
    static ref MODEL_CACHE: Arc<RwLock<Option<ModelConfig>>> = Arc::new(RwLock::new(None));
    static ref CACHE_LOCK: Arc<Mutex<()>> = Arc::new(Mutex::new(()));
}

#[derive(Debug, Clone)]
pub struct ModelConfig {
    pub vertex_models: Vec<String>,
    pub vertex_express_models: Vec<String>,
}

impl ModelConfig {
    pub fn new() -> Self {
        Self {
            vertex_models: Vec::new(),
            vertex_express_models: Vec::new(),
        }
    }

    pub fn with_models(vertex_models: Vec<String>, vertex_express_models: Vec<String>) -> Self {
        Self {
            vertex_models,
            vertex_express_models,
        }
    }
}

/// Fetch and parse models configuration from remote URL
pub async fn fetch_and_parse_models_config(settings: &Settings) -> Result<ModelConfig> {
    // Get models config URL from settings or use default
    let models_config_url = settings.models_config_url.as_ref()
        .map(|s| s.as_str())
        .or_else(|| std::env::var("VERTEX_MODELS_CONFIG_URL").ok().as_deref())
        .unwrap_or("https://raw.githubusercontent.com/gzzhongqi/vertex2openai/refs/heads/main/vertexModels.json");

    if models_config_url.is_empty() {
        log::error!("MODELS_CONFIG_URL is not set in the environment/config");
        log::info!("Using default model configuration with empty lists");
        return Ok(ModelConfig::new());
    }

    log::info!("Fetching model configuration from: {}", models_config_url);

    // Retry mechanism
    let max_retries = 3;
    let mut retry_delay = 1; // Initial delay 1 second

    for retry in 0..max_retries {
        match try_fetch_models_config(models_config_url).await {
            Ok(config) => {
                log::info!("Successfully fetched and parsed model configuration on attempt {}", retry + 1);
                return Ok(config);
            }
            Err(e) => {
                log::warn!("Attempt {} failed: {}", retry + 1, e);
                if retry < max_retries - 1 {
                    log::info!("Waiting {} seconds before retry...", retry_delay);
                    tokio::time::sleep(tokio::time::Duration::from_secs(retry_delay)).await;
                    retry_delay *= 2; // Exponential backoff
                }
            }
        }
    }

    log::error!("Failed to fetch model configuration after {} attempts, using empty configuration", max_retries);
    Ok(ModelConfig::new())
}

/// Single attempt to fetch models config
async fn try_fetch_models_config(url: &str) -> Result<ModelConfig> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    log::info!("Attempting to fetch model configuration");
    let response = client.get(url).send().await?;

    if !response.status().is_success() {
        return Err(anyhow!("HTTP error: {}", response.status()));
    }

    let response_text = response.text().await?;
    log::debug!("Received response, length: {} characters", response_text.len());

    // Parse JSON response
    let json_data: Value = serde_json::from_str(&response_text)
        .map_err(|e| anyhow!("Failed to parse JSON response: {}", e))?;

    // Extract model lists
    let vertex_models = extract_model_list(&json_data, "vertex_models")?;
    let vertex_express_models = extract_model_list(&json_data, "vertex_express_models")?;

    log::info!("Successfully parsed {} vertex models and {} vertex express models",
              vertex_models.len(), vertex_express_models.len());

    Ok(ModelConfig::with_models(vertex_models, vertex_express_models))
}

/// Extract model list from JSON data
fn extract_model_list(json_data: &Value, key: &str) -> Result<Vec<String>> {
    match json_data.get(key) {
        Some(Value::Array(models)) => {
            let model_list: Result<Vec<String>, _> = models
                .iter()
                .map(|v| v.as_str().ok_or_else(|| anyhow!("Model name is not a string")))
                .map(|r| r.map(|s| s.to_string()))
                .collect();

            let models = model_list?;
            log::debug!("Found {} models for key '{}'", models.len(), key);
            Ok(models)
        }
        Some(_) => {
            log::warn!("Key '{}' exists but is not an array", key);
            Ok(Vec::new())
        }
        None => {
            log::warn!("Key '{}' not found in configuration", key);
            Ok(Vec::new())
        }
    }
}

/// Get cached model configuration or fetch if not available
pub async fn get_models_config(settings: &Settings) -> Result<ModelConfig> {
    let _lock = CACHE_LOCK.lock().await;

    // Try to get from cache first
    {
        let cache = MODEL_CACHE.read().await;
        if let Some(ref config) = *cache {
            log::debug!("Returning cached model configuration");
            return Ok(config.clone());
        }
    }

    // Cache miss, fetch new configuration
    log::info!("Model cache is empty, fetching configuration");
    let config = fetch_and_parse_models_config(settings).await?;

    // Update cache
    {
        let mut cache = MODEL_CACHE.write().await;
        *cache = Some(config.clone());
    }

    log::info!("Model configuration cached successfully");
    Ok(config)
}

/// Refresh the models configuration cache
pub async fn refresh_models_config_cache(settings: &Settings) -> Result<()> {
    let _lock = CACHE_LOCK.lock().await;

    log::info!("Refreshing model configuration cache");
    let config = fetch_and_parse_models_config(settings).await?;

    // Update cache
    {
        let mut cache = MODEL_CACHE.write().await;
        *cache = Some(config);
    }

    log::info!("Model configuration cache refreshed successfully");
    Ok(())
}

/// Get vertex models list
pub async fn get_vertex_models(settings: &Settings) -> Result<Vec<String>> {
    let config = get_models_config(settings).await?;
    Ok(config.vertex_models)
}

/// Get vertex express models list
pub async fn get_vertex_express_models(settings: &Settings) -> Result<Vec<String>> {
    let config = get_models_config(settings).await?;
    Ok(config.vertex_express_models)
}

/// Check if a model is a vertex model
pub async fn is_vertex_model(settings: &Settings, model_name: &str) -> Result<bool> {
    let vertex_models = get_vertex_models(settings).await?;
    Ok(vertex_models.contains(&model_name.to_string()))
}

/// Check if a model is a vertex express model
pub async fn is_vertex_express_model(settings: &Settings, model_name: &str) -> Result<bool> {
    let vertex_express_models = get_vertex_express_models(settings).await?;
    Ok(vertex_express_models.contains(&model_name.to_string()))
}

/// Clear the model cache
pub async fn clear_models_cache() {
    let _lock = CACHE_LOCK.lock().await;
    let mut cache = MODEL_CACHE.write().await;
    *cache = None;
    log::info!("Model configuration cache cleared");
}

/// Get model configuration summary for debugging
pub async fn get_models_summary(settings: &Settings) -> Result<HashMap<String, usize>> {
    let config = get_models_config(settings).await?;

    let mut summary = HashMap::new();
    summary.insert("vertex_models_count".to_string(), config.vertex_models.len());
    summary.insert("vertex_express_models_count".to_string(), config.vertex_express_models.len());
    summary.insert("total_models_count".to_string(),
                   config.vertex_models.len() + config.vertex_express_models.len());

    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_config_new() {
        let config = ModelConfig::new();
        assert!(config.vertex_models.is_empty());
        assert!(config.vertex_express_models.is_empty());
    }

    #[test]
    fn test_model_config_with_models() {
        let vertex_models = vec!["model1".to_string(), "model2".to_string()];
        let vertex_express_models = vec!["express1".to_string()];

        let config = ModelConfig::with_models(vertex_models.clone(), vertex_express_models.clone());
        assert_eq!(config.vertex_models, vertex_models);
        assert_eq!(config.vertex_express_models, vertex_express_models);
    }

    #[test]
    fn test_extract_model_list() {
        let json_data = serde_json::json!({
            "vertex_models": ["model1", "model2"],
            "vertex_express_models": ["express1"]
        });

        let vertex_models = extract_model_list(&json_data, "vertex_models").unwrap();
        assert_eq!(vertex_models, vec!["model1".to_string(), "model2".to_string()]);

        let vertex_express_models = extract_model_list(&json_data, "vertex_express_models").unwrap();
        assert_eq!(vertex_express_models, vec!["express1".to_string()]);

        // Test missing key
        let missing = extract_model_list(&json_data, "missing_key").unwrap();
        assert!(missing.is_empty());
    }

    #[tokio::test]
    async fn test_clear_models_cache() {
        clear_models_cache().await;
        // This test just ensures the function doesn't panic
    }
}