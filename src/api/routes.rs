use axum::{
    middleware,
    routing::{delete, get, post},
    Router,
};
use tower_http::cors::{Any, CorsLayer};
use std::sync::Arc;

use super::auth::{require_api_key, ApiKey};
use super::handlers;
use super::ratelimit;
use crate::api::handlers::AppState;

pub fn build_router(state: Arc<AppState>, api_key: String) -> Router {
    let limiter = ratelimit::create_limiter(60);

    let authed_routes = Router::new()
        .route("/messages", post(handlers::send_message))
        .route("/messages", get(handlers::list_messages))
        .route("/messages/{id}", get(handlers::get_message))
        .route("/messages/{id}/react", post(handlers::send_reaction))
        .route("/conversations", get(handlers::list_conversations))
        .route("/conversations/{id}", get(handlers::get_conversation))
        .route("/conversations/{id}/typing", post(handlers::send_typing))
        .route("/webhooks", post(handlers::create_webhook))
        .route("/webhooks", get(handlers::list_webhooks))
        .route("/webhooks/{id}", delete(handlers::delete_webhook))
        .layer(middleware::from_fn(ratelimit::rate_limit))
        .layer(axum::Extension(limiter))
        .layer(middleware::from_fn(require_api_key))
        .layer(axum::Extension(ApiKey(api_key)));

    let public_routes = Router::new()
        .route("/health", get(handlers::health))
        .route("/ws", get(handlers::ws_handler));

    Router::new()
        .nest("/api/v1", authed_routes.merge(public_routes))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .with_state(state)
}
