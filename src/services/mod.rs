pub mod gemini;
pub mod embedding;
pub mod openai;
pub mod response_wrapper;

// Re-export main service structs and traits for easy access - equivalent to Python's __init__.py
pub use gemini::{GeminiClient, GeminiClientTrait};
pub use embedding::EmbeddingClient;
pub use openai::OpenAIClient;
pub use response_wrapper::{GeminiResponseWrapper, GeneratedText, wrap_gemini_response};