# AiMessage

Turn any Mac into an iMessage API server. Single binary, zero external dependencies.

AiMessage reads your iMessage database directly, sends messages and attachments via AppleScript, and exposes everything through a REST API with webhook and WebSocket delivery for real-time events.

## How it works

- **Read path**: Polls `~/Library/Messages/chat.db` (SQLite) for new messages and reactions
- **Write path**: Sends messages and attachments via `osascript` controlling Messages.app

## Requirements

- macOS (tested on Ventura+)
- Rust toolchain (`rustup`, `cargo`)
- Messages.app signed into an Apple ID

## Quick start

```bash
# Clone and build
git clone <repo-url> && cd aimessage
cargo build --release

# Create the .app bundle (for sandboxed permissions)
bash scripts/build-app.sh

# Grant permissions (one-time):
# 1. System Settings → Privacy & Security → Full Disk Access → add bundle/AiMessage.app
# 2. Automation permission is prompted on first launch

# Run
open bundle/AiMessage.app
```

On first run, a config file is generated at `~/.aimessage/config.toml` with a random API key. The server exits after generating it — just run again.

```bash
# Check your API key
cat ~/.aimessage/config.toml

# Verify it's working
curl http://localhost:3001/api/v1/health
```

## Configuration

`~/.aimessage/config.toml` — generated automatically on first run.

```toml
[server]
host = "127.0.0.1"
port = 3001

[auth]
api_key = "your-generated-uuid"

[imessage]
chat_db_path = "/Users/you/Library/Messages/chat.db"  # auto-detected
poll_interval_ms = 1000                                 # how often to check for new messages
```

The default bind address is `127.0.0.1` (localhost only). Change to `0.0.0.0` to expose the server on the network.

## API

All endpoints require `X-API-Key` header except health.

### Messages

```bash
# List recent messages
curl -H "X-API-Key: $KEY" "http://localhost:3001/api/v1/messages?limit=10"

# List messages in a conversation
curl -H "X-API-Key: $KEY" "http://localhost:3001/api/v1/messages?conversation_id=iMessage;-;+15551234567&limit=10"

# Get a specific message by ID
curl -H "X-API-Key: $KEY" "http://localhost:3001/api/v1/messages/12345"

# Send a message
curl -X POST -H "X-API-Key: $KEY" -H "Content-Type: application/json" -d '{"recipient": "+15551234567", "body": "Hello from AiMessage"}' http://localhost:3001/api/v1/messages

# Send an image/file
curl -X POST -H "X-API-Key: $KEY" -H "Content-Type: application/json" -d '{"recipient": "+15551234567", "body": "", "attachments": ["/path/to/image.jpg"]}' http://localhost:3001/api/v1/messages

# Send a message with an attachment
curl -X POST -H "X-API-Key: $KEY" -H "Content-Type: application/json" -d '{"recipient": "+15551234567", "body": "Check this out", "attachments": ["/path/to/photo.png"]}' http://localhost:3001/api/v1/messages

```

Query parameters for `GET /messages`: `conversation_id`, `since` (ISO 8601), `limit` (default 50, max 200), `offset`.

Incoming messages include attachment file paths in the `attachments` array (e.g., `/Users/you/Library/Messages/Attachments/.../IMG_1234.jpeg`).

### Conversations

```bash
# List conversations
curl -H "X-API-Key: $KEY" "http://localhost:3001/api/v1/conversations?limit=10"

# Get a specific conversation
curl -H "X-API-Key: $KEY" "http://localhost:3001/api/v1/conversations/iMessage;-;+15551234567"
```

### Webhooks

Register URLs to receive real-time events when messages or reactions arrive.

```bash
# Register a webhook
curl -X POST -H "X-API-Key: $KEY" -H "Content-Type: application/json" -d '{"url": "http://127.0.0.1:8080/webhook", "events": ["message.received", "reaction.added"]}' http://localhost:3001/api/v1/webhooks

# Register with a secret (AiMessage sends HMAC-SHA256 signature as X-Webhook-Signature header)
curl -X POST -H "X-API-Key: $KEY" -H "Content-Type: application/json" -d '{"url": "http://127.0.0.1:8080/webhook", "events": ["message.received"], "secret": "my-secret-token"}' http://localhost:3001/api/v1/webhooks

# List webhooks
curl -H "X-API-Key: $KEY" http://localhost:3001/api/v1/webhooks

# Delete a webhook
curl -X DELETE -H "X-API-Key: $KEY" http://localhost:3001/api/v1/webhooks/<id>
```

Events: `message.received`, `message.sent`, `reaction.added`, `reaction.removed`.

The `secret` field is optional. When provided, AiMessage computes an HMAC-SHA256 signature over the raw request body using the secret as the key, and sends it as `X-Webhook-Signature: sha256=<hex>` on every delivery. To verify: compute `HMAC-SHA256(secret, raw_body)` and compare the hex digest to the value after `sha256=` in the header. For single-machine setups, binding your webhook listener to `127.0.0.1` is also recommended.

**Rate limiting**: The API enforces a global limit of 60 requests per minute. Requests that exceed this limit receive `429 Too Many Requests`.

Webhook payload format:

```json
{
  "type": "message.received",
  "data": {
    "id": "94711",
    "guid": "F568F54A-...",
    "conversation_id": "iMessage;-;+15551234567",
    "sender": "+15551234567",
    "body": "Hey!",
    "timestamp": "2026-03-23T23:49:54Z",
    "is_from_me": false,
    "status": "delivered"
  }
}
```

Failed deliveries are retried 3 times (1s, 5s backoff).

### WebSocket

Real-time event streaming as an alternative to webhooks. Connect and receive all events as they happen.

```bash
# Connect (using websocat, wscat, or any WS client)
websocat "ws://localhost:3001/api/v1/ws?api_key=YOUR_KEY"
```

Auth is via query parameter (`api_key`). Each event is sent as a JSON text frame with the same format as webhook payloads:

```json
{"type":"message.received","data":{"id":"94711","guid":"F568F54A-...","conversation_id":"any;-;+15551234567","sender":"+15551234567","body":"Hey!","attachments":[],"timestamp":"2026-03-23T23:49:54Z","is_from_me":false,"status":"delivered"}}
```

Multiple clients can connect simultaneously. If a client is too slow, lagged events are skipped rather than buffering indefinitely.

### Health

```bash
curl http://localhost:3001/api/v1/health
# {"status":"ok","backend":{"connected":true,"message":null}}
```

## Permissions

| Permission | Required for | How to grant |
|-----------|-------------|-------------|
| Full Disk Access | Reading chat.db | System Settings → Privacy & Security → Full Disk Access → add AiMessage.app |
| Automation | Sending messages via AppleScript | Prompted on first launch, or System Settings → Automation |

## Development

```bash
# Build (debug)
cargo build

# Run directly (requires FDA on your terminal)
cargo run

# Run with structured JSON logs
RUST_LOG=aimessage=debug cargo run

# Run unit tests (16 tests)
cargo test

# Lint
cargo clippy

# Build release + app bundle
bash scripts/build-app.sh
```

### Project structure

```
src/
├── main.rs              # Entry point, wiring
├── config.rs            # TOML config, auto-generation
├── api/                 # HTTP layer (Axum)
│   ├── auth.rs          # X-API-Key middleware
│   ├── handlers.rs      # Request handlers
│   ├── routes.rs        # Router definition
│   └── types.rs         # Request/response DTOs
├── core_layer/          # Domain logic
│   ├── types.rs         # Message, Conversation, Event, Reaction
│   ├── backend.rs       # MessageBackend trait
│   ├── webhook.rs       # Webhook dispatcher with retry
│   └── errors.rs        # Error types → HTTP status mapping
├── imessage/            # iMessage integration
│   ├── chatdb.rs        # chat.db SQLite reader + poller
│   ├── applescript.rs   # osascript message sender
│   ├── private_api.rs   # IMCore stub (SIP check)
│   └── backend.rs       # MessageBackend implementation
└── storage/
    └── sqlite.rs        # App DB: webhooks, message log, state
```

### Architecture

Three layers:

1. **API** (Axum) — HTTP endpoints, auth, request validation
2. **Core** — `MessageBackend` trait, webhook dispatch, domain types
3. **iMessage** — direct macOS integration (chat.db reads, AppleScript sends, optional IMCore)

The backend polls chat.db every second for new messages (by tracking the highest ROWID). New events are pushed into a broadcast channel. Both the webhook dispatcher and any connected WebSocket clients subscribe to this channel and receive events in real-time.

### Key implementation details

- **Timestamps**: iMessage uses Mac Absolute Time (seconds since Jan 1, 2001). Offset: 978,307,200s from Unix epoch. Ventura+ uses nanoseconds for some fields.
- **Reactions**: Stored as messages with `associated_message_type` 2000-2005 (add) / 3000-3005 (remove).
- **chat.db**: Opened read-only with WAL mode for concurrent access while Messages.app writes.
- **State persistence**: Last processed ROWID is saved to `~/.aimessage/aimessage.db` so the server resumes correctly after restart.

## License

TBD
