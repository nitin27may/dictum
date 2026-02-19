#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "windows")]
pub mod windows;

use anyhow::Result;

/// Deletes the character before the cursor (removes the Option+Space non-breaking space).
/// No-op on platforms where this isn't implemented.
pub fn delete_preceding_char() {
    #[cfg(target_os = "macos")]
    macos::delete_preceding_char();
}

pub async fn inject_text(text: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        macos::inject_text(text).await
    }
    #[cfg(target_os = "windows")]
    {
        windows::inject_text(text).await
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        anyhow::bail!("Text injection not supported on this platform")
    }
}

pub fn check_accessibility_permission() -> bool {
    #[cfg(target_os = "macos")]
    {
        macos::check_accessibility_permission()
    }
    #[cfg(target_os = "windows")]
    {
        windows::check_accessibility_permission()
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        false
    }
}
