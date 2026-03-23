# AiMessage Server Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Rust/Axum REST API server that wraps BlueBubbles to expose iMessage send/receive functionality with webhook delivery.

**Architecture:** Three-layer design — API (Axum HTTP handlers), Core (domain types, `MessageBackend` trait, webhook dispatcher), Backend (BlueBubbles adapter). SQLite for webhook registration and delivery tracking.

**Tech Stack:** Rust, Axum, Tokio, SQLite (via rusqlite), reqwest, serde, toml, tracing, uuid

**Spec:** `docs/superpowers/specs/2026-03-23-aimessage-server-design.md`

---

## File Structure

| File | Responsibility |
|------|---------------|
| `Cargo.toml` | Dependencies and project metadata |
| `src/main.rs` | Entry point: load config, init storage, init backend, build router, start server |
| `src/config.rs` | Parse TOML config, generate default config on first run, validate credentials |
| `src/core_layer/mod.rs` | Re-exports for core module |
| `src/core_layer/types.rs` | Domain types: Message, Conversation, SendMessageRequest, MessageQuery, PaginationQuery, BackendStatus |
| `src/core_layer/errors.rs` | Error types: BackendError, ApiError (Axum IntoResponse impl) |
| `src/core_layer/backend.rs` | `MessageBackend` trait definition |
| `src/core_layer/webhook.rs` | WebhookDispatcher: reads from mpsc::Receiver, POSTs to registered URLs, retries with backoff |
| `src/core_layer/bb_parse.rs` | Shared BlueBubbles JSON → domain type mapping (used by both backend adapter and internal webhook handler) |
| `src/api/mod.rs` | Re-exports for api module |
| `src/api/types.rs` | Request/response DTOs with serde Serialize/Deserialize |
| `src/api/auth.rs` | Axum middleware: extract X-API-Key header, compare to config |
| `src/api/routes.rs` | Axum Router construction, route definitions |
| `src/api/handlers.rs` | Handler functions for each endpoint |
| `src/backends/mod.rs` | Re-exports for backends module |
| `src/backends/bluebubbles.rs` | BlueBubblesBackend: implements MessageBackend, HTTP calls to BB API |
| `src/storage/mod.rs` | Re-exports for storage module |
| `src/storage/sqlite.rs` | SQLite connection, migrations, webhook CRUD, message_log CRUD |

Note: Rust reserves `core` as a crate name, so we use `core_layer` as the module name throughout.

---

### Task 0: Git Init

**Files:**
- Create: `.gitignore`

- [ ] **Step 1: Initialize git repo and create .gitignore**

Run: `cd /Users/msilverblatt/Projects/aimessage && git init`

```
/target
*.db
*.db-journal
```

- [ ] **Step 2: Commit**

```bash
git add .gitignore docs/
git commit -m "chore: initialize repo with design docs"
```

---

### Task 1: Project Scaffolding and Dependencies

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`

- [ ] **Step 1: Initialize Cargo project**

Run: `cd /Users/msilverblatt/Projects/aimessage && cargo init --name aimessage`

Note: This will create `Cargo.toml` and `src/main.rs` in the existing directory. It won't touch existing files like `docs/`.

- [ ] **Step 2: Add dependencies to Cargo.toml**

```toml
[package]
name = "aimessage"
version = "0.1.0"
edition = "2021"

[dependencies]
axum = "0.8"
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"
reqwest = { version = "0.12", features = ["json"] }
rusqlite = { version = "0.32", features = ["bundled"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["json", "env-filter"] }
uuid = { version = "1", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
async-trait = "0.1"
tower-http = { version = "0.6", features = ["trace"] }
rand = "0.9"
dirs = "6"
thiserror = "2"
```

- [ ] **Step 3: Write minimal main.rs that compiles**

```rust
#[tokio::main]
async fn main() {
    println!("aimessage server starting...");
}
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo build`
Expected: Compiles successfully, downloads dependencies

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock src/main.rs
git commit -m "feat: initialize project with dependencies"
```

---

### Task 2: Core Domain Types and Backend Trait

**Files:**
- Create: `src/core_layer/mod.rs`
- Create: `src/core_layer/types.rs`
- Create: `src/core_layer/errors.rs`
- Create: `src/core_layer/backend.rs`
- Create: `src/core_layer/webhook.rs` (stub)
- Create: `src/core_layer/bb_parse.rs` (stub)

- [ ] **Step 1: Create all core_layer files**

`src/core_layer/mod.rs`:
```rust
pub mod backend;
pub mod bb_parse;
pub mod errors;
pub mod types;
pub mod webhook;
```

`src/core_layer/types.rs`:
```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
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
    pub backend_type: String,
    pub message: Option<String>,
}
```

`src/core_layer/errors.rs`:
```rust
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BackendError {
    #[error("Backend unavailable: {0}")]
    Unavailable(String),
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Request failed: {0}")]
    RequestFailed(String),
    #[error("Invalid request: {0}")]
    InvalidRequest(String),
}

#[derive(Debug, Error)]
pub enum ApiError {
    #[error(transparent)]
    Backend(#[from] BackendError),
    #[error("Storage error: {0}")]
    Storage(String),
    #[error("Unauthorized")]
    Unauthorized,
    #[error("Bad request: {0}")]
    BadRequest(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            ApiError::Backend(BackendError::Unavailable(_)) => {
                (StatusCode::SERVICE_UNAVAILABLE, self.to_string())
            }
            ApiError::Backend(BackendError::NotFound(_)) => {
                (StatusCode::NOT_FOUND, self.to_string())
            }
            ApiError::Backend(BackendError::InvalidRequest(_)) => {
                (StatusCode::BAD_REQUEST, self.to_string())
            }
            ApiError::Backend(BackendError::RequestFailed(_)) => {
                (StatusCode::INTERNAL_SERVER_ERROR, self.to_string())
            }
            ApiError::Storage(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            ApiError::Unauthorized => (StatusCode::UNAUTHORIZED, self.to_string()),
            ApiError::BadRequest(_) => (StatusCode::BAD_REQUEST, self.to_string()),
        };

        let body = serde_json::to_string(&json!({ "error": message })).unwrap();
        (status, [("content-type", "application/json")], body).into_response()
    }
}
```

`src/core_layer/backend.rs`:
```rust
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
```

`src/core_layer/webhook.rs` (stub — implemented in Task 8):
```rust
// Webhook dispatcher — implemented in Task 8
```

`src/core_layer/bb_parse.rs` (stub — implemented in Task 7):
```rust
// BlueBubbles JSON parsing utilities — implemented in Task 7
```

- [ ] **Step 2: Add module declaration to main.rs**

```rust
mod core_layer;

#[tokio::main]
async fn main() {
    println!("aimessage server starting...");
}
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo build`
Expected: Compiles successfully

- [ ] **Step 4: Commit**

```bash
git add src/core_layer/ src/main.rs
git commit -m "feat: add core domain types, errors, and backend trait"
```

---

### Task 3: Configuration

**Files:**
- Create: `src/config.rs`

- [ ] **Step 1: Write config parsing and generation**

```rust
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub auth: AuthConfig,
    pub backend: BackendConfig,
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
pub struct BackendConfig {
    #[serde(rename = "type")]
    pub backend_type: String,
    pub bluebubbles: Option<BlueBubblesConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlueBubblesConfig {
    pub url: String,
    pub password: String,
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

        match self.backend.backend_type.as_str() {
            "bluebubbles" => {
                let bb = self.backend.bluebubbles.as_ref().ok_or(
                    "backend.type is 'bluebubbles' but [backend.bluebubbles] section is missing"
                        .to_string(),
                )?;
                if bb.password.is_empty() || bb.password == "CHANGE_ME" {
                    return Err(
                        "BlueBubbles password not configured. Edit ~/.aimessage/config.toml"
                            .to_string(),
                    );
                }
            }
            other => return Err(format!("Unknown backend type: {}", other)),
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
            backend: BackendConfig {
                backend_type: "bluebubbles".to_string(),
                bluebubbles: Some(BlueBubblesConfig {
                    url: "http://localhost:1234".to_string(),
                    password: "CHANGE_ME".to_string(),
                }),
            },
        };

        let dir = path.parent().unwrap();
        fs::create_dir_all(dir).expect("Failed to create config directory");
        let content = toml::to_string_pretty(&default).unwrap();
        fs::write(path, &content).expect("Failed to write default config");

        format!(
            "Generated default config at {}.\nYour API key: {}\nEdit the file to set your BlueBubbles password, then restart.",
            path.display(),
            api_key
        )
    }
}
```

- [ ] **Step 2: Wire config into main.rs**

```rust
mod config;
mod core_layer;

use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(EnvFilter::from_default_env().add_directive("aimessage=info".parse().unwrap()))
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
        backend = %config.backend.backend_type,
        "Config loaded"
    );
}
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo build`
Expected: Compiles successfully

- [ ] **Step 4: Commit**

```bash
git add src/config.rs src/main.rs
git commit -m "feat: add TOML config loading with first-run generation"
```

---

### Task 4: SQLite Storage Layer

**Files:**
- Create: `src/storage/mod.rs`
- Create: `src/storage/sqlite.rs`

- [ ] **Step 1: Create storage/mod.rs**

```rust
pub mod sqlite;
```

- [ ] **Step 2: Write SQLite storage with migrations and webhook CRUD**

Note: Uses `std::sync::Mutex` (not tokio's) intentionally — rusqlite's `Connection` is not `Send`, and none of our Storage methods are async, so `std::sync::Mutex` is correct and avoids holding a lock across `.await` points.

```rust
use rusqlite::{params, Connection};
use std::path::Path;
use std::sync::Mutex;
use uuid::Uuid;

pub struct Storage {
    conn: Mutex<Connection>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WebhookRecord {
    pub id: String,
    pub url: String,
    pub events: Vec<String>,
    pub created_at: String,
}

impl Storage {
    pub fn new(path: &Path) -> Result<Self, String> {
        let conn = Connection::open(path)
            .map_err(|e| format!("Failed to open database: {}", e))?;

        let storage = Storage {
            conn: Mutex::new(conn),
        };
        storage.run_migrations()?;
        Ok(storage)
    }

    fn run_migrations(&self) -> Result<(), String> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS webhooks (
                id TEXT PRIMARY KEY,
                url TEXT NOT NULL,
                events TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE UNIQUE INDEX IF NOT EXISTS idx_webhooks_url ON webhooks(url);

            CREATE TABLE IF NOT EXISTS message_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                backend_message_id TEXT NOT NULL UNIQUE,
                conversation_id TEXT NOT NULL,
                delivered_at TEXT NOT NULL DEFAULT (datetime('now')),
                webhook_delivery_status TEXT NOT NULL DEFAULT 'pending'
            );"
        ).map_err(|e| format!("Migration failed: {}", e))?;
        Ok(())
    }

    pub fn create_or_update_webhook(&self, url: &str, events: &[String]) -> Result<WebhookRecord, String> {
        let conn = self.conn.lock().unwrap();
        let events_json = serde_json::to_string(events).unwrap();

        let existing: Option<String> = conn
            .query_row(
                "SELECT id FROM webhooks WHERE url = ?1",
                params![url],
                |row| row.get(0),
            )
            .ok();

        let id = if let Some(existing_id) = existing {
            conn.execute(
                "UPDATE webhooks SET events = ?1 WHERE id = ?2",
                params![events_json, existing_id],
            )
            .map_err(|e| format!("Failed to update webhook: {}", e))?;
            existing_id
        } else {
            let new_id = Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO webhooks (id, url, events) VALUES (?1, ?2, ?3)",
                params![new_id, url, events_json],
            )
            .map_err(|e| format!("Failed to create webhook: {}", e))?;
            new_id
        };

        // Must drop conn before calling get_webhook (which also locks)
        drop(conn);
        self.get_webhook(&id)
    }

    pub fn get_webhook(&self, id: &str) -> Result<WebhookRecord, String> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT id, url, events, created_at FROM webhooks WHERE id = ?1",
            params![id],
            |row| {
                let events_str: String = row.get(2)?;
                let events: Vec<String> = serde_json::from_str(&events_str).unwrap_or_default();
                Ok(WebhookRecord {
                    id: row.get(0)?,
                    url: row.get(1)?,
                    events,
                    created_at: row.get(3)?,
                })
            },
        )
        .map_err(|e| format!("Webhook not found: {}", e))
    }

    pub fn list_webhooks(&self) -> Result<Vec<WebhookRecord>, String> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT id, url, events, created_at FROM webhooks")
            .map_err(|e| format!("Failed to query webhooks: {}", e))?;

        let rows = stmt
            .query_map([], |row| {
                let events_str: String = row.get(2)?;
                let events: Vec<String> = serde_json::from_str(&events_str).unwrap_or_default();
                Ok(WebhookRecord {
                    id: row.get(0)?,
                    url: row.get(1)?,
                    events,
                    created_at: row.get(3)?,
                })
            })
            .map_err(|e| format!("Failed to read webhooks: {}", e))?;

        let mut webhooks = Vec::new();
        for row in rows {
            webhooks.push(row.map_err(|e| format!("Row error: {}", e))?);
        }
        Ok(webhooks)
    }

    pub fn delete_webhook(&self, id: &str) -> Result<bool, String> {
        let conn = self.conn.lock().unwrap();
        let affected = conn
            .execute("DELETE FROM webhooks WHERE id = ?1", params![id])
            .map_err(|e| format!("Failed to delete webhook: {}", e))?;
        Ok(affected > 0)
    }

    pub fn get_webhooks_for_event(&self, event: &str) -> Result<Vec<WebhookRecord>, String> {
        let all = self.list_webhooks()?;
        Ok(all.into_iter().filter(|w| w.events.contains(&event.to_string())).collect())
    }

    pub fn log_message(
        &self,
        backend_message_id: &str,
        conversation_id: &str,
    ) -> Result<bool, String> {
        let conn = self.conn.lock().unwrap();
        let result = conn.execute(
            "INSERT OR IGNORE INTO message_log (backend_message_id, conversation_id) VALUES (?1, ?2)",
            params![backend_message_id, conversation_id],
        );
        match result {
            Ok(count) => Ok(count > 0),
            Err(e) => Err(format!("Failed to log message: {}", e)),
        }
    }

    pub fn update_delivery_status(
        &self,
        backend_message_id: &str,
        status: &str,
    ) -> Result<(), String> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE message_log SET webhook_delivery_status = ?1 WHERE backend_message_id = ?2",
            params![status, backend_message_id],
        )
        .map_err(|e| format!("Failed to update status: {}", e))?;
        Ok(())
    }
}
```

- [ ] **Step 3: Wire storage into main.rs**

Add `mod storage;` to main.rs and after config loading:
```rust
let db_path = config::Config::config_dir().join("aimessage.db");
let storage = std::sync::Arc::new(
    storage::sqlite::Storage::new(&db_path)
        .expect("Failed to initialize database")
);
tracing::info!(path = %db_path.display(), "Database initialized");
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo build`
Expected: Compiles successfully

- [ ] **Step 5: Commit**

```bash
git add src/storage/ src/main.rs
git commit -m "feat: add SQLite storage layer with webhook and message_log tables"
```

---

### Task 5: API Auth Middleware and Stubs

**Files:**
- Create: `src/api/mod.rs`
- Create: `src/api/auth.rs`
- Create: `src/api/handlers.rs` (stub)
- Create: `src/api/routes.rs` (stub)
- Create: `src/api/types.rs` (stub)

- [ ] **Step 1: Create all api files including stubs**

`src/api/mod.rs`:
```rust
pub mod auth;
pub mod handlers;
pub mod routes;
pub mod types;
```

`src/api/auth.rs`:
```rust
use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::Response,
};

pub async fn require_api_key(
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let expected_key = request
        .extensions()
        .get::<ApiKey>()
        .map(|k| k.0.clone());

    let provided_key = request
        .headers()
        .get("X-API-Key")
        .and_then(|v| v.to_str().ok())
        .map(String::from);

    match (expected_key, provided_key) {
        (Some(expected), Some(provided)) if expected == provided => {
            Ok(next.run(request).await)
        }
        _ => Err(StatusCode::UNAUTHORIZED),
    }
}

#[derive(Clone)]
pub struct ApiKey(pub String);
```

`src/api/handlers.rs` (stub):
```rust
// Handler functions — implemented in Task 6
```

`src/api/routes.rs` (stub):
```rust
// Router construction — implemented in Task 9
```

`src/api/types.rs` (stub):
```rust
// API DTOs — implemented in Task 6
```

- [ ] **Step 2: Add `mod api;` to main.rs**

- [ ] **Step 3: Verify it compiles**

Run: `cargo build`
Expected: Compiles successfully

- [ ] **Step 4: Commit**

```bash
git add src/api/
git commit -m "feat: add API key auth middleware with module stubs"
```

---

### Task 6: API Types and Handlers

**Files:**
- Modify: `src/api/types.rs`
- Modify: `src/api/handlers.rs`

- [ ] **Step 1: Write request/response DTOs in types.rs**

```rust
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
```

- [ ] **Step 2: Write all handler functions in handlers.rs**

```rust
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
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

// Webhooks

pub async fn create_webhook(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateWebhookBody>,
) -> Result<Json<WebhookResponse>, ApiError> {
    let record = state
        .storage
        .create_or_update_webhook(&body.url, &body.events)
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
                backend_type: status.backend_type,
                message: status.message,
            },
        }),
        Err(e) => Json(HealthResponse {
            status: "degraded".to_string(),
            backend: BackendHealthResponse {
                connected: false,
                backend_type: "unknown".to_string(),
                message: Some(e.to_string()),
            },
        }),
    }
}

// Internal: BlueBubbles webhook receiver

pub async fn bb_webhook_handler(
    State(state): State<Arc<AppState>>,
    Json(body): Json<serde_json::Value>,
) -> StatusCode {
    tracing::info!("Received BlueBubbles webhook");

    let event_type = body.get("type").and_then(|t| t.as_str()).unwrap_or("unknown");

    if event_type != "new-message" {
        tracing::debug!(event_type = %event_type, "Ignoring non-message BB webhook");
        return StatusCode::OK;
    }

    let data = match body.get("data") {
        Some(d) => d,
        None => {
            tracing::warn!("BB webhook missing data field");
            return StatusCode::OK;
        }
    };

    let message = crate::core_layer::bb_parse::parse_bb_message(data);
    if let Some(msg) = message {
        state.backend.push_incoming_message(msg).await;
    }

    StatusCode::OK
}
```

Note: `push_incoming_message` will be added to the backend trait in Task 7 — it allows the webhook handler to route through the backend rather than holding a direct Sender reference.

- [ ] **Step 3: Verify it compiles**

Run: `cargo build`
Expected: Will not fully compile yet (references to `push_incoming_message` and `bb_parse` not yet implemented). That's expected — these are wired in Tasks 7-8.

- [ ] **Step 4: Commit**

```bash
git add src/api/types.rs src/api/handlers.rs
git commit -m "feat: add API types and handler functions"
```

---

### Task 7: Shared BlueBubbles Parsing + Backend Push Method

**Files:**
- Modify: `src/core_layer/bb_parse.rs`
- Modify: `src/core_layer/backend.rs`

- [ ] **Step 1: Write shared BB message parser in bb_parse.rs**

```rust
use crate::core_layer::types::{Message, MessageStatus, Conversation};

pub fn parse_bb_message(value: &serde_json::Value) -> Option<Message> {
    let guid = value.get("guid")?.as_str()?;
    let text = value.get("text").and_then(|v| v.as_str()).unwrap_or("");
    let is_from_me = value.get("isFromMe").and_then(|v| v.as_bool()).unwrap_or(false);
    let date_created = value.get("dateCreated").and_then(|v| v.as_i64()).unwrap_or(0);

    let chat_guid = value
        .get("chats")
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.first())
        .and_then(|c| c.get("guid"))
        .and_then(|g| g.as_str())
        .unwrap_or("");

    let handle = value
        .get("handle")
        .and_then(|h| h.get("address"))
        .and_then(|a| a.as_str())
        .unwrap_or("");

    let timestamp = chrono::DateTime::from_timestamp_millis(date_created)
        .unwrap_or_else(chrono::Utc::now);

    Some(Message {
        id: guid.to_string(),
        conversation_id: chat_guid.to_string(),
        sender: if is_from_me { "me".to_string() } else { handle.to_string() },
        body: text.to_string(),
        attachments: vec![],
        timestamp,
        is_from_me,
        status: MessageStatus::Sent,
    })
}

pub fn parse_bb_chat(value: &serde_json::Value) -> Option<Conversation> {
    let guid = value.get("guid")?.as_str()?;
    let display_name = value.get("displayName").and_then(|v| v.as_str()).map(String::from);
    let participants: Vec<String> = value
        .get("participants")
        .and_then(|p| p.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|p| p.get("address").and_then(|a| a.as_str()).map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let is_group = participants.len() > 1;

    Some(Conversation {
        id: guid.to_string(),
        participants,
        display_name,
        is_group,
        latest_message: None,
    })
}
```

- [ ] **Step 2: Add `push_incoming_message` to backend trait**

Add this method to the `MessageBackend` trait in `backend.rs`:
```rust
    /// Push an incoming message into the backend's channel (used by internal webhook handler)
    async fn push_incoming_message(&self, message: Message);
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo build`
Expected: Compiles (trait isn't implemented yet, but stubs are fine)

- [ ] **Step 4: Commit**

```bash
git add src/core_layer/bb_parse.rs src/core_layer/backend.rs
git commit -m "feat: add shared BB parsing utilities and backend push method"
```

---

### Task 8: Webhook Dispatcher

**Files:**
- Modify: `src/core_layer/webhook.rs`

- [ ] **Step 1: Write the webhook dispatcher**

```rust
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
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo build`
Expected: Compiles successfully

- [ ] **Step 3: Commit**

```bash
git add src/core_layer/webhook.rs
git commit -m "feat: add webhook dispatcher with 3-attempt retry"
```

---

### Task 9: BlueBubbles Backend Adapter

**Files:**
- Create: `src/backends/mod.rs`
- Create: `src/backends/bluebubbles.rs`

- [ ] **Step 1: Create backends/mod.rs**

```rust
pub mod bluebubbles;
```

- [ ] **Step 2: Add `mod backends;` to main.rs**

- [ ] **Step 3: Write BlueBubbles adapter**

```rust
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
    /// The URL that BlueBubbles should POST webhook events to
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
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo build`
Expected: Compiles successfully

- [ ] **Step 5: Commit**

```bash
git add src/backends/ src/main.rs
git commit -m "feat: add BlueBubbles backend adapter with webhook registration"
```

---

### Task 10: API Router

**Files:**
- Modify: `src/api/routes.rs`

- [ ] **Step 1: Write router construction**

```rust
use axum::{
    extract::ConnectInfo,
    http::StatusCode,
    middleware,
    routing::{delete, get, post},
    Router,
};
use std::net::SocketAddr;
use std::sync::Arc;

use super::auth::{require_api_key, ApiKey};
use super::handlers::{self, AppState};

pub fn build_router(state: Arc<AppState>, api_key: String) -> Router {
    let authed_routes = Router::new()
        .route("/messages", post(handlers::send_message))
        .route("/messages", get(handlers::list_messages))
        .route("/messages/{id}", get(handlers::get_message))
        .route("/conversations", get(handlers::list_conversations))
        .route("/conversations/{id}", get(handlers::get_conversation))
        .route("/webhooks", post(handlers::create_webhook))
        .route("/webhooks", get(handlers::list_webhooks))
        .route("/webhooks/{id}", delete(handlers::delete_webhook))
        .layer(middleware::from_fn(require_api_key))
        .layer(axum::Extension(ApiKey(api_key)));

    let public_routes = Router::new()
        .route("/health", get(handlers::health));

    let internal_routes = Router::new()
        .route("/bb-webhook", post(handlers::bb_webhook_handler));

    Router::new()
        .nest("/api/v1", authed_routes.merge(public_routes))
        .nest("/internal", internal_routes)
        .with_state(state)
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo build`
Expected: Compiles successfully

- [ ] **Step 3: Commit**

```bash
git add src/api/routes.rs
git commit -m "feat: add Axum router with auth and internal routes"
```

---

### Task 11: Wire Everything Together in main.rs

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Write the full main.rs**

```rust
mod api;
mod backends;
mod config;
mod core_layer;
mod storage;

use std::sync::Arc;

use api::handlers::AppState;
use backends::bluebubbles::BlueBubblesBackend;
use core_layer::backend::MessageBackend;
use core_layer::webhook::WebhookDispatcher;
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
        backend = %config.backend.backend_type,
        "Config loaded"
    );

    // Init storage
    let db_path = config::Config::config_dir().join("aimessage.db");
    let storage = Arc::new(
        storage::sqlite::Storage::new(&db_path).expect("Failed to initialize database"),
    );
    tracing::info!(path = %db_path.display(), "Database initialized");

    // Init backend
    let bb_config = config
        .backend
        .bluebubbles
        .clone()
        .expect("BlueBubbles config missing");
    let backend = Arc::new(BlueBubblesBackend::new(bb_config, config.server.port));

    // Start backend and get message receiver
    let receiver = backend
        .start()
        .await
        .expect("Failed to start backend");

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

- [ ] **Step 2: Verify it compiles**

Run: `cargo build`
Expected: Compiles successfully with no errors. This is the first full integration compile.

- [ ] **Step 3: Commit**

```bash
git add src/main.rs
git commit -m "feat: wire all components together in main.rs"
```

---

### Task 12: Lint and Smoke Test

**Files:**
- None (verification only)

- [ ] **Step 1: Run cargo clippy**

Run: `cargo clippy -- -D warnings`
Expected: No warnings or errors. Fix any that appear.

- [ ] **Step 2: Run the server**

Run: `cargo run`
Expected: Server starts, prints config loaded message, binds to port 3001. Will log warnings about BlueBubbles not being reachable, but should not crash.

- [ ] **Step 3: Test health endpoint**

Run: `curl http://localhost:3001/api/v1/health`
Expected: Returns JSON with `"status": "degraded"` and backend `"connected": false`

- [ ] **Step 4: Test auth rejection**

Run: `curl http://localhost:3001/api/v1/messages`
Expected: Returns 401 Unauthorized

- [ ] **Step 5: Test auth acceptance**

Run: `curl -H "X-API-Key: <your-key-from-config>" http://localhost:3001/api/v1/webhooks`
Expected: Returns `{"data":[],"count":0}`

- [ ] **Step 6: Test webhook CRUD**

```bash
# Create
curl -X POST -H "X-API-Key: <key>" -H "Content-Type: application/json" \
  -d '{"url":"http://localhost:9999/hook","events":["message.received"]}' \
  http://localhost:3001/api/v1/webhooks

# List
curl -H "X-API-Key: <key>" http://localhost:3001/api/v1/webhooks

# Delete (use id from create response)
curl -X DELETE -H "X-API-Key: <key>" http://localhost:3001/api/v1/webhooks/<id>
```

Expected: Create returns webhook with UUID, list shows it, delete removes it.

- [ ] **Step 7: Commit any fixes**

```bash
git add -A
git commit -m "fix: address clippy warnings and smoke test issues"
```
