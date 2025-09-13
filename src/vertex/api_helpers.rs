use std::collections::HashMap;
use serde_json::{Value, json};
use crate::vertex::models::OpenAIRequest;
use anyhow::Result;

// Rust equivalent of Python vertex/api_helpers.py

/// Create OpenAI error response format
pub fn create_openai_error_response(
    status_code: u16,
    message: &str,
    error_type: &str,
) -> Value {
    json!({
        "error": {
            "message": message,
            "type": error_type,
            "code": status_code,
            "param": null,
        }
    })
}

/// Create generation configuration from OpenAI request
pub fn create_generation_config(request: &OpenAIRequest) -> HashMap<String, Value> {
    let mut config = HashMap::new();

    if let Some(temperature) = request.temperature {
        config.insert("temperature".to_string(), json!(temperature));
    }

    if let Some(max_tokens) = request.max_tokens {
        config.insert("max_output_tokens".to_string(), json!(max_tokens));
    }

    if let Some(top_p) = request.top_p {
        config.insert("top_p".to_string(), json!(top_p));
    }

    if let Some(top_k) = request.top_k {
        config.insert("top_k".to_string(), json!(top_k));
    }

    if let Some(ref stop) = request.stop {
        config.insert("stop_sequences".to_string(), json!(stop));
    }

    if let Some(seed) = request.seed {
        config.insert("seed".to_string(), json!(seed));
    }

    if let Some(n) = request.n {
        config.insert("candidate_count".to_string(), json!(n));
    }

    config
}

/// Handle rate limiting errors
pub fn handle_rate_limit_error(error_message: &str) -> Value {
    log::warn!("Rate limit exceeded: {}", error_message);
    create_openai_error_response(
        429,
        "Rate limit exceeded. Please try again later.",
        "rate_limit_exceeded",
    )
}

/// Handle authentication errors
pub fn handle_auth_error(error_message: &str) -> Value {
    log::error!("Authentication error: {}", error_message);
    create_openai_error_response(
        401,
        "Authentication failed. Please check your API key.",
        "authentication_error",
    )
}

/// Handle quota exceeded errors
pub fn handle_quota_error(error_message: &str) -> Value {
    log::error!("Quota exceeded: {}", error_message);
    create_openai_error_response(
        429,
        "Quota exceeded. Please check your billing and usage limits.",
        "quota_exceeded",
    )
}

/// Handle permission denied errors
pub fn handle_permission_error(error_message: &str) -> Value {
    log::error!("Permission denied: {}", error_message);
    create_openai_error_response(
        403,
        "Permission denied. You don't have access to this resource.",
        "permission_denied",
    )
}

/// Handle model not found errors
pub fn handle_model_not_found_error(model_name: &str) -> Value {
    log::error!("Model not found: {}", model_name);
    create_openai_error_response(
        404,
        &format!("Model '{}' not found. Please check the model name.", model_name),
        "model_not_found",
    )
}

/// Handle invalid request errors
pub fn handle_invalid_request_error(error_message: &str) -> Value {
    log::error!("Invalid request: {}", error_message);
    create_openai_error_response(
        400,
        &format!("Invalid request: {}", error_message),
        "invalid_request",
    )
}

/// Handle service unavailable errors
pub fn handle_service_unavailable_error(error_message: &str) -> Value {
    log::error!("Service unavailable: {}", error_message);
    create_openai_error_response(
        503,
        "Service temporarily unavailable. Please try again later.",
        "service_unavailable",
    )
}

/// Handle generic errors
pub fn handle_generic_error(error_message: &str) -> Value {
    log::error!("Generic error: {}", error_message);
    create_openai_error_response(
        500,
        "An unexpected error occurred. Please try again later.",
        "internal_error",
    )
}

/// Determine error type and create appropriate response
pub fn classify_and_handle_error(error_message: &str) -> Value {
    let error_lower = error_message.to_lowercase();

    if error_lower.contains("rate limit") || error_lower.contains("too many requests") {
        handle_rate_limit_error(error_message)
    } else if error_lower.contains("authentication") || error_lower.contains("unauthorized") {
        handle_auth_error(error_message)
    } else if error_lower.contains("quota") || error_lower.contains("billing") {
        handle_quota_error(error_message)
    } else if error_lower.contains("permission") || error_lower.contains("forbidden") {
        handle_permission_error(error_message)
    } else if error_lower.contains("model") && error_lower.contains("not found") {
        // Extract model name from error if possible
        handle_model_not_found_error("unknown")
    } else if error_lower.contains("invalid") || error_lower.contains("bad request") {
        handle_invalid_request_error(error_message)
    } else if error_lower.contains("unavailable") || error_lower.contains("timeout") {
        handle_service_unavailable_error(error_message)
    } else {
        handle_generic_error(error_message)
    }
}

/// Create streaming response chunk in OpenAI format
pub fn create_streaming_chunk(
    content: &str,
    model: &str,
    finish_reason: Option<&str>,
) -> Value {
    let mut choice = json!({
        "index": 0,
        "delta": {
            "content": content
        }
    });

    if let Some(reason) = finish_reason {
        choice["finish_reason"] = json!(reason);
    } else {
        choice["finish_reason"] = json!(null);
    }

    json!({
        "id": format!("chatcmpl-{}", uuid::Uuid::new_v4()),
        "object": "chat.completion.chunk",
        "created": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        "model": model,
        "choices": [choice]
    })
}

/// Create final streaming chunk
pub fn create_final_streaming_chunk(model: &str) -> Value {
    json!({
        "id": format!("chatcmpl-{}", uuid::Uuid::new_v4()),
        "object": "chat.completion.chunk",
        "created": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        "model": model,
        "choices": [{
            "index": 0,
            "delta": {},
            "finish_reason": "stop"
        }]
    })
}

/// Validate request parameters
pub fn validate_request_parameters(request: &OpenAIRequest) -> Result<()> {
    // Check model
    if request.model.trim().is_empty() {
        return Err(anyhow::anyhow!("Model name cannot be empty"));
    }

    // Check messages
    if request.messages.is_empty() {
        return Err(anyhow::anyhow!("Messages array cannot be empty"));
    }

    // Check temperature range
    if let Some(temp) = request.temperature {
        if temp < 0.0 || temp > 2.0 {
            return Err(anyhow::anyhow!("Temperature must be between 0.0 and 2.0"));
        }
    }

    // Check top_p range
    if let Some(top_p) = request.top_p {
        if top_p < 0.0 || top_p > 1.0 {
            return Err(anyhow::anyhow!("top_p must be between 0.0 and 1.0"));
        }
    }

    // Check max_tokens
    if let Some(max_tokens) = request.max_tokens {
        if max_tokens <= 0 {
            return Err(anyhow::anyhow!("max_tokens must be positive"));
        }
    }

    // Check top_k
    if let Some(top_k) = request.top_k {
        if top_k <= 0 {
            return Err(anyhow::anyhow!("top_k must be positive"));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_openai_error_response() {
        let response = create_openai_error_response(400, "Test error", "test_error");
        assert_eq!(response["error"]["message"], "Test error");
        assert_eq!(response["error"]["code"], 400);
        assert_eq!(response["error"]["type"], "test_error");
    }

    #[test]
    fn test_classify_and_handle_error() {
        let rate_limit_response = classify_and_handle_error("Rate limit exceeded");
        assert_eq!(rate_limit_response["error"]["code"], 429);

        let auth_response = classify_and_handle_error("Authentication failed");
        assert_eq!(auth_response["error"]["code"], 401);
    }

    #[test]
    fn test_create_generation_config() {
        use crate::vertex::models::*;

        let request = OpenAIRequest {
            model: "test-model".to_string(),
            messages: vec![],
            temperature: Some(0.7),
            max_tokens: Some(1000),
            top_p: Some(0.9),
            top_k: Some(40),
            stream: Some(false),
            stop: None,
            presence_penalty: None,
            frequency_penalty: None,
            seed: None,
            logprobs: None,
            response_logprobs: None,
            n: None,
            extra: HashMap::new(),
        };

        let config = create_generation_config(&request);
        assert_eq!(config.get("temperature").unwrap(), &json!(0.7));
        assert_eq!(config.get("max_output_tokens").unwrap(), &json!(1000));
        assert_eq!(config.get("top_p").unwrap(), &json!(0.9));
        assert_eq!(config.get("top_k").unwrap(), &json!(40));
    }
}