# iMessage Integration

## Reading chat.db

`chat.db` is a SQLite database that Messages.app maintains at `~/Library/Messages/chat.db`. AiMessage opens it read-only in WAL (Write-Ahead Logging) mode, which allows concurrent reads while Messages.app is actively writing.

### ROWID polling

AiMessage tracks the highest `ROWID` it has processed in its own `aimessage.db`. On each poll cycle:

1. Query `chat.db` for rows with `ROWID > last_processed_rowid`.
2. Parse each new row into a domain type.
3. Publish events to the broadcast channel.
4. Update `last_processed_rowid` in `aimessage.db`.

This approach is simple, reliable, and avoids filesystem events or SQLite triggers.

### Timestamps

iMessage stores timestamps in **Mac Absolute Time**: seconds (or nanoseconds) since January 1, 2001 00:00:00 UTC.

To convert to Unix epoch, add the offset: `978,307,200` seconds.

macOS Ventura and later use nanoseconds for some timestamp fields. AiMessage detects this by checking whether the raw value is large enough to be nanoseconds (values above approximately `1e10` seconds would be in the far future if interpreted as seconds, so any value above that threshold is treated as nanoseconds and divided by `1e9` before applying the epoch offset).

### Read-only safety

Opening the database read-only ensures AiMessage cannot corrupt `chat.db`. WAL mode means reads do not block writes from Messages.app, and writes from Messages.app do not block reads.

---

## Sending via AppleScript

AiMessage sends messages by invoking `osascript` with an AppleScript that controls Messages.app:

```applescript
tell application "Messages"
    send "Hello" to buddy "+15551234567" of (first service whose service type = iMessage)
end tell
```

### Environment variable safety

The message body and recipient are passed to the script via environment variables rather than interpolated directly into the script string. This prevents injection — if a message body contained AppleScript syntax characters, direct interpolation could produce unexpected behavior or errors. Reading from environment variables makes the boundary between code and data explicit.

### Attachments

To send an attachment, `osascript` is called with a script that uses `send` with a POSIX file reference. The file must exist on the local filesystem at the path provided.

