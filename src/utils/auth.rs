use anyhow::Result;
use axum::{
    extract::{Request, Query},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::Response,
};
use serde::Deserialize;
use std::sync::Arc;
use tracing::{debug, warn};

use crate::config::Settings;

#[derive(Debug, Clone)]
pub struct AuthState {
    settings: Arc<Settings>,
}

impl AuthState {
    pub fn new(settings: Arc<Settings>) -> Self {
        Self { settings }
    }
}

#[derive(Debug, Deserialize)]
pub struct AuthQuery {
    key: Option<String>,
    password: Option<String>,
}

pub async fn auth_middleware(
    headers: HeaderMap,
    Query(query): Query<AuthQuery>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let auth_state = request
        .extensions()
        .get::<Arc<AuthState>>()
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    // Extract authentication from various sources
    let auth_token = extract_auth_token(&headers, &query);

    if let Some(token) = auth_token {
        if validate_auth_token(&token, &auth_state.settings) {
            debug!("Authentication successful");
            Ok(next.run(request).await)
        } else {
            warn!("Authentication failed: invalid token");
            Err(StatusCode::UNAUTHORIZED)
        }
    } else {
        warn!("Authentication failed: no token provided");
        Err(StatusCode::UNAUTHORIZED)
    }
}

fn extract_auth_token(headers: &HeaderMap, query: &AuthQuery) -> Option<String> {
    // 1. Check Authorization header (Bearer token)
    if let Some(auth_header) = headers.get("authorization") {
        if let Ok(auth_str) = auth_header.to_str() {
            if let Some(token) = auth_str.strip_prefix("Bearer ") {
                return Some(token.to_string());
            }
        }
    }

    // 2. Check x-goog-api-key header (Gemini style)
    if let Some(api_key_header) = headers.get("x-goog-api-key") {
        if let Ok(api_key) = api_key_header.to_str() {
            return Some(api_key.to_string());
        }
    }

    // 3. Check custom API key header
    if let Some(api_key_header) = headers.get("x-api-key") {
        if let Ok(api_key) = api_key_header.to_str() {
            return Some(api_key.to_string());
        }
    }

    // 4. Check query parameters
    if let Some(key) = &query.key {
        return Some(key.clone());
    }

    if let Some(password) = &query.password {
        return Some(password.clone());
    }

    None
}

fn validate_auth_token(token: &str, settings: &Settings) -> bool {
    // Check against configured password
    if token == settings.password || token == settings.web_password {
        return true;
    }

    // Check against valid API keys
    if settings.get_valid_api_keys().contains(&token.to_string()) {
        return true;
    }

    // Check against whitelist user agents if configured
    if !settings.whitelist_user_agent.is_empty() {
        // This would need access to the User-Agent header
        // For now, we'll skip this check in this context
    }

    false
}

pub async fn web_auth_middleware(
    headers: HeaderMap,
    Query(query): Query<AuthQuery>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let auth_state = request
        .extensions()
        .get::<Arc<AuthState>>()
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    let auth_token = extract_auth_token(&headers, &query);

    if let Some(token) = auth_token {
        if token == auth_state.settings.web_password {
            debug!("Web authentication successful");
            Ok(next.run(request).await)
        } else {
            warn!("Web authentication failed: invalid password");
            Err(StatusCode::UNAUTHORIZED)
        }
    } else {
        warn!("Web authentication failed: no password provided");
        Err(StatusCode::UNAUTHORIZED)
    }
}

pub fn validate_user_agent(user_agent: Option<&str>, settings: &Settings) -> bool {
    if settings.whitelist_user_agent.is_empty() {
        return true; // No whitelist configured, allow all
    }

    if let Some(ua) = user_agent {
        let ua_lower = ua.to_lowercase();
        return settings
            .whitelist_user_agent
            .iter()
            .any(|allowed| ua_lower.contains(&allowed.to_lowercase()));
    }

    false // No user agent provided, but whitelist is configured
}

pub fn is_public_mode(settings: &Settings) -> bool {
    settings.public_mode
}

#[derive(Debug)]
pub struct AuthResult {
    pub authenticated: bool,
    pub user_id: Option<String>,
    pub scope: AuthScope,
}

#[derive(Debug, Clone)]
pub enum AuthScope {
    Public,
    Authenticated,
    Admin,
}

pub fn authenticate_request(
    headers: &HeaderMap,
    query: &AuthQuery,
    settings: &Settings,
) -> AuthResult {
    if settings.public_mode {
        return AuthResult {
            authenticated: true,
            user_id: Some("public".to_string()),
            scope: AuthScope::Public,
        };
    }

    if let Some(token) = extract_auth_token(headers, query) {
        if validate_auth_token(&token, settings) {
            let scope = if token == settings.web_password {
                AuthScope::Admin
            } else {
                AuthScope::Authenticated
            };

            return AuthResult {
                authenticated: true,
                user_id: Some(format!("user_{}", &token[..8.min(token.len())])),
                scope,
            };
        }
    }

    AuthResult {
        authenticated: false,
        user_id: None,
        scope: AuthScope::Public,
    }
}

pub fn verify_web_password(password: &str, settings: &Settings) -> bool {
    password == settings.web_password
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn test_extract_auth_token() {
        let mut headers = HeaderMap::new();
        headers.insert("authorization", HeaderValue::from_static("Bearer test_token"));

        let query = AuthQuery {
            key: None,
            password: None,
        };

        let token = extract_auth_token(&headers, &query);
        assert_eq!(token, Some("test_token".to_string()));
    }

    #[test]
    fn test_validate_user_agent() {
        use std::collections::HashSet;

        let mut whitelist = HashSet::new();
        whitelist.insert("mozilla".to_string());
        whitelist.insert("curl".to_string());

        let settings = Settings {
            whitelist_user_agent: whitelist,
            ..Default::default()
        };

        assert!(validate_user_agent(Some("Mozilla/5.0"), &settings));
        assert!(validate_user_agent(Some("curl/7.68.0"), &settings));
        assert!(!validate_user_agent(Some("BadBot/1.0"), &settings));
        assert!(!validate_user_agent(None, &settings));
    }
}