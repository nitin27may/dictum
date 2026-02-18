use crate::AppState;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use tauri::{AppHandle, State};

#[tauri::command]
pub async fn start_recording(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let mut capture = state.audio_capture.lock().await;
    if capture.is_recording() {
        return Err("Already recording".to_string());
    }
    capture.start(app).map_err(|e| {
        log::error!("Failed to start recording: {}", e);
        e.to_string()
    })
}

#[tauri::command]
pub async fn stop_recording(state: State<'_, AppState>) -> Result<Vec<u8>, String> {
    let mut capture = state.audio_capture.lock().await;
    if !capture.is_recording() {
        return Err("Not recording".to_string());
    }

    let samples = capture.stop().map_err(|e| {
        log::error!("Failed to stop recording: {}", e);
        e.to_string()
    })?;

    if samples.is_empty() {
        return Err("No audio data captured".to_string());
    }

    let duration = crate::audio::encoder::samples_duration_secs(samples.len());
    log::info!("Stopped recording: {:.1}s, {} samples", duration, samples.len());

    let wav_bytes = crate::audio::encoder::encode_wav(&samples).map_err(|e| {
        log::error!("WAV encoding failed: {}", e);
        e.to_string()
    })?;

    log::info!("Encoded WAV: {} bytes", wav_bytes.len());
    Ok(wav_bytes)
}

#[tauri::command]
pub async fn get_audio_devices() -> Result<Vec<String>, String> {
    crate::audio::capture::list_input_devices().map_err(|e| e.to_string())
}

/// Returns true = granted, false = denied/restricted, None = not yet determined.
#[tauri::command]
pub fn check_microphone_permission() -> Option<bool> {
    #[cfg(target_os = "macos")]
    {
        use objc::runtime::Object;
        use objc::{class, msg_send, sel, sel_impl};
        unsafe {
            let ns_string = class!(NSString);
            // AVMediaTypeAudio = "soun"
            let media_type: *mut Object =
                msg_send![ns_string, stringWithUTF8String: b"soun\0".as_ptr() as *const std::ffi::c_char];
            let av_device = class!(AVCaptureDevice);
            let status: i64 = msg_send![av_device, authorizationStatusForMediaType: media_type];
            // 0=NotDetermined 1=Restricted 2=Denied 3=Authorized
            match status {
                3 => Some(true),
                1 | 2 => Some(false),
                _ => None,
            }
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}

/// Builds a brief cpal stream to trigger the macOS microphone TCC dialog,
/// then returns the resulting permission state.
#[tauri::command]
pub async fn request_microphone_permission() -> Result<Option<bool>, String> {
    tokio::task::spawn_blocking(|| {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| "No microphone device found".to_string())?;
        let config = device
            .default_input_config()
            .map_err(|e| e.to_string())?;
        // Building + playing the stream triggers the macOS permission dialog
        let stream = device
            .build_input_stream(
                &config.into(),
                |_data: &[f32], _info: &cpal::InputCallbackInfo| {},
                |_err: cpal::StreamError| {},
                None,
            )
            .map_err(|e| e.to_string())?;
        stream.play().map_err(|e| e.to_string())?;
        std::thread::sleep(std::time::Duration::from_millis(300));
        drop(stream);
        Ok(check_microphone_permission())
    })
    .await
    .map_err(|e| e.to_string())?
}
