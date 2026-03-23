use reqwest::Client;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing;

use crate::core_layer::types::Message;
use crate::storage::sqlite::Storage;

pub struct WebhookDispatcher {
    storage: Arc<Storage>,
    client: Client,
}

impl WebhookDispatcher {
    pub fn new(storage: Arc<Storage>) -> Self {
        WebhookDispatcher {
            storage,
            client: Client::new(),
        }
    }

    pub fn spawn(self, mut receiver: mpsc::Receiver<Message>) {
        tokio::spawn(async move {
            tracing::info!("Webhook dispatcher started");
            while let Some(message) = receiver.recv().await {
                self.handle_message(&message).await;
            }
            tracing::info!("Webhook dispatcher stopped");
        });
    }

    async fn handle_message(&self, message: &Message) {
        let is_new = self
            .storage
            .log_message(&message.id, &message.conversation_id);

        match is_new {
            Ok(false) => {
                tracing::debug!(message_id = %message.id, "Duplicate message, skipping");
                return;
            }
            Err(e) => {
                tracing::error!(error = %e, "Failed to log message");
                return;
            }
            Ok(true) => {}
        }

        let event = if message.is_from_me {
            "message.sent"
        } else {
            "message.received"
        };

        let webhooks = match self.storage.get_webhooks_for_event(event) {
            Ok(w) => w,
            Err(e) => {
                tracing::error!(error = %e, "Failed to get webhooks");
                return;
            }
        };

        let payload = serde_json::json!({
            "event": event,
            "data": message,
        });

        for webhook in &webhooks {
            let delivered = self.deliver_with_retry(&webhook.url, &payload).await;
            let status = if delivered { "delivered" } else { "failed" };
            if let Err(e) = self.storage.update_delivery_status(&message.id, status) {
                tracing::error!(error = %e, "Failed to update delivery status");
            }
        }
    }

    async fn deliver_with_retry(&self, url: &str, payload: &serde_json::Value) -> bool {
        let delays_after_failure = [
            std::time::Duration::from_secs(1),
            std::time::Duration::from_secs(5),
            std::time::Duration::from_secs(30),
        ];

        // First attempt (no delay)
        if self.try_deliver(url, payload).await {
            return true;
        }

        // Retry up to 2 more times (3 attempts total per spec)
        for (retry, delay) in delays_after_failure.iter().take(2).enumerate() {
            tracing::info!(url = %url, retry = retry + 1, "Retrying webhook delivery");
            tokio::time::sleep(*delay).await;
            if self.try_deliver(url, payload).await {
                return true;
            }
        }

        tracing::error!(url = %url, "Webhook delivery permanently failed after 3 attempts");
        false
    }

    async fn try_deliver(&self, url: &str, payload: &serde_json::Value) -> bool {
        match self.client.post(url).json(payload).send().await {
            Ok(resp) if resp.status().is_success() => {
                tracing::info!(url = %url, "Webhook delivered");
                true
            }
            Ok(resp) => {
                tracing::warn!(url = %url, status = %resp.status(), "Webhook delivery failed");
                false
            }
            Err(e) => {
                tracing::warn!(url = %url, error = %e, "Webhook delivery error");
                false
            }
        }
    }
}
