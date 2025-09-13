use std::env;
use std::path::PathBuf;
use crate::config::Settings;
use anyhow::Result;

// Rust equivalent of Python vertex/config.py

#[derive(Debug, Clone)]
pub struct VertexConfig {
    pub credentials_dir: PathBuf,
    pub api_key: String,
    pub google_credentials_json: Option<String>,
    pub project_id: Option<String>,
    pub location: String,
    pub models_config_url: String,
    pub vertex_express_api_keys: Vec<String>,
    pub fake_streaming_enabled: bool,
    pub fake_streaming_interval_seconds: f64,
    pub fake_streaming_chunk_size: usize,
    pub fake_streaming_delay_per_chunk: f64,
}

impl VertexConfig {
    pub fn from_settings(settings: &Settings) -> Self {
        // Set default credentials directory if not present
        let credentials_dir = settings.credentials_dir
            .as_ref()
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                let mut path = PathBuf::from(&settings.storage_dir);
                path.push("credentials");
                path
            });

        log::info!("Using credentials directory: {:?}", credentials_dir);

        // API Key configuration
        let api_key = if !settings.password.is_empty() {
            log::info!("Using API Key authentication");
            settings.password.clone()
        } else {
            log::info!("No API Key found, falling back to credentials file");
            String::new()
        };

        // Google Credentials JSON
        let google_credentials_json = settings.google_credentials_json.clone();
        if google_credentials_json.is_some() {
            log::info!("Using GOOGLE_CREDENTIALS_JSON environment variable for authentication");
        }

        // Project and location configuration
        let project_id = env::var("VERTEX_PROJECT_ID").ok()
            .or_else(|| settings.vertex_project_id.clone());

        let location = env::var("VERTEX_LOCATION")
            .unwrap_or_else(|_| settings.vertex_location.clone().unwrap_or_else(|| "us-central1".to_string()));

        // Model configuration URL
        let default_models_config_url = "https://raw.githubusercontent.com/gzzhongqi/vertex2openai/refs/heads/main/vertexModels.json";
        let models_config_url = env::var("VERTEX_MODELS_CONFIG_URL")
            .unwrap_or_else(|_| default_models_config_url.to_string());
        log::info!("Using models config URL: {}", models_config_url);

        // Vertex Express API Key configuration
        let vertex_express_api_keys = settings.vertex_express_api_key
            .as_ref()
            .map(|keys| {
                let key_list: Vec<String> = keys
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();

                if !key_list.is_empty() {
                    log::info!("Loaded {} Vertex Express API keys from settings", key_list.len());
                }
                key_list
            })
            .unwrap_or_default();

        // Fake streaming configuration
        let fake_streaming_enabled = settings.fake_streaming.unwrap_or(false);
        let fake_streaming_interval_seconds = settings.fake_streaming_interval.unwrap_or(1.0);
        let fake_streaming_chunk_size = settings.fake_streaming_chunk_size.unwrap_or(10);
        let fake_streaming_delay_per_chunk = settings.fake_streaming_delay_per_chunk.unwrap_or(0.1);

        log::info!(
            "Fake streaming is {} with interval {} seconds, chunk size {}, delay per chunk {} seconds",
            if fake_streaming_enabled { "enabled" } else { "disabled" },
            fake_streaming_interval_seconds,
            fake_streaming_chunk_size,
            fake_streaming_delay_per_chunk
        );

        Self {
            credentials_dir,
            api_key,
            google_credentials_json,
            project_id,
            location,
            models_config_url,
            vertex_express_api_keys,
            fake_streaming_enabled,
            fake_streaming_interval_seconds,
            fake_streaming_chunk_size,
            fake_streaming_delay_per_chunk,
        }
    }

    /// Update environment variable in memory
    pub fn update_env_var(name: &str, value: &str) {
        env::set_var(name, value);
        log::info!("Updated environment variable: {}", name);
    }

    /// Update configuration values
    pub fn update_config(&mut self, settings: &mut Settings, name: &str, value: String) -> Result<()> {
        match name {
            "VERTEX_API_KEY" => {
                settings.password = value.clone();
                self.api_key = value;
                log::info!("Updated API Key");
            }
            "GOOGLE_CREDENTIALS_JSON" => {
                settings.google_credentials_json = Some(value.clone());
                self.google_credentials_json = Some(value);
                log::info!("Updated Google Credentials JSON");
            }
            "VERTEX_PROJECT_ID" => {
                env::set_var("VERTEX_PROJECT_ID", &value);
                settings.vertex_project_id = Some(value.clone());
                self.project_id = Some(value.clone());
                log::info!("Updated Project ID to {}", value);
            }
            "VERTEX_LOCATION" => {
                env::set_var("VERTEX_LOCATION", &value);
                settings.vertex_location = Some(value.clone());
                self.location = value.clone();
                log::info!("Updated Location to {}", value);
            }
            "VERTEX_MODELS_CONFIG_URL" => {
                env::set_var("VERTEX_MODELS_CONFIG_URL", &value);
                self.models_config_url = value.clone();
                log::info!("Updated Models Config URL to {}", value);
            }
            "VERTEX_EXPRESS_API_KEY" => {
                settings.vertex_express_api_key = Some(value.clone());
                self.vertex_express_api_keys = value
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                log::info!(
                    "Updated Vertex Express API Key, now have {} keys",
                    self.vertex_express_api_keys.len()
                );
            }
            "FAKE_STREAMING" => {
                let bool_val = value.parse::<bool>()?;
                settings.fake_streaming = Some(bool_val);
                self.fake_streaming_enabled = bool_val;
                log::info!("Updated FAKE_STREAMING to {}", bool_val);
            }
            "FAKE_STREAMING_INTERVAL" => {
                let float_val = value.parse::<f64>()?;
                settings.fake_streaming_interval = Some(float_val);
                self.fake_streaming_interval_seconds = float_val;
                log::info!("Updated FAKE_STREAMING_INTERVAL to {}", float_val);
            }
            "FAKE_STREAMING_CHUNK_SIZE" => {
                let int_val = value.parse::<usize>()?;
                settings.fake_streaming_chunk_size = Some(int_val);
                self.fake_streaming_chunk_size = int_val;
                log::info!("Updated FAKE_STREAMING_CHUNK_SIZE to {}", int_val);
            }
            "FAKE_STREAMING_DELAY_PER_CHUNK" => {
                let float_val = value.parse::<f64>()?;
                settings.fake_streaming_delay_per_chunk = Some(float_val);
                self.fake_streaming_delay_per_chunk = float_val;
                log::info!("Updated FAKE_STREAMING_DELAY_PER_CHUNK to {}", float_val);
            }
            _ => {
                log::warn!("Unknown config variable: {}", name);
                return Ok(());
            }
        }

        // Update environment variable
        Self::update_env_var(name, &value);
        Ok(())
    }

    /// Reload configuration - usually called after persistent settings are loaded
    pub fn reload_config(&mut self, settings: &Settings) {
        // Reload Google Credentials JSON
        self.google_credentials_json = settings.google_credentials_json.clone();
        if self.google_credentials_json.is_some() {
            log::info!("Reloaded GOOGLE_CREDENTIALS_JSON configuration");
        }

        // Reload Vertex Express API Key
        self.vertex_express_api_keys = settings.vertex_express_api_key
            .as_ref()
            .map(|keys| {
                let key_list: Vec<String> = keys
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();

                if !key_list.is_empty() {
                    log::info!("Reloaded {} Vertex Express API keys", key_list.len());
                }
                key_list
            })
            .unwrap_or_default();

        // Reload API Key
        self.api_key = if !settings.password.is_empty() {
            log::info!("Reloaded API Key configuration");
            settings.password.clone()
        } else {
            String::new()
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_update_env_var() {
        VertexConfig::update_env_var("TEST_VAR", "test_value");
        assert_eq!(env::var("TEST_VAR").unwrap(), "test_value");
    }
}