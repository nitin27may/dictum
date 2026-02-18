use crate::AppState;
use tauri::{AppHandle, State};

#[tauri::command]
pub async fn register_hotkey(
    shortcut: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    crate::hotkey::unregister_all(&app).map_err(|e| e.to_string())?;
    crate::hotkey::register_hotkey(&app, &shortcut).map_err(|e| {
        log::error!("Failed to register hotkey '{}': {}", shortcut, e);
        e.to_string()
    })?;

    *state.current_hotkey.lock().await = shortcut;
    Ok(())
}

#[tauri::command]
pub fn get_platform() -> &'static str {
    #[cfg(target_os = "macos")]
    return "macos";
    #[cfg(target_os = "windows")]
    return "windows";
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    return "other";
}

#[tauri::command]
pub async fn get_current_hotkey(state: State<'_, AppState>) -> Result<String, String> {
    Ok(state.current_hotkey.lock().await.clone())
}

/// Called by the frontend on startup and whenever settings change.
/// Pushes the current API config into Rust so transcription can run without IPC.
#[tauri::command]
pub async fn set_api_config(
    config: serde_json::Value,
    state: State<'_, AppState>,
) -> Result<(), String> {
    log::info!("API config updated (provider: {})", config["provider"].as_str().unwrap_or("?"));
    *state.api_config.lock().await = config;
    Ok(())
}
