use rand::{distributions::Alphanumeric, Rng};
use serde_json::{Value, json};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use chrono::Utc;
use uuid::Uuid;

pub fn generate_random_string(length: usize) -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(length)
        .map(char::from)
        .collect()
}

pub fn sanitize_response_content(content: &str) -> String {
    // Remove any potential sensitive information or unwanted content
    content
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

pub fn extract_text_from_value(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Array(arr) => {
            arr.iter()
                .filter_map(|item| {
                    if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                        Some(text.to_string())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join("")
        }
        _ => value.to_string(),
    }
}

pub fn estimate_tokens(text: &str) -> u32 {
    // Simple token estimation: roughly 4 characters per token
    (text.len() as f32 / 4.0).ceil() as u32
}

pub fn create_error_response(message: &str, error_type: &str) -> Response {
    let error_json = serde_json::json!({
        "error": {
            "message": message,
            "type": error_type,
            "code": null,
            "param": null
        }
    });

    let status = match error_type {
        "authentication_error" => StatusCode::UNAUTHORIZED,
        "forbidden_error" => StatusCode::FORBIDDEN,
        "invalid_model" => StatusCode::BAD_REQUEST,
        "service_unavailable" => StatusCode::SERVICE_UNAVAILABLE,
        "api_error" => StatusCode::INTERNAL_SERVER_ERROR,
        "stream_error" => StatusCode::INTERNAL_SERVER_ERROR,
        _ => StatusCode::BAD_REQUEST,
    };

    (status, Json(error_json)).into_response()
}

pub fn create_error_json(message: &str, error_type: &str) -> Value {
    serde_json::json!({
        "error": {
            "message": message,
            "type": error_type,
            "code": null,
            "param": null
        }
    })
}

pub fn create_sse_data(data: &str) -> String {
    format!("data: {}\n\n", data)
}

pub fn create_sse_event(event: &str, data: &str) -> String {
    format!("event: {}\ndata: {}\n\n", event, data)
}

/// Convert text to OpenAI format response - equivalent to Python's openAI_from_text()
pub fn openai_from_text(
    text: &str,
    model: &str,
    usage_prompt_tokens: Option<u32>,
    usage_completion_tokens: Option<u32>,
    stream: bool,
) -> Value {
    let prompt_tokens = usage_prompt_tokens.unwrap_or_else(|| estimate_tokens(text));
    let completion_tokens = usage_completion_tokens.unwrap_or_else(|| estimate_tokens(text));
    let total_tokens = prompt_tokens + completion_tokens;

    if stream {
        // For streaming response, return chunk format
        json!({
            "id": format!("chatcmpl-{}", Uuid::new_v4()),
            "object": "chat.completion.chunk",
            "created": Utc::now().timestamp(),
            "model": model,
            "choices": [{
                "index": 0,
                "delta": {
                    "content": text
                },
                "finish_reason": null
            }]
        })
    } else {
        // For non-streaming response
        json!({
            "id": format!("chatcmpl-{}", Uuid::new_v4()),
            "object": "chat.completion",
            "created": Utc::now().timestamp(),
            "model": model,
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": text
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": prompt_tokens,
                "completion_tokens": completion_tokens,
                "total_tokens": total_tokens
            }
        })
    }
}

/// Convert text to Gemini format response - equivalent to Python's gemini_from_text()
pub fn gemini_from_text(text: &str, model: &str) -> Value {
    json!({
        "candidates": [{
            "content": {
                "parts": [{
                    "text": text
                }],
                "role": "model"
            },
            "finishReason": "STOP",
            "index": 0,
            "safetyRatings": []
        }],
        "usageMetadata": {
            "promptTokenCount": estimate_tokens(text),
            "candidatesTokenCount": estimate_tokens(text),
            "totalTokenCount": estimate_tokens(text) * 2
        },
        "modelVersion": model
    })
}

/// Convert Gemini response to OpenAI format - equivalent to Python's openAI_from_Gemini()
pub fn openai_from_gemini(gemini_response: &Value, stream: bool) -> Value {
    // Extract text content from Gemini response
    let content = extract_gemini_content(gemini_response);
    let model = gemini_response
        .get("modelVersion")
        .and_then(|v| v.as_str())
        .unwrap_or("gemini-pro");

    // Extract usage information if available
    let usage = extract_gemini_usage(gemini_response);

    if stream {
        // For streaming response
        json!({
            "id": format!("chatcmpl-{}", Uuid::new_v4()),
            "object": "chat.completion.chunk",
            "created": Utc::now().timestamp(),
            "model": model,
            "choices": [{
                "index": 0,
                "delta": {
                    "content": content
                },
                "finish_reason": null
            }]
        })
    } else {
        // For non-streaming response
        json!({
            "id": format!("chatcmpl-{}", Uuid::new_v4()),
            "object": "chat.completion",
            "created": Utc::now().timestamp(),
            "model": model,
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": content
                },
                "finish_reason": "stop"
            }],
            "usage": usage
        })
    }
}

/// Extract content from Gemini response
fn extract_gemini_content(gemini_response: &Value) -> String {
    if let Some(candidates) = gemini_response.get("candidates") {
        if let Some(candidate) = candidates.get(0) {
            if let Some(content) = candidate.get("content") {
                if let Some(parts) = content.get("parts") {
                    if let Some(part) = parts.get(0) {
                        if let Some(text) = part.get("text") {
                            return text.as_str().unwrap_or("").to_string();
                        }
                    }
                }
            }
        }
    }

    // Fallback: try to extract from other possible locations
    if let Some(text) = gemini_response.get("text") {
        return text.as_str().unwrap_or("").to_string();
    }

    "".to_string()
}

/// Extract usage information from Gemini response
fn extract_gemini_usage(gemini_response: &Value) -> Value {
    if let Some(usage_metadata) = gemini_response.get("usageMetadata") {
        let prompt_tokens = usage_metadata
            .get("promptTokenCount")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;

        let completion_tokens = usage_metadata
            .get("candidatesTokenCount")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;

        let total_tokens = usage_metadata
            .get("totalTokenCount")
            .and_then(|v| v.as_u64())
            .unwrap_or((prompt_tokens + completion_tokens) as u64) as u32;

        return json!({
            "prompt_tokens": prompt_tokens,
            "completion_tokens": completion_tokens,
            "total_tokens": total_tokens
        });
    }

    // Default usage if not available
    json!({
        "prompt_tokens": 0,
        "completion_tokens": 0,
        "total_tokens": 0
    })
}

/// Create streaming completion chunk
pub fn create_completion_chunk(content: &str, model: &str, finish_reason: Option<&str>) -> String {
    let chunk = json!({
        "id": format!("chatcmpl-{}", Uuid::new_v4()),
        "object": "chat.completion.chunk",
        "created": Utc::now().timestamp(),
        "model": model,
        "choices": [{
            "index": 0,
            "delta": if content.is_empty() { json!({}) } else { json!({"content": content}) },
            "finish_reason": finish_reason
        }]
    });

    format!("data: {}\n\n", chunk)
}

/// Create final streaming chunk
pub fn create_final_chunk(model: &str) -> String {
    let chunk = json!({
        "id": format!("chatcmpl-{}", Uuid::new_v4()),
        "object": "chat.completion.chunk",
        "created": Utc::now().timestamp(),
        "model": model,
        "choices": [{
            "index": 0,
            "delta": {},
            "finish_reason": "stop"
        }]
    });

    format!("data: {}\n\ndata: [DONE]\n\n", chunk)
}

/// Create models list response
pub fn create_models_response(models: Vec<&str>) -> Value {
    let model_objects: Vec<Value> = models
        .into_iter()
        .map(|model| {
            json!({
                "id": model,
                "object": "model",
                "created": Utc::now().timestamp(),
                "owned_by": "openai",
                "permission": [],
                "root": model,
                "parent": null
            })
        })
        .collect();

    json!({
        "object": "list",
        "data": model_objects
    })
}

/// Create embedding response format
pub fn create_embedding_response(
    embeddings: Vec<Vec<f32>>,
    model: &str,
    input_tokens: u32,
) -> Value {
    let embedding_objects: Vec<Value> = embeddings
        .into_iter()
        .enumerate()
        .map(|(index, embedding)| {
            json!({
                "object": "embedding",
                "embedding": embedding,
                "index": index
            })
        })
        .collect();

    json!({
        "object": "list",
        "data": embedding_objects,
        "model": model,
        "usage": {
            "prompt_tokens": input_tokens,
            "total_tokens": input_tokens
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_random_string() {
        let s1 = generate_random_string(10);
        let s2 = generate_random_string(10);

        assert_eq!(s1.len(), 10);
        assert_eq!(s2.len(), 10);
        assert_ne!(s1, s2); // Should be different
    }

    #[test]
    fn test_estimate_tokens() {
        assert_eq!(estimate_tokens("hello"), 2); // 5 chars / 4 = 1.25 -> 2
        assert_eq!(estimate_tokens("hello world"), 3); // 11 chars / 4 = 2.75 -> 3
    }

    #[test]
    fn test_sanitize_response_content() {
        let input = "  hello  \n\n  world  \n\n  ";
        let expected = "hello\nworld";
        assert_eq!(sanitize_response_content(input), expected);
    }

    #[test]
    fn test_openai_from_text() {
        let response = openai_from_text("Hello world", "gpt-4", Some(10), Some(5), false);

        assert_eq!(response["model"], "gpt-4");
        assert_eq!(response["object"], "chat.completion");
        assert_eq!(response["choices"][0]["message"]["content"], "Hello world");
        assert_eq!(response["usage"]["prompt_tokens"], 10);
        assert_eq!(response["usage"]["completion_tokens"], 5);
        assert_eq!(response["usage"]["total_tokens"], 15);
    }

    #[test]
    fn test_gemini_from_text() {
        let response = gemini_from_text("Hello world", "gemini-pro");

        assert_eq!(response["modelVersion"], "gemini-pro");
        assert_eq!(response["candidates"][0]["content"]["parts"][0]["text"], "Hello world");
        assert_eq!(response["candidates"][0]["finishReason"], "STOP");
    }

    #[test]
    fn test_extract_gemini_content() {
        let gemini_response = json!({
            "candidates": [{
                "content": {
                    "parts": [{
                        "text": "Hello from Gemini"
                    }]
                }
            }]
        });

        let content = extract_gemini_content(&gemini_response);
        assert_eq!(content, "Hello from Gemini");
    }

    #[test]
    fn test_openai_from_gemini() {
        let gemini_response = json!({
            "candidates": [{
                "content": {
                    "parts": [{
                        "text": "Response from Gemini"
                    }]
                }
            }],
            "modelVersion": "gemini-pro",
            "usageMetadata": {
                "promptTokenCount": 10,
                "candidatesTokenCount": 15,
                "totalTokenCount": 25
            }
        });

        let openai_response = openai_from_gemini(&gemini_response, false);

        assert_eq!(openai_response["model"], "gemini-pro");
        assert_eq!(openai_response["choices"][0]["message"]["content"], "Response from Gemini");
        assert_eq!(openai_response["usage"]["prompt_tokens"], 10);
        assert_eq!(openai_response["usage"]["completion_tokens"], 15);
        assert_eq!(openai_response["usage"]["total_tokens"], 25);
    }

    #[test]
    fn test_create_completion_chunk() {
        let chunk = create_completion_chunk("Hello", "gpt-4", None);
        assert!(chunk.starts_with("data: "));
        assert!(chunk.contains("Hello"));
        assert!(chunk.contains("gpt-4"));
    }

    #[test]
    fn test_create_final_chunk() {
        let chunk = create_final_chunk("gpt-4");
        assert!(chunk.contains("data: [DONE]"));
        assert!(chunk.contains("finish_reason"));
    }
}