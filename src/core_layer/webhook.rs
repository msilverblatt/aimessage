use reqwest::Client;
use std::sync::Arc;
use tokio::sync::mpsc;
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

    pub fn spawn(self, mut receiver: mpsc::Receiver<Event>) {
        tokio::spawn(async move {
            tracing::info!("Webhook dispatcher started");
            while let Some(event) = receiver.recv().await {
                self.handle_event(&event).await;
            }
            tracing::info!("Webhook dispatcher stopped");
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
            let delivered = self.deliver_with_retry(&webhook.url, &payload, webhook.secret.as_deref()).await;
            let status = if delivered { "delivered" } else { "failed" };
            if let Err(e) = self.storage.update_delivery_status(event_id, status) {
                tracing::error!(error = %e, "Failed to update delivery status");
            }
        }
    }

    async fn deliver_with_retry(&self, url: &str, payload: &serde_json::Value, secret: Option<&str>) -> bool {
        let delays_after_failure = [
            std::time::Duration::from_secs(1),
            std::time::Duration::from_secs(5),
            std::time::Duration::from_secs(30),
        ];

        if self.try_deliver(url, payload, secret).await {
            return true;
        }

        for (retry, delay) in delays_after_failure.iter().take(2).enumerate() {
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
        let mut request = self.client.post(url).json(payload);
        if let Some(s) = secret {
            request = request.header("X-Webhook-Secret", s);
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
