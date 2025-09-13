use std::collections::HashMap;
use serde_json::{Value, json};
use base64::{Engine, engine::general_purpose};
use regex::Regex;
use url::Url;
use crate::vertex::models::{OpenAIMessage, MessageContent, ContentPart, ImageUrl};
use anyhow::{Result, anyhow};

// Rust equivalent of Python vertex/message_processing.py

// Define supported roles for Gemini API
const SUPPORTED_ROLES: &[&str] = &["user", "model"];

/// Convert OpenAI messages to Gemini format
pub fn create_gemini_prompt(messages: &[OpenAIMessage]) -> Result<Vec<Value>> {
    log::debug!("Converting OpenAI messages to Gemini format...");

    let mut gemini_messages = Vec::new();

    for (idx, message) in messages.iter().enumerate() {
        let content = match &message.content {
            MessageContent::Text(text) => {
                if text.trim().is_empty() {
                    log::warn!("Skipping message {} due to empty content (Role: {})", idx, message.role);
                    continue;
                }
                text.clone()
            }
            MessageContent::Parts(parts) => {
                process_message_parts(parts)?
            }
        };

        let mut role = message.role.clone();
        if role == "system" {
            role = "user".to_string();
        } else if role == "assistant" {
            role = "model".to_string();
        }

        if !SUPPORTED_ROLES.contains(&role.as_str()) {
            if role == "tool" {
                role = "user".to_string();
            } else if idx == messages.len() - 1 {
                role = "user".to_string();
            } else {
                log::warn!("Unsupported role '{}', converting to 'user'", role);
                role = "user".to_string();
            }
        }

        let gemini_message = json!({
            "role": role,
            "parts": [{
                "text": content
            }]
        });

        gemini_messages.push(gemini_message);
        log::debug!("Processed message {}: role={}, content_length={}", idx, role, content.len());
    }

    log::debug!("Converted {} messages to Gemini format", gemini_messages.len());
    Ok(gemini_messages)
}

/// Process message parts (text and images)
fn process_message_parts(parts: &[ContentPart]) -> Result<String> {
    let mut text_parts = Vec::new();

    for part in parts {
        match part {
            ContentPart::Text { text } => {
                text_parts.push(text.clone());
            }
            ContentPart::Image { image_url } => {
                // Handle image content - for now, we'll add a placeholder
                // In a full implementation, you'd process the image data
                let image_info = format!("[Image: {}]", image_url.url);
                text_parts.push(image_info);
                log::debug!("Processed image URL: {}", image_url.url);
            }
        }
    }

    Ok(text_parts.join("\n"))
}

/// Deobfuscate text by removing common obfuscation patterns
pub fn deobfuscate_text(text: &str) -> String {
    log::debug!("Deobfuscating text of length {}", text.len());

    let mut result = text.to_string();

    // Remove zero-width characters and similar Unicode obfuscation
    let zero_width_chars = ['\u{200B}', '\u{200C}', '\u{200D}', '\u{FEFF}'];
    for &ch in &zero_width_chars {
        result = result.replace(ch, "");
    }

    // Remove excessive whitespace and normalize
    let whitespace_regex = Regex::new(r"\s+").unwrap();
    result = whitespace_regex.replace_all(&result, " ").to_string();

    // Remove markdown formatting artifacts that might be used for obfuscation
    let markdown_regex = Regex::new(r"\*{1,3}([^*]+)\*{1,3}").unwrap();
    result = markdown_regex.replace_all(&result, "$1").to_string();

    // Remove HTML-like tags
    let html_regex = Regex::new(r"<[^>]*>").unwrap();
    result = html_regex.replace_all(&result, "").to_string();

    result = result.trim().to_string();

    if result != text {
        log::debug!("Text deobfuscated: {} -> {}", text.len(), result.len());
    }

    result
}

/// Convert Gemini response to OpenAI format
pub fn convert_to_openai_format(
    response: &Value,
    model: &str,
    usage_info: Option<&Value>,
) -> Result<Value> {
    log::debug!("Converting Gemini response to OpenAI format");

    // Extract content from Gemini response
    let content = extract_gemini_content(response)?;
    let deobfuscated_content = deobfuscate_text(&content);

    // Create OpenAI format response
    let mut openai_response = json!({
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
                "content": deobfuscated_content
            },
            "finish_reason": "stop"
        }]
    });

    // Add usage information if provided
    if let Some(usage) = usage_info {
        openai_response["usage"] = usage.clone();
    } else {
        // Provide default usage info
        openai_response["usage"] = json!({
            "prompt_tokens": estimate_tokens(&content),
            "completion_tokens": estimate_tokens(&deobfuscated_content),
            "total_tokens": estimate_tokens(&content) + estimate_tokens(&deobfuscated_content)
        });
    }

    Ok(openai_response)
}

/// Convert Gemini streaming chunk to OpenAI format
pub fn convert_chunk_to_openai(
    chunk: &str,
    model: &str,
    is_final: bool,
) -> Result<String> {
    let deobfuscated_chunk = deobfuscate_text(chunk);

    let finish_reason = if is_final { "stop" } else { null };

    let openai_chunk = json!({
        "id": format!("chatcmpl-{}", uuid::Uuid::new_v4()),
        "object": "chat.completion.chunk",
        "created": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        "model": model,
        "choices": [{
            "index": 0,
            "delta": if !deobfuscated_chunk.is_empty() {
                json!({ "content": deobfuscated_chunk })
            } else {
                json!({})
            },
            "finish_reason": finish_reason
        }]
    });

    Ok(format!("data: {}\n\n", openai_chunk))
}

/// Create final chunk for streaming response
pub fn create_final_chunk(model: &str) -> String {
    let final_chunk = json!({
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
    });

    format!("data: {}\n\ndata: [DONE]\n\n", final_chunk)
}

/// Extract content from Gemini response
fn extract_gemini_content(response: &Value) -> Result<String> {
    // Try to extract from different possible response structures
    if let Some(candidates) = response.get("candidates") {
        if let Some(candidate) = candidates.get(0) {
            if let Some(content) = candidate.get("content") {
                if let Some(parts) = content.get("parts") {
                    if let Some(part) = parts.get(0) {
                        if let Some(text) = part.get("text") {
                            return Ok(text.as_str().unwrap_or("").to_string());
                        }
                    }
                }
            }
        }
    }

    // Alternative extraction paths
    if let Some(text) = response.get("text") {
        return Ok(text.as_str().unwrap_or("").to_string());
    }

    if let Some(content) = response.get("content") {
        return Ok(content.as_str().unwrap_or("").to_string());
    }

    Err(anyhow!("Could not extract content from Gemini response"))
}

/// Parse Gemini response for reasoning and content
pub fn parse_gemini_response_for_reasoning_and_content(response: &Value) -> Result<(String, String)> {
    let content = extract_gemini_content(response)?;

    // Try to separate reasoning from final answer
    // This is a simplified implementation - could be made more sophisticated
    if let Some(thinking_end) = content.find("</thinking>") {
        let reasoning = content[..thinking_end].replace("<thinking>", "").trim().to_string();
        let final_content = content[thinking_end + 11..].trim().to_string();
        Ok((reasoning, final_content))
    } else {
        // If no clear separation, return empty reasoning and full content
        Ok((String::new(), content))
    }
}

/// Estimate token count (rough approximation)
fn estimate_tokens(text: &str) -> i32 {
    // Rough estimation: ~4 characters per token for most languages
    ((text.len() as f64) / 4.0).ceil() as i32
}

/// Validate image URL format
pub fn validate_image_url(url: &str) -> Result<()> {
    // Check if it's a data URL
    if url.starts_with("data:image/") {
        return Ok(());
    }

    // Check if it's a valid HTTP/HTTPS URL
    let parsed_url = Url::parse(url)
        .map_err(|_| anyhow!("Invalid URL format"))?;

    if parsed_url.scheme() != "http" && parsed_url.scheme() != "https" {
        return Err(anyhow!("Only HTTP/HTTPS URLs are supported"));
    }

    Ok(())
}

/// Extract base64 image data from data URL
pub fn extract_base64_image_data(data_url: &str) -> Result<(String, Vec<u8>)> {
    if !data_url.starts_with("data:image/") {
        return Err(anyhow!("Not a valid image data URL"));
    }

    let parts: Vec<&str> = data_url.splitn(2, ',').collect();
    if parts.len() != 2 {
        return Err(anyhow!("Invalid data URL format"));
    }

    // Extract mime type
    let header_parts: Vec<&str> = parts[0].split(';').collect();
    let mime_type = header_parts[0].strip_prefix("data:").unwrap_or("");

    // Decode base64 data
    let image_data = general_purpose::STANDARD
        .decode(parts[1])
        .map_err(|_| anyhow!("Failed to decode base64 image data"))?;

    Ok((mime_type.to_string(), image_data))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deobfuscate_text() {
        let obfuscated = "This\u{200B}is\u{200C}test\u{FEFF}text";
        let result = deobfuscate_text(obfuscated);
        assert_eq!(result, "This is test text");
    }

    #[test]
    fn test_estimate_tokens() {
        assert_eq!(estimate_tokens("hello world"), 3);
        assert_eq!(estimate_tokens("a"), 1);
        assert_eq!(estimate_tokens(""), 0);
    }

    #[test]
    fn test_validate_image_url() {
        assert!(validate_image_url("https://example.com/image.jpg").is_ok());
        assert!(validate_image_url("data:image/png;base64,iVBORw0KGgo=").is_ok());
        assert!(validate_image_url("ftp://example.com/image.jpg").is_err());
    }
}