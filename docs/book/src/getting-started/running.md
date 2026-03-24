# Running the Server

## Launch

```bash
open bundle/AiMessage.app
```

AiMessage runs as a menu bar application. When running, its icon appears in the macOS menu bar. Click the icon to see server status or quit.

## First-run flow

1. **First launch**: Config is generated at `~/.aimessage/config.toml` and the process exits. Check the generated file.
2. **Second launch**: Server starts. On this launch (or the first launch after config already exists), macOS may prompt for Automation permission — click OK to allow AiMessage to control Messages.app.
3. **Server is running**: The menu bar icon appears. The HTTP server is listening on the configured host and port (default `0.0.0.0:3001`).

## Verify the server is running

```bash
curl http://localhost:3001/api/v1/health
```

Expected response:

```json
{"status":"ok","backend":{"connected":true,"message":null}}
```

`connected: true` means AiMessage has successfully opened `chat.db`. If this is `false`, check that Full Disk Access has been granted to the app bundle.

## Running the bare binary

If you want to run without the app bundle (e.g. during development):

```bash
cargo run
```

This requires Full Disk Access to be granted to your terminal emulator. See [Permissions](./permissions.md) for details.

To enable verbose logging:

```bash
RUST_LOG=aimessage=debug cargo run
```

## State persistence

AiMessage stores the last processed message ROWID in `~/.aimessage/aimessage.db`. This ensures that after a restart, it resumes from where it left off rather than replaying all historical messages as new events.
