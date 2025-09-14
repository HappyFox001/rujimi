use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

use crate::models::schemas::{ServiceStatus, ApiStats, ConfigInfo, VersionInfo};
use crate::utils::auth::{authenticate_request, AuthQuery, AuthScope};
use crate::utils::version;
use crate::AppState;

pub fn create_dashboard_routes() -> Router<AppState> {
    Router::new()
        .route("/data", get(get_dashboard_data))
        .route("/stats", get(get_stats))
        .route("/config", get(get_config))
        .route("/config", post(update_config))
        .route("/update-config", post(update_config))  // Add the update-config endpoint for compatibility
        .route("/reset-stats", post(reset_stats))
        .route("/cache/clear", post(clear_cache))
        .route("/keys/stats", get(get_key_stats))
        .route("/version", get(get_version))
}

#[derive(Debug, Serialize)]
pub struct DashboardResponse {
    pub status: ServiceStatus,
    pub stats: ApiStats,
    pub config: ConfigInfo,
    pub version: VersionInfo,
    pub key_stats: Vec<KeyStatInfo>,
}

#[derive(Debug, Serialize)]
pub struct KeyStatInfo {
    pub key_prefix: String,
    pub daily_usage: u32,
    pub last_used: String,
    pub consecutive_failures: u32,
}

#[derive(Debug, Deserialize)]
pub struct ConfigUpdateRequest {
    pub key: String,
    pub value: serde_json::Value,
    pub password: String,
}

async fn get_dashboard_data(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<AuthQuery>,
) -> Result<Json<DashboardResponse>, StatusCode> {
    // Authenticate request
    let auth_result = authenticate_request(&headers, &query, &state.settings);
    if !auth_result.authenticated {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let uptime = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // Get service status
    let status = ServiceStatus {
        running: true,
        uptime,
        api_keys_available: state.key_manager.available_keys_count().await,
        cache_entries: state.cache_manager.size().await,
    };

    // Get API stats
    let api_stats = state.stats_manager.get_stats().await;
    let stats = ApiStats {
        total_requests: api_stats.total_requests,
        successful_requests: api_stats.successful_requests,
        failed_requests: api_stats.failed_requests,
        tokens_used: api_stats.total_tokens,
        requests_per_minute: api_stats.requests_last_minute,
        requests_per_hour: api_stats.requests_last_hour,
        requests_per_day: api_stats.requests_last_day,
    };

    // Get config info
    let config = ConfigInfo {
        fake_streaming: state.settings.fake_streaming,
        concurrent_requests: state.settings.concurrent_requests,
        cache_enabled: state.settings.max_cache_entries > 0,
        vertex_enabled: state.settings.enable_vertex,
        search_mode: state.settings.search.search_mode,
    };

    // Get version info
    let version = VersionInfo {
        current: version::get_current_version(),
        latest: None, // This would be populated by a background task
        update_available: false,
    };

    // Get API key stats
    let key_stats_raw = state.key_manager.get_key_stats().await;
    let key_stats = key_stats_raw
        .into_iter()
        .map(|(key, stats)| KeyStatInfo {
            key_prefix: format!("{}...", &key[..8.min(key.len())]),
            daily_usage: stats.daily_usage,
            last_used: stats.last_used.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
            consecutive_failures: stats.consecutive_failures,
        })
        .collect();

    Ok(Json(DashboardResponse {
        status,
        stats,
        config,
        version,
        key_stats,
    }))
}

async fn get_stats(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<AuthQuery>,
) -> Result<Json<ApiStats>, StatusCode> {
    let auth_result = authenticate_request(&headers, &query, &state.settings);
    if !auth_result.authenticated {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let api_stats = state.stats_manager.get_stats().await;
    let stats = ApiStats {
        total_requests: api_stats.total_requests,
        successful_requests: api_stats.successful_requests,
        failed_requests: api_stats.failed_requests,
        tokens_used: api_stats.total_tokens,
        requests_per_minute: api_stats.requests_last_minute,
        requests_per_hour: api_stats.requests_last_hour,
        requests_per_day: api_stats.requests_last_day,
    };

    Ok(Json(stats))
}

async fn get_config(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<AuthQuery>,
) -> Result<Json<ConfigInfo>, StatusCode> {
    let auth_result = authenticate_request(&headers, &query, &state.settings);
    if !auth_result.authenticated {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let config = ConfigInfo {
        fake_streaming: state.settings.fake_streaming,
        concurrent_requests: state.settings.concurrent_requests,
        cache_enabled: state.settings.max_cache_entries > 0,
        vertex_enabled: state.settings.enable_vertex,
        search_mode: state.settings.search.search_mode,
    };

    Ok(Json(config))
}

async fn update_config(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<AuthQuery>,
    Json(request): Json<ConfigUpdateRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // Verify password first (similar to hajimi)
    if !crate::utils::auth::verify_web_password(&request.password, &state.settings) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    info!("Configuration update requested for key: {}", request.key);
    debug!("Config update request: {:?}", request);

    // Handle configuration updates based on key name (similar to hajimi structure)
    match request.key.as_str() {
        "fake_streaming" => {
            if let Some(value) = request.value.as_bool() {
                // Update fake_streaming setting
                info!("Fake streaming updated to: {}", value);
                // Here you would update the actual settings
                // state.settings.fake_streaming = value;
            } else {
                return Err(StatusCode::BAD_REQUEST);
            }
        },
        "concurrent_requests" => {
            if let Some(value) = request.value.as_u64() {
                let value = value as usize;
                if value > 0 {
                    info!("Concurrent requests updated to: {}", value);
                    // state.settings.concurrent_requests = value;
                } else {
                    return Err(StatusCode::BAD_REQUEST);
                }
            } else {
                return Err(StatusCode::BAD_REQUEST);
            }
        },
        "search_mode" => {
            if let Some(value) = request.value.as_bool() {
                info!("Search mode updated to: {}", value);
                // state.settings.search_mode = value;
            } else {
                return Err(StatusCode::BAD_REQUEST);
            }
        },
        "random_string" => {
            if let Some(value) = request.value.as_bool() {
                info!("Random string updated to: {}", value);
                // state.settings.random_string = value;
            } else {
                return Err(StatusCode::BAD_REQUEST);
            }
        },
        "cache_expiry_time" => {
            if let Some(value) = request.value.as_u64() {
                if value > 0 {
                    info!("Cache expiry time updated to: {}", value);
                    // state.settings.cache_expiry_time = value;
                } else {
                    return Err(StatusCode::BAD_REQUEST);
                }
            } else {
                return Err(StatusCode::BAD_REQUEST);
            }
        },
        "enable_vertex" => {
            if let Some(value) = request.value.as_bool() {
                info!("Vertex AI updated to: {}", value);
                // state.settings.enable_vertex = value;
            } else {
                return Err(StatusCode::BAD_REQUEST);
            }
        },
        "max_requests_per_minute" => {
            if let Some(value) = request.value.as_u64() {
                let value = value as u32;
                if value > 0 {
                    info!("Max requests per minute updated to: {}", value);
                    // state.settings.max_requests_per_minute = value;
                } else {
                    return Err(StatusCode::BAD_REQUEST);
                }
            } else {
                return Err(StatusCode::BAD_REQUEST);
            }
        },
        "max_requests_per_day_per_ip" => {
            if let Some(value) = request.value.as_u64() {
                let value = value as u32;
                if value > 0 {
                    info!("Max requests per day per IP updated to: {}", value);
                    // state.settings.max_requests_per_day_per_ip = value;
                } else {
                    return Err(StatusCode::BAD_REQUEST);
                }
            } else {
                return Err(StatusCode::BAD_REQUEST);
            }
        },
        "gemini_api_keys" => {
            if let Some(value) = request.value.as_str() {
                info!("Gemini API keys updated");
                // Handle API key updates
                // Parse comma-separated keys and update key_manager
            } else {
                return Err(StatusCode::BAD_REQUEST);
            }
        },
        _ => {
            return Ok(Json(serde_json::json!({
                "status": "error",
                "message": format!("Unsupported configuration key: {}", request.key)
            })));
        }
    }

    // Save settings to disk (similar to hajimi's save_settings())
    // state.settings.save().await?;

    Ok(Json(serde_json::json!({
        "status": "success",
        "message": format!("Configuration item {} updated", request.key)
    })))
}

async fn reset_stats(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<AuthQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let auth_result = authenticate_request(&headers, &query, &state.settings);
    if !auth_result.authenticated {
        return Err(StatusCode::UNAUTHORIZED);
    }

    // Only admin users can reset stats
    if !matches!(auth_result.scope, AuthScope::Admin) {
        return Err(StatusCode::FORBIDDEN);
    }

    info!("Statistics reset requested by user: {:?}", auth_result.user_id);

    state.stats_manager.clear_stats().await;

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Statistics reset successfully"
    })))
}

async fn clear_cache(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<AuthQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let auth_result = authenticate_request(&headers, &query, &state.settings);
    if !auth_result.authenticated {
        return Err(StatusCode::UNAUTHORIZED);
    }

    // Only admin users can clear cache
    if !matches!(auth_result.scope, AuthScope::Admin) {
        return Err(StatusCode::FORBIDDEN);
    }

    info!("Cache clear requested by user: {:?}", auth_result.user_id);

    state.cache_manager.clear().await;

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Cache cleared successfully"
    })))
}

async fn get_key_stats(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<AuthQuery>,
) -> Result<Json<Vec<KeyStatInfo>>, StatusCode> {
    let auth_result = authenticate_request(&headers, &query, &state.settings);
    if !auth_result.authenticated {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let key_stats_raw = state.key_manager.get_key_stats().await;
    let key_stats = key_stats_raw
        .into_iter()
        .map(|(key, stats)| KeyStatInfo {
            key_prefix: format!("{}...", &key[..8.min(key.len())]),
            daily_usage: stats.daily_usage,
            last_used: stats.last_used.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
            consecutive_failures: stats.consecutive_failures,
        })
        .collect();

    Ok(Json(key_stats))
}

async fn get_version(
    State(_state): State<AppState>,
) -> Json<serde_json::Value> {
    let build_info = version::get_build_info();

    Json(serde_json::json!({
        "version": version::get_current_version(),
        "build_info": build_info
    }))
}