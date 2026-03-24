use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing;

use crate::config::IMessageConfig;
use crate::core_layer::backend::MessageBackend;
use crate::core_layer::errors::BackendError;
use crate::core_layer::types::*;
use crate::storage::sqlite::Storage;
use super::applescript;
use super::chatdb::ChatDb;
use super::private_api::PrivateApi;

pub struct IMessageBackend {
    config: IMessageConfig,
    storage: Arc<Storage>,
    private_api: PrivateApi,
}

impl IMessageBackend {
    pub fn new(config: IMessageConfig, storage: Arc<Storage>) -> Self {
        let private_api = PrivateApi::new(config.private_api);
        IMessageBackend {
            config,
            storage,
            private_api,
        }
    }
}

#[async_trait]
impl MessageBackend for IMessageBackend {
    async fn send_message(&self, request: SendMessageRequest) -> Result<Message, BackendError> {
        let recipient = request.recipient.clone();
        let body = request.body.clone();

        // Send text body via AppleScript (if non-empty)
        if !body.is_empty() {
            applescript::send_message(&recipient, &body)
                .await
                .map_err(BackendError::RequestFailed)?;
        }

        // Send each attachment
        for file_path in &request.attachments {
            applescript::send_attachment(&recipient, file_path)
                .await
                .map_err(BackendError::RequestFailed)?;
        }

        // Poll chat.db for the sent message (up to 3 seconds)
        let db_path = self.config.chat_db_path.clone();
        let poll_body = body.clone();
        let poll_recipient = recipient.clone();
        let result = tokio::task::spawn_blocking(move || {
            for _ in 0..15 {
                std::thread::sleep(std::time::Duration::from_millis(200));
                let found = ChatDb::with_connection(&db_path, |chatdb| {
                    chatdb.find_sent_message(&poll_recipient, &poll_body)
                }).map_err(BackendError::Unavailable)?;
                if let Some(msg) = found {
                    return Ok(msg);
                }
            }

            // Return provisional response if not found
            Ok(Message {
                id: String::new(),
                guid: String::new(),
                conversation_id: String::new(),
                sender: "me".to_string(),
                body: poll_body,
                attachments: vec![],
                timestamp: chrono::Utc::now(),
                is_from_me: true,
                status: MessageStatus::Sent,
            })
        })
        .await
        .map_err(|e| BackendError::RequestFailed(format!("Task join error: {}", e)))?;

        result
    }

    async fn send_reaction(&self, message_id: &str, reaction: ReactionType) -> Result<(), BackendError> {
        // Look up the message guid from ROWID
        let db_path = self.config.chat_db_path.clone();
        let mid = message_id.to_string();
        let guid = tokio::task::spawn_blocking(move || {
            ChatDb::with_connection(&db_path, |chatdb| chatdb.guid_for_rowid(&mid))
                .map_err(BackendError::Unavailable)
        })
        .await
        .map_err(|e| BackendError::RequestFailed(format!("Task join error: {}", e)))??;

        self.private_api.send_reaction(&guid, &reaction)
    }

    async fn send_typing(&self, conversation_id: &str) -> Result<(), BackendError> {
        self.private_api.send_typing(conversation_id)
    }

    async fn get_messages(&self, query: MessageQuery) -> Result<Vec<Message>, BackendError> {
        let db_path = self.config.chat_db_path.clone();
        tokio::task::spawn_blocking(move || {
            ChatDb::with_connection(&db_path, |chatdb| chatdb.get_messages(&query))
                .map_err(BackendError::Unavailable)
        })
        .await
        .map_err(|e| BackendError::RequestFailed(format!("Task join error: {}", e)))?
    }

    async fn get_message(&self, id: &str) -> Result<Message, BackendError> {
        let db_path = self.config.chat_db_path.clone();
        let id = id.to_string();
        tokio::task::spawn_blocking(move || {
            ChatDb::with_connection(&db_path, |chatdb| chatdb.get_message(&id))
                .map_err(BackendError::Unavailable)
        })
        .await
        .map_err(|e| BackendError::RequestFailed(format!("Task join error: {}", e)))?
    }

    async fn get_conversations(&self, query: PaginationQuery) -> Result<Vec<Conversation>, BackendError> {
        let db_path = self.config.chat_db_path.clone();
        tokio::task::spawn_blocking(move || {
            ChatDb::with_connection(&db_path, |chatdb| chatdb.get_conversations(&query))
                .map_err(BackendError::Unavailable)
        })
        .await
        .map_err(|e| BackendError::RequestFailed(format!("Task join error: {}", e)))?
    }

    async fn get_conversation(&self, id: &str) -> Result<Conversation, BackendError> {
        let db_path = self.config.chat_db_path.clone();
        let id = id.to_string();
        tokio::task::spawn_blocking(move || {
            ChatDb::with_connection(&db_path, |chatdb| chatdb.get_conversation(&id))
                .map_err(BackendError::Unavailable)
        })
        .await
        .map_err(|e| BackendError::RequestFailed(format!("Task join error: {}", e)))?
    }

    async fn start(&self) -> Result<broadcast::Sender<Event>, BackendError> {
        let (sender, _) = broadcast::channel(256);
        let db_path = self.config.chat_db_path.clone();
        let poll_interval = std::time::Duration::from_millis(self.config.poll_interval_ms);
        let storage = self.storage.clone();
        let tx = sender.clone();

        // Get starting ROWID from state table (resume after restart)
        let start_rowid = storage.get_last_rowid()
            .map_err(BackendError::RequestFailed)?;

        // If no saved state, start from current max to avoid replaying entire history
        let start_rowid = if start_rowid == 0 {
            ChatDb::with_connection(&db_path, |chatdb| chatdb.get_max_rowid())
                .map_err(BackendError::Unavailable)?
        } else {
            start_rowid
        };

        tracing::info!(start_rowid = start_rowid, "Starting chat.db poller");

        tokio::spawn(async move {
            let mut last_rowid = start_rowid;

            loop {
                tokio::time::sleep(poll_interval).await;

                let db_path_clone = db_path.clone();
                let current_rowid = last_rowid;
                let poll_result = tokio::task::spawn_blocking(move || {
                    ChatDb::with_connection(&db_path_clone, |chatdb| {
                        chatdb.poll_new_events(current_rowid)
                    })
                }).await;

                match poll_result {
                    Ok(Ok((events, new_max_rowid))) => {
                        if new_max_rowid > last_rowid {
                            last_rowid = new_max_rowid;
                            if let Err(e) = storage.set_last_rowid(last_rowid) {
                                tracing::error!(error = %e, "Failed to persist ROWID");
                            }
                        }
                        for event in events {
                            // send() on broadcast returns Err only if there are no receivers;
                            // that's fine — just means no subscribers are connected yet.
                            let _ = tx.send(event);
                        }
                    }
                    Ok(Err(e)) => {
                        tracing::error!(error = %e, "chat.db poll error");
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "Poll task join error");
                    }
                }
            }
        });

        Ok(sender)
    }

    async fn shutdown(&self) -> Result<(), BackendError> {
        Ok(())
    }

    async fn health_check(&self) -> Result<BackendStatus, BackendError> {
        // Check chat.db is readable
        let db_path = self.config.chat_db_path.clone();
        let connected = tokio::task::spawn_blocking(move || {
            ChatDb::with_connection(&db_path, |_| Ok(())).is_ok()
        }).await.unwrap_or(false);

        Ok(BackendStatus {
            connected,
            private_api_available: self.private_api.is_available(),
            message: if connected { None } else { Some("Cannot read chat.db".to_string()) },
        })
    }
}
