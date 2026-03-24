use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

/// Send a plain text message via AppleScript.
/// User input is passed via environment variables to prevent AppleScript injection.
pub async fn send_message(recipient: &str, body: &str) -> Result<(), String> {
    let script = r#"
set recipientAddr to system attribute "AIMSG_RECIPIENT"
set messageBody to system attribute "AIMSG_BODY"
tell application "Messages"
    set targetService to 1st service whose service type = iMessage
    set targetBuddy to buddy recipientAddr of targetService
    send messageBody to targetBuddy
end tell
"#;

    let output = timeout(
        Duration::from_secs(10),
        Command::new("osascript")
            .arg("-e")
            .arg(script)
            .env("AIMSG_RECIPIENT", recipient)
            .env("AIMSG_BODY", body)
            .output(),
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

/// Send a file attachment via AppleScript.
/// File path is passed via environment variable to prevent injection.
pub async fn send_attachment(recipient: &str, file_path: &str) -> Result<(), String> {
    // Validate the file path
    if file_path.contains("..") {
        return Err("Attachment path cannot contain '..'".to_string());
    }
    let path = std::path::Path::new(file_path);
    if !path.exists() {
        return Err(format!("Attachment file not found: {}", file_path));
    }
    if !path.is_file() {
        return Err(format!("Attachment path is not a file: {}", file_path));
    }

    let script = r#"
set recipientAddr to system attribute "AIMSG_RECIPIENT"
set filePath to system attribute "AIMSG_FILE"
tell application "Messages"
    set targetService to 1st service whose service type = iMessage
    set targetBuddy to buddy recipientAddr of targetService
    send POSIX file filePath to targetBuddy
end tell
"#;

    let output = timeout(
        Duration::from_secs(10),
        Command::new("osascript")
            .arg("-e")
            .arg(script)
            .env("AIMSG_RECIPIENT", recipient)
            .env("AIMSG_FILE", file_path)
            .output(),
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
