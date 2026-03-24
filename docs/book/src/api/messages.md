# Messages

## Send a message

```
POST /api/v1/messages
```

**Request body:**

```json
{
  "recipient": "+15551234567",
  "body": "Hello from AiMessage"
}
```

```bash
curl -X POST -H "X-API-Key: $KEY" -H "Content-Type: application/json" -d '{"recipient": "+15551234567", "body": "Hello from AiMessage"}' http://localhost:3001/api/v1/messages
```

The `recipient` field accepts a phone number or email address — anything Messages.app accepts as a destination.

## Send with attachments

Include an `attachments` array of absolute file paths on the Mac running AiMessage:

```bash
curl -X POST -H "X-API-Key: $KEY" -H "Content-Type: application/json" -d '{"recipient": "+15551234567", "body": "Check this out", "attachments": ["/path/to/photo.png"]}' http://localhost:3001/api/v1/messages
```

To send a file without a text body:

```bash
curl -X POST -H "X-API-Key: $KEY" -H "Content-Type: application/json" -d '{"recipient": "+15551234567", "body": "", "attachments": ["/path/to/image.jpg"]}' http://localhost:3001/api/v1/messages
```

Multiple attachments can be included in a single request by adding more paths to the array.

---

## List messages

```
GET /api/v1/messages
```

### Query parameters

| Parameter | Type | Default | Description |
|---|---|---|---|
| `conversation_id` | string | — | Filter by conversation GUID (e.g. `iMessage;-;+15551234567`). |
| `since` | string | — | ISO 8601 timestamp. Returns messages after this time. |
| `limit` | integer | `50` | Number of messages to return. Maximum `200`. |
| `offset` | integer | `0` | Pagination offset. |

### Examples

```bash
# Most recent 10 messages across all conversations
curl -H "X-API-Key: $KEY" "http://localhost:3001/api/v1/messages?limit=10"

# Messages in a specific conversation
curl -H "X-API-Key: $KEY" "http://localhost:3001/api/v1/messages?conversation_id=iMessage;-;+15551234567&limit=10"

# Messages since a specific time
curl -H "X-API-Key: $KEY" "http://localhost:3001/api/v1/messages?since=2026-03-23T00:00:00Z"
```

### Response

```json
[
  {
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
]
```

Incoming messages that include files have their attachment file paths in the `attachments` array:

```json
{
  "attachments": ["/Users/you/Library/Messages/Attachments/00/00/UUID/IMG_1234.jpeg"]
}
```

---

## Get a message by ID

```
GET /api/v1/messages/{id}
```

`id` is the numeric ROWID from `chat.db`.

```bash
curl -H "X-API-Key: $KEY" http://localhost:3001/api/v1/messages/94711
```

### Response

Same object shape as entries in the list response.
