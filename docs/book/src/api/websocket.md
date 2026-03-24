# WebSocket

The WebSocket endpoint streams all events in real-time as an alternative to webhooks. Multiple clients can connect simultaneously.

## Connection URL

```
ws://localhost:3001/api/v1/ws?api_key=YOUR_KEY
```

Authentication is via query parameter because WebSocket handshake headers are not universally supported by client libraries.

## Connect with websocat

```bash
websocat "ws://localhost:3001/api/v1/ws?api_key=YOUR_KEY"
```

Other compatible clients: `wscat`, any browser `WebSocket` API, or any language's WebSocket library.

## Event format

Each event is delivered as a JSON text frame with the same envelope format used by webhooks:

```json
{"type":"message.received","data":{"id":"94711","guid":"F568F54A-...","conversation_id":"iMessage;-;+15551234567","sender":"+15551234567","body":"Hey!","attachments":[],"timestamp":"2026-03-23T23:49:54Z","is_from_me":false,"status":"delivered"}}
```

Events are not filtered — all event types are sent to all connected clients. If you only need specific event types, filter on the client side by checking the `type` field.

## Lagged clients

AiMessage uses a broadcast channel internally. If a connected client is too slow to consume events, lagged events are skipped rather than buffered indefinitely. This prevents a slow consumer from causing unbounded memory growth. Design your client to process events promptly or accept that it may miss events under high load.

## Example: Python client

```python
import asyncio
import json
import websockets

async def listen():
    url = "ws://localhost:3001/api/v1/ws?api_key=YOUR_KEY"
    async with websockets.connect(url) as ws:
        async for message in ws:
            event = json.loads(message)
            print(event["type"], event["data"])

asyncio.run(listen())
```
