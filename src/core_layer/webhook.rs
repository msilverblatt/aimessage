use hmac::{Hmac, Mac};
use reqwest::Client;
use sha2::Sha256;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing;

use crate::core_layer::types::Event;
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

    pub fn spawn(self, mut receiver: broadcast::Receiver<Event>) {
        tokio::spawn(async move {
            tracing::info!("Webhook dispatcher started");
            loop {
                match receiver.recv().await {
                    Ok(event) => self.handle_event(&event).await,
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!(skipped = n, "Webhook dispatcher lagged, skipping events");
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        tracing::info!("Webhook dispatcher stopped");
                        break;
                    }
                }
            }
        });
    }

    async fn handle_event(&self, event: &Event) {
        let event_name = event.event_name();

        let event_id = match event {
            Event::NewMessage(m) | Event::MessageSent(m) => &m.id,
            Event::ReactionAdded(r) | Event::ReactionRemoved(r) => &r.id,
        };
        let conversation_id = match event {
            Event::NewMessage(m) | Event::MessageSent(m) => &m.conversation_id,
            Event::ReactionAdded(r) | Event::ReactionRemoved(r) => &r.message_id,
        };

        let is_new = self.storage.log_message(event_id, conversation_id);
        match is_new {
            Ok(false) => {
                tracing::debug!(event_id = %event_id, "Duplicate event, skipping");
                return;
            }
            Err(e) => {
                tracing::error!(error = %e, "Failed to log event");
                return;
            }
            Ok(true) => {}
        }

        let webhooks = match self.storage.get_webhooks_for_event(event_name) {
            Ok(w) => w,
            Err(e) => {
                tracing::error!(error = %e, "Failed to get webhooks");
                return;
            }
        };

        let payload = serde_json::to_value(event).unwrap();

        for webhook in &webhooks {
            if let Err(e) = self.storage.log_delivery(event_id, &webhook.id) {
                tracing::error!(error = %e, "Failed to log delivery");
            }
            let delivered = self.deliver_with_retry(&webhook.url, &payload, webhook.secret.as_deref()).await;
            let status = if delivered { "delivered" } else { "failed" };
            if let Err(e) = self.storage.update_delivery_status(event_id, &webhook.id, status) {
                tracing::error!(error = %e, "Failed to update delivery status");
            }
        }
    }

    async fn deliver_with_retry(&self, url: &str, payload: &serde_json::Value, secret: Option<&str>) -> bool {
        let retry_delays = [
            std::time::Duration::from_secs(1),
            std::time::Duration::from_secs(5),
        ];

        // First attempt (no delay)
        if self.try_deliver(url, payload, secret).await {
            return true;
        }

        // 2 retries = 3 total attempts
        for (retry, delay) in retry_delays.iter().enumerate() {
            tracing::info!(url = %url, retry = retry + 1, "Retrying webhook delivery");
            tokio::time::sleep(*delay).await;
            if self.try_deliver(url, payload, secret).await {
                return true;
            }
        }

        tracing::error!(url = %url, "Webhook delivery permanently failed after 3 attempts");
        false
    }

    async fn try_deliver(&self, url: &str, payload: &serde_json::Value, secret: Option<&str>) -> bool {
        let body = serde_json::to_string(payload).unwrap();
        let mut request = self.client
            .post(url)
            .header("Content-Type", "application/json")
            .body(body.clone());
        if let Some(s) = secret {
            let mut mac = Hmac::<Sha256>::new_from_slice(s.as_bytes())
                .expect("HMAC accepts keys of any length");
            mac.update(body.as_bytes());
            let signature = hex::encode(mac.finalize().into_bytes());
            request = request.header("X-Webhook-Signature", format!("sha256={}", signature));
        }
        match request.send().await {
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
