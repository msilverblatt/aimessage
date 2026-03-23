# AiMessage Server — Design Spec

## Overview

An open-source tool that turns a Mac into an iMessage API server. Targets developers who want a self-hosted iMessage API for their own apps (chatbots, agents, CRM integrations), with a path toward a turnkey hosted product.

**MVP scope:** Bidirectional messaging (send + receive) via REST API with webhook delivery for incoming messages. Full iMessage feature set (reactions, group chats, read receipts, typing indicators) planned for post-MVP.

**Prerequisite:** A Mac (e.g., Mac Mini) running 24/7 with BlueBubbles server installed and configured.

## Architecture

Three layers, each with a single responsibility:

### 1. API Layer (Rust/Axum)

Public-facing HTTP server. Exposes REST endpoints for sending messages, querying conversations, and managing webhook registrations. Handles auth, validation, rate limiting. Knows nothing about iMessage internals.

### 2. Core Layer

Business logic and the `MessageBackend` trait. Defines the interface contract that all backends must implement. Owns the webhook dispatcher — when a backend reports a new incoming message via its channel, Core delivers it to all registered webhook URLs.

### 3. Backend Layer

Implementations of `MessageBackend`. MVP ships with `BlueBubblesBackend`, which translates trait calls into BlueBubbles API requests. Future backends (imessage-rs, direct SQLite/AppleScript) slot in here.

### Data Flow

```
Consumer App  →  API Layer  →  Core Layer  →  Backend Layer  →  BlueBubbles  →  iMessage
                                                                                    ↓
Consumer App  ←  Webhook POST  ←  Core Layer  ←  Backend Channel  ←  BlueBubbles webhook
```

## API Surface (MVP)

All endpoints behind an API key header (`X-API-Key`). JSON payloads.

### Messages

- `POST /api/v1/messages` — Send a message (recipient phone/email, body text, optional attachments)
- `GET /api/v1/messages` — List messages. Query params: `conversation_id` (optional), `since` (ISO 8601 timestamp, optional), `limit` (default 50, max 200), `offset` (numeric, default 0)
- `GET /api/v1/messages/:id` — Get a specific message

### Conversations

- `GET /api/v1/conversations` — List conversations (with latest message preview). Query params: `limit` (default 50, max 200), `offset` (numeric, default 0)
- `GET /api/v1/conversations/:id` — Get conversation details. Messages are fetched separately via `GET /api/v1/messages?conversation_id=:id`

### Webhooks

- `POST /api/v1/webhooks` — Register a webhook. Body: `{ "url": "https://...", "events": ["message.received", "message.sent"] }`. Registering a duplicate URL updates the existing webhook's event list.
- `GET /api/v1/webhooks` — List registered webhooks
- `DELETE /api/v1/webhooks/:id` — Remove a webhook

### Health

- `GET /api/v1/health` — Server and backend status

### Webhook Events (MVP)

- `message.received` — New incoming message
- `message.sent` — Confirmation that an outbound message was accepted by iMessage (left the Mac). Note: this is "sent," not "delivered to recipient device" — delivery confirmation is a post-MVP feature

### Future Webhook Events

- `message.read`, `message.reaction`, `typing.started`, `group.member_added`, etc.

### Auth

Single API key stored in local config file. Generated on first run. No user accounts or OAuth for MVP.

## Backend Abstraction

```rust
#[async_trait]
pub trait MessageBackend: Send + Sync {
    // Sending
    async fn send_message(&self, request: SendMessageRequest) -> Result<Message, BackendError>;

    // Reading
    async fn get_messages(&self, query: MessageQuery) -> Result<Vec<Message>, BackendError>;
    async fn get_message(&self, id: &str) -> Result<Message, BackendError>;
    async fn get_conversations(&self, query: PaginationQuery) -> Result<Vec<Conversation>, BackendError>;
    async fn get_conversation(&self, id: &str) -> Result<Conversation, BackendError>;

    // Lifecycle — start() returns a tokio::sync::mpsc::Receiver that the Core layer owns for incoming messages
    async fn start(&self) -> Result<tokio::sync::mpsc::Receiver<Message>, BackendError>;
    async fn shutdown(&self) -> Result<(), BackendError>;
    async fn health_check(&self) -> Result<BackendStatus, BackendError>;
}
```

The `start()` method transfers ownership of the message `Receiver` to the Core layer, which spawns a task to read from it and dispatch webhooks. The backend holds the `Sender` side and pushes incoming messages into it.

### Domain Types

- `Message` — id, conversation_id, sender, body, attachments, timestamp, is_from_me, status
- `Conversation` — id, participants, display_name, is_group, latest_message
- `SendMessageRequest` — recipient (phone or email), body, optional attachment paths. The backend adapter is responsible for translating the raw phone/email into whatever internal addressing the backend requires (e.g., BlueBubbles chat GUIDs).
- `MessageQuery` — conversation_id (optional), since (DateTime<Utc>, optional), limit (u32, default 50, max 200), offset (u32, default 0)
- `PaginationQuery` — limit (u32, default 50, max 200), offset (u32, default 0)

### BlueBubbles Adapter (MVP Backend)

Translates trait calls into HTTP requests to BlueBubbles' local API (configurable URL, default `http://localhost:1234`). For incoming messages, receives BlueBubbles webhook POSTs on an internal endpoint (`/internal/bb-webhook`) and pushes them into the message channel via the `Sender` side.

The `/internal/bb-webhook` endpoint is mounted on the same Axum server but excluded from API key auth (it only accepts requests from localhost). No separate listener needed for MVP.

## Configuration

Single TOML file at `~/.aimessage/config.toml`:

```toml
[server]
host = "0.0.0.0"
port = 3001

[auth]
api_key = "generated-on-first-run"

[backend]
type = "bluebubbles"

[backend.bluebubbles]
url = "http://localhost:1234"
password = "your-bb-password"
```

On first run with no config file, the server generates one with a random API key and prints it to stdout. User fills in BlueBubbles credentials and restarts. If the server starts with missing or placeholder backend credentials, it refuses to start with a clear error message indicating which fields need to be set.

## Storage

SQLite database at `~/.aimessage/aimessage.db`. Two tables for MVP:

- `webhooks` — id (UUID), url, events (JSON array), created_at. `POST` response returns the created webhook object including its id.
- `message_log` — id (auto-increment PK), backend_message_id (unique, used for deduplication), conversation_id, delivered_at, webhook_delivery_status (pending | delivered | failed)

The message log is a thin cache for deduplication and delivery tracking. BlueBubbles (and the macOS Messages database behind it) remains the source of truth for message content.

## Error Handling & Resilience

### BlueBubbles Down/Unreachable

- Health check endpoint reports backend status
- Send attempts return `503 Service Unavailable`
- Server stays up — does not crash if BB is temporarily unavailable

### Webhook Delivery Failures

- Retry with exponential backoff: 1s, 5s, 30s (3 attempts total), then mark as permanently failed
- Failed deliveries logged to `message_log` table with status `failed`
- No dead letter queue for MVP — consumers catch up via GET endpoints

### Server Restart / Mac Reboot

- On startup, reconnects to BlueBubbles and re-registers its internal webhook
- No message replay on restart for MVP — missed messages queryable via GET endpoints but not re-delivered as webhooks

### Logging

Structured JSON logs to stdout. Standard levels (info, warn, error).

## Project Structure

```
aimessage/
├── Cargo.toml
├── src/
│   ├── main.rs              # Entry point, config loading, server startup
│   ├── config.rs             # Config file parsing (TOML)
│   ├── api/
│   │   ├── mod.rs
│   │   ├── routes.rs         # Axum route definitions
│   │   ├── handlers.rs       # Request handlers
│   │   ├── auth.rs           # API key middleware
│   │   └── types.rs          # Request/response DTOs
│   ├── core/
│   │   ├── mod.rs
│   │   ├── types.rs          # Domain types (Message, Conversation, etc.)
│   │   ├── backend.rs        # MessageBackend trait definition
│   │   ├── webhook.rs        # Webhook dispatcher
│   │   └── errors.rs         # Error types
│   ├── backends/
│   │   ├── mod.rs
│   │   └── bluebubbles.rs    # BlueBubbles adapter
│   └── storage/
│       ├── mod.rs
│       └── sqlite.rs         # SQLite for webhooks + message log
```

## Post-MVP Roadmap

1. Full iMessage features: reactions, group chats, read receipts, attachments, typing indicators
2. WebSocket support for real-time message streaming
3. Polling endpoint as fallback delivery mechanism
4. Alternative backends (imessage-rs, direct SQLite)
5. One-command install script / Homebrew formula
6. Docker packaging
7. Hosted product exploration
