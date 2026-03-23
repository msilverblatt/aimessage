use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::storage::sqlite::WebhookRecord;

#[derive(Debug, Deserialize)]
pub struct SendMessageBody {
    pub recipient: String,
    pub body: String,
    #[serde(default)]
    pub attachments: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct MessageQueryParams {
    pub conversation_id: Option<String>,
    pub since: Option<DateTime<Utc>>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct PaginationParams {
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct CreateWebhookBody {
    pub url: String,
    pub events: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct WebhookResponse {
    pub id: String,
    pub url: String,
    pub events: Vec<String>,
    pub created_at: String,
}

impl From<WebhookRecord> for WebhookResponse {
    fn from(r: WebhookRecord) -> Self {
        WebhookResponse {
            id: r.id,
            url: r.url,
            events: r.events,
            created_at: r.created_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub backend: BackendHealthResponse,
}

#[derive(Debug, Serialize)]
pub struct BackendHealthResponse {
    pub connected: bool,
    pub backend_type: String,
    pub message: Option<String>,
}
