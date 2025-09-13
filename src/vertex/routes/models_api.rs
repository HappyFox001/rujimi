use serde_json::{Value, json};
use anyhow::Result;
use crate::config::Settings;
use crate::vertex::model_loader::{get_vertex_models, get_vertex_express_models, refresh_models_config_cache};

// Rust equivalent of Python vertex/routes/models_api.py

/// List available models for Vertex generation
/// Returns a list of models in OpenAI-compatible format
pub async fn list_models(settings: &Settings) -> Result<Value> {
    log::info!("Retrieving list of available models");

    // Get available models
    let standard_models = get_vertex_models(settings).await?;
    let express_models = get_vertex_express_models(settings).await?;

    let mut all_models = Vec::new();

    // Format standard models
    for model_name in standard_models {
        all_models.push(json!({
            "id": model_name,
            "object": "model",
            "created": 1677610602, // Placeholder timestamp
            "owned_by": "google",
            "permission": [],
            "root": model_name,
            "parent": null,
            "max_tokens": 32768, // Default max tokens for Vertex models
            "type": "vertex"
        }));
    }

    // Format express models
    for model_name in express_models {
        all_models.push(json!({
            "id": model_name,
            "object": "model",
            "created": 1677610602, // Placeholder timestamp
            "owned_by": "google",
            "permission": [],
            "root": model_name,
            "parent": null,
            "max_tokens": 32768, // Default max tokens for Vertex Express models
            "type": "vertex_express"
        }));
    }

    log::info!("Found {} total models ({} standard, {} express)",
              all_models.len(),
              all_models.iter().filter(|m| m["type"] == "vertex").count(),
              all_models.iter().filter(|m| m["type"] == "vertex_express").count());

    Ok(json!({
        "object": "list",
        "data": all_models
    }))
}

/// Get specific model information
pub async fn get_model_info(settings: &Settings, model_id: &str) -> Result<Value> {
    log::debug!("Getting info for model: {}", model_id);

    let standard_models = get_vertex_models(settings).await?;
    let express_models = get_vertex_express_models(settings).await?;

    // Check if model exists in standard models
    if standard_models.contains(&model_id.to_string()) {
        return Ok(json!({
            "id": model_id,
            "object": "model",
            "created": 1677610602,
            "owned_by": "google",
            "permission": [],
            "root": model_id,
            "parent": null,
            "max_tokens": 32768,
            "type": "vertex"
        }));
    }

    // Check if model exists in express models
    if express_models.contains(&model_id.to_string()) {
        return Ok(json!({
            "id": model_id,
            "object": "model",
            "created": 1677610602,
            "owned_by": "google",
            "permission": [],
            "root": model_id,
            "parent": null,
            "max_tokens": 32768,
            "type": "vertex_express"
        }));
    }

    Err(anyhow::anyhow!("Model '{}' not found", model_id))
}

/// Refresh models configuration cache
pub async fn refresh_models_cache(settings: &Settings) -> Result<Value> {
    log::info!("Refreshing models configuration cache");

    refresh_models_config_cache(settings).await?;

    let standard_models = get_vertex_models(settings).await?;
    let express_models = get_vertex_express_models(settings).await?;

    Ok(json!({
        "status": "success",
        "message": "Model configuration cache refreshed successfully",
        "vertex_models_count": standard_models.len(),
        "vertex_express_models_count": express_models.len(),
        "total_models": standard_models.len() + express_models.len()
    }))
}

/// Check if a model is available
pub async fn is_model_available(settings: &Settings, model_id: &str) -> Result<bool> {
    let standard_models = get_vertex_models(settings).await?;
    let express_models = get_vertex_express_models(settings).await?;

    Ok(standard_models.contains(&model_id.to_string()) ||
       express_models.contains(&model_id.to_string()))
}

/// Get model type (vertex or vertex_express)
pub async fn get_model_type(settings: &Settings, model_id: &str) -> Result<String> {
    let standard_models = get_vertex_models(settings).await?;
    let express_models = get_vertex_express_models(settings).await?;

    if standard_models.contains(&model_id.to_string()) {
        Ok("vertex".to_string())
    } else if express_models.contains(&model_id.to_string()) {
        Ok("vertex_express".to_string())
    } else {
        Err(anyhow::anyhow!("Model '{}' not found", model_id))
    }
}

/// Get model capabilities and limitations
pub async fn get_model_capabilities(settings: &Settings, model_id: &str) -> Result<Value> {
    let model_type = get_model_type(settings, model_id).await?;

    let capabilities = match model_type.as_str() {
        "vertex" => json!({
            "supports_streaming": true,
            "supports_functions": true,
            "supports_vision": model_id.contains("vision") || model_id.contains("gemini"),
            "max_tokens": 32768,
            "context_window": if model_id.contains("gemini") { 1000000 } else { 32768 },
            "supports_json_mode": true,
            "rate_limits": {
                "requests_per_minute": 60,
                "tokens_per_minute": 60000
            }
        }),
        "vertex_express" => json!({
            "supports_streaming": true,
            "supports_functions": false,
            "supports_vision": false,
            "max_tokens": 8192,
            "context_window": 8192,
            "supports_json_mode": false,
            "rate_limits": {
                "requests_per_minute": 600,
                "tokens_per_minute": 100000
            }
        }),
        _ => json!({})
    };

    Ok(json!({
        "model_id": model_id,
        "model_type": model_type,
        "capabilities": capabilities
    }))
}

/// List models by type
pub async fn list_models_by_type(settings: &Settings, model_type: &str) -> Result<Value> {
    match model_type {
        "vertex" => {
            let models = get_vertex_models(settings).await?;
            Ok(json!({
                "object": "list",
                "type": "vertex",
                "data": models
            }))
        }
        "vertex_express" => {
            let models = get_vertex_express_models(settings).await?;
            Ok(json!({
                "object": "list",
                "type": "vertex_express",
                "data": models
            }))
        }
        _ => Err(anyhow::anyhow!("Invalid model type: {}. Use 'vertex' or 'vertex_express'", model_type))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_list_models() {
        let settings = Settings::default();
        let result = list_models(&settings).await;
        assert!(result.is_ok());

        let models = result.unwrap();
        assert_eq!(models["object"], "list");
        assert!(models["data"].is_array());
    }

    #[tokio::test]
    async fn test_is_model_available() {
        let settings = Settings::default();
        // This will return false for any model since we don't have real model data in tests
        let result = is_model_available(&settings, "gemini-1.5-pro").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_list_models_by_type() {
        let settings = Settings::default();

        let vertex_result = list_models_by_type(&settings, "vertex").await;
        assert!(vertex_result.is_ok());

        let express_result = list_models_by_type(&settings, "vertex_express").await;
        assert!(express_result.is_ok());

        let invalid_result = list_models_by_type(&settings, "invalid_type").await;
        assert!(invalid_result.is_err());
    }
}