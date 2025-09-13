use serde_json::Value;
use std::collections::HashMap;
use tracing::error;

pub fn translate_error(error_message: &str) -> String {
    // Map of common Gemini API errors to user-friendly messages
    let error_mappings = create_error_mappings();

    for (pattern, friendly_message) in &error_mappings {
        if error_message.to_lowercase().contains(&pattern.to_lowercase()) {
            return friendly_message.clone();
        }
    }

    // If no specific mapping found, return sanitized original message
    sanitize_error_message(error_message)
}

fn create_error_mappings() -> HashMap<&'static str, String> {
    let mut mappings = HashMap::new();

    // API Key related errors
    mappings.insert("invalid api key", "API key is invalid or expired".to_string());
    mappings.insert("quota exceeded", "API quota has been exceeded".to_string());
    mappings.insert("rate limit", "Rate limit exceeded, please try again later".to_string());

    // Content related errors
    mappings.insert("safety", "Content was blocked due to safety policies".to_string());
    mappings.insert("blocked", "Content was blocked by content filter".to_string());
    mappings.insert("recitation", "Content may contain copyrighted material".to_string());

    // Model related errors
    mappings.insert("model not found", "The specified model is not available".to_string());
    mappings.insert("unsupported", "This operation is not supported".to_string());

    // Request related errors
    mappings.insert("invalid request", "Invalid request format or parameters".to_string());
    mappings.insert("token limit", "Request exceeds maximum token limit".to_string());
    mappings.insert("timeout", "Request timed out, please try again".to_string());

    // Server related errors
    mappings.insert("internal error", "Internal server error occurred".to_string());
    mappings.insert("service unavailable", "Service is temporarily unavailable".to_string());
    mappings.insert("bad gateway", "Gateway error, please try again".to_string());

    mappings
}

fn sanitize_error_message(message: &str) -> String {
    // Remove potentially sensitive information
    let sensitive_patterns = [
        r"API_KEY_\w+",
        r"Bearer \w+",
        r"token_\w+",
        r"\b\w{32,}\b", // Long alphanumeric strings that might be keys
    ];

    let mut sanitized = message.to_string();

    for pattern in &sensitive_patterns {
        if let Ok(regex) = regex::Regex::new(pattern) {
            sanitized = regex.replace_all(&sanitized, "[REDACTED]").to_string();
        }
    }

    // Limit message length
    if sanitized.len() > 200 {
        sanitized = format!("{}...", &sanitized[..197]);
    }

    sanitized
}

pub fn create_gemini_error_response(status: u16, message: &str) -> Value {
    let error_type = match status {
        400 => "invalid_request_error",
        401 => "authentication_error",
        403 => "permission_error",
        404 => "not_found_error",
        429 => "rate_limit_error",
        500 => "api_error",
        502 => "api_connection_error",
        503 => "api_error",
        _ => "api_error",
    };

    serde_json::json!({
        "error": {
            "message": translate_error(message),
            "type": error_type,
            "code": status.to_string(),
        }
    })
}

pub fn log_error_details(error: &str, context: &str, extra_info: Option<&str>) {
    if let Some(extra) = extra_info {
        error!("Error in {}: {} | Additional info: {}", context, error, extra);
    } else {
        error!("Error in {}: {}", context, error);
    }
}

pub fn is_retryable_error(error_message: &str) -> bool {
    let retryable_patterns = [
        "timeout",
        "connection",
        "network",
        "temporary",
        "try again",
        "rate limit",
        "503",
        "502",
        "500",
    ];

    let error_lower = error_message.to_lowercase();
    retryable_patterns.iter().any(|pattern| error_lower.contains(pattern))
}

pub fn extract_error_code(error_message: &str) -> Option<String> {
    // Try to extract HTTP status codes or error codes from error messages
    let patterns = [
        r"status:?\s*(\d{3})",
        r"code:?\s*(\d{3})",
        r"error\s*(\d{3})",
        r"HTTP\s*(\d{3})",
    ];

    for pattern in &patterns {
        if let Ok(regex) = regex::Regex::new(pattern) {
            if let Some(captures) = regex.captures(error_message) {
                if let Some(code) = captures.get(1) {
                    return Some(code.as_str().to_string());
                }
            }
        }
    }

    None
}

#[derive(Debug, Clone)]
pub struct ErrorContext {
    pub operation: String,
    pub model: Option<String>,
    pub api_key_prefix: Option<String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub request_id: Option<String>,
}

impl ErrorContext {
    pub fn new(operation: &str) -> Self {
        Self {
            operation: operation.to_string(),
            model: None,
            api_key_prefix: None,
            timestamp: chrono::Utc::now(),
            request_id: None,
        }
    }

    pub fn with_model(mut self, model: &str) -> Self {
        self.model = Some(model.to_string());
        self
    }

    pub fn with_api_key(mut self, api_key: &str) -> Self {
        self.api_key_prefix = Some(format!("{}...", &api_key[..8.min(api_key.len())]));
        self
    }

    pub fn with_request_id(mut self, request_id: &str) -> Self {
        self.request_id = Some(request_id.to_string());
        self
    }

    pub fn log_error(&self, error: &str) {
        error!(
            operation = %self.operation,
            model = ?self.model,
            api_key_prefix = ?self.api_key_prefix,
            request_id = ?self.request_id,
            timestamp = %self.timestamp,
            "Operation failed: {}",
            error
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_translate_error() {
        assert_eq!(
            translate_error("Invalid API key provided"),
            "API key is invalid or expired"
        );

        assert_eq!(
            translate_error("Rate limit exceeded"),
            "Rate limit exceeded, please try again later"
        );

        assert_eq!(
            translate_error("Some unknown error"),
            "Some unknown error"
        );
    }

    #[test]
    fn test_sanitize_error_message() {
        let message = "Error with API_KEY_abc123def456 token";
        let sanitized = sanitize_error_message(message);
        assert!(sanitized.contains("[REDACTED]"));
        assert!(!sanitized.contains("API_KEY_abc123def456"));
    }

    #[test]
    fn test_is_retryable_error() {
        assert!(is_retryable_error("Connection timeout occurred"));
        assert!(is_retryable_error("Rate limit exceeded"));
        assert!(is_retryable_error("HTTP 503 Service Unavailable"));
        assert!(!is_retryable_error("Invalid API key"));
        assert!(!is_retryable_error("Content blocked"));
    }

    #[test]
    fn test_extract_error_code() {
        assert_eq!(extract_error_code("HTTP 404 Not Found"), Some("404".to_string()));
        assert_eq!(extract_error_code("Status: 429"), Some("429".to_string()));
        assert_eq!(extract_error_code("Error code 500"), Some("500".to_string()));
        assert_eq!(extract_error_code("No code here"), None);
    }
}