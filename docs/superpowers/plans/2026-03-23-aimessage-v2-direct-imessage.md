# AiMessage v2: Direct iMessage Integration — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the BlueBubbles backend with direct iMessage integration — reading chat.db, sending via AppleScript, and optionally using IMCore Private API for reactions and typing indicators.

**Architecture:** Refactor existing three-layer codebase. Core types gain `Event`, `Reaction`, `ReactionType`. Backend trait gains `send_reaction`/`send_typing`, `start()` returns `Receiver<Event>`. New `imessage/` module replaces `backends/bluebubbles.rs`. Webhook dispatcher handles events instead of messages. Config drops BlueBubbles, adds `[imessage]` section. Storage gains `state` table.

**Tech Stack:** Rust, Axum, Tokio, rusqlite (for both chat.db reads and app storage), serde, AppleScript via `std::process::Command`

**Spec:** `docs/superpowers/specs/2026-03-23-aimessage-server-design.md` (v2)

**Existing code:** The API layer (auth, routes, handlers), storage layer, webhook dispatcher, and config module already exist from v1. This plan modifies them in-place.

---

## File Structure — What Changes

| File | Action | Responsibility |
|------|--------|---------------|
| `src/core_layer/types.rs` | **Modify** | Add `Event`, `Reaction`, `ReactionType` enums; add `guid` field to `Message`; update `BackendStatus` |
| `src/core_layer/backend.rs` | **Modify** | Add `send_reaction`, `send_typing`; change `start()` to return `Receiver<Event>`; remove `push_incoming_message` |
| `src/core_layer/errors.rs` | **Modify** | Add `FeatureUnavailable` variant to `BackendError` |
| `src/core_layer/webhook.rs` | **Modify** | Handle `Event` enum instead of `Message` |
| `src/core_layer/bb_parse.rs` | **Delete** | No longer needed |
| `src/config.rs` | **Modify** | Replace `[backend]` with `[imessage]` section |
| `src/storage/sqlite.rs` | **Modify** | Add `state` table, rename `backend_message_id` to `imessage_rowid`, add state get/set methods |
| `src/imessage/mod.rs` | **Create** | Module declaration |
| `src/imessage/chatdb.rs` | **Create** | chat.db SQLite reader + ROWID poller |
| `src/imessage/applescript.rs` | **Create** | AppleScript message sender via osascript |
| `src/imessage/private_api.rs` | **Create** | IMCore availability check + stub for future dylib loading |
| `src/imessage/backend.rs` | **Create** | `IMessageBackend` implementing `MessageBackend` trait |
| `src/api/handlers.rs` | **Modify** | Add `send_reaction`, `send_typing` handlers; remove `bb_webhook_handler` |
| `src/api/routes.rs` | **Modify** | Add reaction/typing routes; remove `/internal/bb-webhook` |
| `src/api/types.rs` | **Modify** | Add reaction/typing request types; add `private_api_available` to health |
| `src/main.rs` | **Modify** | Replace BlueBubbles init with IMessage init |
| `src/backends/` | **Delete** | Entire directory (bluebubbles.rs, mod.rs) |

---

### Task 1: Update Core Types

**Files:**
- Modify: `src/core_layer/types.rs`
- Modify: `src/core_layer/errors.rs`

- [ ] **Step 1: Replace types.rs with updated domain types**

Add `guid` to `Message`, add `Event`, `Reaction`, `ReactionType`, update `BackendStatus`:

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,          // ROWID as string
    pub guid: String,        // iMessage guid (used for reaction linking)
    pub conversation_id: String,
    pub sender: String,
    pub body: String,
    pub attachments: Vec<String>,
    pub timestamp: DateTime<Utc>,
    pub is_from_me: bool,
    pub status: MessageStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageStatus {
    Sent,
    Delivered,
    Read,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub id: String,
    pub participants: Vec<String>,
    pub display_name: Option<String>,
    pub is_group: bool,
    pub latest_message: Option<Message>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reaction {
    pub id: String,           // ROWID of the reaction message
    pub message_id: String,   // ROWID of the target message
    pub message_guid: String, // Raw associated_message_guid
    pub sender: String,
    pub reaction_type: ReactionType,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReactionType {
    Love,
    ThumbsUp,
    ThumbsDown,
    HaHa,
    Exclamation,
    Question,
}

impl ReactionType {
    /// Map from chat.db associated_message_type to ReactionType
    pub fn from_associated_type(t: i64) -> Option<Self> {
        match t {
            2000 | 3000 => Some(ReactionType::Love),
            2001 | 3001 => Some(ReactionType::ThumbsUp),
            2002 | 3002 => Some(ReactionType::ThumbsDown),
            2003 | 3003 => Some(ReactionType::HaHa),
            2004 | 3004 => Some(ReactionType::Exclamation),
            2005 | 3005 => Some(ReactionType::Question),
            _ => None,
        }
    }

    /// Whether the associated_message_type is an "add" (2000-2005) vs "remove" (3000-3005)
    pub fn is_removal(associated_type: i64) -> bool {
        (3000..=3005).contains(&associated_type)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum Event {
    #[serde(rename = "message.received")]
    NewMessage(Message),
    #[serde(rename = "message.sent")]
    MessageSent(Message),
    #[serde(rename = "reaction.added")]
    ReactionAdded(Reaction),
    #[serde(rename = "reaction.removed")]
    ReactionRemoved(Reaction),
}

impl Event {
    pub fn event_name(&self) -> &'static str {
        match self {
            Event::NewMessage(_) => "message.received",
            Event::MessageSent(_) => "message.sent",
            Event::ReactionAdded(_) => "reaction.added",
            Event::ReactionRemoved(_) => "reaction.removed",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageRequest {
    pub recipient: String,
    pub body: String,
    #[serde(default)]
    pub attachments: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageQuery {
    pub conversation_id: Option<String>,
    pub since: Option<DateTime<Utc>>,
    #[serde(default = "default_limit")]
    pub limit: u32,
    #[serde(default)]
    pub offset: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginationQuery {
    #[serde(default = "default_limit")]
    pub limit: u32,
    #[serde(default)]
    pub offset: u32,
}

fn default_limit() -> u32 {
    50
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendStatus {
    pub connected: bool,
    pub private_api_available: bool,
    pub message: Option<String>,
}
```

- [ ] **Step 2: Add FeatureUnavailable to BackendError in errors.rs**

Add after `InvalidRequest`:
```rust
    #[error("Feature unavailable: {0}")]
    FeatureUnavailable(String),
```

Add mapping in `IntoResponse` — insert this arm **before** the `BackendError::RequestFailed` arm in the match block:
```rust
            ApiError::Backend(BackendError::FeatureUnavailable(_)) => {
                (StatusCode::NOT_IMPLEMENTED, self.to_string())
            }
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo build`
Expected: Compilation errors in other files referencing old types — that's expected. The types themselves should be valid.

- [ ] **Step 4: Commit**

```bash
git add src/core_layer/types.rs src/core_layer/errors.rs
git commit -m "feat: update core types for direct iMessage — add Event, Reaction, ReactionType"
```

---

### Task 2: Update Backend Trait

**Files:**
- Modify: `src/core_layer/backend.rs`

- [ ] **Step 1: Replace backend.rs with updated trait**

```rust
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
```

- [ ] **Step 2: Commit**

```bash
git add src/core_layer/backend.rs
git commit -m "feat: update MessageBackend trait — add reactions, typing, Event channel"
```

---

### Task 3: Update Webhook Dispatcher for Events

**Files:**
- Modify: `src/core_layer/webhook.rs`

- [ ] **Step 1: Replace webhook.rs to handle Event instead of Message**

```rust
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

        // Dedup by event id (ROWID)
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
            let delivered = self.deliver_with_retry(&webhook.url, &payload).await;
            let status = if delivered { "delivered" } else { "failed" };
            if let Err(e) = self.storage.update_delivery_status(event_id, status) {
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

        if self.try_deliver(url, payload).await {
            return true;
        }

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
```

- [ ] **Step 2: Delete bb_parse.rs and update core_layer/mod.rs**

Delete `src/core_layer/bb_parse.rs`. Update `src/core_layer/mod.rs` to remove `pub mod bb_parse;`.

- [ ] **Step 3: Commit**

```bash
git add src/core_layer/
git commit -m "feat: webhook dispatcher handles Event enum, remove bb_parse"
```

---

### Task 4: Update Config

**Files:**
- Modify: `src/config.rs`

- [ ] **Step 1: Replace config.rs — drop BlueBubbles, add [imessage] section**

```rust
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub auth: AuthConfig,
    pub imessage: IMessageConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub api_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IMessageConfig {
    pub chat_db_path: String,
    pub poll_interval_ms: u64,
    pub private_api: bool,
}

impl Config {
    pub fn config_dir() -> PathBuf {
        dirs::home_dir()
            .expect("Could not determine home directory")
            .join(".aimessage")
    }

    pub fn config_path() -> PathBuf {
        Self::config_dir().join("config.toml")
    }

    pub fn default_chat_db_path() -> String {
        dirs::home_dir()
            .expect("Could not determine home directory")
            .join("Library/Messages/chat.db")
            .to_string_lossy()
            .to_string()
    }

    pub fn load() -> Result<Self, String> {
        let path = Self::config_path();

        if !path.exists() {
            return Err(Self::generate_default(&path));
        }

        let content = fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read config at {}: {}", path.display(), e))?;

        let config: Config = toml::from_str(&content)
            .map_err(|e| format!("Failed to parse config: {}", e))?;

        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<(), String> {
        if self.auth.api_key == "CHANGE_ME" || self.auth.api_key.is_empty() {
            return Err("API key not configured. Edit ~/.aimessage/config.toml".to_string());
        }

        let db_path = Path::new(&self.imessage.chat_db_path);
        if !db_path.exists() {
            return Err(format!(
                "chat.db not found at {}.\n\
                 This usually means Full Disk Access is not granted.\n\
                 Go to: System Settings → Privacy & Security → Full Disk Access\n\
                 Add your terminal or the aimessage binary.",
                self.imessage.chat_db_path
            ));
        }

        Ok(())
    }

    fn generate_default(path: &Path) -> String {
        let api_key = uuid::Uuid::new_v4().to_string();

        let default = Config {
            server: ServerConfig {
                host: "0.0.0.0".to_string(),
                port: 3001,
            },
            auth: AuthConfig {
                api_key: api_key.clone(),
            },
            imessage: IMessageConfig {
                chat_db_path: Self::default_chat_db_path(),
                poll_interval_ms: 1000,
                private_api: false,
            },
        };

        let dir = path.parent().unwrap();
        fs::create_dir_all(dir).expect("Failed to create config directory");
        let content = toml::to_string_pretty(&default).unwrap();
        fs::write(path, &content).expect("Failed to write default config");

        format!(
            "Generated default config at {}.\nYour API key: {}\nThe server will auto-detect your iMessage database. Just restart.",
            path.display(),
            api_key
        )
    }
}
```

- [ ] **Step 2: Commit**

```bash
git add src/config.rs
git commit -m "feat: config drops BlueBubbles, adds [imessage] section with auto-detect"
```

---

### Task 5: Update Storage — Add State Table

**Files:**
- Modify: `src/storage/sqlite.rs`

- [ ] **Step 1: Add state table to migrations and state get/set methods**

Add to `run_migrations` after the existing tables:
```sql
CREATE TABLE IF NOT EXISTS state (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
```

Rename `backend_message_id` to `imessage_rowid` in the `message_log` table creation. Since this is a dev database, just drop and recreate — update the migration SQL.

Add these methods to `Storage`:
```rust
    pub fn get_state(&self, key: &str) -> Result<Option<String>, String> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT value FROM state WHERE key = ?1",
            params![key],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| format!("Failed to get state: {}", e))
    }

    pub fn set_state(&self, key: &str, value: &str) -> Result<(), String> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO state (key, value) VALUES (?1, ?2)",
            params![key, value],
        )
        .map_err(|e| format!("Failed to set state: {}", e))?;
        Ok(())
    }

    pub fn get_last_rowid(&self) -> Result<i64, String> {
        self.get_state("last_processed_rowid")
            .map(|v| v.and_then(|s| s.parse().ok()).unwrap_or(0))
    }

    pub fn set_last_rowid(&self, rowid: i64) -> Result<(), String> {
        self.set_state("last_processed_rowid", &rowid.to_string())
    }
```

Note: `OptionalExtension` is already imported at the top of the `chatdb.rs` code block above. Ensure it's also added to `storage/sqlite.rs` for the `get_state` method's `.optional()` call.

Also rename `backend_message_id` to `imessage_rowid` in the migration and update these existing methods:

In `log_message`, change the SQL to:
```sql
INSERT OR IGNORE INTO message_log (imessage_rowid, conversation_id) VALUES (?1, ?2)
```

In `update_delivery_status`, change the SQL to:
```sql
UPDATE message_log SET webhook_delivery_status = ?1 WHERE imessage_rowid = ?2
```

- [ ] **Step 2: Commit**

```bash
git add src/storage/sqlite.rs
git commit -m "feat: add state table for ROWID tracking, rename to imessage_rowid"
```

---

### Task 6: Create iMessage chat.db Reader

**Files:**
- Create: `src/imessage/mod.rs`
- Create: `src/imessage/chatdb.rs`

- [ ] **Step 1: Create imessage/mod.rs**

```rust
pub mod applescript;
pub mod backend;
pub mod chatdb;
pub mod private_api;
```

- [ ] **Step 2: Create chatdb.rs — the chat.db reader and poller**

This is the core of the new backend. Key responsibilities:
- Open chat.db in read-only mode
- Query messages, conversations, handles
- Convert Mac epoch timestamps
- Detect reactions via associated_message_type
- Poll for new messages by ROWID

```rust
use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use std::path::Path;

use crate::core_layer::types::*;

/// Mac epoch offset: seconds between Unix epoch (1970) and Mac epoch (2001)
const MAC_EPOCH_OFFSET: i64 = 978_307_200;
/// Threshold for detecting nanosecond timestamps
const NANO_THRESHOLD: i64 = 1_000_000_000_000;

pub struct ChatDb {
    conn: Connection,
}

impl ChatDb {
    pub fn open(path: &Path) -> Result<Self, String> {
        let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|e| format!("Failed to open chat.db: {}", e))?;
        Ok(ChatDb { conn })
    }

    /// Convert a Mac absolute time to UTC DateTime
    fn mac_to_utc(mac_time: i64) -> chrono::DateTime<chrono::Utc> {
        let seconds = if mac_time > NANO_THRESHOLD {
            mac_time / 1_000_000_000
        } else {
            mac_time
        };
        let unix_ts = seconds + MAC_EPOCH_OFFSET;
        chrono::DateTime::from_timestamp(unix_ts, 0).unwrap_or_else(chrono::Utc::now)
    }

    /// Get all new messages and reactions since the given ROWID
    pub fn poll_new_events(&self, since_rowid: i64) -> Result<(Vec<Event>, i64), String> {
        let mut events = Vec::new();
        let mut max_rowid = since_rowid;

        let mut stmt = self.conn.prepare(
            "SELECT m.ROWID, m.guid, m.text, m.handle_id, m.date, m.is_from_me,
                    m.associated_message_guid, m.associated_message_type,
                    m.cache_has_attachments,
                    h.id as handle_address,
                    c.guid as chat_guid, c.display_name, c.chat_identifier
             FROM message m
             LEFT JOIN handle h ON m.handle_id = h.ROWID
             LEFT JOIN chat_message_join cmj ON m.ROWID = cmj.message_id
             LEFT JOIN chat c ON cmj.chat_id = c.ROWID
             WHERE m.ROWID > ?1
             ORDER BY m.ROWID ASC"
        ).map_err(|e| format!("Failed to prepare poll query: {}", e))?;

        let rows = stmt.query_map(params![since_rowid], |row| {
            let rowid: i64 = row.get(0)?;
            let guid: String = row.get(1)?;
            let text: Option<String> = row.get(2)?;
            let _handle_id: Option<i64> = row.get(3)?;
            let date: i64 = row.get(4)?;
            let is_from_me: bool = row.get(5)?;
            let assoc_guid: Option<String> = row.get(6)?;
            let assoc_type: i64 = row.get::<_, Option<i64>>(7)?.unwrap_or(0);
            let _has_attachments: bool = row.get::<_, Option<bool>>(8)?.unwrap_or(false);
            let handle_address: Option<String> = row.get(9)?;
            let chat_guid: Option<String> = row.get(10)?;
            let _display_name: Option<String> = row.get(11)?;
            let _chat_identifier: Option<String> = row.get(12)?;

            Ok((rowid, guid, text, date, is_from_me, assoc_guid, assoc_type,
                handle_address, chat_guid))
        }).map_err(|e| format!("Failed to poll: {}", e))?;

        for row in rows {
            let (rowid, guid, text, date, is_from_me, assoc_guid, assoc_type,
                 handle_address, chat_guid) = row.map_err(|e| format!("Row error: {}", e))?;

            if rowid > max_rowid {
                max_rowid = rowid;
            }

            let timestamp = Self::mac_to_utc(date);
            let sender = if is_from_me {
                "me".to_string()
            } else {
                handle_address.unwrap_or_default()
            };
            let conv_id = chat_guid.unwrap_or_default();

            if assoc_type >= 2000 && assoc_type <= 3005 {
                // This is a reaction
                if let Some(reaction_type) = ReactionType::from_associated_type(assoc_type) {
                    let target_guid = assoc_guid.unwrap_or_default();
                    // Look up the target message's ROWID by its guid
                    let target_rowid = self.rowid_for_guid(&target_guid)
                        .unwrap_or_default()
                        .map(|r| r.to_string())
                        .unwrap_or_default();

                    let reaction = Reaction {
                        id: rowid.to_string(),
                        message_id: target_rowid,
                        message_guid: target_guid,
                        sender,
                        reaction_type,
                        timestamp,
                    };

                    if ReactionType::is_removal(assoc_type) {
                        events.push(Event::ReactionRemoved(reaction));
                    } else {
                        events.push(Event::ReactionAdded(reaction));
                    }
                }
            } else {
                // Regular message
                let message = Message {
                    id: rowid.to_string(),
                    guid,
                    conversation_id: conv_id,
                    sender,
                    body: text.unwrap_or_default(),
                    attachments: vec![], // TODO: attachment support post-MVP
                    timestamp,
                    is_from_me,
                    status: if is_from_me { MessageStatus::Sent } else { MessageStatus::Delivered },
                };

                if is_from_me {
                    events.push(Event::MessageSent(message));
                } else {
                    events.push(Event::NewMessage(message));
                }
            }
        }

        Ok((events, max_rowid))
    }

    /// Look up a message ROWID by its guid
    fn rowid_for_guid(&self, guid: &str) -> Result<Option<i64>, String> {
        // associated_message_guid has format "p:0/<guid>" — extract the guid part
        let clean_guid = if let Some(pos) = guid.rfind('/') {
            &guid[pos + 1..]
        } else {
            guid
        };

        self.conn.query_row(
            "SELECT ROWID FROM message WHERE guid = ?1",
            params![clean_guid],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| format!("Failed to look up guid: {}", e))
    }

    /// Get messages with query parameters (for GET /api/v1/messages)
    pub fn get_messages(&self, query: &MessageQuery) -> Result<Vec<Message>, String> {
        let mut sql = String::from(
            "SELECT m.ROWID, m.guid, m.text, m.date, m.is_from_me,
                    h.id as handle_address, c.guid as chat_guid
             FROM message m
             LEFT JOIN handle h ON m.handle_id = h.ROWID
             LEFT JOIN chat_message_join cmj ON m.ROWID = cmj.message_id
             LEFT JOIN chat c ON cmj.chat_id = c.ROWID
             WHERE m.associated_message_type = 0"
        );

        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        let mut param_idx = 1;

        if let Some(ref conv_id) = query.conversation_id {
            sql.push_str(&format!(" AND c.guid = ?{}", param_idx));
            param_values.push(Box::new(conv_id.clone()));
            param_idx += 1;
        }

        if let Some(since) = query.since {
            let mac_time = since.timestamp() - MAC_EPOCH_OFFSET;
            sql.push_str(&format!(" AND m.date > ?{}", param_idx));
            param_values.push(Box::new(mac_time));
            param_idx += 1;
        }

        sql.push_str(" ORDER BY m.ROWID DESC");
        sql.push_str(&format!(" LIMIT ?{}", param_idx));
        param_values.push(Box::new(query.limit));
        param_idx += 1;
        sql.push_str(&format!(" OFFSET ?{}", param_idx));
        param_values.push(Box::new(query.offset));

        let params_ref: Vec<&dyn rusqlite::types::ToSql> = param_values.iter().map(|p| p.as_ref()).collect();

        let mut stmt = self.conn.prepare(&sql)
            .map_err(|e| format!("Failed to prepare query: {}", e))?;

        let rows = stmt.query_map(params_ref.as_slice(), |row| {
            let rowid: i64 = row.get(0)?;
            let guid: String = row.get(1)?;
            let text: Option<String> = row.get(2)?;
            let date: i64 = row.get(3)?;
            let is_from_me: bool = row.get(4)?;
            let handle: Option<String> = row.get(5)?;
            let chat_guid: Option<String> = row.get(6)?;

            Ok(Message {
                id: rowid.to_string(),
                guid,
                conversation_id: chat_guid.unwrap_or_default(),
                sender: if is_from_me { "me".to_string() } else { handle.unwrap_or_default() },
                body: text.unwrap_or_default(),
                attachments: vec![],
                timestamp: Self::mac_to_utc(date),
                is_from_me,
                status: if is_from_me { MessageStatus::Sent } else { MessageStatus::Delivered },
            })
        }).map_err(|e| format!("Failed to query messages: {}", e))?;

        let mut messages = Vec::new();
        for row in rows {
            messages.push(row.map_err(|e| format!("Row error: {}", e))?);
        }
        Ok(messages)
    }

    /// Get a single message by ROWID
    pub fn get_message(&self, rowid: &str) -> Result<Message, String> {
        let rowid_int: i64 = rowid.parse().map_err(|_| format!("Invalid message id: {}", rowid))?;

        self.conn.query_row(
            "SELECT m.ROWID, m.guid, m.text, m.date, m.is_from_me,
                    h.id as handle_address, c.guid as chat_guid
             FROM message m
             LEFT JOIN handle h ON m.handle_id = h.ROWID
             LEFT JOIN chat_message_join cmj ON m.ROWID = cmj.message_id
             LEFT JOIN chat c ON cmj.chat_id = c.ROWID
             WHERE m.ROWID = ?1",
            params![rowid_int],
            |row| {
                let guid: String = row.get(1)?;
                let text: Option<String> = row.get(2)?;
                let date: i64 = row.get(3)?;
                let is_from_me: bool = row.get(4)?;
                let handle: Option<String> = row.get(5)?;
                let chat_guid: Option<String> = row.get(6)?;

                Ok(Message {
                    id: rowid.to_string(),
                    guid,
                    conversation_id: chat_guid.unwrap_or_default(),
                    sender: if is_from_me { "me".to_string() } else { handle.unwrap_or_default() },
                    body: text.unwrap_or_default(),
                    attachments: vec![],
                    timestamp: Self::mac_to_utc(date),
                    is_from_me,
                    status: if is_from_me { MessageStatus::Sent } else { MessageStatus::Delivered },
                })
            }
        ).map_err(|e| format!("Message not found: {}", e))
    }

    /// Get the guid for a message by ROWID
    pub fn guid_for_rowid(&self, rowid: &str) -> Result<String, String> {
        let rowid_int: i64 = rowid.parse().map_err(|_| format!("Invalid message id: {}", rowid))?;
        self.conn.query_row(
            "SELECT guid FROM message WHERE ROWID = ?1",
            params![rowid_int],
            |row| row.get(0),
        ).map_err(|e| format!("Message not found: {}", e))
    }

    /// Get conversations
    pub fn get_conversations(&self, query: &PaginationQuery) -> Result<Vec<Conversation>, String> {
        let mut stmt = self.conn.prepare(
            "SELECT c.guid, c.display_name, c.chat_identifier
             FROM chat c
             ORDER BY c.ROWID DESC
             LIMIT ?1 OFFSET ?2"
        ).map_err(|e| format!("Failed to prepare query: {}", e))?;

        let rows = stmt.query_map(params![query.limit, query.offset], |row| {
            let guid: String = row.get(0)?;
            let display_name: Option<String> = row.get(1)?;
            let _chat_identifier: Option<String> = row.get(2)?;
            Ok(guid)
        }).map_err(|e| format!("Failed to query conversations: {}", e))?;

        let mut conversations = Vec::new();
        for row in rows {
            let guid = row.map_err(|e| format!("Row error: {}", e))?;
            let participants = self.get_chat_participants(&guid)?;
            let is_group = participants.len() > 1;
            conversations.push(Conversation {
                id: guid,
                participants,
                display_name,
                is_group,
                latest_message: None, // Could populate via subquery, skip for now
            });
        }
        Ok(conversations)
    }

    /// Get a single conversation by chat guid
    pub fn get_conversation(&self, chat_guid: &str) -> Result<Conversation, String> {
        self.conn.query_row(
            "SELECT guid, display_name FROM chat WHERE guid = ?1",
            params![chat_guid],
            |row| {
                let guid: String = row.get(0)?;
                let display_name: Option<String> = row.get(1)?;
                Ok((guid, display_name))
            }
        ).map_err(|e| format!("Conversation not found: {}", e))
        .and_then(|(guid, display_name)| {
            let participants = self.get_chat_participants(&guid)?;
            let is_group = participants.len() > 1;
            Ok(Conversation {
                id: guid,
                participants,
                display_name,
                is_group,
                latest_message: None,
            })
        })
    }

    fn get_chat_participants(&self, chat_guid: &str) -> Result<Vec<String>, String> {
        let mut stmt = self.conn.prepare(
            "SELECT h.id FROM handle h
             JOIN chat_handle_join chj ON h.ROWID = chj.handle_id
             JOIN chat c ON chj.chat_id = c.ROWID
             WHERE c.guid = ?1"
        ).map_err(|e| format!("Failed to get participants: {}", e))?;

        let rows = stmt.query_map(params![chat_guid], |row| {
            row.get::<_, String>(0)
        }).map_err(|e| format!("Failed to read participants: {}", e))?;

        let mut participants = Vec::new();
        for row in rows {
            participants.push(row.map_err(|e| format!("Row error: {}", e))?);
        }
        Ok(participants)
    }

    /// Find a recently sent message matching recipient + body (for post-send lookup)
    pub fn find_sent_message(&self, recipient: &str, body: &str) -> Result<Option<Message>, String> {
        // Look for a recent is_from_me=1 message to this recipient with matching body
        let result = self.conn.query_row(
            "SELECT m.ROWID, m.guid, m.text, m.date,
                    c.guid as chat_guid
             FROM message m
             LEFT JOIN chat_message_join cmj ON m.ROWID = cmj.message_id
             LEFT JOIN chat c ON cmj.chat_id = c.ROWID
             LEFT JOIN chat_handle_join chj ON c.ROWID = chj.chat_id
             LEFT JOIN handle h ON chj.handle_id = h.ROWID
             WHERE m.is_from_me = 1 AND m.text = ?1 AND h.id = ?2
             ORDER BY m.ROWID DESC LIMIT 1",
            params![body, recipient],
            |row| {
                let rowid: i64 = row.get(0)?;
                let guid: String = row.get(1)?;
                let text: Option<String> = row.get(2)?;
                let date: i64 = row.get(3)?;
                let chat_guid: Option<String> = row.get(4)?;

                Ok(Message {
                    id: rowid.to_string(),
                    guid,
                    conversation_id: chat_guid.unwrap_or_default(),
                    sender: "me".to_string(),
                    body: text.unwrap_or_default(),
                    attachments: vec![],
                    timestamp: Self::mac_to_utc(date),
                    is_from_me: true,
                    status: MessageStatus::Sent,
                })
            }
        );

        match result {
            Ok(msg) => Ok(Some(msg)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(format!("Failed to find sent message: {}", e)),
        }
    }

    /// Get the current highest ROWID in chat.db
    pub fn get_max_rowid(&self) -> Result<i64, String> {
        self.conn.query_row(
            "SELECT MAX(ROWID) FROM message",
            [],
            |row| row.get::<_, Option<i64>>(0).map(|v| v.unwrap_or(0)),
        ).map_err(|e| format!("Failed to get max ROWID: {}", e))
    }
}
```

- [ ] **Step 3: Verify the file compiles in isolation**

Run: `cargo build`
Expected: Will fail on missing modules, but chatdb.rs itself should be syntactically valid.

- [ ] **Step 4: Commit**

```bash
git add src/imessage/
git commit -m "feat: add chat.db reader with message/reaction polling and query support"
```

---

### Task 7: Create AppleScript Sender

**Files:**
- Create: `src/imessage/applescript.rs`

- [ ] **Step 1: Write AppleScript sender**

```rust
use std::process::Command;
use std::time::Duration;

/// Send a plain text message via AppleScript
pub fn send_message(recipient: &str, body: &str) -> Result<(), String> {
    // Escape single quotes and backslashes for AppleScript string
    let escaped_body = body.replace('\\', "\\\\").replace('"', "\\\"");
    let escaped_recipient = recipient.replace('\\', "\\\\").replace('"', "\\\"");

    let script = format!(
        r#"tell application "Messages"
    set targetService to 1st service whose service type = iMessage
    set targetBuddy to buddy "{}" of targetService
    send "{}" to targetBuddy
end tell"#,
        escaped_recipient, escaped_body
    );

    let output = Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .timeout(Duration::from_secs(10))
        .output()
        .map_err(|e| format!("Failed to run osascript: {}", e))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("AppleScript failed: {}", stderr.trim()))
    }
}

/// Check if Messages.app is reachable via AppleScript
pub fn check_automation_permission() -> Result<(), String> {
    let output = Command::new("osascript")
        .arg("-e")
        .arg(r#"tell application "Messages" to count of chats"#)
        .timeout(Duration::from_secs(5))
        .output()
        .map_err(|e| format!("Failed to run osascript: {}", e))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!(
            "Cannot reach Messages.app via AppleScript.\n\
             Error: {}\n\
             Go to: System Settings → Privacy & Security → Automation\n\
             Grant access for your terminal or the aimessage binary.",
            stderr.trim()
        ))
    }
}
```

**Important:** The code above uses `Command::timeout()` which does not exist on stable Rust. The implementer MUST replace it with `tokio::process::Command` + `tokio::time::timeout`:

```rust
use tokio::process::Command as TokioCommand;
use tokio::time::timeout;
use std::time::Duration;

pub async fn send_message(recipient: &str, body: &str) -> Result<(), String> {
    // ... escape body/recipient same as above ...
    let output = timeout(
        Duration::from_secs(10),
        TokioCommand::new("osascript").arg("-e").arg(&script).output()
    ).await
    .map_err(|_| "AppleScript timed out after 10 seconds".to_string())?
    .map_err(|e| format!("Failed to run osascript: {}", e))?;
    // ... check output.status same as above ...
}
```

Similarly for `check_automation_permission`. Both functions become `async`.

- [ ] **Step 2: Commit**

```bash
git add src/imessage/applescript.rs
git commit -m "feat: add AppleScript sender for iMessage"
```

---

### Task 8: Create Private API Stub

**Files:**
- Create: `src/imessage/private_api.rs`

- [ ] **Step 1: Write Private API availability check and stubs**

For MVP, the Private API is a stub that checks availability but does not actually load IMCore. The real implementation will come later.

```rust
use crate::core_layer::errors::BackendError;
use crate::core_layer::types::ReactionType;

pub struct PrivateApi {
    available: bool,
}

impl PrivateApi {
    pub fn new(enabled: bool) -> Self {
        let available = if enabled {
            Self::check_availability()
        } else {
            false
        };

        if enabled && !available {
            tracing::warn!("Private API enabled in config but not available. SIP may not be disabled.");
        } else if available {
            tracing::info!("Private API available — reactions and typing indicators enabled");
        }

        PrivateApi { available }
    }

    pub fn is_available(&self) -> bool {
        self.available
    }

    fn check_availability() -> bool {
        // Check if SIP is disabled by running csrutil status
        let output = std::process::Command::new("csrutil")
            .arg("status")
            .output();

        match output {
            Ok(o) => {
                let stdout = String::from_utf8_lossy(&o.stdout);
                stdout.contains("disabled")
            }
            Err(_) => false,
        }
    }

    pub fn send_reaction(&self, _message_guid: &str, _reaction: &ReactionType) -> Result<(), BackendError> {
        if !self.available {
            return Err(BackendError::FeatureUnavailable(
                "Private API not available. Disable SIP and set private_api = true in config.".to_string(),
            ));
        }
        // TODO: Implement IMCore dylib loading and reaction sending
        Err(BackendError::FeatureUnavailable(
            "Private API reaction sending not yet implemented.".to_string(),
        ))
    }

    pub fn send_typing(&self, _chat_guid: &str) -> Result<(), BackendError> {
        if !self.available {
            return Err(BackendError::FeatureUnavailable(
                "Private API not available. Disable SIP and set private_api = true in config.".to_string(),
            ));
        }
        // TODO: Implement IMCore dylib loading and typing indicator
        Err(BackendError::FeatureUnavailable(
            "Private API typing indicator not yet implemented.".to_string(),
        ))
    }
}
```

- [ ] **Step 2: Commit**

```bash
git add src/imessage/private_api.rs
git commit -m "feat: add Private API stub with SIP availability check"
```

---

### Task 9: Create IMessage Backend

**Files:**
- Create: `src/imessage/backend.rs`

- [ ] **Step 1: Write IMessageBackend implementing MessageBackend trait**

This ties chatdb, applescript, and private_api together:

```rust
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::mpsc;
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

        // Send via AppleScript (blocking, so spawn_blocking)
        let send_recipient = recipient.clone();
        let send_body = body.clone();
        tokio::task::spawn_blocking(move || {
            applescript::send_message(&send_recipient, &send_body)
        })
        .await
        .map_err(|e| BackendError::RequestFailed(format!("Task join error: {}", e)))?
        .map_err(|e| BackendError::RequestFailed(e))?;

        // Poll chat.db for the sent message (up to 3 seconds)
        let db_path = self.config.chat_db_path.clone();
        let poll_body = body.clone();
        let poll_recipient = recipient.clone();
        let result = tokio::task::spawn_blocking(move || {
            let chatdb = ChatDb::open(std::path::Path::new(&db_path))
                .map_err(|e| BackendError::Unavailable(e))?;

            for _ in 0..15 {
                std::thread::sleep(std::time::Duration::from_millis(200));
                if let Ok(Some(msg)) = chatdb.find_sent_message(&poll_recipient, &poll_body) {
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
            let chatdb = ChatDb::open(std::path::Path::new(&db_path))
                .map_err(|e| BackendError::Unavailable(e))?;
            chatdb.guid_for_rowid(&mid).map_err(|e| BackendError::NotFound(e))
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
            let chatdb = ChatDb::open(std::path::Path::new(&db_path))
                .map_err(|e| BackendError::Unavailable(e))?;
            chatdb.get_messages(&query).map_err(|e| BackendError::RequestFailed(e))
        })
        .await
        .map_err(|e| BackendError::RequestFailed(format!("Task join error: {}", e)))?
    }

    async fn get_message(&self, id: &str) -> Result<Message, BackendError> {
        let db_path = self.config.chat_db_path.clone();
        let id = id.to_string();
        tokio::task::spawn_blocking(move || {
            let chatdb = ChatDb::open(std::path::Path::new(&db_path))
                .map_err(|e| BackendError::Unavailable(e))?;
            chatdb.get_message(&id).map_err(|e| BackendError::NotFound(e))
        })
        .await
        .map_err(|e| BackendError::RequestFailed(format!("Task join error: {}", e)))?
    }

    async fn get_conversations(&self, query: PaginationQuery) -> Result<Vec<Conversation>, BackendError> {
        let db_path = self.config.chat_db_path.clone();
        tokio::task::spawn_blocking(move || {
            let chatdb = ChatDb::open(std::path::Path::new(&db_path))
                .map_err(|e| BackendError::Unavailable(e))?;
            chatdb.get_conversations(&query).map_err(|e| BackendError::RequestFailed(e))
        })
        .await
        .map_err(|e| BackendError::RequestFailed(format!("Task join error: {}", e)))?
    }

    async fn get_conversation(&self, id: &str) -> Result<Conversation, BackendError> {
        let db_path = self.config.chat_db_path.clone();
        let id = id.to_string();
        tokio::task::spawn_blocking(move || {
            let chatdb = ChatDb::open(std::path::Path::new(&db_path))
                .map_err(|e| BackendError::Unavailable(e))?;
            chatdb.get_conversation(&id).map_err(|e| BackendError::NotFound(e))
        })
        .await
        .map_err(|e| BackendError::RequestFailed(format!("Task join error: {}", e)))?
    }

    async fn start(&self) -> Result<mpsc::Receiver<Event>, BackendError> {
        let (sender, receiver) = mpsc::channel(256);
        let db_path = self.config.chat_db_path.clone();
        let poll_interval = std::time::Duration::from_millis(self.config.poll_interval_ms);
        let storage = self.storage.clone();

        // Get starting ROWID from state table (resume after restart)
        let start_rowid = storage.get_last_rowid()
            .map_err(|e| BackendError::RequestFailed(e))?;

        // If no saved state, start from current max to avoid replaying entire history
        let start_rowid = if start_rowid == 0 {
            let chatdb = ChatDb::open(std::path::Path::new(&db_path))
                .map_err(|e| BackendError::Unavailable(e))?;
            chatdb.get_max_rowid().map_err(|e| BackendError::RequestFailed(e))?
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
                    let chatdb = ChatDb::open(std::path::Path::new(&db_path_clone))?;
                    chatdb.poll_new_events(current_rowid)
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
                            if sender.send(event).await.is_err() {
                                tracing::error!("Event channel closed, stopping poller");
                                return;
                            }
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

        Ok(receiver)
    }

    async fn shutdown(&self) -> Result<(), BackendError> {
        Ok(())
    }

    async fn health_check(&self) -> Result<BackendStatus, BackendError> {
        // Check chat.db is readable
        let db_path = self.config.chat_db_path.clone();
        let connected = tokio::task::spawn_blocking(move || {
            ChatDb::open(std::path::Path::new(&db_path)).is_ok()
        }).await.unwrap_or(false);

        Ok(BackendStatus {
            connected,
            private_api_available: self.private_api.is_available(),
            message: if connected { None } else { Some("Cannot read chat.db".to_string()) },
        })
    }
}
```

- [ ] **Step 2: Commit**

```bash
git add src/imessage/backend.rs
git commit -m "feat: add IMessageBackend — ties chatdb, applescript, and private_api together"
```

---

### Task 10: Update API Layer

**Files:**
- Modify: `src/api/handlers.rs`
- Modify: `src/api/routes.rs`
- Modify: `src/api/types.rs`

- [ ] **Step 1: Update types.rs — add reaction/typing request types, update health response**

Add these types:
```rust
#[derive(Debug, Deserialize)]
pub struct SendReactionBody {
    pub reaction: String,  // "love", "thumbsup", etc.
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub backend: BackendHealthResponse,
}

#[derive(Debug, Serialize)]
pub struct BackendHealthResponse {
    pub connected: bool,
    pub private_api_available: bool,
    pub message: Option<String>,
}
```

(Replace the existing `BackendHealthResponse` — it now has `private_api_available` instead of `backend_type`.)

- [ ] **Step 2: Update handlers.rs — add reaction/typing handlers, remove bb_webhook_handler**

Add `send_reaction` handler:
```rust
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
```

Add `send_typing` handler:
```rust
pub async fn send_typing(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state.backend.send_typing(&id).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}
```

Update `health` handler to use new `BackendHealthResponse` with `private_api_available`.

Remove `bb_webhook_handler` entirely.

- [ ] **Step 3: Update routes.rs — add new routes, remove internal BB webhook**

```rust
pub fn build_router(state: Arc<AppState>, api_key: String) -> Router {
    let authed_routes = Router::new()
        .route("/messages", post(handlers::send_message))
        .route("/messages", get(handlers::list_messages))
        .route("/messages/{id}", get(handlers::get_message))
        .route("/messages/{id}/react", post(handlers::send_reaction))
        .route("/conversations", get(handlers::list_conversations))
        .route("/conversations/{id}", get(handlers::get_conversation))
        .route("/conversations/{id}/typing", post(handlers::send_typing))
        .route("/webhooks", post(handlers::create_webhook))
        .route("/webhooks", get(handlers::list_webhooks))
        .route("/webhooks/{id}", delete(handlers::delete_webhook))
        .layer(middleware::from_fn(require_api_key))
        .layer(axum::Extension(ApiKey(api_key)));

    let public_routes = Router::new()
        .route("/health", get(handlers::health));

    Router::new()
        .nest("/api/v1", authed_routes.merge(public_routes))
        .with_state(state)
}
```

No more `/internal` routes.

- [ ] **Step 4: Commit**

```bash
git add src/api/
git commit -m "feat: add reaction/typing API endpoints, remove BB webhook endpoint"
```

---

### Task 11: Update main.rs and Clean Up

**Files:**
- Modify: `src/main.rs`
- Delete: `src/backends/bluebubbles.rs`
- Delete: `src/backends/mod.rs`

- [ ] **Step 1: Delete backends/ directory**

Remove `src/backends/` entirely.

- [ ] **Step 2: Replace main.rs**

```rust
mod api;
mod config;
mod core_layer;
mod imessage;
mod storage;

use std::sync::Arc;

use api::handlers::AppState;
use core_layer::backend::MessageBackend;
use core_layer::webhook::WebhookDispatcher;
use imessage::backend::IMessageBackend;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(
            EnvFilter::from_default_env()
                .add_directive("aimessage=info".parse().unwrap()),
        )
        .init();

    let config = match config::Config::load() {
        Ok(c) => c,
        Err(msg) => {
            eprintln!("{}", msg);
            std::process::exit(1);
        }
    };

    tracing::info!(
        host = %config.server.host,
        port = %config.server.port,
        "Config loaded"
    );

    // Init storage
    let db_path = config::Config::config_dir().join("aimessage.db");
    let storage = Arc::new(
        storage::sqlite::Storage::new(&db_path).expect("Failed to initialize database"),
    );
    tracing::info!(path = %db_path.display(), "Database initialized");

    // Check Automation permission (spec requirement: verify on startup)
    if let Err(e) = imessage::applescript::check_automation_permission().await {
        eprintln!("{}", e);
        std::process::exit(1);
    }
    tracing::info!("Automation permission verified");

    // Init iMessage backend
    let backend = Arc::new(IMessageBackend::new(
        config.imessage.clone(),
        storage.clone(),
    ));

    // Start backend — begins polling chat.db
    let receiver = backend
        .start()
        .await
        .expect("Failed to start iMessage backend");

    // Start webhook dispatcher
    let dispatcher = WebhookDispatcher::new(storage.clone());
    dispatcher.spawn(receiver);

    // Build app state and router
    let state = Arc::new(AppState {
        backend: backend as Arc<dyn MessageBackend>,
        storage: storage.clone(),
    });

    let app = api::routes::build_router(state, config.auth.api_key);

    let addr = format!("{}:{}", config.server.host, config.server.port);
    tracing::info!(addr = %addr, "Server starting");

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind");

    axum::serve(listener, app)
        .await
        .expect("Server error");
}
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo build`
Expected: Compiles. May have warnings. Fix any errors.

- [ ] **Step 4: Run clippy**

Run: `cargo clippy`
Expected: Fix any warnings.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: replace BlueBubbles with direct iMessage backend — single binary, zero deps"
```

---

### Task 12: Smoke Test

**Files:**
- None (manual verification)

- [ ] **Step 1: Delete old config to force regeneration**

Run: `rm ~/.aimessage/config.toml ~/.aimessage/aimessage.db`

- [ ] **Step 2: Run the server**

Run: `cargo run`
Expected: Generates new config with `[imessage]` section. If chat.db is accessible (Full Disk Access granted), server starts and begins polling. If not, prints clear permission instructions.

- [ ] **Step 3: Test health endpoint**

Run: `curl -s http://localhost:3001/api/v1/health | python3 -m json.tool`
Expected: `{"status":"ok","backend":{"connected":true,"private_api_available":false}}`

- [ ] **Step 4: Test auth**

Run: `curl -s -o /dev/null -w "%{http_code}" http://localhost:3001/api/v1/messages`
Expected: `401`

- [ ] **Step 5: Test list messages (reads real chat.db)**

Run: `curl -s -H "X-API-Key: <key>" "http://localhost:3001/api/v1/messages?limit=3" | python3 -m json.tool`
Expected: Returns actual iMessage history with real message data.

- [ ] **Step 6: Test list conversations**

Run: `curl -s -H "X-API-Key: <key>" "http://localhost:3001/api/v1/conversations?limit=3" | python3 -m json.tool`
Expected: Returns actual conversations.

- [ ] **Step 7: Test reaction endpoint (should return 501)**

Run: `curl -s -w "\n%{http_code}" -X POST -H "X-API-Key: <key>" -H "Content-Type: application/json" -d '{"reaction":"love"}' http://localhost:3001/api/v1/messages/1/react`
Expected: 501 (Private API not available)

- [ ] **Step 8: Test typing endpoint (should return 501)**

Run: `curl -s -w "\n%{http_code}" -X POST -H "X-API-Key: <key>" http://localhost:3001/api/v1/conversations/test/typing`
Expected: 501

- [ ] **Step 9: Test webhook CRUD still works**

Run webhook create/list/delete as before.

- [ ] **Step 10: Commit any fixes**

```bash
git add -A
git commit -m "fix: smoke test fixes"
```
