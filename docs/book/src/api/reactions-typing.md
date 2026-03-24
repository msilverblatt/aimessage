# Reactions & Typing

Both reactions and typing indicators require the Private API to be enabled. Without it, these endpoints return `501 Not Implemented`.

To enable: set `private_api = true` in `~/.aimessage/config.toml` and ensure SIP is disabled on the Mac. See [Permissions](../getting-started/permissions.md) for SIP instructions.

---

## React to a message

```
POST /api/v1/messages/{id}/react
```

`{id}` is the numeric message ROWID.

**Request body:**

```json
{
  "reaction": "love"
}
```

```bash
curl -X POST -H "X-API-Key: $KEY" -H "Content-Type: application/json" -d '{"reaction": "love"}' http://localhost:3001/api/v1/messages/12345/react
```

### Reaction types

| Value | Tapback |
|---|---|
| `love` | Heart |
| `thumbsup` | Thumbs up |
| `thumbsdown` | Thumbs down |
| `haha` | Ha ha |
| `exclamation` | Exclamation |
| `question` | Question mark |

### Without Private API

```
HTTP/1.1 501 Not Implemented
```

---

## Send a typing indicator

```
POST /api/v1/conversations/{id}/typing
```

`{id}` is the conversation GUID (e.g. `iMessage;-;+15551234567`).

```bash
curl -X POST -H "X-API-Key: $KEY" http://localhost:3001/api/v1/conversations/iMessage;-;+15551234567/typing
```

This sends a typing indicator to the conversation. The indicator clears automatically after a few seconds as per normal Messages.app behavior.

### Without Private API

```
HTTP/1.1 501 Not Implemented
```
