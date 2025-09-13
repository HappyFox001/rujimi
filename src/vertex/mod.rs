// Vertex AI module - Rust equivalent of Python vertex/
// This module contains Vertex AI specific functionality with complete feature parity

pub mod client;
pub mod models;
pub mod auth;
pub mod config;
pub mod credentials_manager;
pub mod api_helpers;
pub mod message_processing;
pub mod model_loader;
pub mod vertex_ai_init;
pub mod main;
pub mod routes;

// Re-export commonly used items
pub use client::VertexClient;
pub use models::{OpenAIRequest, OpenAIMessage, GeminiChatRequest, GeminiCompletionRequest};
pub use auth::{validate_api_key, extract_api_key, validate_vertex_settings};
pub use config::VertexConfig;
pub use credentials_manager::CredentialManager;
pub use vertex_ai_init::{init_vertex_ai, is_vertex_ai_available, get_vertex_ai_status};
pub use main::{create_vertex_router, init_vertex_app, vertex_health_check};