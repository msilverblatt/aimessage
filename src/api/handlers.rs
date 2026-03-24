use axum::{
    extract::{Path, Query, State},
    Json,
};
use std::sync::Arc;

use crate::api::types::*;
use crate::core_layer::backend::MessageBackend;
use crate::core_layer::errors::ApiError;
use crate::core_layer::types::{MessageQuery, PaginationQuery, SendMessageRequest};
use crate::storage::sqlite::Storage;

pub struct AppState {
    pub backend: Arc<dyn MessageBackend>,
    pub storage: Arc<Storage>,
}

// Messages

pub async fn send_message(
    State(state): State<Arc<AppState>>,
    Json(body): Json<SendMessageBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let request = SendMessageRequest {
        recipient: body.recipient,
        body: body.body,
        attachments: body.attachments,
    };
    let message = state.backend.send_message(request).await?;
    Ok(Json(serde_json::to_value(message).unwrap()))
}

pub async fn list_messages(
    State(state): State<Arc<AppState>>,
    Query(params): Query<MessageQueryParams>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let query = MessageQuery {
        conversation_id: params.conversation_id,
        since: params.since,
        limit: params.limit.unwrap_or(50).min(200),
        offset: params.offset.unwrap_or(0),
    };
    let messages = state.backend.get_messages(query).await?;
    let count = messages.len();
    Ok(Json(serde_json::json!({ "data": messages, "count": count })))
}

pub async fn get_message(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let message = state.backend.get_message(&id).await?;
    Ok(Json(serde_json::to_value(message).unwrap()))
}

// Conversations

pub async fn list_conversations(
    State(state): State<Arc<AppState>>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let query = PaginationQuery {
        limit: params.limit.unwrap_or(50).min(200),
        offset: params.offset.unwrap_or(0),
    };
    let conversations = state.backend.get_conversations(query).await?;
    let count = conversations.len();
    Ok(Json(serde_json::json!({ "data": conversations, "count": count })))
}

pub async fn get_conversation(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let conversation = state.backend.get_conversation(&id).await?;
    Ok(Json(serde_json::to_value(conversation).unwrap()))
}

// Reactions / Typing

pub async fn send_reaction(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<SendReactionBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let reaction_type = match body.reaction.as_str() {
        "love" => crate::core_layer::types::ReactionType::Love,
        "thumbsup" => crate::core_layer::types::ReactionType::ThumbsUp,
        "thumbsdown" => crate::core_layer::types::ReactionType::ThumbsDown,
        "haha" => crate::core_layer::types::ReactionType::HaHa,
        "exclamation" => crate::core_layer::types::ReactionType::Exclamation,
        "question" => crate::core_layer::types::ReactionType::Question,
        other => return Err(ApiError::BadRequest(format!("Unknown reaction: {}", other))),
    };
    state.backend.send_reaction(&id, reaction_type).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn send_typing(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state.backend.send_typing(&id).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

// Webhooks

pub async fn create_webhook(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateWebhookBody>,
) -> Result<Json<WebhookResponse>, ApiError> {
    let record = state
        .storage
        .create_or_update_webhook(&body.url, &body.events, body.secret.as_deref())
        .map_err(ApiError::Storage)?;
    Ok(Json(WebhookResponse::from(record)))
}

pub async fn list_webhooks(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let webhooks = state
        .storage
        .list_webhooks()
        .map_err(ApiError::Storage)?;
    let responses: Vec<WebhookResponse> = webhooks.into_iter().map(WebhookResponse::from).collect();
    let count = responses.len();
    Ok(Json(serde_json::json!({ "data": responses, "count": count })))
}

pub async fn delete_webhook(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let deleted = state
        .storage
        .delete_webhook(&id)
        .map_err(ApiError::Storage)?;
    if deleted {
        Ok(Json(serde_json::json!({ "deleted": true })))
    } else {
        Err(ApiError::Backend(crate::core_layer::errors::BackendError::NotFound(
            format!("Webhook {} not found", id),
        )))
    }
}

// Health

pub async fn health(
    State(state): State<Arc<AppState>>,
) -> Json<HealthResponse> {
    let backend_status = state.backend.health_check().await;
    match backend_status {
        Ok(status) => Json(HealthResponse {
            status: "ok".to_string(),
            backend: BackendHealthResponse {
                connected: status.connected,
                private_api_available: status.private_api_available,
                message: status.message,
            },
        }),
        Err(e) => Json(HealthResponse {
            status: "degraded".to_string(),
            backend: BackendHealthResponse {
                connected: false,
                private_api_available: false,
                message: Some(e.to_string()),
            },
        }),
    }
}
