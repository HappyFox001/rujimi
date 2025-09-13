use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Define data models - Rust equivalent of Python vertex/models.py

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageUrl {
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum ContentPart {
    #[serde(rename = "image_url")]
    Image {
        image_url: ImageUrl,
    },
    #[serde(rename = "text")]
    Text {
        text: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Parts(Vec<ContentPart>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIMessage {
    pub role: String,
    pub content: MessageContent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIRequest {
    pub model: String,
    pub messages: Vec<OpenAIMessage>,
    #[serde(default = "default_temperature")]
    pub temperature: Option<f32>,
    pub max_tokens: Option<i32>,
    #[serde(default = "default_top_p")]
    pub top_p: Option<f32>,
    pub top_k: Option<i32>,
    #[serde(default)]
    pub stream: Option<bool>,
    pub stop: Option<Vec<String>>,
    pub presence_penalty: Option<f32>,
    pub frequency_penalty: Option<f32>,
    pub seed: Option<i32>,
    pub logprobs: Option<i32>,
    pub response_logprobs: Option<bool>,
    /// Maps to candidate_count in Vertex AI
    pub n: Option<i32>,
    /// Allow extra fields to pass through without causing validation errors
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    #[serde(default)]
    pub completion_tokens: i32,
    #[serde(default)]
    pub prompt_tokens: i32,
    #[serde(default)]
    pub total_tokens: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiMessage {
    pub role: String, // 'user' or 'model'
    pub content: String,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiChatRequest {
    pub model: String,
    pub messages: Vec<GeminiMessage>,
    #[serde(default = "default_gemini_temperature")]
    pub temperature: Option<f32>,
    #[serde(default = "default_gemini_top_p")]
    pub top_p: Option<f32>,
    #[serde(default = "default_gemini_top_k")]
    pub top_k: Option<i32>,
    #[serde(default = "default_max_output_tokens")]
    pub max_output_tokens: Option<i32>,
    #[serde(default)]
    pub stream: Option<bool>,
}

impl GeminiChatRequest {
    pub fn log_request(&self) {
        log::info!("Chat request for model: {}", self.model);
        log::debug!(
            "Request parameters: temp={:?}, top_p={:?}, max_tokens={:?}",
            self.temperature, self.top_p, self.max_output_tokens
        );
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiCompletionRequest {
    pub model: String,
    pub prompt: String,
    #[serde(default = "default_gemini_temperature")]
    pub temperature: Option<f32>,
    #[serde(default = "default_gemini_top_p")]
    pub top_p: Option<f32>,
    #[serde(default = "default_gemini_top_k")]
    pub top_k: Option<i32>,
    #[serde(default = "default_max_output_tokens")]
    pub max_output_tokens: Option<i32>,
    #[serde(default)]
    pub stream: Option<bool>,
}

impl GeminiCompletionRequest {
    pub fn log_request(&self) {
        log::info!("Completion request for model: {}", self.model);
        log::debug!(
            "Request parameters: temp={:?}, top_p={:?}, max_tokens={:?}",
            self.temperature, self.top_p, self.max_output_tokens
        );
        let prompt_preview = if self.prompt.len() > 50 {
            format!("{}...", &self.prompt[..50])
        } else {
            self.prompt.clone()
        };
        log::debug!("Prompt preview: {}", prompt_preview);
    }
}

// Default value functions
fn default_temperature() -> Option<f32> {
    Some(1.0)
}

fn default_top_p() -> Option<f32> {
    Some(1.0)
}

fn default_gemini_temperature() -> Option<f32> {
    Some(0.7)
}

fn default_gemini_top_p() -> Option<f32> {
    Some(0.95)
}

fn default_gemini_top_k() -> Option<i32> {
    Some(40)
}

fn default_max_output_tokens() -> Option<i32> {
    Some(2048)
}