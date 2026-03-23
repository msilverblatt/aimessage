use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

/// Send a plain text message via AppleScript
pub async fn send_message(recipient: &str, body: &str) -> Result<(), String> {
    let escaped_body = body.replace('\\', "\\\\").replace('"', "\\\"");
    let escaped_recipient = recipient.replace('\\', "\\\\").replace('"', "\\\"");

    let script = format!(
        r#"tell application "Messages"
    set targetService to 1st service whose service type = iMessage
    set targetBuddy to buddy "{}" of targetService
    send "{}" to targetBuddy
end tell"#,
        escaped_recipient, escaped_body
    );

    let output = timeout(
        Duration::from_secs(10),
        Command::new("osascript").arg("-e").arg(&script).output(),
    )
    .await
    .map_err(|_| "AppleScript timed out after 10 seconds".to_string())?
    .map_err(|e| format!("Failed to run osascript: {}", e))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("AppleScript failed: {}", stderr.trim()))
    }
}

/// Check if Messages.app is reachable via AppleScript
pub async fn check_automation_permission() -> Result<(), String> {
    let output = timeout(
        Duration::from_secs(5),
        Command::new("osascript")
            .arg("-e")
            .arg(r#"tell application "Messages" to count of chats"#)
            .output(),
    )
    .await
    .map_err(|_| "AppleScript permission check timed out".to_string())?
    .map_err(|e| format!("Failed to run osascript: {}", e))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!(
            "Cannot reach Messages.app via AppleScript.\n\
             Error: {}\n\
             Go to: System Settings → Privacy & Security → Automation\n\
             Grant access for your terminal or the aimessage binary.",
            stderr.trim()
        ))
    }
}
