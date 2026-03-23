use async_trait::async_trait;
use reqwest::Client;
use tokio::sync::mpsc;
use tracing;

use crate::config::BlueBubblesConfig;
use crate::core_layer::backend::MessageBackend;
use crate::core_layer::bb_parse;
use crate::core_layer::errors::BackendError;
use crate::core_layer::types::*;

pub struct BlueBubblesBackend {
    config: BlueBubblesConfig,
    client: Client,
    sender: mpsc::Sender<Message>,
    receiver: tokio::sync::Mutex<Option<mpsc::Receiver<Message>>>,
    callback_url: String,
}

impl BlueBubblesBackend {
    pub fn new(config: BlueBubblesConfig, server_port: u16) -> Self {
        let (sender, receiver) = mpsc::channel(256);
        let callback_url = format!("http://localhost:{}/internal/bb-webhook", server_port);
        BlueBubblesBackend {
            config,
            client: Client::new(),
            sender,
            receiver: tokio::sync::Mutex::new(Some(receiver)),
            callback_url,
        }
    }

    fn base_url(&self) -> &str {
        &self.config.url
    }

    fn password(&self) -> &str {
        &self.config.password
    }
}

#[async_trait]
impl MessageBackend for BlueBubblesBackend {
    async fn send_message(&self, request: SendMessageRequest) -> Result<Message, BackendError> {
        let url = format!("{}/api/v1/message/text", self.base_url());
        let body = serde_json::json!({
            "chatGuid": format!("iMessage;-;{}", request.recipient),
            "message": request.body,
            "tempGuid": uuid::Uuid::new_v4().to_string(),
        });

        let resp = self
            .client
            .post(&url)
            .query(&[("password", self.password())])
            .json(&body)
            .send()
            .await
            .map_err(|e| BackendError::Unavailable(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(BackendError::RequestFailed(format!("{}: {}", status, text)));
        }

        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| BackendError::RequestFailed(e.to_string()))?;

        let data = json.get("data").unwrap_or(&json);
        bb_parse::parse_bb_message(data)
            .ok_or_else(|| BackendError::RequestFailed("Failed to parse BB response".to_string()))
    }

    async fn get_messages(&self, query: MessageQuery) -> Result<Vec<Message>, BackendError> {
        let url = if let Some(ref chat_id) = query.conversation_id {
            format!("{}/api/v1/chat/{}/message", self.base_url(), chat_id)
        } else {
            format!("{}/api/v1/message", self.base_url())
        };

        let mut params = vec![
            ("password".to_string(), self.password().to_string()),
            ("limit".to_string(), query.limit.to_string()),
            ("offset".to_string(), query.offset.to_string()),
            ("sort".to_string(), "DESC".to_string()),
        ];

        if let Some(since) = query.since {
            params.push(("after".to_string(), since.timestamp_millis().to_string()));
        }

        let resp = self
            .client
            .get(&url)
            .query(&params)
            .send()
            .await
            .map_err(|e| BackendError::Unavailable(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(BackendError::RequestFailed(format!("{}: {}", status, text)));
        }

        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| BackendError::RequestFailed(e.to_string()))?;

        let data = json
            .get("data")
            .and_then(|d| d.as_array())
            .ok_or_else(|| BackendError::RequestFailed("Unexpected BB response format".to_string()))?;

        Ok(data.iter().filter_map(|v| bb_parse::parse_bb_message(v)).collect())
    }

    async fn get_message(&self, id: &str) -> Result<Message, BackendError> {
        let url = format!("{}/api/v1/message/{}", self.base_url(), id);
        let resp = self
            .client
            .get(&url)
            .query(&[("password", self.password())])
            .send()
            .await
            .map_err(|e| BackendError::Unavailable(e.to_string()))?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(BackendError::NotFound(format!("Message {} not found", id)));
        }

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(BackendError::RequestFailed(format!("{}: {}", status, text)));
        }

        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| BackendError::RequestFailed(e.to_string()))?;

        let data = json.get("data").unwrap_or(&json);
        bb_parse::parse_bb_message(data)
            .ok_or_else(|| BackendError::RequestFailed("Failed to parse BB response".to_string()))
    }

    async fn get_conversations(&self, query: PaginationQuery) -> Result<Vec<Conversation>, BackendError> {
        let url = format!("{}/api/v1/chat", self.base_url());
        let params = vec![
            ("password".to_string(), self.password().to_string()),
            ("limit".to_string(), query.limit.to_string()),
            ("offset".to_string(), query.offset.to_string()),
        ];

        let resp = self
            .client
            .get(&url)
            .query(&params)
            .send()
            .await
            .map_err(|e| BackendError::Unavailable(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(BackendError::RequestFailed(format!("{}: {}", status, text)));
        }

        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| BackendError::RequestFailed(e.to_string()))?;

        let data = json
            .get("data")
            .and_then(|d| d.as_array())
            .ok_or_else(|| BackendError::RequestFailed("Unexpected BB response format".to_string()))?;

        Ok(data.iter().filter_map(|v| bb_parse::parse_bb_chat(v)).collect())
    }

    async fn get_conversation(&self, id: &str) -> Result<Conversation, BackendError> {
        let url = format!("{}/api/v1/chat/{}", self.base_url(), id);
        let resp = self
            .client
            .get(&url)
            .query(&[("password", self.password())])
            .send()
            .await
            .map_err(|e| BackendError::Unavailable(e.to_string()))?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(BackendError::NotFound(format!("Conversation {} not found", id)));
        }

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(BackendError::RequestFailed(format!("{}: {}", status, text)));
        }

        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| BackendError::RequestFailed(e.to_string()))?;

        let data = json.get("data").unwrap_or(&json);
        bb_parse::parse_bb_chat(data)
            .ok_or_else(|| BackendError::RequestFailed("Failed to parse BB response".to_string()))
    }

    async fn start(&self) -> Result<mpsc::Receiver<Message>, BackendError> {
        // Register our webhook URL with BlueBubbles so it sends us incoming messages
        let url = format!("{}/api/v1/server/webhook", self.base_url());
        let body = serde_json::json!({
            "url": self.callback_url,
        });

        match self
            .client
            .post(&url)
            .query(&[("password", self.password())])
            .json(&body)
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                tracing::info!(callback_url = %self.callback_url, "Registered webhook with BlueBubbles");
            }
            Ok(resp) => {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                tracing::warn!(
                    status = %status,
                    body = %text,
                    "Failed to register webhook with BlueBubbles — incoming messages may not work"
                );
            }
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "Could not reach BlueBubbles to register webhook — incoming messages may not work until BB is available"
                );
            }
        }

        self.receiver
            .lock()
            .await
            .take()
            .ok_or_else(|| BackendError::RequestFailed("Backend already started".to_string()))
    }

    async fn push_incoming_message(&self, message: Message) {
        if let Err(e) = self.sender.send(message).await {
            tracing::error!(error = %e, "Failed to push incoming message to channel");
        }
    }

    async fn shutdown(&self) -> Result<(), BackendError> {
        Ok(())
    }

    async fn health_check(&self) -> Result<BackendStatus, BackendError> {
        let url = format!("{}/api/v1/server/info", self.base_url());
        let resp = self
            .client
            .get(&url)
            .query(&[("password", self.password())])
            .send()
            .await
            .map_err(|e| BackendError::Unavailable(e.to_string()))?;

        if resp.status().is_success() {
            Ok(BackendStatus {
                connected: true,
                backend_type: "bluebubbles".to_string(),
                message: None,
            })
        } else {
            Ok(BackendStatus {
                connected: false,
                backend_type: "bluebubbles".to_string(),
                message: Some(format!("BB returned {}", resp.status())),
            })
        }
    }
}
