# Event System

## The Event enum

All real-time notifications are modeled as variants of a single `Event` enum defined in `src/core_layer/types.rs`. Every event flows through the broadcast channel from the iMessage layer to all consumers (webhook dispatcher and WebSocket clients).

```rust
pub enum Event {
    MessageReceived(Message),
    MessageSent(Message),
    ReactionAdded(Reaction),
    ReactionRemoved(Reaction),
}
```

When serialized for delivery (webhook payloads, WebSocket frames), events use a `type` string field:

| Enum variant | `type` string |
|---|---|
| `MessageReceived` | `"message.received"` |
| `MessageSent` | `"message.sent"` |
| `ReactionAdded` | `"reaction.added"` |
| `ReactionRemoved` | `"reaction.removed"` |

---

## Event types

### `message.received`

Fired when an incoming message (not sent by the local account) is detected in `chat.db`.

### `message.sent`

Fired when an outgoing message appears in `chat.db`. This happens after Messages.app confirms the send — typically within a second of the `send_message` API call completing.

### `reaction.added`

Fired when a Tapback reaction is added to any message in a conversation. The `data` payload includes the reaction type and the GUID of the message being reacted to.

### `reaction.removed`

Fired when a previously added Tapback reaction is removed.

---

## How reactions are stored in chat.db

Reactions are not stored as a separate table in `chat.db`. They appear as ordinary message rows with special values in the `associated_message_type` and `associated_message_guid` columns.

### `associated_message_type` mapping

| Value range | Meaning |
|---|---|
| `2000` | Heart (love) — added |
| `2001` | Thumbs up — added |
| `2002` | Thumbs down — added |
| `2003` | Ha ha — added |
| `2004` | Exclamation — added |
| `2005` | Question mark — added |
| `3000` | Heart (love) — removed |
| `3001` | Thumbs up — removed |
| `3002` | Thumbs down — removed |
| `3003` | Ha ha — removed |
| `3004` | Exclamation — removed |
| `3005` | Question mark — removed |

Values in the 2000–2005 range indicate a reaction being added; values in 3000–3005 indicate removal. The `associated_message_guid` column holds the GUID of the message that was reacted to.

AiMessage's poller inspects `associated_message_type` on every new message row. Rows with a non-zero value in this column are classified as reactions and published as `ReactionAdded` or `ReactionRemoved` events rather than message events.
