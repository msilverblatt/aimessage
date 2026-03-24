# Project Structure

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
│   └── backend.rs       # MessageBackend implementation
└── storage/
    └── sqlite.rs        # App DB: webhooks, message log, state
```

---

## Module descriptions

### `main.rs`

Reads config, initializes the storage layer, constructs the iMessage backend, starts the polling task, sets up the broadcast channel, wires up the webhook dispatcher task, and starts the Axum HTTP server. All the pieces come together here.

### `config.rs`

Handles loading `~/.aimessage/config.toml`, auto-generating it with a random API key on first run, and exiting with a prompt if the file was just created. Uses the `toml` and `serde` crates.

### `api/auth.rs`

Axum middleware extractor that reads the `X-API-Key` header and returns `401` if it is missing or does not match the configured key.

### `api/handlers.rs`

One function per route. Handlers receive validated request types, call into the `MessageBackend` trait, and return serialized response types.

### `api/routes.rs`

Defines the Axum `Router`, attaches handlers to paths, and applies the auth middleware to protected routes.

### `api/types.rs`

`serde`-derived structs for request bodies and response payloads. Kept separate from domain types so the API shape can evolve independently.

### `core_layer/types.rs`

Core domain types: `Message`, `Conversation`, `Reaction`, and the `Event` enum. These are the shared language between layers.

### `core_layer/backend.rs`

The `MessageBackend` trait defines the interface the API layer uses to interact with iMessage: `send_message`, `list_messages`, `get_message`, `list_conversations`, `get_conversation`. The iMessage layer provides the implementation; this trait makes the API layer testable without a real Mac.

### `core_layer/webhook.rs`

Spawns a task that subscribes to the broadcast channel, receives events, and delivers them to all registered webhook URLs. Implements the retry logic (3 attempts, 1s and 5s backoff).

### `core_layer/errors.rs`

Defines the application error type and its `IntoResponse` implementation, mapping error variants to appropriate HTTP status codes.

### `imessage/chatdb.rs`

Opens `chat.db` read-only in WAL mode. Contains the SQL queries for reading messages, conversations, and reactions. Runs the polling loop, tracks the last processed ROWID, and publishes events to the broadcast channel.

### `imessage/applescript.rs`

Constructs and runs `osascript` commands to send messages and attachments via Messages.app. Uses environment variables to pass message content safely.

### `imessage/backend.rs`

Implements the `MessageBackend` trait using `chatdb.rs` and `applescript.rs`. This is the concrete implementation that runs in production.

### `storage/sqlite.rs`

Manages `~/.aimessage/aimessage.db` using SQLite. Stores: registered webhooks (URL, events, secret), the last processed ROWID for resuming after restart.
