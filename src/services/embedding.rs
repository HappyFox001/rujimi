use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::Duration;
use tracing::{debug, error};

use crate::config::Settings;
use crate::models::schemas::{EmbeddingRequest, EmbeddingResponse, EmbeddingData, EmbeddingUsage, EmbeddingInput};
use crate::utils::logging::log;

#[derive(Debug, Clone)]
pub struct EmbeddingClient {
    client: Client,
    settings: std::sync::Arc<Settings>,
}

#[derive(Debug, Serialize, Deserialize)]
struct GeminiEmbeddingRequest {
    model: String,
    content: GeminiContent,
    #[serde(skip_serializing_if = "Option::is_none")]
    task_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct GeminiContent {
    parts: Vec<GeminiPart>,
}

#[derive(Debug, Serialize, Deserialize)]
struct GeminiPart {
    text: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct GeminiBatchEmbeddingRequest {
    requests: Vec<GeminiEmbeddingRequest>,
}

#[derive(Debug, Serialize, Deserialize)]
struct GeminiEmbeddingResponse {
    embedding: GeminiEmbedding,
}

#[derive(Debug, Serialize, Deserialize)]
struct GeminiEmbedding {
    values: Vec<f64>,
}

#[derive(Debug, Serialize, Deserialize)]
struct GeminiBatchEmbeddingResponse {
    embeddings: Vec<GeminiEmbeddingResponse>,
}

impl EmbeddingClient {
    pub fn new(settings: std::sync::Arc<Settings>) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .expect("Failed to create HTTP client");

        Self { client, settings }
    }

    /// Generate embeddings for text input - equivalent to Python's generate_embeddings
    pub async fn generate_embeddings(
        &self,
        request: EmbeddingRequest,
    ) -> Result<EmbeddingResponse, Box<dyn std::error::Error>> {
        let default_key = String::new();
        let api_key = self.settings.gemini_api_keys.first().unwrap_or(&default_key);

        // Log the request
        log(
            "info",
            "开始生成文本嵌入",
            Some({
                let mut extra = std::collections::HashMap::new();
                extra.insert("model".to_string(), json!(request.model));
                extra.insert("input_count".to_string(), json!(self.get_input_count(&request.input)));
                extra
            }),
        );

        let embeddings = match &request.input {
            EmbeddingInput::String(text) => {
                vec![self.get_single_embedding(text, &request.model, api_key).await?]
            }
            EmbeddingInput::ArrayOfStrings(texts) => {
                self.get_batch_embeddings_from_strings(texts, &request.model, api_key).await?
            }
            EmbeddingInput::ArrayOfTokens(_) | EmbeddingInput::ArrayOfTokenArrays(_) => {
                return Err("Token-based input not supported yet".into());
            }
        };

        let total_tokens = embeddings.len() as i32; // Simplified token count

        let response = EmbeddingResponse {
            object: "list".to_string(),
            data: embeddings.into_iter().enumerate().map(|(index, embedding)| {
                EmbeddingData {
                    object: "embedding".to_string(),
                    embedding,
                    index: index as u32,
                }
            }).collect(),
            model: request.model.clone(),
            usage: EmbeddingUsage {
                prompt_tokens: total_tokens as u32,
                total_tokens: total_tokens as u32,
            },
        };

        log(
            "info",
            "文本嵌入生成完成",
            Some({
                let mut extra = std::collections::HashMap::new();
                extra.insert("embeddings_count".to_string(), json!(response.data.len()));
                extra.insert("total_tokens".to_string(), json!(response.usage.total_tokens));
                extra
            }),
        );

        Ok(response)
    }

    async fn get_single_embedding(
        &self,
        text: &str,
        model: &str,
        api_key: &str,
    ) -> Result<Vec<f64>, Box<dyn std::error::Error>> {
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:embedContent?key={}",
            model, api_key
        );

        let request_body = GeminiEmbeddingRequest {
            model: format!("models/{}", model),
            content: GeminiContent {
                parts: vec![GeminiPart {
                    text: text.to_string(),
                }],
            },
            task_type: Some("RETRIEVAL_DOCUMENT".to_string()),
            title: None,
        };

        debug!("发送单个嵌入请求到: {}", url);

        let response = self
            .client
            .post(&url)
            .json(&request_body)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            error!("嵌入API请求失败: {}", error_text);
            return Err(format!("Embedding API error: {}", error_text).into());
        }

        let embedding_response: GeminiEmbeddingResponse = response.json().await?;
        Ok(embedding_response.embedding.values)
    }

    async fn get_batch_embeddings_from_strings(
        &self,
        texts: &[String],
        model: &str,
        api_key: &str,
    ) -> Result<Vec<Vec<f64>>, Box<dyn std::error::Error>> {
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:batchEmbedContents?key={}",
            model, api_key
        );

        let requests: Vec<GeminiEmbeddingRequest> = texts
            .iter()
            .map(|text| {
                GeminiEmbeddingRequest {
                    model: format!("models/{}", model),
                    content: GeminiContent {
                        parts: vec![GeminiPart { text: text.clone() }],
                    },
                    task_type: Some("RETRIEVAL_DOCUMENT".to_string()),
                    title: None,
                }
            })
            .collect();

        let batch_request = GeminiBatchEmbeddingRequest {
            requests,
        };

        debug!("发送批量嵌入请求到: {}", url);

        let response = self
            .client
            .post(&url)
            .json(&batch_request)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            error!("批量嵌入API请求失败: {}", error_text);
            return Err(format!("Batch embedding API error: {}", error_text).into());
        }

        let batch_response: GeminiBatchEmbeddingResponse = response.json().await?;
        Ok(batch_response
            .embeddings
            .into_iter()
            .map(|emb| emb.embedding.values)
            .collect())
    }

    fn get_input_count(&self, input: &EmbeddingInput) -> usize {
        match input {
            EmbeddingInput::String(_) => 1,
            EmbeddingInput::ArrayOfStrings(arr) => arr.len(),
            EmbeddingInput::ArrayOfTokens(arr) => arr.len(),
            EmbeddingInput::ArrayOfTokenArrays(arr) => arr.len(),
        }
    }

    /// Get supported embedding models
    pub fn get_supported_models() -> Vec<&'static str> {
        vec![
            "text-embedding-004",
            "text-multilingual-embedding-002",
            "embedding-001",
        ]
    }

    /// Validate if model is supported
    pub fn is_model_supported(model: &str) -> bool {
        Self::get_supported_models().contains(&model)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Settings;

    #[test]
    fn test_embedding_client_creation() {
        let settings = std::sync::Arc::new(Settings::default());
        let client = EmbeddingClient::new(settings);
        assert!(!client.settings.gemini_api_keys.is_empty());
    }

    #[test]
    fn test_model_validation() {
        assert!(EmbeddingClient::is_model_supported("text-embedding-004"));
        assert!(!EmbeddingClient::is_model_supported("invalid-model"));
    }

    #[test]
    fn test_input_count() {
        let settings = std::sync::Arc::new(Settings::default());
        let client = EmbeddingClient::new(settings);

        // Test string input - 对应Python中单个字符串输入
        let string_input = EmbeddingInput::String("test text".to_string());
        assert_eq!(client.get_input_count(&string_input), 1);

        // Test array input - 对应Python中字符串数组输入
        let array_input = EmbeddingInput::ArrayOfStrings(vec![
            "text1".to_string(),
            "text2".to_string(),
            "text3".to_string()
        ]);
        assert_eq!(client.get_input_count(&array_input), 3);

        // Test token array input - 对应Python中token数组输入
        let token_input = EmbeddingInput::ArrayOfTokens(vec![1, 2, 3, 4, 5]);
        assert_eq!(client.get_input_count(&token_input), 5);

        // Test nested token arrays - 对应Python中嵌套token数组输入
        let nested_token_input = EmbeddingInput::ArrayOfTokenArrays(vec![
            vec![1, 2],
            vec![3, 4, 5],
            vec![6]
        ]);
        assert_eq!(client.get_input_count(&nested_token_input), 3);
    }

    #[test]
    fn test_gemini_request_serialization() {
        let request = GeminiEmbeddingRequest {
            model: "models/text-embedding-004".to_string(),
            content: GeminiContent {
                parts: vec![GeminiPart {
                    text: "Test text".to_string(),
                }],
            },
            task_type: Some("RETRIEVAL_DOCUMENT".to_string()),
            title: None,
        };

        let json_str = serde_json::to_string(&request).unwrap();
        assert!(json_str.contains("Test text"));
        assert!(json_str.contains("RETRIEVAL_DOCUMENT"));
    }
}