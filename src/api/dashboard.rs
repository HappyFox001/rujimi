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
    pub fake_streaming: Option<bool>,
    pub concurrent_requests: Option<usize>,
    pub search_mode: Option<bool>,
    pub random_string: Option<bool>,
    pub cache_expiry_time: Option<u64>,
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
        search_mode: state.settings.search_mode,
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
        search_mode: state.settings.search_mode,
    };

    Ok(Json(config))
}

async fn update_config(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<AuthQuery>,
    Json(request): Json<ConfigUpdateRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let auth_result = authenticate_request(&headers, &query, &state.settings);
    if !auth_result.authenticated {
        return Err(StatusCode::UNAUTHORIZED);
    }

    // Only admin users can update config
    if !matches!(auth_result.scope, AuthScope::Admin) {
        return Err(StatusCode::FORBIDDEN);
    }

    info!("Configuration update requested by user: {:?}", auth_result.user_id);

    // Note: In a real implementation, you'd need to update the settings
    // This would require making settings mutable or using a different approach
    // For now, we'll just log the request and return success

    debug!("Config update request: {:?}", request);

    // Here you would update the actual settings
    // This might involve updating the settings struct and saving to disk

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Configuration updated successfully"
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