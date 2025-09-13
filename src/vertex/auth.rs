use anyhow::{Result, anyhow};
use axum::{
    extract::Request,
    http::{HeaderMap, HeaderValue, StatusCode},
    response::Response,
    middleware::Next,
};
use crate::config::Settings;
use std::sync::Arc;

// Rust equivalent of Python vertex/auth.py

/// Validate API key - currently returns true for all keys
pub fn validate_api_key(_api_key: &str) -> bool {
    // TODO: Implement actual API key validation logic
    true
}

/// Extract API key from Authorization header
pub fn extract_api_key(headers: &HeaderMap) -> Result<String> {
    let auth_header = headers.get("authorization")
        .ok_or_else(|| anyhow!("Missing API key. Please include 'Authorization: Bearer YOUR_API_KEY' header."))?;

    let auth_str = auth_header.to_str()
        .map_err(|_| anyhow!("Invalid API key format. Use 'Authorization: Bearer YOUR_API_KEY'"))?;

    if !auth_str.starts_with("Bearer ") {
        return Err(anyhow!("Invalid API key format. Use 'Authorization: Bearer YOUR_API_KEY'"));
    }

    let api_key = auth_str.strip_prefix("Bearer ").unwrap().to_string();

    if !validate_api_key(&api_key) {
        return Err(anyhow!("Invalid API key"));
    }

    Ok(api_key)
}

/// Middleware for API key validation
pub async fn api_key_middleware(
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    match extract_api_key(&headers) {
        Ok(_api_key) => {
            // API key is valid, proceed with the request
            Ok(next.run(request).await)
        }
        Err(_) => {
            // API key is invalid or missing
            Err(StatusCode::UNAUTHORIZED)
        }
    }
}

/// Validate settings for Vertex API access - Rust equivalent of validate_settings()
pub fn validate_vertex_settings(settings: &Settings) -> Result<()> {
    log::info!("Validating Vertex AI settings");

    // Check API key
    let api_key = if !settings.password.is_empty() {
        &settings.password
    } else {
        return Err(anyhow!("API key is not set. Some functionality may be limited."));
    };

    if api_key.is_empty() {
        log::warn!("API key is not set. Some functionality may be limited.");
    }

    // Check Google credentials JSON
    if let Some(ref google_creds) = settings.google_credentials_json {
        if !google_creds.is_empty() {
            // Try to parse JSON to ensure it's valid
            serde_json::from_str::<serde_json::Value>(google_creds)
                .map_err(|_| anyhow!("Google Credentials JSON is not valid JSON. Please check the format."))?;
            log::info!("Google Credentials JSON is valid");
        }
    }

    // Check project ID
    if let Some(ref project_id) = settings.vertex_project_id {
        if project_id.is_empty() {
            log::warn!("Vertex AI Project ID is not set. Required for non-API key methods.");
        }
    } else {
        log::warn!("Vertex AI Project ID is not set. Required for non-API key methods.");
    }

    // Check location
    let location = settings.vertex_location.as_deref().unwrap_or("us-central1");
    if location == "us-central1" {
        log::warn!("Vertex AI Location is not set, using default: us-central1");
    }

    // Verify credentials directory
    if let Some(ref creds_dir) = settings.credentials_dir {
        if !std::path::Path::new(creds_dir).exists() {
            std::fs::create_dir_all(creds_dir)
                .map_err(|e| anyhow!("Failed to create credentials directory: {}", e))?;
            log::info!("Created credentials directory at: {}", creds_dir);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_api_key() {
        assert!(validate_api_key("test_key"));
        assert!(validate_api_key(""));
    }

    #[test]
    fn test_extract_api_key() {
        let mut headers = HeaderMap::new();
        headers.insert("authorization", HeaderValue::from_static("Bearer test_key"));

        let result = extract_api_key(&headers);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "test_key");
    }

    #[test]
    fn test_extract_api_key_missing_header() {
        let headers = HeaderMap::new();
        let result = extract_api_key(&headers);
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_api_key_invalid_format() {
        let mut headers = HeaderMap::new();
        headers.insert("authorization", HeaderValue::from_static("test_key"));

        let result = extract_api_key(&headers);
        assert!(result.is_err());
    }
}