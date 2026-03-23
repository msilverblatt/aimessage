# AiMessage Server — Design Spec (v2)

## Overview

An open-source tool that turns a Mac into an iMessage API server. Single binary, zero external dependencies. Targets developers who want a self-hosted iMessage API for their own apps (chatbots, agents, CRM integrations), with a path toward a turnkey hosted product.

**MVP scope:** Bidirectional messaging (send + receive), reactions (read + send), and typing indicators — all via REST API with webhook delivery for real-time events.

**Prerequisite:** A Mac (e.g., Mac Mini) running 24/7. The user grants Full Disk Access and Automation permissions to the aimessage binary. For advanced features (sending reactions, typing indicators), SIP must be disabled.

## Architecture

Three layers, each with a single responsibility:

### 1. API Layer (Rust/Axum)

Public-facing HTTP server. Exposes REST endpoints for sending messages, reacting, querying conversations, and managing webhook registrations. Handles auth, validation, rate limiting. Knows nothing about iMessage internals.

### 2. Core Layer

Business logic and the `MessageBackend` trait. Defines the interface contract. Owns the webhook dispatcher — when the backend reports a new event via its channel, Core delivers it to all registered webhook URLs.

### 3. Backend Layer — Direct iMessage Integration

No external dependencies. Talks to iMessage directly via three mechanisms:

**a) chat.db reader (read path)**
Opens `~/Library/Messages/chat.db` as a read-only SQLite connection. Queries the `message`, `chat`, `handle`, `chat_message_join`, `attachment`, and `message_attachment_join` tables directly. Requires Full Disk Access permission.

**b) AppleScript sender (write path — basic)**
Sends messages via `osascript` calling the Messages.app AppleScript dictionary. Requires Automation permission. Supports plain text messages and attachments.

**c) IMCore Private API (write path — advanced, optional)**
For sending reactions/tapbacks and typing indicators, loads an Objective-C dylib that calls Apple's private IMCore framework. Requires SIP to be disabled. This is opt-in — the server works without it, but some features are unavailable.

### Data Flow

```
Consumer App  →  API Layer  →  Core Layer  →  Backend Layer  →  AppleScript/IMCore  →  iMessage
                                                                                          ↓
Consumer App  ←  Webhook POST  ←  Core Layer  ←  Backend Channel  ←  chat.db poll  ←  Messages.app
```

### Incoming Message Detection

The backend polls `chat.db` for new messages by tracking the highest seen `message.ROWID`. On each poll cycle (configurable, default 1 second):
1. Query `SELECT * FROM message WHERE ROWID > ?` joined with `chat_message_join` and `handle`
2. Map results to domain `Message` types, converting Mac epoch timestamps
3. Push new messages into the mpsc channel for webhook dispatch
4. Check for new reactions via `associated_message_type` values: 2000-2005 (reaction added) and 3000-3005 (reaction removed). Emit `ReactionAdded` or `ReactionRemoved` events accordingly.

### Timestamp Handling

iMessage uses **Mac Absolute Time**: seconds since January 1, 2001 (not Unix epoch). The offset is 978,307,200 seconds. On macOS Ventura+, some timestamp fields are in nanoseconds — detect by checking if the value exceeds `1_000_000_000_000` (about 33,000 years in seconds, which is always true for nanosecond values), and divide by `1_000_000_000` before converting.

## API Surface (MVP)

All endpoints behind an API key header (`X-API-Key`). JSON payloads.

### Messages

- `POST /api/v1/messages` — Send a message (recipient phone/email, body text, optional attachments)
- `POST /api/v1/messages/:id/react` — Send a reaction/tapback to a message. Body: `{ "reaction": "love" | "thumbsup" | "thumbsdown" | "haha" | "exclamation" | "question" }`. Requires Private API (returns 501 if unavailable).
- `GET /api/v1/messages` — List messages. Query params: `conversation_id` (optional), `since` (ISO 8601 timestamp, optional), `limit` (default 50, max 200), `offset` (numeric, default 0)
- `GET /api/v1/messages/:id` — Get a specific message

### Conversations

- `GET /api/v1/conversations` — List conversations (with latest message preview). Query params: `limit` (default 50, max 200), `offset` (numeric, default 0)
- `GET /api/v1/conversations/:id` — Get conversation details. Messages are fetched separately via `GET /api/v1/messages?conversation_id=:id`

### Typing

- `POST /api/v1/conversations/:id/typing` — Send a typing indicator to a conversation. Requires Private API (returns 501 if unavailable).

### Webhooks

- `POST /api/v1/webhooks` — Register a webhook. Body: `{ "url": "https://...", "events": ["message.received"] }`. Valid event names: `message.received`, `message.sent`, `reaction.added`, `reaction.removed`. Registering a duplicate URL updates the existing webhook's event list.
- `GET /api/v1/webhooks` — List registered webhooks
- `DELETE /api/v1/webhooks/:id` — Remove a webhook

### Health

- `GET /api/v1/health` — Server and backend status, including whether Private API is available

### Webhook Events (MVP)

- `message.received` — New incoming message (includes text, sender, conversation)
- `message.sent` — Confirmation that an outbound message was sent (detected in chat.db with is_from_me=1)
- `reaction.added` — A reaction/tapback was added to a message (includes reaction type, target message id, sender)
- `reaction.removed` — A reaction was removed

### Future Webhook Events

- `message.read`, `typing.started`, `group.member_added`, `message.delivered`, etc.

### Auth

Single API key stored in local config file. Generated on first run. No user accounts or OAuth for MVP.

## Backend Trait

```rust
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
    async fn start(&self) -> Result<tokio::sync::mpsc::Receiver<Event>, BackendError>;
    async fn shutdown(&self) -> Result<(), BackendError>;
    async fn health_check(&self) -> Result<BackendStatus, BackendError>;
}
```

Note: `start()` now returns `Receiver<Event>` instead of `Receiver<Message>` — events include messages, reactions, and future event types.

### Domain Types

- `Event` — enum: `NewMessage(Message)`, `MessageSent(Message)`, `ReactionAdded(Reaction)`, `ReactionRemoved(Reaction)`
- `Message` — id (ROWID as string, used in API paths), guid (iMessage's internal guid, used for reaction linking), conversation_id (chat guid), sender (handle address), body, attachments, timestamp, is_from_me, status
- `Conversation` — id (chat guid), participants, display_name, is_group, latest_message
- `Reaction` — id, message_id (ROWID of the target message, resolved by looking up `associated_message_guid` in the message table), message_guid (the raw `associated_message_guid`), sender, reaction_type, timestamp. The API endpoint `POST /api/v1/messages/:id/react` accepts a ROWID as `:id`, and the backend looks up the corresponding guid to pass to IMCore.
- `ReactionType` — enum: Love, ThumbsUp, ThumbsDown, HaHa, Exclamation, Question
- `SendMessageRequest` — recipient (phone or email), body, optional attachment paths
- `MessageQuery` — conversation_id (optional), since (DateTime<Utc>, optional), limit (u32, default 50, max 200), offset (u32, default 0)
- `PaginationQuery` — limit (u32, default 50, max 200), offset (u32, default 0)
- `BackendStatus` — connected (bool), private_api_available (bool), message (optional string)

### Reaction Mapping (chat.db)

Reactions in chat.db are stored as messages with special `associated_message_type` values:

| associated_message_type | Meaning |
|------------------------|---------|
| 2000 | Love |
| 2001 | Thumbs Up |
| 2002 | Thumbs Down |
| 2003 | Ha Ha |
| 2004 | Exclamation |
| 2005 | Question |
| 3000 | Remove Love |
| 3001 | Remove Thumbs Up |
| 3002 | Remove Thumbs Down |
| 3003 | Remove Ha Ha |
| 3004 | Remove Exclamation |
| 3005 | Remove Question |

The `associated_message_guid` field links the reaction to the original message's `guid`. Values 2000-2005 are "add reaction", 3000-3005 are "remove reaction".

### Direct Backend Implementation

**chat.db reader:**
- Opens `~/Library/Messages/chat.db` in read-only mode (`SQLITE_OPEN_READONLY`). chat.db uses WAL journal mode, which allows concurrent reads while Messages.app writes.
- Polls for new messages by ROWID on a configurable interval (default 1s)
- Joins across `message`, `chat_message_join`, `chat`, `handle`, `attachment`, `message_attachment_join`
- Converts Mac epoch timestamps to UTC
- Detects reactions by checking `associated_message_type > 0`

**AppleScript sender:**
- Sends messages via `std::process::Command` running `osascript -e 'tell application "Messages" to send "text" to buddy "recipient" of service "iMessage"'`
- Returns success/failure based on osascript exit code
- Timeout of 10 seconds per send (AppleScript can hang if Messages.app is unresponsive)
- AppleScript does not return the sent message's ID. After a successful send, the server polls chat.db briefly (up to 3 seconds, checking every 200ms) for a new `is_from_me=1` message matching the recipient and body text. If found, returns the full `Message` with its real ROWID. If not found within the timeout, returns a provisional response with `status: "sent"` and `id: null` — the message will still appear in subsequent GET queries once chat.db catches up.

**Private API (optional):**
- On startup, checks if SIP is disabled and IMCore framework is accessible
- If available, loads a helper dylib for reaction and typing indicator support
- If not available, `send_reaction()` and `send_typing()` return `BackendError::FeatureUnavailable`
- Health check reports `private_api_available: true/false`

## Configuration

Single TOML file at `~/.aimessage/config.toml`:

```toml
[server]
host = "0.0.0.0"
port = 3001

[auth]
api_key = "generated-on-first-run"

[imessage]
chat_db_path = "/Users/you/Library/Messages/chat.db"  # auto-detected from $HOME, override with absolute path if needed
poll_interval_ms = 1000
private_api = false  # set to true if SIP is disabled and you want reactions/typing
```

On first run with no config file, the server generates one with a random API key and prints it to stdout. No manual configuration needed to start — it auto-detects chat.db location and works with defaults.

On startup, the server verifies:
1. chat.db exists and is readable (Full Disk Access check)
2. AppleScript can reach Messages.app (Automation permission check)
3. If `private_api = true`, verifies SIP is disabled and IMCore is loadable

If checks fail, the server prints clear instructions on how to grant permissions and exits.

## Storage

SQLite database at `~/.aimessage/aimessage.db`. Two tables for MVP:

- `webhooks` — id (UUID), url, events (JSON array), created_at. `POST` response returns the created webhook object including its id.
- `message_log` — id (auto-increment PK), imessage_rowid (unique, used for deduplication), conversation_id, delivered_at, webhook_delivery_status (pending | delivered | failed)
- `state` — key (TEXT PRIMARY KEY), value (TEXT). Stores `last_processed_rowid` for resuming after restart.

The `message_log` tracks webhook delivery attempts. The `state` table stores the high-water mark ROWID independently — this is always updated on every poll cycle regardless of whether any webhooks are registered, ensuring no messages are missed after restart. chat.db remains the source of truth for message content.

## Error Handling & Resilience

### Permission Issues

- On startup, checks Full Disk Access and Automation permissions
- If missing, prints step-by-step instructions and exits with clear error
- Health endpoint reports permission status

### Messages.app Unresponsive

- AppleScript send has a 10-second timeout
- If Messages.app is not running, AppleScript will launch it automatically
- If sends fail repeatedly, health endpoint reports degraded status

### Webhook Delivery Failures

- Retry with exponential backoff: 1s, 5s, 30s (3 attempts total), then mark as permanently failed
- Failed deliveries logged to `message_log` table with status `failed`
- No dead letter queue for MVP — consumers catch up via GET endpoints

### Server Restart / Mac Reboot

- On startup, reads the last processed ROWID from the message_log table to resume where it left off
- Messages received while the server was down are detected on next poll and dispatched as webhooks

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
│   ├── core_layer/
│   │   ├── mod.rs
│   │   ├── types.rs          # Domain types (Message, Conversation, Event, Reaction, etc.)
│   │   ├── backend.rs        # MessageBackend trait definition
│   │   ├── webhook.rs        # Webhook dispatcher
│   │   └── errors.rs         # Error types
│   ├── imessage/
│   │   ├── mod.rs
│   │   ├── chatdb.rs         # chat.db SQLite reader + poller
│   │   ├── applescript.rs    # AppleScript message sender
│   │   ├── private_api.rs    # IMCore dylib loader (optional)
│   │   └── backend.rs        # MessageBackend impl tying it all together
│   └── storage/
│       ├── mod.rs
│       └── sqlite.rs         # SQLite for webhooks + message log
```

## Post-MVP Roadmap

1. Read receipts (mark as read via Private API)
2. Group chat management (create, add/remove members via Private API)
3. Attachment handling (send/receive images, files with proper MIME types)
4. WebSocket support for real-time event streaming
5. Polling endpoint as fallback delivery mechanism
6. One-command install script / Homebrew formula
7. Hosted product exploration
