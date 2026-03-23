use async_trait::async_trait;
use tokio::sync::mpsc;

use super::errors::BackendError;
use super::types::{
    BackendStatus, Conversation, Event, Message, MessageQuery, PaginationQuery,
    ReactionType, SendMessageRequest,
};

#[async_trait]
pub trait MessageBackend: Send + Sync {
    // Sending
    async fn send_message(&self, request: SendMessageRequest) -> Result<Message, BackendError>;
    async fn send_reaction(&self, message_id: &str, reaction: ReactionType) -> Result<(), BackendError>;
    async fn send_typing(&self, conversation_id: &str) -> Result<(), BackendError>;

    // Reading
    async fn get_messages(&self, query: MessageQuery) -> Result<Vec<Message>, BackendError>;
    async fn get_message(&self, id: &str) -> Result<Message, BackendError>;
    async fn get_conversations(&self, query: PaginationQuery) -> Result<Vec<Conversation>, BackendError>;
    async fn get_conversation(&self, id: &str) -> Result<Conversation, BackendError>;

    // Lifecycle
    async fn start(&self) -> Result<mpsc::Receiver<Event>, BackendError>;
    async fn shutdown(&self) -> Result<(), BackendError>;
    async fn health_check(&self) -> Result<BackendStatus, BackendError>;
}
