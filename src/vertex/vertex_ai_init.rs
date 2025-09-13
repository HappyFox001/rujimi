use std::sync::Arc;
use tokio::sync::RwLock;
use crate::vertex::credentials_manager::CredentialManager;
use crate::vertex::model_loader::refresh_models_config_cache;
use crate::vertex::config::VertexConfig;
use crate::config::Settings;
use anyhow::Result;

// Rust equivalent of Python vertex/vertex_ai_init.py

lazy_static::lazy_static! {
    /// Global fallback client for Vertex AI operations
    static ref GLOBAL_FALLBACK_CLIENT: Arc<RwLock<Option<VertexAIClient>>> = Arc::new(RwLock::new(None));
}

#[derive(Debug, Clone)]
pub struct VertexAIClient {
    pub credential_manager: Arc<CredentialManager>,
    pub config: VertexConfig,
    pub is_initialized: bool,
}

impl VertexAIClient {
    pub fn new(credential_manager: CredentialManager, config: VertexConfig) -> Self {
        Self {
            credential_manager: Arc::new(credential_manager),
            config,
            is_initialized: false,
        }
    }

    /// Check if the client has valid credentials
    pub async fn has_credentials(&self) -> bool {
        // Check if we have environment credentials
        if !self.config.api_key.is_empty() || self.config.google_credentials_json.is_some() {
            return true;
        }

        // Check if we have file-based credentials
        match self.credential_manager.get_all_credential_files() {
            Ok(files) => !files.is_empty(),
            Err(_) => false,
        }
    }

    /// Initialize the client with credentials
    pub async fn initialize(&mut self) -> Result<()> {
        if self.is_initialized {
            log::debug!("Vertex AI client already initialized");
            return Ok(());
        }

        log::info!("Initializing Vertex AI client");

        // Validate that we have some form of credentials
        if !self.has_credentials().await {
            log::warn!("No Vertex AI credentials found");
            return Ok(()); // Don't error, just warn
        }

        // Mark as initialized
        self.is_initialized = true;
        log::info!("Vertex AI client initialized successfully");

        Ok(())
    }
}

/// Reset the global fallback client
pub async fn reset_global_fallback_client() {
    let mut client = GLOBAL_FALLBACK_CLIENT.write().await;
    *client = None;
    log::info!("Global fallback client has been reset");
}

/// Initialize Vertex AI with credentials and configuration
pub async fn init_vertex_ai(
    settings: &Settings,
    credential_manager: Option<CredentialManager>,
) -> Result<bool> {
    log::info!("Starting Vertex AI initialization");

    // Create or use provided credential manager
    let cred_manager = match credential_manager {
        Some(manager) => {
            log::info!("Using provided CredentialManager instance");
            manager
        }
        None => {
            log::info!("Creating new CredentialManager instance");
            let credentials_dir = settings.credentials_dir
                .as_ref()
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| {
                    let mut path = std::path::PathBuf::from(&settings.storage_dir);
                    path.push("credentials");
                    path
                });
            CredentialManager::new(credentials_dir)
        }
    };

    let mut env_creds_loaded_into_manager = false;

    // Process Google credentials JSON if available
    if let Some(ref credentials_json_str) = settings.google_credentials_json {
        if !credentials_json_str.trim().is_empty() {
            log::info!("Processing GOOGLE_CREDENTIALS_JSON from environment");

            match CredentialManager::parse_multiple_json_credentials(credentials_json_str) {
                Ok(credentials_list) => {
                    if !credentials_list.is_empty() {
                        log::info!("Parsed {} credential(s) from GOOGLE_CREDENTIALS_JSON", credentials_list.len());

                        match cred_manager.save_multiple_credentials_to_files(&credentials_list) {
                            Ok(saved_files) => {
                                log::info!("Successfully saved {} credential file(s)", saved_files.len());
                                env_creds_loaded_into_manager = true;
                            }
                            Err(e) => {
                                log::error!("Failed to save credentials to files: {}", e);
                            }
                        }
                    } else {
                        log::warn!("No valid credentials found in GOOGLE_CREDENTIALS_JSON");
                    }
                }
                Err(e) => {
                    log::error!("Failed to parse GOOGLE_CREDENTIALS_JSON: {}", e);
                }
            }
        } else {
            log::debug!("GOOGLE_CREDENTIALS_JSON is empty, skipping");
        }
    } else {
        log::debug!("GOOGLE_CREDENTIALS_JSON not set in environment");
    }

    // Create Vertex configuration
    let vertex_config = VertexConfig::from_settings(settings);

    // Create and initialize client
    let mut client = VertexAIClient::new(cred_manager, vertex_config);

    match client.initialize().await {
        Ok(()) => {
            // Set as global fallback client
            {
                let mut global_client = GLOBAL_FALLBACK_CLIENT.write().await;
                *global_client = Some(client.clone());
            }

            log::info!("Vertex AI initialization completed successfully");

            // Refresh model configuration cache
            if let Err(e) = refresh_models_config_cache(settings).await {
                log::warn!("Failed to refresh model configuration cache: {}", e);
            }

            Ok(true)
        }
        Err(e) => {
            log::error!("Failed to initialize Vertex AI client: {}", e);
            Ok(false)
        }
    }
}

/// Get the global fallback client
pub async fn get_global_fallback_client() -> Option<VertexAIClient> {
    let client = GLOBAL_FALLBACK_CLIENT.read().await;
    client.clone()
}

/// Check if Vertex AI is initialized and available
pub async fn is_vertex_ai_available() -> bool {
    match get_global_fallback_client().await {
        Some(client) => client.is_initialized && client.has_credentials().await,
        None => false,
    }
}

/// Reinitialize Vertex AI with updated settings
pub async fn reinitialize_vertex_ai(settings: &Settings) -> Result<bool> {
    log::info!("Reinitializing Vertex AI");

    // Reset the global client first
    reset_global_fallback_client().await;

    // Initialize with new settings
    init_vertex_ai(settings, None).await
}

/// Get initialization status and diagnostic information
pub async fn get_vertex_ai_status() -> serde_json::Value {
    use serde_json::json;

    let client = get_global_fallback_client().await;

    match client {
        Some(client) => {
            let has_creds = client.has_credentials().await;
            json!({
                "initialized": client.is_initialized,
                "has_credentials": has_creds,
                "api_key_set": !client.config.api_key.is_empty(),
                "google_credentials_set": client.config.google_credentials_json.is_some(),
                "project_id": client.config.project_id,
                "location": client.config.location,
                "vertex_express_keys_count": client.config.vertex_express_api_keys.len()
            })
        }
        None => {
            json!({
                "initialized": false,
                "has_credentials": false,
                "error": "No Vertex AI client available"
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_reset_global_fallback_client() {
        reset_global_fallback_client().await;
        assert!(get_global_fallback_client().await.is_none());
    }

    #[tokio::test]
    async fn test_vertex_ai_client_new() {
        let temp_dir = TempDir::new().unwrap();
        let cred_manager = CredentialManager::new(temp_dir.path().to_path_buf());

        let settings = Settings::default();
        let config = VertexConfig::from_settings(&settings);

        let client = VertexAIClient::new(cred_manager, config);
        assert!(!client.is_initialized);
    }

    #[tokio::test]
    async fn test_is_vertex_ai_available() {
        // Initially should be false
        assert!(!is_vertex_ai_available().await);
    }

    #[tokio::test]
    async fn test_get_vertex_ai_status() {
        let status = get_vertex_ai_status().await;
        assert_eq!(status["initialized"], false);
    }
}