use futures_util::{Stream, StreamExt};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use tokio_stream::wrappers::ReceiverStream;
use tracing::{debug, error, info, warn};

use crate::config::Settings;
use crate::models::schemas::{
    ChatCompletionRequest, ChatCompletionResponse, ChatCompletionStreamResponse, Message,
};
use crate::utils::logging::log;

#[derive(Debug, Clone)]
pub struct OpenAIClient {
    client: Client,
    settings: Arc<Settings>,
    whitelist: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct FilteredRequest {
    #[serde(flatten)]
    base: ChatCompletionRequest,
    #[serde(skip_serializing_if = "Option::is_none")]
    search: Option<bool>,
}

impl OpenAIClient {
    /// Create a new OpenAI client - equivalent to Python's __init__
    pub fn new(settings: Arc<Settings>) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .expect("Failed to create HTTP client");

        // Default whitelist of allowed fields
        let whitelist = vec![
            "model".to_string(),
            "messages".to_string(),
            "temperature".to_string(),
            "top_p".to_string(),
            "n".to_string(),
            "stream".to_string(),
            "stop".to_string(),
            "max_tokens".to_string(),
            "presence_penalty".to_string(),
            "frequency_penalty".to_string(),
            "logit_bias".to_string(),
            "user".to_string(),
            "response_format".to_string(),
            "seed".to_string(),
            "tools".to_string(),
            "tool_choice".to_string(),
            "search".to_string(), // Custom field for search mode
        ];

        Self {
            client,
            settings,
            whitelist,
        }
    }

    /// Stream chat completion - equivalent to Python's stream_chat
    pub async fn stream_chat(
        &self,
        request: ChatCompletionRequest,
    ) -> Result<
        Pin<Box<dyn Stream<Item = Result<ChatCompletionStreamResponse, Box<dyn std::error::Error>>> + Send>>,
        Box<dyn std::error::Error>,
    > {
        // Filter request data based on whitelist
        let filtered_data = self.filter_request_data(&request)?;

        // Log the streaming request
        log(
            "info",
            "开始OpenAI兼容流式聊天请求",
            Some({
                let mut extra = HashMap::new();
                extra.insert("model".to_string(), json!(request.model));
                extra.insert("messages_count".to_string(), json!(request.messages.len()));
                extra.insert("stream".to_string(), json!(true));
                extra.insert("search_mode".to_string(), json!(filtered_data.search.unwrap_or(false)));
                extra
            }),
        );

        // Construct the URL for Gemini's OpenAI-compatible endpoint
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions?key={}",
            self.settings.api_key
        );

        // Ensure streaming is enabled
        let mut streaming_request = filtered_data;
        streaming_request.base.stream = Some(true);

        debug!("发送流式请求到OpenAI兼容端点: {}", url);

        // Make the streaming request
        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&streaming_request)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            error!("OpenAI兼容API请求失败: {} - {}", response.status(), error_text);
            return Err(format!("OpenAI API error: {} - {}", response.status(), error_text).into());
        }

        let (tx, rx) = tokio::sync::mpsc::channel(100);

        // Spawn a task to handle the streaming response
        let client_clone = self.client.clone();
        tokio::spawn(async move {
            let mut response_stream = response.bytes_stream();
            let mut buffer = Vec::new();

            while let Some(chunk_result) = response_stream.next().await {
                match chunk_result {
                    Ok(chunk) => {
                        buffer.extend_from_slice(&chunk);

                        // Process complete lines
                        let buffer_str = String::from_utf8_lossy(&buffer);
                        let mut lines: Vec<&str> = buffer_str.lines().collect();

                        // Keep the last incomplete line in buffer
                        if !buffer_str.ends_with('\n') && !lines.is_empty() {
                            let last_line = lines.pop().unwrap();
                            buffer = last_line.as_bytes().to_vec();
                        } else {
                            buffer.clear();
                        }

                        for line in lines {
                            if let Some(chunk_response) = Self::parse_sse_line(line) {
                                if tx.send(Ok(chunk_response)).await.is_err() {
                                    return; // Receiver dropped
                                }
                            }
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(Err(e.into())).await;
                        return;
                    }
                }
            }

            // Process any remaining data in buffer
            if !buffer.is_empty() {
                let buffer_str = String::from_utf8_lossy(&buffer);
                for line in buffer_str.lines() {
                    if let Some(chunk_response) = Self::parse_sse_line(line) {
                        let _ = tx.send(Ok(chunk_response)).await;
                    }
                }
            }

            debug!("OpenAI兼容流式响应处理完成");
        });

        Ok(Box::pin(ReceiverStream::new(rx)))
    }

    /// Filter request data based on whitelist - equivalent to Python's filter_data
    fn filter_request_data(&self, request: &ChatCompletionRequest) -> Result<FilteredRequest, Box<dyn std::error::Error>> {
        let request_json = serde_json::to_value(request)?;
        let mut filtered = serde_json::Map::new();

        if let Value::Object(obj) = request_json {
            for (key, value) in obj {
                if self.whitelist.contains(&key) {
                    filtered.insert(key, value);
                } else {
                    debug!("过滤掉不支持的字段: {}", key);
                }
            }
        }

        let filtered_value = Value::Object(filtered);
        let filtered_request: FilteredRequest = serde_json::from_value(filtered_value)?;

        Ok(filtered_request)
    }

    /// Parse Server-Sent Events line - equivalent to Python's SSE parsing
    fn parse_sse_line(line: &str) -> Option<ChatCompletionStreamResponse> {
        let line = line.trim();

        // Skip empty lines and comments
        if line.is_empty() || line.starts_with(':') {
            return None;
        }

        // Handle data: prefix
        if line.starts_with("data: ") {
            let data = &line[6..]; // Skip "data: "

            // Handle [DONE] signal
            if data.trim() == "[DONE]" {
                return None;
            }

            // Try to parse JSON
            if let Ok(parsed) = serde_json::from_str::<ChatCompletionStreamResponse>(data) {
                return Some(parsed);
            } else {
                debug!("无法解析SSE数据: {}", data);
            }
        }

        None
    }

    /// Check if search mode is enabled in request
    pub fn is_search_mode_enabled(request: &ChatCompletionRequest) -> bool {
        // Check if any message contains search-related content or if search parameter is set
        request.messages.iter().any(|msg| {
            msg.content.as_ref().map_or(false, |content| {
                content.to_lowercase().contains("search") ||
                content.to_lowercase().contains("搜索")
            })
        })
    }

    /// Get supported models for OpenAI-compatible endpoint
    pub fn get_supported_models() -> Vec<&'static str> {
        vec![
            "gemini-1.5-flash",
            "gemini-1.5-pro",
            "gemini-1.0-pro",
            "gemini-exp-1121",
            "gemini-exp-1206",
        ]
    }

    /// Validate if model is supported
    pub fn is_model_supported(model: &str) -> bool {
        Self::get_supported_models().contains(&model)
    }

    /// Set custom whitelist for request filtering
    pub fn set_whitelist(&mut self, whitelist: Vec<String>) {
        self.whitelist = whitelist;
    }

    /// Get current whitelist
    pub fn get_whitelist(&self) -> &[String] {
        &self.whitelist
    }

    /// Health check for OpenAI-compatible endpoint
    pub async fn health_check(&self) -> bool {
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions?key={}",
            self.settings.api_key
        );

        // Simple health check request
        let health_request = json!({
            "model": "gemini-1.5-flash",
            "messages": [{"role": "user", "content": "ping"}],
            "max_tokens": 1,
            "stream": false
        });

        match self.client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&health_request)
            .send()
            .await
        {
            Ok(response) => {
                let is_healthy = response.status().is_success();
                if !is_healthy {
                    warn!("OpenAI兼容端点健康检查失败: {}", response.status());
                }
                is_healthy
            }
            Err(e) => {
                error!("OpenAI兼容端点健康检查错误: {}", e);
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Settings;

    #[test]
    fn test_openai_client_creation() {
        let settings = Arc::new(Settings::default());
        let client = OpenAIClient::new(settings);
        assert!(!client.whitelist.is_empty());
        assert!(client.whitelist.contains(&"model".to_string()));
    }

    #[test]
    fn test_model_validation() {
        assert!(OpenAIClient::is_model_supported("gemini-1.5-flash"));
        assert!(!OpenAIClient::is_model_supported("invalid-model"));
    }

    #[test]
    fn test_search_mode_detection() {
        let mut request = ChatCompletionRequest {
            model: "gemini-1.5-flash".to_string(),
            messages: vec![
                Message {
                    role: "user".to_string(),
                    content: Some("Please search for information about AI".to_string()),
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                }
            ],
            temperature: None,
            top_p: None,
            n: None,
            stream: None,
            stop: None,
            max_tokens: None,
            presence_penalty: None,
            frequency_penalty: None,
            logit_bias: None,
            user: None,
            response_format: None,
            seed: None,
            tools: None,
            tool_choice: None,
        };

        assert!(OpenAIClient::is_search_mode_enabled(&request));

        request.messages[0].content = Some("Just a normal question".to_string());
        assert!(!OpenAIClient::is_search_mode_enabled(&request));
    }

    #[test]
    fn test_whitelist_management() {
        let settings = Arc::new(Settings::default());
        let mut client = OpenAIClient::new(settings);

        let custom_whitelist = vec!["model".to_string(), "messages".to_string()];
        client.set_whitelist(custom_whitelist.clone());

        assert_eq!(client.get_whitelist(), custom_whitelist.as_slice());
    }

    #[test]
    fn test_sse_line_parsing() {
        // Test normal data line
        let json_data = r#"{"id":"123","object":"chat.completion.chunk","choices":[{"delta":{"content":"hello"}}]}"#;
        let line = format!("data: {}", json_data);
        let parsed = OpenAIClient::parse_sse_line(&line);
        assert!(parsed.is_some());

        // Test [DONE] signal
        let done_line = "data: [DONE]";
        let parsed_done = OpenAIClient::parse_sse_line(done_line);
        assert!(parsed_done.is_none());

        // Test empty line
        let empty_line = "";
        let parsed_empty = OpenAIClient::parse_sse_line(empty_line);
        assert!(parsed_empty.is_none());

        // Test comment line
        let comment_line = ": this is a comment";
        let parsed_comment = OpenAIClient::parse_sse_line(comment_line);
        assert!(parsed_comment.is_none());
    }
}