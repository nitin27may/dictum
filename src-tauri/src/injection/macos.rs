use anyhow::{anyhow, Result};
use std::process::Command;

/// Injects text at the current cursor position using clipboard + Cmd+V paste.
///
/// Flow:
///   1. Save current clipboard content
///   2. Set clipboard to the transcribed text
///   3. Send Cmd+V to the focused application
///   4. Wait 500ms, then restore original clipboard
///
/// Known quirk: if the user pastes within the 500ms window,
/// they receive the transcription text instead of their original clipboard.
pub async fn inject_text(text: &str) -> Result<()> {
    // 1. Save current clipboard
    let saved = get_clipboard()?;

    // 2. Set clipboard to transcription
    set_clipboard(text)?;

    // 3. Send Cmd+V via AppleScript
    send_paste()?;

    // 4. Restore clipboard after delay
    let saved_clone = saved.clone();
    tokio::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        if let Err(e) = restore_clipboard(&saved_clone) {
            log::warn!("Failed to restore clipboard: {}", e);
        }
    });

    Ok(())
}

fn get_clipboard() -> Result<String> {
    let output = Command::new("pbpaste")
        .output()
        .map_err(|e| anyhow!("pbpaste failed: {}", e))?;
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
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

fn restore_clipboard(content: &str) -> Result<()> {
    set_clipboard(content)
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
