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

#[tauri::command]
pub async fn set_general_config(
    config: serde_json::Value,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let max_recording_seconds = config["maxRecordingSeconds"]
        .as_u64()
        .filter(|secs| *secs > 0)
        .unwrap_or(crate::DEFAULT_MAX_RECORD_SECS);

    let audio_device = config["audioDevice"].as_str().map(str::trim).filter(|s| !s.is_empty());

    log::info!(
        "General config updated (max recording: {} sec, audio device: {})",
        max_recording_seconds,
        audio_device.unwrap_or("default")
    );

    *state.max_record_secs.lock().await = max_recording_seconds;
    state
        .audio_capture
        .lock()
        .await
        .set_device_name(audio_device.map(|s| s.to_string()));

    Ok(())
}

#[tauri::command]
pub async fn set_audio_device(
    device_name: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    if state.audio_capture.lock().await.is_recording() {
        return Err("Cannot switch microphone while recording".to_string());
    }

    let normalized = device_name
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    if let Some(ref name) = normalized {
        let available = crate::audio::capture::list_input_devices().map_err(|e| e.to_string())?;
        if !available.iter().any(|d| d == name) {
            return Err(format!("Microphone '{}' is not available", name));
        }
    }

    log::info!(
        "Audio device switched to: {}",
        normalized.as_deref().unwrap_or("system default")
    );

    state.audio_capture.lock().await.set_device_name(normalized);
    Ok(())
}
