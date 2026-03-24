# Conversations

## Conversation IDs

Conversations are identified by their chat GUID from `chat.db`. The format is:

```
{service};-;{identifier}
```

Examples:

- `iMessage;-;+15551234567` — iMessage with a phone number
- `iMessage;-;user@example.com` — iMessage with an email address
- `SMS;-;+15551234567` — SMS conversation

These GUIDs are returned in all message and conversation responses as `conversation_id`. Use them to filter messages or look up a specific conversation.

---

## List conversations

```
GET /api/v1/conversations
```

### Query parameters

| Parameter | Type | Default | Description |
|---|---|---|---|
| `limit` | integer | `50` | Number of conversations to return. |
| `offset` | integer | `0` | Pagination offset. |

### Example

```bash
curl -H "X-API-Key: $KEY" "http://localhost:3001/api/v1/conversations?limit=10"
```

### Response

```json
[
  {
    "id": "iMessage;-;+15551234567",
    "display_name": null,
    "participants": ["+15551234567"],
    "last_message_at": "2026-03-23T23:49:54Z"
  }
]
```

`display_name` is set for named group conversations; it is `null` for 1-on-1 conversations.

---

## Get a conversation by ID

```
GET /api/v1/conversations/{id}
```

The `{id}` is the full chat GUID, URL-encoded when necessary.

```bash
curl -H "X-API-Key: $KEY" "http://localhost:3001/api/v1/conversations/iMessage;-;+15551234567"
```

### Response

```json
{
  "id": "iMessage;-;+15551234567",
  "display_name": null,
  "participants": ["+15551234567"],
  "last_message_at": "2026-03-23T23:49:54Z"
}
```
