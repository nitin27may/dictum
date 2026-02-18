use anyhow::{anyhow, Result};
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

/// Registers the global hotkey on the main thread.
///
/// CRITICAL: This MUST be called on the Tauri main thread.
/// macOS requires global shortcuts to be registered on the main thread.
pub fn register_hotkey(app: &AppHandle, shortcut_str: &str) -> Result<()> {
    let shortcut: Shortcut = shortcut_str
        .parse()
        .map_err(|_| anyhow!("Invalid shortcut string: {}", shortcut_str))?;

    app.global_shortcut()
        .on_shortcut(shortcut, move |app_handle, _shortcut, event| {
            let app = app_handle.clone();
            let state = app_handle.state::<crate::AppState>();

            match event.state() {
                ShortcutState::Pressed => {
                    log::debug!("Hotkey pressed");
                    tauri::async_runtime::spawn(crate::flow::on_press(app, state.inner().clone()));
                }
                ShortcutState::Released => {
                    log::debug!("Hotkey released");
                    tauri::async_runtime::spawn(crate::flow::on_release(app, state.inner().clone()));
                }
            }
        })
        .map_err(|e| anyhow!("Failed to register shortcut '{}': {}", shortcut_str, e))?;

    log::info!("Registered global hotkey: {}", shortcut_str);
    Ok(())
}

pub fn unregister_all(app: &AppHandle) -> Result<()> {
    app.global_shortcut()
        .unregister_all()
        .map_err(|e| anyhow!("Failed to unregister shortcuts: {}", e))
}
