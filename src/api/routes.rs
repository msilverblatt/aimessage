use axum::{
    middleware,
    routing::{delete, get, post},
    Router,
};
use std::sync::Arc;

use super::auth::{require_api_key, ApiKey};
use super::handlers;
use crate::api::handlers::AppState;

pub fn build_router(state: Arc<AppState>, api_key: String) -> Router {
    let authed_routes = Router::new()
        .route("/messages", post(handlers::send_message))
        .route("/messages", get(handlers::list_messages))
        .route("/messages/{id}", get(handlers::get_message))
        .route("/conversations", get(handlers::list_conversations))
        .route("/conversations/{id}", get(handlers::get_conversation))
        .route("/webhooks", post(handlers::create_webhook))
        .route("/webhooks", get(handlers::list_webhooks))
        .route("/webhooks/{id}", delete(handlers::delete_webhook))
        .layer(middleware::from_fn(require_api_key))
        .layer(axum::Extension(ApiKey(api_key)));

    let public_routes = Router::new()
        .route("/health", get(handlers::health));

    let internal_routes = Router::new()
        .route("/bb-webhook", post(handlers::bb_webhook_handler));

    Router::new()
        .nest("/api/v1", authed_routes.merge(public_routes))
        .nest("/internal", internal_routes)
        .with_state(state)
}
