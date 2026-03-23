# AiMessage

Turn any Mac into an iMessage API server. Single binary, zero external dependencies.

AiMessage reads your iMessage database directly, sends messages via AppleScript, and exposes everything through a clean REST API with webhook delivery for real-time events.

## How it works

- **Read path**: Polls `~/Library/Messages/chat.db` (SQLite) for new messages and reactions
- **Write path**: Sends messages via `osascript` controlling Messages.app
- **Advanced features** (optional): Reactions and typing indicators via Apple's private IMCore framework (requires SIP disabled)

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
host = "0.0.0.0"
port = 3001

[auth]
api_key = "your-generated-uuid"

[imessage]
chat_db_path = "/Users/you/Library/Messages/chat.db"  # auto-detected
poll_interval_ms = 1000                                 # how often to check for new messages
private_api = false                                     # set true if SIP is disabled
```

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
curl -X POST -H "X-API-Key: $KEY" -H "Content-Type: application/json" \
  -d '{"recipient": "+15551234567", "body": "Hello from AiMessage"}' \
  http://localhost:3001/api/v1/messages

# React to a message (requires private_api = true)
curl -X POST -H "X-API-Key: $KEY" -H "Content-Type: application/json" \
  -d '{"reaction": "love"}' \
  http://localhost:3001/api/v1/messages/12345/react
```

Query parameters for `GET /messages`: `conversation_id`, `since` (ISO 8601), `limit` (default 50, max 200), `offset`.

Reaction types: `love`, `thumbsup`, `thumbsdown`, `haha`, `exclamation`, `question`.

### Conversations

```bash
# List conversations
curl -H "X-API-Key: $KEY" "http://localhost:3001/api/v1/conversations?limit=10"

# Get a specific conversation
curl -H "X-API-Key: $KEY" "http://localhost:3001/api/v1/conversations/iMessage;-;+15551234567"

# Send typing indicator (requires private_api = true)
curl -X POST -H "X-API-Key: $KEY" \
  http://localhost:3001/api/v1/conversations/iMessage;-;+15551234567/typing
```

### Webhooks

Register URLs to receive real-time events when messages or reactions arrive.

```bash
# Register a webhook
curl -X POST -H "X-API-Key: $KEY" -H "Content-Type: application/json" \
  -d '{"url": "https://your-server.com/webhook", "events": ["message.received", "reaction.added"]}' \
  http://localhost:3001/api/v1/webhooks

# List webhooks
curl -H "X-API-Key: $KEY" http://localhost:3001/api/v1/webhooks

# Delete a webhook
curl -X DELETE -H "X-API-Key: $KEY" http://localhost:3001/api/v1/webhooks/<id>
```

Events: `message.received`, `message.sent`, `reaction.added`, `reaction.removed`.

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

### Health

```bash
curl http://localhost:3001/api/v1/health
# {"status":"ok","backend":{"connected":true,"private_api_available":false,"message":null}}
```

## Permissions

| Permission | Required for | How to grant |
|-----------|-------------|-------------|
| Full Disk Access | Reading chat.db | System Settings → Privacy & Security → Full Disk Access → add AiMessage.app |
| Automation | Sending messages via AppleScript | Prompted on first launch, or System Settings → Automation |
| SIP disabled | Reactions, typing indicators (optional) | Boot to Recovery Mode → `csrutil disable` |

## Development

```bash
# Build (debug)
cargo build

# Run directly (requires FDA on your terminal)
cargo run

# Run with structured JSON logs
RUST_LOG=aimessage=debug cargo run

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

The backend polls chat.db every second for new messages (by tracking the highest ROWID). New events are pushed into a channel, picked up by the webhook dispatcher, and delivered to registered URLs.

### Key implementation details

- **Timestamps**: iMessage uses Mac Absolute Time (seconds since Jan 1, 2001). Offset: 978,307,200s from Unix epoch. Ventura+ uses nanoseconds for some fields.
- **Reactions**: Stored as messages with `associated_message_type` 2000-2005 (add) / 3000-3005 (remove).
- **chat.db**: Opened read-only with WAL mode for concurrent access while Messages.app writes.
- **State persistence**: Last processed ROWID is saved to `~/.aimessage/aimessage.db` so the server resumes correctly after restart.

## License

TBD
