# Authentication

## API key

All endpoints except `/api/v1/health` require authentication via the `X-API-Key` header.

```bash
curl -H "X-API-Key: your-api-key" http://localhost:3001/api/v1/messages
```

## Finding your key

Your API key is generated on first run and stored in the config file:

```bash
cat ~/.aimessage/config.toml
```

```toml
[auth]
api_key = "550e8400-e29b-41d4-a716-446655440000"
```

You can change it to any string. Restart the server after editing the config.

## Missing or invalid key

Any request without a valid `X-API-Key` header returns:

```
HTTP/1.1 401 Unauthorized
```

## WebSocket authentication

The WebSocket endpoint does not support request headers in the initial handshake across all clients. Pass the key as a query parameter instead:

```
ws://localhost:3001/api/v1/ws?api_key=your-api-key
```

See [WebSocket](./websocket.md) for full details.

## Rate limiting

The API enforces a global limit of **60 requests per minute**. Requests that exceed this limit receive `429 Too Many Requests`. The rate limit applies across all endpoints (authenticated or not).

## Health endpoint

`GET /api/v1/health` is unauthenticated and can be used to verify the server is running without a key.
