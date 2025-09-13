use anyhow::Result;
use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use std::{net::SocketAddr, sync::Arc};
use tokio::net::TcpListener;
use tower::ServiceBuilder;
use tower_http::{
    cors::CorsLayer,
    services::ServeDir,
    trace::TraceLayer,
    compression::CompressionLayer,
};
use tracing::{info, error};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod api;
mod config;
mod models;
mod services;
mod utils;

use config::Settings;
use utils::{
    api_key::ApiKeyManager,
    cache::ResponseCacheManager,
    stats::ApiStatsManager,
    auth::AuthState,
};
use services::gemini::GeminiClient;

#[derive(Clone)]
pub struct AppState {
    pub settings: Arc<Settings>,
    pub key_manager: Arc<ApiKeyManager>,
    pub cache_manager: Arc<ResponseCacheManager>,
    pub stats_manager: Arc<ApiStatsManager>,
    pub gemini_client: Arc<GeminiClient>,
    pub auth_state: Arc<AuthState>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "rujimi=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("🚀 Starting Rujimi - High-performance Gemini API Proxy");

    // Load configuration
    let settings = Arc::new(Settings::load()?);
    info!("✅ Configuration loaded successfully");

    // Initialize components
    let key_manager = Arc::new(ApiKeyManager::new(settings.clone()));
    let cache_manager = Arc::new(ResponseCacheManager::new(settings.clone()));
    let stats_manager = Arc::new(ApiStatsManager::new());
    let gemini_client = Arc::new(GeminiClient::new(settings.clone()));
    let auth_state = Arc::new(AuthState::new(settings.clone()));

    // Initialize API keys
    if let Err(e) = key_manager.initialize().await {
        error!("Failed to initialize API keys: {}", e);
        return Err(e);
    }

    // Start background tasks
    tokio::spawn(cache_manager.clone().start_cleanup_task());
    tokio::spawn(stats_manager.clone().start_cleanup_task());

    info!("🔑 API key manager initialized");
    info!("💾 Cache manager started");
    info!("📊 Stats manager started");

    // Create application state
    let app_state = AppState {
        settings: settings.clone(),
        key_manager,
        cache_manager,
        stats_manager,
        gemini_client,
        auth_state,
    };

    // Build our application with routes
    let app = build_app(app_state).await?;

    // Determine bind address
    let port = settings.port.unwrap_or(7860);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    info!("🌐 Server starting on http://0.0.0.0:{}", port);
    info!("📱 Dashboard available at http://127.0.0.1:{}", port);

    // Start the server
    let listener = TcpListener::bind(&addr).await?;
    info!("🎯 Listening on {}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}

async fn build_app(state: AppState) -> Result<Router> {
    // CORS configuration
    let cors = CorsLayer::new()
        .allow_origin(tower_http::cors::Any)
        .allow_methods(tower_http::cors::Any)
        .allow_headers(tower_http::cors::Any);

    // Build router
    let app = Router::new()
        // API routes
        .nest("/v1", api::routes::create_v1_routes())
        .nest("/api", api::routes::create_api_routes())
        .nest("/dashboard-api", api::dashboard::create_dashboard_routes())
        .nest("/api/auth", api::auth::create_auth_routes())

        // Static file serving for frontend
        .nest_service("/assets", ServeDir::new("assets"))

        // Root routes
        .route("/", get(serve_login_page))
        .route("/dashboard", get(serve_dashboard_page))

        // Health check
        .route("/health", get(health_check))

        // State
        .with_state(state)

        // Middleware
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(CompressionLayer::new())
                .layer(cors)
        );

    Ok(app)
}

async fn serve_login_page() -> impl IntoResponse {
    let html = include_str!("../assets/index.html");
    Html(html)
}

async fn serve_dashboard_page() -> impl IntoResponse {
    let html = include_str!("../assets/index.html");
    Html(html)
}

async fn health_check(State(state): State<AppState>) -> impl IntoResponse {
    let status = serde_json::json!({
        "status": "healthy",
        "version": "1.0.2",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "api_keys_available": state.key_manager.available_keys_count().await,
        "cache_entries": state.cache_manager.size().await,
    });

    (StatusCode::OK, axum::Json(status))
}
