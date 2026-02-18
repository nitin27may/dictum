/**
 * flow.rs — Full recording + transcription lifecycle, driven by Rust.
 *
 * This module owns the hot path:
 *   hotkey press → start cpal → emit audio-level events
 *   hotkey release → stop cpal → encode WAV → POST Whisper API → inject text
 *
 * The overlay window only receives lightweight state events for UI:
 *   recording-started | recording-tick | processing-started | recording-success | recording-error
 *
 * Moving this out of JS eliminates fragile async IPC listener setup in a hidden window.
 */
use crate::AppState;
use anyhow::{anyhow, Result};
use serde_json::Value;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

const MIN_HOLD_MS: u128 = 200;
const MAX_RECORD_SECS: u64 = 60;

/// Called on hotkey press. Shows overlay, starts cpal capture.
pub async fn on_press(app: AppHandle, state: AppState) {
    // Re-entrancy guard
    {
        let mut down = state.is_recording.lock().await;
        if *down {
            return;
        }
        *down = true;
    }

    *state.pressed_at.lock().await = Some(Instant::now());

    show_overlay(&app);
    emit_overlay(&app, "recording-started", ());

    let mut capture = state.audio_capture.lock().await;
    if let Err(e) = capture.start(app.clone()) {
        log::error!("Failed to start capture: {}", e);
        emit_overlay(&app, "recording-error", e.to_string());
        hide_overlay_after(&app, Duration::from_millis(1500));
        *state.is_recording.lock().await = false;
    }
}

/// Called on hotkey release. Stops capture, transcribes, injects text.
pub async fn on_release(app: AppHandle, state: AppState) {
    let pressed_at = state.pressed_at.lock().await.take();
    *state.is_recording.lock().await = false;

    let hold_ms = pressed_at
        .map(|t| t.elapsed().as_millis())
        .unwrap_or(0);

    let samples = {
        let mut capture = state.audio_capture.lock().await;
        if !capture.is_recording() {
            hide_overlay(&app);
            return;
        }
        match capture.stop() {
            Ok(s) => s,
            Err(e) => {
                log::error!("Failed to stop capture: {}", e);
                emit_overlay(&app, "recording-error", e.to_string());
                hide_overlay_after(&app, Duration::from_millis(1500));
                return;
            }
        }
    };

    // Tap too short — ignore silently
    if hold_ms < MIN_HOLD_MS {
        hide_overlay(&app);
        return;
    }

    if samples.is_empty() {
        emit_overlay(&app, "recording-error", "No audio captured".to_string());
        hide_overlay_after(&app, Duration::from_millis(1500));
        return;
    }

    emit_overlay(&app, "processing-started", ());

    // Encode WAV
    let wav_bytes = match crate::audio::encoder::encode_wav(&samples) {
        Ok(b) => b,
        Err(e) => {
            log::error!("WAV encode failed: {}", e);
            emit_overlay(&app, "recording-error", e.to_string());
            hide_overlay_after(&app, Duration::from_millis(1500));
            return;
        }
    };

    log::info!(
        "Stopped recording: {:.1}s, {} samples, {} WAV bytes",
        hold_ms as f64 / 1000.0,
        samples.len(),
        wav_bytes.len()
    );

    // Read API config from AppState (pushed by JS on startup / settings change)
    let config = state.api_config.lock().await.clone();

    match transcribe(wav_bytes, config).await {
        Ok(text) => {
            let text = text.trim().to_string();
            log::info!("Transcription: \"{}\"", text);

            if text.is_empty() {
                emit_overlay(&app, "recording-error", "No speech detected".to_string());
                hide_overlay_after(&app, Duration::from_millis(1500));
                return;
            }

            let char_count = text.len();

            if let Err(e) = crate::injection::inject_text(&text).await {
                log::error!("Text injection failed: {}", e);
                emit_overlay(&app, "recording-error", format!("Injection failed: {e}"));
                hide_overlay_after(&app, Duration::from_millis(1500));
                return;
            }

            emit_overlay(&app, "recording-success", char_count);
            hide_overlay_after(&app, Duration::from_millis(800));
        }
        Err(e) => {
            log::error!("Transcription failed: {}", e);
            emit_overlay(&app, "recording-error", e.to_string());
            hide_overlay_after(&app, Duration::from_millis(1500));
        }
    }
}

// ── Transcription ─────────────────────────────────────────────────────────────

async fn transcribe(wav_bytes: Vec<u8>, config: Value) -> Result<String> {
    let provider = config["provider"].as_str().unwrap_or("openai");

    if provider == "azure" {
        transcribe_azure(wav_bytes, &config).await
    } else {
        transcribe_openai(wav_bytes, &config).await
    }
}

async fn transcribe_openai(wav_bytes: Vec<u8>, config: &Value) -> Result<String> {
    let api_key = config["openai"]["apiKey"]
        .as_str()
        .map(|s| s.to_string())
        .or_else(|| std::env::var("OPENAI_API_KEY").ok())
        .ok_or_else(|| anyhow!("OpenAI API key not configured. Add it in Settings → API."))?;

    let model = config["openai"]["whisperModel"]
        .as_str()
        .unwrap_or("whisper-1")
        .to_string();

    let base_url = config["openai"]["baseUrl"]
        .as_str()
        .unwrap_or("https://api.openai.com");

    let url = format!("{}/v1/audio/transcriptions", base_url.trim_end_matches('/'));

    let file_part = reqwest::multipart::Part::bytes(wav_bytes)
        .file_name("audio.wav")
        .mime_str("audio/wav")?;

    let form = reqwest::multipart::Form::new()
        .part("file", file_part)
        .text("model", model)
        .text("response_format", "json");

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;

    let resp = client
        .post(&url)
        .bearer_auth(api_key)
        .multipart(form)
        .send()
        .await
        .map_err(|e| anyhow!("Request failed: {}", e))?;

    let status = resp.status();
    let body: Value = resp
        .json()
        .await
        .map_err(|e| anyhow!("Failed to parse response: {}", e))?;

    if !status.is_success() {
        let msg = body["error"]["message"]
            .as_str()
            .unwrap_or("Unknown API error");
        return Err(anyhow!("OpenAI API error {}: {}", status, msg));
    }

    body["text"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow!("No text field in API response"))
}

async fn transcribe_azure(wav_bytes: Vec<u8>, config: &Value) -> Result<String> {
    let endpoint = config["azure"]["endpoint"]
        .as_str()
        .ok_or_else(|| anyhow!("Azure endpoint not configured"))?
        .trim_end_matches('/');

    let api_key = config["azure"]["apiKey"]
        .as_str()
        .ok_or_else(|| anyhow!("Azure API key not configured"))?;

    let deployment = config["azure"]["whisperDeployment"]
        .as_str()
        .unwrap_or("whisper");

    let api_version = config["azure"]["apiVersion"]
        .as_str()
        .unwrap_or("2024-02-01");

    let url = format!(
        "{}/openai/deployments/{}/audio/transcriptions?api-version={}",
        endpoint, deployment, api_version
    );

    let file_part = reqwest::multipart::Part::bytes(wav_bytes)
        .file_name("audio.wav")
        .mime_str("audio/wav")?;

    let form = reqwest::multipart::Form::new()
        .part("file", file_part)
        .text("response_format", "json");

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;

    let resp = client
        .post(&url)
        .header("api-key", api_key)
        .multipart(form)
        .send()
        .await
        .map_err(|e| anyhow!("Request failed: {}", e))?;

    let status = resp.status();
    let body: Value = resp
        .json()
        .await
        .map_err(|e| anyhow!("Failed to parse response: {}", e))?;

    if !status.is_success() {
        let msg = body["error"]["message"]
            .as_str()
            .unwrap_or("Unknown API error");
        return Err(anyhow!("Azure API error {}: {}", status, msg));
    }

    body["text"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow!("No text field in Azure response"))
}

// ── Window helpers ─────────────────────────────────────────────────────────────

fn show_overlay(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("overlay") {
        let _ = w.show();
        let _ = w.set_ignore_cursor_events(true);
    }
}

fn hide_overlay(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("overlay") {
        let _ = w.hide();
    }
}

fn hide_overlay_after(app: &AppHandle, delay: Duration) {
    let app = app.clone();
    tokio::spawn(async move {
        tokio::time::sleep(delay).await;
        hide_overlay(&app);
    });
}

fn emit_overlay<S: serde::Serialize + Clone>(app: &AppHandle, event: &str, payload: S) {
    if let Some(w) = app.get_webview_window("overlay") {
        let _ = w.emit(event, payload);
    }
}
