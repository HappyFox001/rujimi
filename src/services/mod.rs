pub mod gemini;
pub mod embedding;
pub mod openai;
pub mod response_wrapper;

// Re-export main service structs and traits for easy access - equivalent to Python's __init__.py
pub use gemini::GeminiClient;

// Note: EmbeddingClient and OpenAIClient exist for API completeness but are not currently used
// in rujimi since GeminiClient handles all API requests. This differs from hajimi's architecture
// where separate clients are used for different services.
#[allow(dead_code)]
pub use embedding::EmbeddingClient;
#[allow(dead_code)]
pub use openai::OpenAIClient;

// Response wrappers are available for advanced response processing but not currently used
#[allow(dead_code)]
pub use response_wrapper::{GeminiResponseWrapper, GeneratedText, wrap_gemini_response};