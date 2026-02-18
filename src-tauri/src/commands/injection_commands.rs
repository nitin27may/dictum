use tauri::State;
use crate::AppState;

#[tauri::command]
pub async fn inject_text(
    text: String,
    _state: State<'_, AppState>,
) -> Result<(), String> {
    log::info!("Injecting {} chars", text.len());
    crate::injection::inject_text(&text).await.map_err(|e| {
        log::error!("Text injection failed: {}", e);
        e.to_string()
    })
}

#[tauri::command]
pub fn check_accessibility_permission() -> bool {
    crate::injection::check_accessibility_permission()
}
