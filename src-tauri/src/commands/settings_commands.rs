use crate::AppState;
use tauri::{AppHandle, State};

#[tauri::command]
pub async fn register_hotkey(
    shortcut: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    // CRITICAL: macOS requires global shortcuts to be registered on the main thread.
    // Tauri async commands run on a thread pool, so we dispatch to main via channel.
    let shortcut_clone = shortcut.clone();
    let app_clone = app.clone();
    let (tx, rx) = tokio::sync::oneshot::channel::<Result<(), String>>();

    app.run_on_main_thread(move || {
        let result = (|| {
            crate::hotkey::unregister_all(&app_clone).map_err(|e| e.to_string())?;
            crate::hotkey::register_hotkey(&app_clone, &shortcut_clone).map_err(|e| {
                log::error!("Failed to register hotkey '{}': {}", shortcut_clone, e);
                e.to_string()
            })?;
            Ok(())
        })();
        let _ = tx.send(result);
    })
    .map_err(|e| e.to_string())?;

    rx.await.map_err(|_| "Main thread channel closed".to_string())??;

    log::info!("Hotkey changed to: {}", shortcut);
    *state.current_hotkey.lock().await = shortcut;
    Ok(())
}

/// Temporarily unregisters all shortcuts (used during hotkey capture in Settings).
#[tauri::command]
pub async fn unregister_hotkeys(app: AppHandle) -> Result<(), String> {
    let app_clone = app.clone();
    let (tx, rx) = tokio::sync::oneshot::channel::<Result<(), String>>();

    app.run_on_main_thread(move || {
        let result = crate::hotkey::unregister_all(&app_clone).map_err(|e| e.to_string());
        let _ = tx.send(result);
    })
    .map_err(|e| e.to_string())?;

    rx.await.map_err(|_| "Main thread channel closed".to_string())??;
    log::info!("All hotkeys unregistered (capture mode)");
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
