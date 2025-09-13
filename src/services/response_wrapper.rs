use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
// Removed unused imports

use crate::models::schemas::{GeminiResponse, GeminiPart, Usage, ToolCall, FunctionCall, GeminiContent, GeminiCandidate, GeminiUsageMetadata};

/// Response wrapper for Gemini API responses - equivalent to Python's GeminiResponseWrapper
#[derive(Debug, Clone)]
pub struct GeminiResponseWrapper {
    pub response: GeminiResponse,
    pub is_thinking_model: bool,
}

/// Extracted text content from Gemini response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedText {
    pub text: String,
    pub thoughts: Option<String>,
    pub token_count: Option<i32>,
    pub finish_reason: Option<String>,
}

impl GeminiResponseWrapper {
    /// Create a new response wrapper
    pub fn new(response: GeminiResponse) -> Self {
        // Detect if this is a thinking model based on response structure
        let is_thinking_model = response.candidates
            .iter()
            .any(|candidate| {
                candidate.content.parts.iter().any(|part| {
                    if let GeminiPart::Text { text } = part {
                        text.contains("<thinking>") || text.contains("</thinking>")
                    } else {
                        false
                    }
                })
            });

        Self {
            response,
            is_thinking_model,
        }
    }

    /// Extract text content - equivalent to Python's get_text()
    pub fn get_text(&self) -> Option<String> {
        if let Some(candidate) = self.response.candidates.first() {
            let mut text_parts = Vec::new();

            for part in &candidate.content.parts {
                if let GeminiPart::Text { text } = part {
                    if self.is_thinking_model {
                        // Extract only the final answer, exclude thinking tags
                        if let Some(final_text) = self.extract_final_answer(text) {
                            text_parts.push(final_text);
                        }
                    } else {
                        text_parts.push(text.clone());
                    }
                }
            }

            if !text_parts.is_empty() {
                Some(text_parts.join(""))
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Extract thinking content for thinking models - equivalent to Python's get_thoughts()
    pub fn get_thoughts(&self) -> Option<String> {
        if !self.is_thinking_model {
            return None;
        }

        if let Some(candidate) = self.response.candidates.first() {
            for part in &candidate.content.parts {
                if let GeminiPart::Text { text } = part {
                    if let Some(thoughts) = self.extract_thinking_content(text) {
                        return Some(thoughts);
                    }
                }
            }
        }

        None
    }

    /// Get function calls - equivalent to Python's get_function_calls()
    pub fn get_function_calls(&self) -> Vec<ToolCall> {
        let mut tool_calls = Vec::new();

        if let Some(candidate) = self.response.candidates.first() {
            for part in &candidate.content.parts {
                if let GeminiPart::FunctionCall { function_call } = part {
                    tool_calls.push(ToolCall {
                        id: format!("call_{}", uuid::Uuid::new_v4()),
                        tool_type: "function".to_string(),
                        function: FunctionCall {
                            name: function_call.name.clone(),
                            arguments: serde_json::to_string(&function_call.args).unwrap_or_default(),
                        },
                    });
                }
            }
        }

        tool_calls
    }

    /// Get token count - equivalent to Python's get_token_count()
    pub fn get_token_count(&self) -> Option<Usage> {
        self.response.usage_metadata.as_ref().map(|meta| Usage {
            prompt_tokens: meta.prompt_token_count.unwrap_or(0),
            completion_tokens: meta.candidates_token_count.unwrap_or(0),
            total_tokens: meta.total_token_count.unwrap_or(0),
        })
    }

    /// Get finish reason - equivalent to Python's get_finish_reason()
    pub fn get_finish_reason(&self) -> Option<String> {
        self.response.candidates
            .first()
            .and_then(|candidate| candidate.finish_reason.clone())
    }

    /// Check if response is blocked by safety filters
    pub fn is_blocked(&self) -> bool {
        self.response.candidates
            .iter()
            .any(|candidate| {
                candidate.finish_reason.as_ref() == Some(&"SAFETY".to_string()) ||
                candidate.finish_reason.as_ref() == Some(&"BLOCKED_SAFETY".to_string())
            })
    }

    /// Get safety ratings
    pub fn get_safety_ratings(&self) -> Vec<Value> {
        self.response.candidates
            .first()
            .map(|candidate| {
                candidate.safety_ratings.clone().unwrap_or_default()
                    .into_iter()
                    .map(|rating| serde_json::to_value(rating).unwrap_or(Value::Null))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Extract generated text with metadata - equivalent to Python's extract_text()
    pub fn extract_text(&self) -> GeneratedText {
        let text = self.get_text().unwrap_or_default();
        let thoughts = self.get_thoughts();
        let token_count = self.get_token_count().map(|u| u.total_tokens as i32);
        let finish_reason = self.get_finish_reason();

        GeneratedText {
            text,
            thoughts,
            token_count,
            finish_reason,
        }
    }

    /// Check if response has content
    pub fn has_content(&self) -> bool {
        !self.response.candidates.is_empty() &&
        self.response.candidates
            .iter()
            .any(|candidate| !candidate.content.parts.is_empty())
    }

    /// Get all text parts (including thoughts for thinking models)
    pub fn get_all_text_parts(&self) -> Vec<String> {
        let mut text_parts = Vec::new();

        if let Some(candidate) = self.response.candidates.first() {
            for part in &candidate.content.parts {
                if let GeminiPart::Text { text } = part {
                    text_parts.push(text.clone());
                }
            }
        }

        text_parts
    }

    /// Extract final answer from thinking model response
    fn extract_final_answer(&self, text: &str) -> Option<String> {
        // Look for content after </thinking> tag
        if let Some(end_pos) = text.rfind("</thinking>") {
            let after_thinking = &text[end_pos + 11..]; // 11 = len("</thinking>")
            let final_answer = after_thinking.trim();
            if !final_answer.is_empty() {
                return Some(final_answer.to_string());
            }
        }

        // If no thinking tags found, return the whole text
        if !text.contains("<thinking>") && !text.contains("</thinking>") {
            return Some(text.to_string());
        }

        None
    }

    /// Extract thinking content from thinking model response
    fn extract_thinking_content(&self, text: &str) -> Option<String> {
        // Look for content between <thinking> and </thinking> tags
        if let (Some(start_pos), Some(end_pos)) = (text.find("<thinking>"), text.find("</thinking>")) {
            if start_pos < end_pos {
                let thinking_content = &text[start_pos + 10..end_pos]; // 10 = len("<thinking>")
                return Some(thinking_content.trim().to_string());
            }
        }

        None
    }

    /// Convert to JSON representation
    pub fn to_json(&self) -> Value {
        json!({
            "text": self.get_text(),
            "thoughts": self.get_thoughts(),
            "function_calls": self.get_function_calls(),
            "token_count": self.get_token_count(),
            "finish_reason": self.get_finish_reason(),
            "is_blocked": self.is_blocked(),
            "safety_ratings": self.get_safety_ratings(),
            "has_content": self.has_content(),
            "is_thinking_model": self.is_thinking_model
        })
    }

    /// Get response metadata
    pub fn get_metadata(&self) -> HashMap<String, Value> {
        let mut metadata = HashMap::new();

        metadata.insert("model_type".to_string(), json!(if self.is_thinking_model { "thinking" } else { "standard" }));
        metadata.insert("candidate_count".to_string(), json!(self.response.candidates.len()));
        metadata.insert("has_function_calls".to_string(), json!(!self.get_function_calls().is_empty()));
        metadata.insert("is_blocked".to_string(), json!(self.is_blocked()));
        metadata.insert("has_usage_metadata".to_string(), json!(self.response.usage_metadata.is_some()));

        if let Some(usage) = &self.response.usage_metadata {
            metadata.insert("prompt_tokens".to_string(), json!(usage.prompt_token_count.unwrap_or(0)));
            metadata.insert("completion_tokens".to_string(), json!(usage.candidates_token_count.unwrap_or(0)));
            metadata.insert("total_tokens".to_string(), json!(usage.total_token_count.unwrap_or(0)));
        }

        metadata
    }
}

impl std::fmt::Display for GeminiResponseWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(text) = self.get_text() {
            write!(f, "{}", text)
        } else {
            write!(f, "[No text content]")
        }
    }
}

/// Utility function to create wrapper from raw response JSON
pub fn wrap_gemini_response(response_json: Value) -> Result<GeminiResponseWrapper, serde_json::Error> {
    let response: GeminiResponse = serde_json::from_value(response_json)?;
    Ok(GeminiResponseWrapper::new(response))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::schemas::{GeminiCandidate, GeminiUsageMetadata};

    fn create_test_response(text: &str, is_thinking: bool) -> GeminiResponse {
        let text_content = if is_thinking {
            format!("<thinking>This is my thought process.</thinking>\n{}", text)
        } else {
            text.to_string()
        };

        GeminiResponse {
            candidates: vec![GeminiCandidate {
                content: GeminiContent {
                    role: "model".to_string(),
                    parts: vec![GeminiPart::Text { text: text_content }],
                },
                finish_reason: Some("STOP".to_string()),
                index: Some(0),
                safety_ratings: None,
            }],
            usage_metadata: Some(GeminiUsageMetadata {
                prompt_token_count: Some(10),
                candidates_token_count: Some(20),
                total_token_count: Some(30),
            }),
            prompt_feedback: None,
        }
    }

    #[test]
    fn test_standard_model_response() {
        let response = create_test_response("Hello, world!", false);
        let wrapper = GeminiResponseWrapper::new(response);

        assert!(!wrapper.is_thinking_model);
        assert_eq!(wrapper.get_text(), Some("Hello, world!".to_string()));
        assert_eq!(wrapper.get_thoughts(), None);
    }

    #[test]
    fn test_thinking_model_response() {
        let response = create_test_response("Hello, world!", true);
        let wrapper = GeminiResponseWrapper::new(response);

        assert!(wrapper.is_thinking_model);
        assert_eq!(wrapper.get_text(), Some("Hello, world!".to_string()));
        assert_eq!(wrapper.get_thoughts(), Some("This is my thought process.".to_string()));
    }

    #[test]
    fn test_token_count() {
        let response = create_test_response("Test", false);
        let wrapper = GeminiResponseWrapper::new(response);
        let usage = wrapper.get_token_count().unwrap();

        assert_eq!(usage.prompt_tokens, 10);
        assert_eq!(usage.completion_tokens, 20);
        assert_eq!(usage.total_tokens, 30);
    }

    #[test]
    fn test_finish_reason() {
        let response = create_test_response("Test", false);
        let wrapper = GeminiResponseWrapper::new(response);

        assert_eq!(wrapper.get_finish_reason(), Some("STOP".to_string()));
    }

    #[test]
    fn test_has_content() {
        let response = create_test_response("Test", false);
        let wrapper = GeminiResponseWrapper::new(response);

        assert!(wrapper.has_content());
    }

    #[test]
    fn test_extract_text() {
        let response = create_test_response("Test response", false);
        let wrapper = GeminiResponseWrapper::new(response);
        let generated_text = wrapper.extract_text();

        assert_eq!(generated_text.text, "Test response");
        assert_eq!(generated_text.token_count, Some(30));
        assert_eq!(generated_text.finish_reason, Some("STOP".to_string()));
    }

    #[test]
    fn test_to_json() {
        let response = create_test_response("Test", false);
        let wrapper = GeminiResponseWrapper::new(response);
        let json = wrapper.to_json();

        assert!(json.get("text").is_some());
        assert!(json.get("token_count").is_some());
        assert_eq!(json.get("is_thinking_model"), Some(&json!(false)));
    }
}