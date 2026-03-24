# Permissions

AiMessage requires macOS permissions to read your message database and control Messages.app.

## Summary

| Permission | Required for | How to grant |
|---|---|---|
| Full Disk Access | Reading `chat.db` | System Settings → Privacy & Security → Full Disk Access → add `AiMessage.app` |
| Automation | Sending messages via AppleScript | Prompted automatically on first launch; or System Settings → Privacy & Security → Automation |

---

## Full Disk Access

**Why**: `~/Library/Messages/chat.db` is protected by macOS's TCC (Transparency, Consent, and Control) subsystem. Without Full Disk Access, any attempt to open the file returns a permission error regardless of Unix file permissions.

**How to grant**:

1. Open **System Settings** → **Privacy & Security** → **Full Disk Access**
2. Click the **+** button
3. Navigate to `bundle/AiMessage.app` and add it
4. Ensure the toggle next to AiMessage is enabled

If you are running the binary directly from a terminal (e.g. `cargo run`), you need to grant Full Disk Access to your terminal emulator (Terminal.app, iTerm2, etc.) instead.

---

## Automation

**Why**: AiMessage sends messages by scripting Messages.app via `osascript`. macOS requires explicit user consent before one application can control another via AppleScript.

**How to grant**:

On first launch, macOS will display a dialog: _"AiMessage wants to control Messages."_ Click **OK**.

If you dismissed that dialog or need to re-grant it:

1. Open **System Settings** → **Privacy & Security** → **Automation**
2. Find **AiMessage** in the list
3. Enable the **Messages** toggle beneath it

