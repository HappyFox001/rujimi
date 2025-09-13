use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use serde_json::{Value, json};

use crate::config::Settings;
use crate::vertex::{
    auth::api_key_middleware,
    credentials_manager::CredentialManager,
    vertex_ai_init::{init_vertex_ai, get_vertex_ai_status},
    routes::{chat_api, models_api},
};

// Rust equivalent of Python vertex/main.py

#[derive(Clone)]
pub struct VertexAppState {
    pub settings: Arc<Settings>,
}

/// Create Vertex AI router with all routes
pub fn create_vertex_router(settings: Arc<Settings>) -> Router {
    let state = VertexAppState { settings };

    Router::new()
        .route("/v1/models", get(handle_models_list))
        .route("/v1/chat/completions", post(handle_chat_completions))
        .route("/v1/completions", post(handle_completions))
        .route("/vertex/status", get(handle_vertex_status))
        .route("/vertex/init", post(handle_vertex_init))
        .route("/vertex/reinit", post(handle_vertex_reinit))
        .layer(axum::middleware::from_fn(api_key_middleware))
        .with_state(state)
}

/// Handle models list endpoint
async fn handle_models_list(
    State(state): State<VertexAppState>,
) -> Result<Json<Value>, StatusCode> {
    match models_api::list_models(&state.settings).await {
        Ok(models) => Ok(Json(models)),
        Err(e) => {
            log::error!("Failed to list models: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Handle chat completions endpoint
async fn handle_chat_completions(
    State(state): State<VertexAppState>,
    Json(request): Json<crate::vertex::models::OpenAIRequest>,
) -> Result<Json<Value>, StatusCode> {
    match chat_api::handle_chat_completion(&state.settings, request).await {
        Ok(response) => Ok(Json(response)),
        Err(e) => {
            log::error!("Chat completion failed: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Handle completions endpoint (legacy)
async fn handle_completions(
    State(state): State<VertexAppState>,
    Json(request): Json<crate::vertex::models::GeminiCompletionRequest>,
) -> Result<Json<Value>, StatusCode> {
    match chat_api::handle_completion(&state.settings, request).await {
        Ok(response) => Ok(Json(response)),
        Err(e) => {
            log::error!("Completion failed: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Handle vertex status endpoint
async fn handle_vertex_status() -> Json<Value> {
    Json(get_vertex_ai_status().await)
}

/// Handle vertex initialization endpoint
async fn handle_vertex_init(
    State(state): State<VertexAppState>,
) -> Result<Json<Value>, StatusCode> {
    log::info!("Manual Vertex AI initialization requested");

    match init_vertex_ai(&state.settings, None).await {
        Ok(success) => {
            if success {
                log::info!("Vertex AI initialized successfully via API");
                Ok(Json(json!({
                    "status": "success",
                    "message": "Vertex AI initialized successfully",
                    "initialized": true
                })))
            } else {
                log::warn!("Vertex AI initialization completed with warnings");
                Ok(Json(json!({
                    "status": "partial",
                    "message": "Vertex AI initialized with warnings (check logs)",
                    "initialized": false
                })))
            }
        }
        Err(e) => {
            log::error!("Failed to initialize Vertex AI via API: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Handle vertex reinitialization endpoint
async fn handle_vertex_reinit(
    State(state): State<VertexAppState>,
) -> Result<Json<Value>, StatusCode> {
    log::info!("Manual Vertex AI reinitialization requested");

    match crate::vertex::vertex_ai_init::reinitialize_vertex_ai(&state.settings).await {
        Ok(success) => {
            if success {
                log::info!("Vertex AI reinitialized successfully via API");
                Ok(Json(json!({
                    "status": "success",
                    "message": "Vertex AI reinitialized successfully",
                    "initialized": true
                })))
            } else {
                log::warn!("Vertex AI reinitialization completed with warnings");
                Ok(Json(json!({
                    "status": "partial",
                    "message": "Vertex AI reinitialized with warnings (check logs)",
                    "initialized": false
                })))
            }
        }
        Err(e) => {
            log::error!("Failed to reinitialize Vertex AI via API: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Initialize Vertex AI application
pub async fn init_vertex_app(settings: Arc<Settings>) -> Result<(), Box<dyn std::error::Error>> {
    log::info!("Initializing Vertex AI application");

    // Initialize Vertex AI
    match init_vertex_ai(&settings, None).await {
        Ok(success) => {
            if success {
                log::info!("Vertex AI application initialized successfully");
            } else {
                log::warn!("Vertex AI application initialized with warnings");
            }
        }
        Err(e) => {
            log::error!("Failed to initialize Vertex AI application: {}", e);
            return Err(e.into());
        }
    }

    Ok(())
}

/// Health check for Vertex AI services
pub async fn vertex_health_check() -> Json<Value> {
    let is_available = crate::vertex::vertex_ai_init::is_vertex_ai_available().await;
    let status = get_vertex_ai_status().await;

    Json(json!({
        "service": "vertex_ai",
        "available": is_available,
        "status": status,
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

/// Get Vertex AI configuration summary
pub async fn get_vertex_config_summary(settings: &Settings) -> Json<Value> {
    use crate::vertex::model_loader::get_models_summary;

    let models_summary = get_models_summary(settings).await.unwrap_or_default();
    let status = get_vertex_ai_status().await;

    Json(json!({
        "vertex_ai": {
            "status": status,
            "models": models_summary,
            "configuration": {
                "project_id": settings.vertex_project_id,
                "location": settings.vertex_location,
                "fake_streaming_enabled": settings.fake_streaming,
                "credentials_dir": settings.credentials_dir
            }
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_vertex_health_check() {
        let health = vertex_health_check().await;
        let json_value = health.0;
        assert_eq!(json_value["service"], "vertex_ai");
        assert!(json_value["available"].is_boolean());
    }

    #[test]
    fn test_create_vertex_router() {
        let settings = Arc::new(Settings::default());
        let router = create_vertex_router(settings);
        // Router creation should not panic
        assert!(true);
    }

    #[tokio::test]
    async fn test_get_vertex_config_summary() {
        let settings = Settings::default();
        let summary = get_vertex_config_summary(&settings).await;
        let json_value = summary.0;
        assert!(json_value["vertex_ai"].is_object());
        assert!(json_value["vertex_ai"]["status"].is_object());
    }
}