use rand::{distributions::Alphanumeric, Rng};
use serde_json::Value;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;

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
}