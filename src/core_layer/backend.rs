use async_trait::async_trait;
use tokio::sync::mpsc;

use super::errors::BackendError;
use super::types::{
    BackendStatus, Conversation, Message, MessageQuery, PaginationQuery, SendMessageRequest,
};

#[async_trait]
pub trait MessageBackend: Send + Sync {
    async fn send_message(&self, request: SendMessageRequest) -> Result<Message, BackendError>;
    async fn get_messages(&self, query: MessageQuery) -> Result<Vec<Message>, BackendError>;
    async fn get_message(&self, id: &str) -> Result<Message, BackendError>;
    async fn get_conversations(
        &self,
        query: PaginationQuery,
    ) -> Result<Vec<Conversation>, BackendError>;
    async fn get_conversation(&self, id: &str) -> Result<Conversation, BackendError>;
    async fn start(&self) -> Result<mpsc::Receiver<Message>, BackendError>;
    async fn shutdown(&self) -> Result<(), BackendError>;
    async fn health_check(&self) -> Result<BackendStatus, BackendError>;
}
