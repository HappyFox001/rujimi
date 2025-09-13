use anyhow::{Context, Result};
use async_trait::async_trait;
use futures_util::{Stream, StreamExt};
use reqwest::Client;
use serde_json::{json, Value};
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::config::Settings;
use crate::models::schemas::{
    ChatCompletionRequest, ChatCompletionResponse, ChatChoice, ChatMessage, Usage,
    ChatCompletionChunk, ChatChoiceDelta, ChatMessageDelta,
    GeminiRequest, GeminiResponse, GeminiContent, GeminiPart, GeminiGenerationConfig,
    GeminiSafetySetting, GeminiTool, GeminiFunctionDeclaration, ToolCall, FunctionCall,
    Model, ModelResponse, EmbeddingRequest, EmbeddingResponse,
};
use crate::utils::response::generate_random_string;

const GEMINI_BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta";
const GEMINI_SEARCH_TOOLS: &str = r#"[{"googleSearchRetrieval": {}}]"#;

#[async_trait]
pub trait GeminiClientTrait {
    async fn chat_completion(&self, request: ChatCompletionRequest, api_key: &str) -> Result<ChatCompletionResponse>;
    async fn chat_completion_stream(&self, request: ChatCompletionRequest, api_key: &str) -> Result<Pin<Box<dyn Stream<Item = Result<ChatCompletionChunk>> + Send>>>;
    async fn list_models(&self, api_key: &str) -> Result<Vec<Model>>;
    async fn embedding(&self, request: EmbeddingRequest, api_key: &str) -> Result<EmbeddingResponse>;
}

#[derive(Debug, Clone)]
pub struct GeminiClient {
    settings: Arc<Settings>,
    client: Client,
    available_models: Arc<RwLock<Vec<String>>>,
}

impl GeminiClient {
    pub fn new(settings: Arc<Settings>) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(600))
            .http2_adaptive_window(true)
            .build()
            .expect("Failed to create HTTP client");

        Self {
            settings,
            client,
            available_models: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn initialize_models(&self, api_key: &str) -> Result<()> {
        match self.fetch_available_models(api_key).await {
            Ok(models) => {
                let model_names: Vec<String> = models
                    .into_iter()
                    .map(|model| model.id.replace("models/", ""))
                    .collect();

                let mut available_models = self.available_models.write().await;
                *available_models = model_names.clone();

                info!("Loaded {} available models", model_names.len());
                Ok(())
            }
            Err(e) => {
                warn!("Failed to load available models: {}", e);
                // Use default models if fetching fails
                let mut available_models = self.available_models.write().await;
                *available_models = self.get_default_models();
                Ok(())
            }
        }
    }

    async fn fetch_available_models(&self, api_key: &str) -> Result<Vec<Model>> {
        let url = format!("{}/models", GEMINI_BASE_URL);

        let response = self.client
            .get(&url)
            .header("x-goog-api-key", api_key)
            .send()
            .await
            .context("Failed to fetch models from Gemini API")?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!("Failed to fetch models: {}", response.status()));
        }

        let model_response: ModelResponse = response.json().await
            .context("Failed to parse models response")?;

        Ok(model_response.data)
    }

    fn get_default_models(&self) -> Vec<String> {
        vec![
            "gemini-1.5-pro".to_string(),
            "gemini-1.5-pro-exp-0827".to_string(),
            "gemini-1.5-flash".to_string(),
            "gemini-1.5-flash-8b".to_string(),
            "gemini-2.0-flash-exp".to_string(),
            "text-embedding-004".to_string(),
        ]
    }

    pub async fn get_available_models(&self) -> Vec<String> {
        let models = self.available_models.read().await;
        models.clone()
    }

    fn convert_to_gemini_request(&self, request: &ChatCompletionRequest) -> Result<GeminiRequest> {
        let mut gemini_contents = Vec::new();

        for message in &request.messages {
            let role = match message.role.as_str() {
                "user" => "user",
                "assistant" => "model",
                "system" => "user", // System messages are converted to user messages
                _ => "user",
            };

            let parts = self.convert_message_content(&message.content)?;

            gemini_contents.push(GeminiContent {
                role: role.to_string(),
                parts,
            });
        }

        let generation_config = GeminiGenerationConfig {
            temperature: request.temperature,
            top_p: request.top_p,
            max_output_tokens: request.max_tokens,
            candidate_count: Some(1),
            ..Default::default()
        };

        let mut tools = None;
        if let Some(openai_tools) = &request.tools {
            tools = Some(vec![GeminiTool {
                function_declarations: openai_tools
                    .iter()
                    .map(|tool| GeminiFunctionDeclaration {
                        name: tool.function.name.clone(),
                        description: tool.function.description.clone().unwrap_or_default(),
                        parameters: tool.function.parameters.clone().unwrap_or(json!({})),
                    })
                    .collect(),
            }]);
        }

        // Add search tools if search mode is enabled and model supports it
        if self.settings.search_mode && request.model.contains("-search") {
            let search_tools: Vec<Value> = serde_json::from_str(GEMINI_SEARCH_TOOLS)?;
            // Merge with existing tools if any
        }

        // Add random string for stealth if enabled
        if self.settings.random_string {
            let random_str = generate_random_string(self.settings.random_string_length);
            if let Some(first_content) = gemini_contents.first_mut() {
                if let Some(GeminiPart::Text { text }) = first_content.parts.first_mut() {
                    text.push_str(&format!(" {}", random_str));
                }
            }
        }

        Ok(GeminiRequest {
            contents: gemini_contents,
            generation_config: Some(generation_config),
            safety_settings: Some(self.get_safety_settings()),
            tools,
            tool_config: None,
        })
    }

    fn convert_message_content(&self, content: &Option<Value>) -> Result<Vec<GeminiPart>> {
        let mut parts = Vec::new();

        if let Some(content_value) = content {
            match content_value {
                Value::String(text) => {
                    parts.push(GeminiPart::Text { text: text.clone() });
                }
                Value::Array(content_array) => {
                    for item in content_array {
                        if let Some(part_type) = item.get("type").and_then(|t| t.as_str()) {
                            match part_type {
                                "text" => {
                                    if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                                        parts.push(GeminiPart::Text { text: text.to_string() });
                                    }
                                }
                                "image_url" => {
                                    if let Some(image_url) = item.get("image_url").and_then(|u| u.get("url")).and_then(|url| url.as_str()) {
                                        if let Ok((mime_type, data)) = self.parse_base64_image(image_url) {
                                            parts.push(GeminiPart::InlineData {
                                                inline_data: crate::models::schemas::GeminiInlineData {
                                                    mime_type,
                                                    data,
                                                }
                                            });
                                        }
                                    }
                                }
                                _ => {
                                    warn!("Unsupported content type: {}", part_type);
                                }
                            }
                        }
                    }
                }
                _ => {
                    parts.push(GeminiPart::Text { text: content_value.to_string() });
                }
            }
        }

        Ok(parts)
    }

    fn parse_base64_image(&self, image_url: &str) -> Result<(String, String)> {
        if image_url.starts_with("data:") {
            let parts: Vec<&str> = image_url.splitn(2, ',').collect();
            if parts.len() == 2 {
                let header = parts[0];
                let data = parts[1];

                if let Some(mime_type) = header.strip_prefix("data:").and_then(|h| h.split(';').next()) {
                    return Ok((mime_type.to_string(), data.to_string()));
                }
            }
        }

        Err(anyhow::anyhow!("Invalid base64 image format"))
    }

    fn get_safety_settings(&self) -> Vec<GeminiSafetySetting> {
        vec![
            GeminiSafetySetting {
                category: "HARM_CATEGORY_HARASSMENT".to_string(),
                threshold: "BLOCK_NONE".to_string(),
            },
            GeminiSafetySetting {
                category: "HARM_CATEGORY_HATE_SPEECH".to_string(),
                threshold: "BLOCK_NONE".to_string(),
            },
            GeminiSafetySetting {
                category: "HARM_CATEGORY_SEXUALLY_EXPLICIT".to_string(),
                threshold: "BLOCK_NONE".to_string(),
            },
            GeminiSafetySetting {
                category: "HARM_CATEGORY_DANGEROUS_CONTENT".to_string(),
                threshold: "BLOCK_NONE".to_string(),
            },
        ]
    }

    fn convert_gemini_response(&self, gemini_response: GeminiResponse, request: &ChatCompletionRequest) -> Result<ChatCompletionResponse> {
        let mut choices = Vec::new();

        for (index, candidate) in gemini_response.candidates.into_iter().enumerate() {
            let message = self.convert_gemini_content_to_message(candidate.content)?;

            choices.push(ChatChoice {
                index: index as u32,
                message,
                finish_reason: candidate.finish_reason,
                logprobs: None,
            });
        }

        let usage = gemini_response.usage_metadata.map(|meta| Usage {
            prompt_tokens: meta.prompt_token_count.unwrap_or(0),
            completion_tokens: meta.candidates_token_count.unwrap_or(0),
            total_tokens: meta.total_token_count.unwrap_or(0),
        });

        Ok(ChatCompletionResponse {
            id: format!("chatcmpl-{}", uuid::Uuid::new_v4()),
            object: "chat.completion".to_string(),
            created: chrono::Utc::now().timestamp() as u64,
            model: request.model.clone(),
            choices,
            usage,
            system_fingerprint: None,
        })
    }

    fn convert_gemini_content_to_message(&self, content: GeminiContent) -> Result<ChatMessage> {
        let mut text_parts = Vec::new();
        let mut tool_calls = Vec::new();

        for part in content.parts {
            match part {
                GeminiPart::Text { text } => {
                    text_parts.push(text);
                }
                GeminiPart::FunctionCall { function_call } => {
                    tool_calls.push(ToolCall {
                        id: format!("call_{}", uuid::Uuid::new_v4()),
                        tool_type: "function".to_string(),
                        function: FunctionCall {
                            name: function_call.name,
                            arguments: serde_json::to_string(&function_call.args)?,
                        },
                    });
                }
                _ => {}
            }
        }

        let role = match content.role.as_str() {
            "model" => "assistant",
            _ => "user",
        };

        Ok(ChatMessage {
            role: role.to_string(),
            content: if text_parts.is_empty() {
                None
            } else {
                Some(Value::String(text_parts.join("")))
            },
            name: None,
            tool_calls: if tool_calls.is_empty() { None } else { Some(tool_calls) },
            tool_call_id: None,
        })
    }

    async fn make_gemini_request(&self, url: &str, api_key: &str, body: Value) -> Result<reqwest::Response> {
        let response = self.client
            .post(url)
            .header("Content-Type", "application/json")
            .header("x-goog-api-key", api_key)
            .json(&body)
            .send()
            .await
            .context("Failed to send request to Gemini API")?;

        Ok(response)
    }
}

impl Default for GeminiGenerationConfig {
    fn default() -> Self {
        Self {
            temperature: None,
            top_p: None,
            top_k: None,
            candidate_count: Some(1),
            max_output_tokens: None,
            stop_sequences: None,
        }
    }
}

#[async_trait]
impl GeminiClientTrait for GeminiClient {
    async fn chat_completion(&self, request: ChatCompletionRequest, api_key: &str) -> Result<ChatCompletionResponse> {
        let model_name = if request.model.contains("-search") {
            request.model.replace("-search", "")
        } else {
            request.model.clone()
        };

        let url = format!("{}/models/{}:generateContent", GEMINI_BASE_URL, model_name);

        let gemini_request = self.convert_to_gemini_request(&request)?;
        let body = serde_json::to_value(gemini_request)?;

        debug!("Sending request to Gemini API: {}", url);

        let response = self.make_gemini_request(&url, api_key, body).await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Gemini API error: {} - {}", status, error_text));
        }

        let gemini_response: GeminiResponse = response.json().await
            .context("Failed to parse Gemini response")?;

        self.convert_gemini_response(gemini_response, &request)
    }

    async fn chat_completion_stream(&self, request: ChatCompletionRequest, api_key: &str) -> Result<Pin<Box<dyn Stream<Item = Result<ChatCompletionChunk>> + Send>>> {
        let model_name = if request.model.contains("-search") {
            request.model.replace("-search", "")
        } else {
            request.model.clone()
        };

        let url = format!("{}/models/{}:streamGenerateContent", GEMINI_BASE_URL, model_name);

        let gemini_request = self.convert_to_gemini_request(&request)?;
        let body = serde_json::to_value(gemini_request)?;

        let response = self.make_gemini_request(&url, api_key, body).await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Gemini API error: {} - {}", status, error_text));
        }

        let stream = response.bytes_stream()
            .map(move |chunk_result| {
                match chunk_result {
                    Ok(chunk) => {
                        // Parse streaming response and convert to OpenAI format
                        // This is a simplified implementation
                        let chunk_str = String::from_utf8_lossy(&chunk);

                        // Create a chat completion chunk
                        Ok(ChatCompletionChunk {
                            id: format!("chatcmpl-{}", uuid::Uuid::new_v4()),
                            object: "chat.completion.chunk".to_string(),
                            created: chrono::Utc::now().timestamp() as u64,
                            model: request.model.clone(),
                            choices: vec![ChatChoiceDelta {
                                index: 0,
                                delta: ChatMessageDelta {
                                    role: Some("assistant".to_string()),
                                    content: Some(chunk_str.to_string()),
                                    tool_calls: None,
                                },
                                finish_reason: None,
                                logprobs: None,
                            }],
                            system_fingerprint: None,
                        })
                    }
                    Err(e) => Err(anyhow::anyhow!("Stream error: {}", e)),
                }
            });

        Ok(Box::pin(stream))
    }

    async fn list_models(&self, api_key: &str) -> Result<Vec<Model>> {
        self.fetch_available_models(api_key).await
    }

    async fn embedding(&self, request: EmbeddingRequest, api_key: &str) -> Result<EmbeddingResponse> {
        let url = format!("{}/models/{}:embedContent", GEMINI_BASE_URL, request.model);

        let content = match &request.input {
            crate::models::schemas::EmbeddingInput::String(text) => text.clone(),
            crate::models::schemas::EmbeddingInput::ArrayOfStrings(texts) => texts.join(" "),
            _ => return Err(anyhow::anyhow!("Unsupported embedding input format")),
        };

        let body = json!({
            "content": {
                "parts": [{"text": content}]
            }
        });

        let response = self.make_gemini_request(&url, api_key, body).await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Gemini API error: {} - {}", status, error_text));
        }
        
        let gemini_response: Value = response.json().await
            .context("Failed to parse Gemini embedding response")?;

        // Convert Gemini embedding response to OpenAI format
        let embedding_data = gemini_response
            .get("embedding")
            .and_then(|e| e.get("values"))
            .and_then(|v| v.as_array())
            .ok_or_else(|| anyhow::anyhow!("Invalid embedding response format"))?;

        let embedding: Vec<f64> = embedding_data
            .iter()
            .filter_map(|v| v.as_f64())
            .collect();

        Ok(EmbeddingResponse {
            object: "list".to_string(),
            data: vec![crate::models::schemas::EmbeddingData {
                object: "embedding".to_string(),
                embedding,
                index: 0,
            }],
            model: request.model,
            usage: crate::models::schemas::EmbeddingUsage {
                prompt_tokens: content.len() as u32 / 4, // Rough estimation
                total_tokens: content.len() as u32 / 4,
            },
        })
    }
}