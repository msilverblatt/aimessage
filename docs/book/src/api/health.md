# Health

The health endpoint is unauthenticated and is suitable for uptime checks, load balancer probes, and verifying the server started correctly.

## Endpoint

```
GET /api/v1/health
```

```bash
curl http://localhost:3001/api/v1/health
```

## Response

```json
{
  "status": "ok",
  "backend": {
    "connected": true,
    "message": null
  }
}
```

## Fields

| Field | Type | Description |
|---|---|---|
| `status` | string | Always `"ok"` when the server is running. |
| `backend.connected` | boolean | `true` if AiMessage has successfully opened `chat.db`. `false` indicates a permissions problem — check Full Disk Access. |
| `backend.message` | string or null | Optional diagnostic message. Non-null when there is a backend error or warning to report. |

## Diagnosing issues

If `connected` is `false`, the most common cause is missing Full Disk Access for the app bundle. See [Permissions](../getting-started/permissions.md).
