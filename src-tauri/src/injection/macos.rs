use anyhow::{anyhow, Result};
use std::process::Command;

/// Injects text at the current cursor position.
///
/// Flow:
///   1. Backspace — removes the non-breaking space macOS types when Option+Space fires
///   2. Set clipboard to transcribed text
///   3. Cmd+V to paste into frontmost application
pub async fn inject_text(text: &str) -> Result<()> {
    // 1. Remove the non-breaking space that Option+Space types in the frontmost app.
    //    This fires here (not in on_press) to ensure the correct target app has focus —
    //    the overlay briefly gains focus at startup and on_press would misfire the backspace.
    delete_preceding_char();

    // 2. Brief pause so the backspace processes before the paste
    tokio::time::sleep(tokio::time::Duration::from_millis(60)).await;

    // 3. Set clipboard and paste.
    //    Note: clipboard save/restore is intentionally omitted — it can trigger
    //    clipboard managers (Alfred, Paste, etc.) which cause a second paste.
    set_clipboard(text)?;
    send_paste()?;

    Ok(())
}


fn set_clipboard(text: &str) -> Result<()> {
    use std::io::Write;
    use std::process::Stdio;

    let mut child = Command::new("pbcopy")
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| anyhow!("pbcopy failed to spawn: {}", e))?;

    if let Some(stdin) = child.stdin.as_mut() {
        stdin
            .write_all(text.as_bytes())
            .map_err(|e| anyhow!("Failed to write to pbcopy: {}", e))?;
    }

    child
        .wait()
        .map_err(|e| anyhow!("pbcopy wait failed: {}", e))?;

    Ok(())
}

fn send_paste() -> Result<()> {
    // Use AppleScript to send Cmd+V to the frontmost application
    let script = r#"tell application "System Events" to keystroke "v" using command down"#;
    let status = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .status()
        .map_err(|e| anyhow!("osascript failed: {}", e))?;

    if !status.success() {
        return Err(anyhow!(
            "osascript returned non-zero exit code. Accessibility permission may be required."
        ));
    }

    Ok(())
}

/// Sends a single Backspace keystroke to the frontmost application.
///
/// Called at the start of recording to undo the non-breaking space that macOS inserts
/// when Option+Space fires via a passive NSEvent monitor (which doesn't consume the event).
pub fn delete_preceding_char() {
    let _ = Command::new("osascript")
        .arg("-e")
        .arg(r#"tell application "System Events" to key code 51"#) // key code 51 = Delete/Backspace
        .status();
    // Errors are silently ignored — if accessibility is not granted, the backspace
    // simply won't fire, which is harmless.
}

/// Check if the app has Accessibility permission (required for osascript keystroke).
pub fn check_accessibility_permission() -> bool {
    let output = Command::new("osascript")
        .arg("-e")
        .arg(r#"tell application "System Events" to get name of first process"#)
        .output();

    match output {
        Ok(o) => o.status.success(),
        Err(_) => false,
    }
}
