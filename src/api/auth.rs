use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::Json,
    routing::post,
    Router,
};
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use crate::utils::auth::{authenticate_request, AuthQuery};
use crate::AppState;

pub fn create_auth_routes() -> Router<AppState> {
    Router::new()
        .route("/login", post(login))
        .route("/verify", post(verify_auth))
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub success: bool,
    pub message: String,
    pub token: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct VerifyResponse {
    pub valid: bool,
    pub user_id: Option<String>,
    pub scope: String,
}

async fn login(
    State(state): State<AppState>,
    Json(request): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, StatusCode> {
    debug!("Login attempt received");

    // Check if password matches
    if request.password == state.settings.web_password || request.password == state.settings.password {
        debug!("Login successful");

        // In a real implementation, you might generate a JWT token here
        // For simplicity, we'll just return the password as the token
        Ok(Json(LoginResponse {
            success: true,
            message: "Login successful".to_string(),
            token: Some(request.password),
        }))
    } else {
        warn!("Login failed: invalid password");

        Ok(Json(LoginResponse {
            success: false,
            message: "Invalid password".to_string(),
            token: None,
        }))
    }
}

async fn verify_auth(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<AuthQuery>,
) -> Json<VerifyResponse> {
    let auth_result = authenticate_request(&headers, &query, &state.settings);

    let scope = match auth_result.scope {
        crate::utils::auth::AuthScope::Public => "public",
        crate::utils::auth::AuthScope::Authenticated => "authenticated",
        crate::utils::auth::AuthScope::Admin => "admin",
    };

    Json(VerifyResponse {
        valid: auth_result.authenticated,
        user_id: auth_result.user_id,
        scope: scope.to_string(),
    })
}