# Permissions

AiMessage requires macOS permissions to read your message database and control Messages.app. Reactions and typing indicators require an additional step.

## Summary

| Permission | Required for | How to grant |
|---|---|---|
| Full Disk Access | Reading `chat.db` | System Settings → Privacy & Security → Full Disk Access → add `AiMessage.app` |
| Automation | Sending messages via AppleScript | Prompted automatically on first launch; or System Settings → Privacy & Security → Automation |
| SIP disabled | Reactions and typing indicators (optional) | Boot to Recovery Mode, run `csrutil disable` |

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

---

## SIP (System Integrity Protection)

**Why**: Reactions and typing indicators require calling into Apple's private `IMCore` framework. Loading private frameworks is blocked by SIP.

**This is optional.** Basic sending, receiving, webhooks, and WebSocket all work with SIP enabled. Only disable SIP if you specifically need reaction and typing indicator support.

**How to disable SIP**:

1. Shut down your Mac
2. Boot into Recovery Mode:
   - **Apple Silicon**: Hold the power button until "Loading startup options" appears, then select Options
   - **Intel**: Hold Cmd+R during startup
3. Open **Terminal** from the Utilities menu
4. Run: `csrutil disable`
5. Restart

To re-enable SIP: repeat the steps above and run `csrutil enable`.

After disabling SIP, set `private_api = true` in your config file (see [Configuration](./configuration.md)).
