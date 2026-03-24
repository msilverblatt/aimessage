# Webhooks

Webhooks let you register an HTTP endpoint to receive real-time event notifications. When AiMessage detects a new message or reaction, it POSTs a JSON payload to every registered URL that subscribes to that event type.

---

## Register a webhook

```
POST /api/v1/webhooks
```

**Request body:**

```json
{
  "url": "http://127.0.0.1:8080/webhook",
  "events": ["message.received", "reaction.added"]
}
```

```bash
curl -X POST -H "X-API-Key: $KEY" -H "Content-Type: application/json" -d '{"url": "http://127.0.0.1:8080/webhook", "events": ["message.received", "reaction.added"]}' http://localhost:3001/api/v1/webhooks
```

### With a secret

The `secret` field is optional. When provided, AiMessage includes an `X-Webhook-Secret` header on every delivery with the value you supplied. Use this to verify that requests to your listener come from AiMessage.

```bash
curl -X POST -H "X-API-Key: $KEY" -H "Content-Type: application/json" -d '{"url": "http://127.0.0.1:8080/webhook", "events": ["message.received"], "secret": "my-secret-token"}' http://localhost:3001/api/v1/webhooks
```

For local integrations, binding your webhook listener to `127.0.0.1` (as shown above) prevents external access and is recommended for single-machine setups.

### Available events

| Event | When it fires |
|---|---|
| `message.received` | An incoming message is detected in `chat.db` |
| `message.sent` | An outgoing message sent by AiMessage is confirmed in `chat.db` |
| `reaction.added` | A reaction is added to a message |
| `reaction.removed` | A reaction is removed from a message |

---

## Payload format

All events use the same envelope:

```json
{
  "type": "message.received",
  "data": {
    "id": "94711",
    "guid": "F568F54A-1234-5678-ABCD-000000000000",
    "conversation_id": "iMessage;-;+15551234567",
    "sender": "+15551234567",
    "body": "Hey!",
    "attachments": [],
    "timestamp": "2026-03-23T23:49:54Z",
    "is_from_me": false,
    "status": "delivered"
  }
}
```

The `data` object shape varies by event type. For `reaction.added` and `reaction.removed`, it includes the reaction type and the ID of the message being reacted to.

---

## Retry behavior

Failed deliveries (non-2xx response or connection error) are retried up to 3 times with backoff: 1 second, then 5 seconds. After 3 failures the delivery is dropped.

---

## List webhooks

```
GET /api/v1/webhooks
```

```bash
curl -H "X-API-Key: $KEY" http://localhost:3001/api/v1/webhooks
```

**Response:**

```json
[
  {
    "id": "a1b2c3d4-...",
    "url": "http://127.0.0.1:8080/webhook",
    "events": ["message.received"],
    "created_at": "2026-03-23T12:00:00Z"
  }
]
```

---

## Delete a webhook

```
DELETE /api/v1/webhooks/{id}
```

```bash
curl -X DELETE -H "X-API-Key: $KEY" http://localhost:3001/api/v1/webhooks/a1b2c3d4-...
```

Returns `204 No Content` on success.
