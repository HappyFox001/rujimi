use serde_json::{Value, json};
use anyhow::Result;
use crate::config::Settings;
use crate::vertex::{
    models::{OpenAIRequest, GeminiCompletionRequest},
    message_processing::{create_gemini_prompt, convert_to_openai_format, deobfuscate_text},
    api_helpers::{create_generation_config, create_openai_error_response, validate_request_parameters},
    credentials_manager::CredentialManager,
    vertex_ai_init::get_global_fallback_client,
};

// Rust equivalent of Python vertex/routes/chat_api.py

/// Handle chat completions request
pub async fn handle_chat_completion(
    settings: &Settings,
    request: OpenAIRequest,
) -> Result<Value> {
    log::info!("Processing chat completion request for model: {}", request.model);

    // Validate request parameters
    validate_request_parameters(&request)?;

    // Log request details
    log::debug!("Request parameters: temp={:?}, max_tokens={:?}, stream={:?}",
               request.temperature, request.max_tokens, request.stream);

    // Check if streaming is requested
    if request.stream.unwrap_or(false) {
        return handle_streaming_chat_completion(settings, request).await;
    }

    // Handle non-streaming request
    handle_non_streaming_chat_completion(settings, request).await
}

/// Handle non-streaming chat completion
async fn handle_non_streaming_chat_completion(
    settings: &Settings,
    request: OpenAIRequest,
) -> Result<Value> {
    log::debug!("Processing non-streaming chat completion");

    // Convert OpenAI messages to Gemini format
    let gemini_messages = create_gemini_prompt(&request.messages)?;
    let generation_config = create_generation_config(&request);

    // For now, return a placeholder response since we don't have the actual Gemini client integration
    // In a full implementation, this would call the Gemini API
    let mock_response = create_mock_chat_response(&request.model, &request.messages);

    log::info!("Chat completion processed successfully");
    Ok(mock_response)
}

/// Handle streaming chat completion
async fn handle_streaming_chat_completion(
    settings: &Settings,
    request: OpenAIRequest,
) -> Result<Value> {
    log::debug!("Processing streaming chat completion");

    // Convert OpenAI messages to Gemini format
    let _gemini_messages = create_gemini_prompt(&request.messages)?;
    let _generation_config = create_generation_config(&request);

    // For streaming, we would typically return a streaming response
    // For now, return an error indicating streaming is not yet implemented
    Err(anyhow::anyhow!("Streaming is not yet implemented in the Rust version"))
}

/// Handle completion request (legacy endpoint)
pub async fn handle_completion(
    settings: &Settings,
    request: GeminiCompletionRequest,
) -> Result<Value> {
    log::info!("Processing completion request for model: {}", request.model);

    request.log_request();

    // For now, return a placeholder response
    let mock_response = create_mock_completion_response(&request.model, &request.prompt);

    log::info!("Completion processed successfully");
    Ok(mock_response)
}

/// Create a mock chat completion response for testing
fn create_mock_chat_response(model: &str, messages: &[crate::vertex::models::OpenAIMessage]) -> Value {
    let last_message = messages.last()
        .map(|m| match &m.content {
            crate::vertex::models::MessageContent::Text(text) => text.clone(),
            crate::vertex::models::MessageContent::Parts(_) => "I received your message with multiple parts.".to_string(),
        })
        .unwrap_or_else(|| "No message received.".to_string());

    let response_content = format!("This is a mock response to: {}", last_message);

    json!({
        "id": format!("chatcmpl-{}", uuid::Uuid::new_v4()),
        "object": "chat.completion",
        "created": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        "model": model,
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": response_content
            },
            "finish_reason": "stop"
        }],
        "usage": {
            "prompt_tokens": estimate_tokens(&last_message),
            "completion_tokens": estimate_tokens(&response_content),
            "total_tokens": estimate_tokens(&last_message) + estimate_tokens(&response_content)
        }
    })
}

/// Create a mock completion response for testing
fn create_mock_completion_response(model: &str, prompt: &str) -> Value {
    let response_content = format!("This is a mock completion for: {}", prompt);

    json!({
        "id": format!("cmpl-{}", uuid::Uuid::new_v4()),
        "object": "text_completion",
        "created": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        "model": model,
        "choices": [{
            "text": response_content,
            "index": 0,
            "logprobs": null,
            "finish_reason": "stop"
        }],
        "usage": {
            "prompt_tokens": estimate_tokens(prompt),
            "completion_tokens": estimate_tokens(&response_content),
            "total_tokens": estimate_tokens(prompt) + estimate_tokens(&response_content)
        }
    })
}

/// Estimate token count (rough approximation)
fn estimate_tokens(text: &str) -> i32 {
    ((text.len() as f64) / 4.0).ceil() as i32
}

/// Validate model access and availability
pub async fn validate_model_access(settings: &Settings, model: &str) -> Result<()> {
    use crate::vertex::routes::models_api::is_model_available;

    if !is_model_available(settings, model).await? {
        return Err(anyhow::anyhow!("Model '{}' is not available or not found", model));
    }

    // Check if we have credentials for the model
    let client = get_global_fallback_client().await;
    match client {
        Some(client) => {
            if !client.has_credentials().await {
                return Err(anyhow::anyhow!("No valid credentials available for model access"));
            }
        }
        None => {
            return Err(anyhow::anyhow!("Vertex AI client not initialized"));
        }
    }

    Ok(())
}

/// Handle errors during chat completion
pub fn handle_chat_completion_error(error: &anyhow::Error) -> Value {
    let error_message = error.to_string();

    if error_message.contains("rate limit") || error_message.contains("quota") {
        create_openai_error_response(429, &error_message, "rate_limit_exceeded")
    } else if error_message.contains("authentication") || error_message.contains("credential") {
        create_openai_error_response(401, "Authentication failed", "authentication_error")
    } else if error_message.contains("not found") {
        create_openai_error_response(404, &error_message, "model_not_found")
    } else if error_message.contains("invalid") || error_message.contains("bad request") {
        create_openai_error_response(400, &error_message, "invalid_request")
    } else {
        create_openai_error_response(500, "Internal server error", "internal_error")
    }
}

/// Process request with retry logic
pub async fn process_request_with_retry<F, Fut, T>(
    operation: F,
    max_retries: usize,
    initial_delay: std::time::Duration,
) -> Result<T>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let mut delay = initial_delay;

    for attempt in 0..max_retries {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                if attempt == max_retries - 1 {
                    return Err(e);
                }

                log::warn!("Request attempt {} failed: {}. Retrying in {:?}",
                          attempt + 1, e, delay);

                tokio::time::sleep(delay).await;
                delay *= 2; // Exponential backoff
            }
        }
    }

    unreachable!()
}

/// Get request metrics and statistics
pub async fn get_request_metrics() -> Value {
    // This would typically track actual request metrics
    // For now, return basic placeholder metrics
    json!({
        "total_requests": 0,
        "successful_requests": 0,
        "failed_requests": 0,
        "average_response_time": 0.0,
        "models_used": {},
        "error_types": {}
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vertex::models::{OpenAIMessage, MessageContent};

    #[test]
    fn test_create_mock_chat_response() {
        let messages = vec![
            OpenAIMessage {
                role: "user".to_string(),
                content: MessageContent::Text("Hello, world!".to_string()),
            }
        ];

        let response = create_mock_chat_response("test-model", &messages);
        assert_eq!(response["object"], "chat.completion");
        assert_eq!(response["model"], "test-model");
        assert!(response["choices"].is_array());
    }

    #[test]
    fn test_create_mock_completion_response() {
        let response = create_mock_completion_response("test-model", "Hello");
        assert_eq!(response["object"], "text_completion");
        assert_eq!(response["model"], "test-model");
        assert!(response["choices"].is_array());
    }

    #[test]
    fn test_estimate_tokens() {
        assert_eq!(estimate_tokens("hello"), 2);
        assert_eq!(estimate_tokens("hello world"), 3);
        assert_eq!(estimate_tokens(""), 0);
    }

    #[tokio::test]
    async fn test_get_request_metrics() {
        let metrics = get_request_metrics().await;
        assert_eq!(metrics["total_requests"], 0);
        assert!(metrics["models_used"].is_object());
    }
}