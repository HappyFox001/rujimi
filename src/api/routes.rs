use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response, Sse},
    routing::{get, post},
    Json, Router,
};
use axum::response::sse::Event;
use futures_util::{stream, StreamExt};
use std::time::Instant;
use tracing::{debug, error, warn};
use anyhow::Error as AnyhowError;

use crate::models::schemas::{
    ChatCompletionRequest, ModelResponse, Model,
    EmbeddingRequest, EmbeddingResponse,
};
use crate::services::gemini::GeminiClientTrait;
use crate::utils::{
    auth::{authenticate_request, AuthQuery, validate_user_agent},
    cache::generate_cache_key,
    response::{create_error_response, create_error_json},
};
use crate::AppState;

// V1 API Routes (OpenAI compatible)
pub fn create_v1_routes() -> Router<AppState> {
    Router::new()
        .route("/chat/completions", post(chat_completions))
        .route("/models", get(list_models))
        .route("/embeddings", post(embeddings))
}

// Legacy API Routes (for backwards compatibility)
pub fn create_api_routes() -> Router<AppState> {
    Router::new()
        .route("/chat/completions", post(chat_completions))
        .route("/models", get(list_models))
        .route("/embeddings", post(embeddings))
}

async fn chat_completions(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<AuthQuery>,
    Json(request): Json<ChatCompletionRequest>,
) -> Result<Response, StatusCode> {
    let start_time = Instant::now();

    // Authenticate request
    let auth_result = authenticate_request(&headers, &query, &state.settings);
    if !auth_result.authenticated {
        return Ok(create_error_response("Unauthorized", "authentication_error"));
    }

    // Validate user agent if configured
    let user_agent = headers.get("user-agent").and_then(|ua| ua.to_str().ok());
    if !validate_user_agent(user_agent, &state.settings) {
        return Ok(create_error_response("Forbidden user agent", "forbidden_error"));
    }

    // Get client IP for rate limiting
    let client_ip = extract_client_ip(&headers);

    // Check rate limits
    if let Err(err) = check_rate_limits(&state, &client_ip).await {
        return Ok(err.into_response());
    }

    // Validate model
    if !is_model_allowed(&request.model, &state.settings) {
        return Ok(create_error_response("Model not allowed", "invalid_model"));
    }

    // Check cache if not streaming
    if !request.stream {
        let cache_key = generate_cache_key(
            &request.messages.iter().map(|m| serde_json::to_value(m).unwrap()).collect::<Vec<_>>(),
            &request.model,
            state.settings.calculate_cache_entries,
            state.settings.precise_cache,
        );

        if let Some(cached_response) = state.cache_manager.get(&cache_key).await {
            debug!("Returning cached response for key: {}", cache_key);

            // Record cache hit in stats
            state.stats_manager.record_api_call(
                request.model.clone(),
                cached_response.usage.as_ref().map(|u| u.total_tokens).unwrap_or(0),
                true,
                start_time.elapsed().as_millis() as u64,
                client_ip,
            ).await;

            return Ok(Json(cached_response).into_response());
        }
    }

    // Get API key
    let api_key = match state.key_manager.get_next_key().await {
        Some(key) => key,
        None => {
            error!("No API keys available");
            return Ok(create_error_response("No API keys available", "service_unavailable"));
        }
    };

    // Handle streaming vs non-streaming
    if request.stream {
        handle_streaming_request(state, request, api_key, client_ip, start_time).await
    } else {
        handle_non_streaming_request(state, request, api_key, client_ip, start_time).await
    }
}

async fn handle_streaming_request(
    state: AppState,
    request: ChatCompletionRequest,
    api_key: String,
    client_ip: Option<String>,
    start_time: Instant,
) -> Result<Response, StatusCode> {
    if state.settings.fake_streaming {
        // Use fake streaming mode
        handle_fake_streaming(state, request, api_key, client_ip, start_time).await
    } else {
        // Use real streaming
        handle_real_streaming(state, request, api_key, client_ip, start_time).await
    }
}

async fn handle_fake_streaming(
    state: AppState,
    request: ChatCompletionRequest,
    api_key: String,
    client_ip: Option<String>,
    start_time: Instant,
) -> Result<Response, StatusCode> {
    // Make a non-streaming request in the background
    let gemini_client = state.gemini_client.clone();
    let model = request.model.clone();

    let stream = stream::unfold(
        (state, request, api_key, client_ip, start_time, false, gemini_client, model),
        move |(state, request, api_key, client_ip, start_time, completed, gemini_client, model)| async move {
            if completed {
                return None;
            }

            match gemini_client.chat_completion(request.clone(), &api_key).await {
                Ok(response) => {
                    // Record successful API call
                    state.stats_manager.record_api_call(
                        model.clone(),
                        response.usage.as_ref().map(|u| u.total_tokens).unwrap_or(0),
                        true,
                        start_time.elapsed().as_millis() as u64,
                        client_ip.clone(),
                    ).await;

                    // Mark API key as successful
                    state.key_manager.mark_key_used(&api_key, true).await;

                    // Convert to streaming format and return final chunk
                    let chunk_data = serde_json::to_string(&response).unwrap_or_default();
                    let event = Event::default().data(chunk_data);
                    Some((Ok::<Event, AnyhowError>(event), (state, request, api_key, client_ip, start_time, true, gemini_client, model)))
                }
                Err(e) => {
                    error!("Fake streaming request failed: {}", e);

                    // Record failed API call
                    state.stats_manager.record_api_call(
                        model.clone(),
                        0,
                        false,
                        start_time.elapsed().as_millis() as u64,
                        client_ip.clone(),
                    ).await;

                    // Mark API key as failed
                    state.key_manager.mark_key_used(&api_key, false).await;

                    let error_data = serde_json::to_string(&create_error_json(&e.to_string(), "api_error")).unwrap_or_default();
                    let event = Event::default().data(error_data);
                    Some((Ok::<Event, AnyhowError>(event), (state, request, api_key, client_ip, start_time, true, gemini_client, model)))
                }
            }
        },
    );

    Ok(Sse::new(stream).into_response())    
}

async fn handle_real_streaming(
    state: AppState,
    request: ChatCompletionRequest,
    api_key: String,
    client_ip: Option<String>,
    start_time: Instant,
) -> Result<Response, StatusCode> {
    match state.gemini_client.chat_completion_stream(request.clone(), &api_key).await {
        Ok(gemini_stream) => {
            let stream = gemini_stream.map(move |chunk_result| {
                match chunk_result {
                    Ok(chunk) => {
                        let chunk_data = serde_json::to_string(&chunk).unwrap_or_default();
                        Ok::<Event, AnyhowError>(Event::default().data(chunk_data))
                    }
                    Err(e) => {
                        error!("Streaming chunk error: {}", e);
                        let error_data = serde_json::to_string(&create_error_json(&e.to_string(), "stream_error")).unwrap_or_default();
                        Ok::<Event, AnyhowError>(Event::default().data(error_data))
                    }
                }
            });

            Ok(Sse::new(stream).into_response())
        }
        Err(e) => {
            error!("Failed to start streaming: {}", e);
            state.key_manager.mark_key_used(&api_key, false).await;

            state.stats_manager.record_api_call(
                request.model,
                0,
                false,
                start_time.elapsed().as_millis() as u64,
                client_ip,
            ).await;

            Ok(create_error_response(&e.to_string(), "stream_error"))
        }
    }
}

async fn handle_non_streaming_request(
    state: AppState,
    request: ChatCompletionRequest,
    api_key: String,
    client_ip: Option<String>,
    start_time: Instant,
) -> Result<Response, StatusCode> {
    let model = request.model.clone();

    match state.gemini_client.chat_completion(request.clone(), &api_key).await {
        Ok(response) => {
            // Record successful API call
            state.stats_manager.record_api_call(
                model.clone(),
                response.usage.as_ref().map(|u| u.total_tokens).unwrap_or(0),
                true,
                start_time.elapsed().as_millis() as u64,
                client_ip,
            ).await;

            // Mark API key as successful
            state.key_manager.mark_key_used(&api_key, true).await;

            // Cache the response
            let cache_key = generate_cache_key(
                &request.messages.iter().map(|m| serde_json::to_value(m).unwrap()).collect::<Vec<_>>(),
                &request.model,
                state.settings.calculate_cache_entries,
                state.settings.precise_cache,
            );

            state.cache_manager.put(cache_key, response.clone()).await;

            Ok(Json(response).into_response())
        }
        Err(e) => {
            error!("Non-streaming request failed: {}", e);

            // Record failed API call
            state.stats_manager.record_api_call(
                model,
                0,
                false,
                start_time.elapsed().as_millis() as u64,
                client_ip,
            ).await;

            // Mark API key as failed
            state.key_manager.mark_key_used(&api_key, false).await;

            Ok(create_error_response(&e.to_string(), "api_error"))
        }
    }
}

async fn list_models(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<AuthQuery>,
) -> Result<Json<ModelResponse>, StatusCode> {
    // Authenticate request
    let auth_result = authenticate_request(&headers, &query, &state.settings);
    if !auth_result.authenticated {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let available_models = state.gemini_client.get_available_models().await;
    let mut models = Vec::new();

    for model_name in available_models {
        // Add regular model
        models.push(Model {
            id: model_name.clone(),
            object: "model".to_string(),
            created: chrono::Utc::now().timestamp() as u64,
            owned_by: "google".to_string(),
        });

        // Add search variant if search mode is enabled
        if state.settings.search_mode && model_name.starts_with("gemini") {
            models.push(Model {
                id: format!("{}-search", model_name),
                object: "model".to_string(),
                created: chrono::Utc::now().timestamp() as u64,
                owned_by: "google".to_string(),
            });
        }
    }

    Ok(Json(ModelResponse {
        object: "list".to_string(),
        data: models,
    }))
}

async fn embeddings(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<AuthQuery>,
    Json(request): Json<EmbeddingRequest>,
) -> Result<Json<EmbeddingResponse>, StatusCode> {
    let start_time = Instant::now();

    // Authenticate request
    let auth_result = authenticate_request(&headers, &query, &state.settings);
    if !auth_result.authenticated {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let client_ip = extract_client_ip(&headers);

    // Get API key
    let api_key = match state.key_manager.get_next_key().await {
        Some(key) => key,
        None => {
            error!("No API keys available for embedding");
            return Err(StatusCode::SERVICE_UNAVAILABLE);
        }
    };

    match state.gemini_client.embedding(request.clone(), &api_key).await {
        Ok(response) => {
            // Record successful API call
            state.stats_manager.record_api_call(
                request.model,
                response.usage.total_tokens,
                true,
                start_time.elapsed().as_millis() as u64,
                client_ip,
            ).await;

            state.key_manager.mark_key_used(&api_key, true).await;
            Ok(Json(response))
        }
        Err(e) => {
            error!("Embedding request failed: {}", e);

            state.stats_manager.record_api_call(
                request.model,
                0,
                false,
                start_time.elapsed().as_millis() as u64,
                client_ip,
            ).await;

            state.key_manager.mark_key_used(&api_key, false).await;
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

// Helper functions

fn extract_client_ip(headers: &HeaderMap) -> Option<String> {
    // Check various headers for client IP
    for header_name in ["x-forwarded-for", "x-real-ip", "cf-connecting-ip"] {
        if let Some(ip_header) = headers.get(header_name) {
            if let Ok(ip_str) = ip_header.to_str() {
                // Take the first IP if there are multiple
                let ip = ip_str.split(',').next().unwrap_or(ip_str).trim();
                return Some(ip.to_string());
            }
        }
    }
    None
}

async fn check_rate_limits(state: &AppState, client_ip: &Option<String>) -> Result<(), StatusCode> {
    if let Some(ip) = client_ip {
        let requests_today = state.stats_manager.get_requests_for_ip_last_day(ip).await;
        if requests_today >= state.settings.max_requests_per_day_per_ip {
            warn!("Rate limit exceeded for IP: {}", ip);
            return Err(StatusCode::TOO_MANY_REQUESTS);
        }
    }

    // Additional rate limiting logic could be added here
    Ok(())
}

fn is_model_allowed(model: &str, settings: &crate::config::Settings) -> bool {
    // Check whitelist first (if configured)
    if !settings.whitelist_models.is_empty() {
        return settings.whitelist_models.contains(&model.to_string());
    }

    // Check blacklist
    !settings.blocked_models.contains(&model.to_string())
}