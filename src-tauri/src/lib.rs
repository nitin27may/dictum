pub mod audio;
pub mod commands;
pub mod flow;
pub mod hotkey;
pub mod injection;

use audio::capture::AudioCapture;
use commands::{
    audio_commands::{
        check_microphone_permission, get_audio_devices, request_microphone_permission,
        start_recording, stop_recording,
    },
    injection_commands::{check_accessibility_permission, inject_text},
    settings_commands::{get_current_hotkey, get_platform, register_hotkey, set_api_config},
};
use std::sync::Arc;
use std::time::Instant;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager,
};
use tokio::sync::Mutex;

pub const DEFAULT_HOTKEY: &str = "Alt+Space";

pub struct AppState {
    pub audio_capture: Arc<Mutex<AudioCapture>>,
    pub current_hotkey: Arc<Mutex<String>>,
    /// True while hotkey is held down (re-entrancy guard)
    pub is_recording: Arc<Mutex<bool>>,
    /// When the hotkey was pressed (for minimum hold check)
    pub pressed_at: Arc<Mutex<Option<Instant>>>,
    /// API config pushed from JS on startup / settings change
    pub api_config: Arc<Mutex<serde_json::Value>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            audio_capture: Arc::new(Mutex::new(AudioCapture::new(None))),
            current_hotkey: Arc::new(Mutex::new(DEFAULT_HOTKEY.to_string())),
            is_recording: Arc::new(Mutex::new(false)),
            pressed_at: Arc::new(Mutex::new(None)),
            api_config: Arc::new(Mutex::new(serde_json::json!({
                "provider": "openai",
                "openai": { "whisperModel": "whisper-1" },
                "azure": {}
            }))),
        }
    }
}

impl Clone for AppState {
    fn clone(&self) -> Self {
        Self {
            audio_capture: Arc::clone(&self.audio_capture),
            current_hotkey: Arc::clone(&self.current_hotkey),
            is_recording: Arc::clone(&self.is_recording),
            pressed_at: Arc::clone(&self.pressed_at),
            api_config: Arc::clone(&self.api_config),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Hide from Dock and App Switcher on macOS — tray-only app
    #[cfg(target_os = "macos")]
    {
        use objc::runtime::Object;
        use objc::{msg_send, sel, sel_impl};
        unsafe {
            let ns_app: *mut Object =
                msg_send![objc::class!(NSApplication), sharedApplication];
            // NSApplicationActivationPolicyAccessory = 1 (no Dock icon, no menu bar)
            let _: () = msg_send![ns_app, setActivationPolicy: 1i64];
        }
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::new().build())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_shell::init())
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            start_recording,
            stop_recording,
            get_audio_devices,
            inject_text,
            check_accessibility_permission,
            check_microphone_permission,
            request_microphone_permission,
            register_hotkey,
            get_platform,
            get_current_hotkey,
            set_api_config,
        ])
        .setup(|app| {
            setup_tray(app)?;
            position_overlay(app)?;
            // Register default hotkey on main thread (macOS requirement)
            hotkey::register_hotkey(&app.handle(), DEFAULT_HOTKEY)?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Failed to run Wisper");
}

fn setup_tray(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let settings_item = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
    let separator = tauri::menu::PredefinedMenuItem::separator(app)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit Wisper", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[&settings_item, &separator, &quit_item])?;

    TrayIconBuilder::with_id("wisper-tray")
        .menu(&menu)
        .icon(app.default_window_icon().unwrap().clone())
        .tooltip("Wisper — Hold Alt+Space to transcribe")
        .on_menu_event(|app, event| match event.id.as_ref() {
            "settings" => show_settings_window(app),
            "quit" => {
                log::info!("Quit requested from tray");
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_settings_window(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}

fn position_overlay(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(window) = app.get_webview_window("overlay") {
        if let Ok(Some(monitor)) = window.primary_monitor() {
            let size = monitor.size();
            let scale = monitor.scale_factor();
            let logical_width = size.width as f64 / scale;
            let logical_height = size.height as f64 / scale;

            let overlay_width = 420.0_f64;
            let overlay_height = 90.0_f64;
            let bottom_margin = 48.0_f64;

            let x = (logical_width / 2.0) - (overlay_width / 2.0);
            let y = logical_height - overlay_height - bottom_margin;

            window.set_position(tauri::PhysicalPosition {
                x: (x * scale) as i32,
                y: (y * scale) as i32,
            })?;
        }

        window.set_ignore_cursor_events(true)?;
    }
    Ok(())
}

fn show_settings_window(app: &AppHandle) {
    // On macOS, LSUIElement apps don't activate automatically on window.show().
    // Must call NSApp.activateIgnoringOtherApps(true) first.
    #[cfg(target_os = "macos")]
    {
        use objc::runtime::Object;
        use objc::{msg_send, sel, sel_impl};
        unsafe {
            let ns_app: *mut Object =
                msg_send![objc::class!(NSApplication), sharedApplication];
            let _: () = msg_send![ns_app, activateIgnoringOtherApps: true];
        }
    }

    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}
