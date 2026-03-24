# Configuration

## Config file location

```
~/.aimessage/config.toml
```

## First-run behavior

On the very first launch, AiMessage generates a config file with a random API key and then exits. This is intentional — it gives you a chance to review and adjust the config before the server starts accepting requests.

```
Config generated at /Users/you/.aimessage/config.toml
Edit it if needed, then run again.
```

Launch AiMessage a second time to start the server.

## Full config reference

```toml
[server]
host = "0.0.0.0"       # Interface to bind. Use "127.0.0.1" to restrict to localhost only.
port = 3001             # Port the HTTP server listens on.

[auth]
api_key = "your-generated-uuid"   # All API requests require this in the X-API-Key header.

[imessage]
chat_db_path = "/Users/you/Library/Messages/chat.db"   # Path to the iMessage SQLite database.
poll_interval_ms = 1000                                  # How often (in milliseconds) to check for new messages.
```

## Field reference

### `[server]`

| Field | Default | Description |
|---|---|---|
| `host` | `"0.0.0.0"` | Bind address. Use `"127.0.0.1"` to accept only local connections. |
| `port` | `3001` | TCP port for the HTTP server. |

### `[auth]`

| Field | Description |
|---|---|
| `api_key` | A UUID generated on first run. Required on all API requests as the `X-API-Key` header. Change this to any string you prefer. |

### `[imessage]`

| Field | Default | Description |
|---|---|---|
| `chat_db_path` | Auto-detected | Path to `chat.db`. Auto-detected as `~/Library/Messages/chat.db`. Override only if your database is in a non-standard location. |
| `poll_interval_ms` | `1000` | Polling interval in milliseconds. Lower values reduce latency but increase CPU and disk I/O. |

## Auto-detection of chat.db

If `chat_db_path` is not specified, AiMessage resolves it from `$HOME`. This covers the standard single-user macOS setup. Override it explicitly if you run AiMessage as a service under a different user account or have a non-standard Messages setup.

## Finding your API key

```bash
cat ~/.aimessage/config.toml
```
