use anyhow::Result;
use std::sync::Arc;

use crate::config::Settings;
use crate::models::schemas::{ChatCompletionRequest, ChatCompletionResponse};

#[derive(Debug, Clone)]
pub struct VertexClient {
    settings: Arc<Settings>,
}

impl VertexClient {
    pub fn new(settings: Arc<Settings>) -> Self {
        Self { settings }
    }

    pub async fn chat_completion(
        &self,
        _request: ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse> {
        // Placeholder implementation - will be implemented later
        Err(anyhow::anyhow!("Vertex AI not implemented yet"))
    }

    pub fn is_enabled(&self) -> bool {
        self.settings.enable_vertex
    }
}