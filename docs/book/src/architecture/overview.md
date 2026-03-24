# Architecture Overview

AiMessage is organized into three layers. Each layer has a single responsibility, and dependencies only flow downward.

## Layer diagram

```
┌─────────────────────────────────────────────────┐
│                  API Layer (Axum)                │
│  HTTP endpoints · Auth middleware · Request DTOs │
│  WebSocket handler · Route definitions           │
└────────────────────────┬────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────┐
│                  Core Layer                      │
│  MessageBackend trait · Domain types             │
│  Webhook dispatcher with retry                   │
│  Broadcast channel (event bus)                   │
└────────────────────────┬────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────┐
│               iMessage Layer                     │
│  chat.db reader (ROWID polling)                  │
│  AppleScript sender (osascript)                  │
│  MessageBackend implementation                   │
└─────────────────────────────────────────────────┘
```

## Data flow

### Inbound (receiving messages)

1. The iMessage layer polls `chat.db` every `poll_interval_ms` milliseconds, comparing the current max ROWID against the last processed ROWID.
2. New rows are read, parsed into `Message` or `Reaction` domain types, and wrapped in `Event` variants.
3. Events are published to a `tokio::sync::broadcast` channel.
4. Two subscribers consume from that channel simultaneously:
   - The **webhook dispatcher** (in the core layer) fans out to all registered webhook URLs with retry logic.
   - Each connected **WebSocket client** receives the event as a JSON text frame.

### Outbound (sending messages)

1. An API request hits `POST /api/v1/messages`.
2. The handler calls the `MessageBackend` trait method `send_message`.
3. The iMessage layer implementation invokes `osascript` with the recipient and body.
4. Messages.app sends the message. The sent message eventually appears in `chat.db` and is picked up by the polling loop as a `message.sent` event.

## Broadcast channel

The broadcast channel is the central event bus. It decouples the iMessage poller from all consumers. Key properties:

- Multiple consumers (webhooks, WebSocket clients) subscribe independently.
- The channel has a fixed capacity. Lagged consumers (WebSocket clients that are too slow) have events skipped rather than the channel blocking.
- The webhook dispatcher is a single task that reads from the channel and fans out to all registered URLs concurrently.

## Storage

AiMessage uses two SQLite databases:

| Database | Location | Contents |
|---|---|---|
| `chat.db` | `~/Library/Messages/chat.db` | iMessage data — read-only |
| `aimessage.db` | `~/.aimessage/aimessage.db` | App state: registered webhooks, last processed ROWID |
