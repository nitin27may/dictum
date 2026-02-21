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

/// Called on hotkey press. Starts cpal capture and drives the overlay via events.
pub async fn on_press(app: AppHandle, state: AppState) {
    // Re-entrancy guard
    {
        let mut down = state.is_recording.lock().await;
        if *down {
            return;
        }
        *down = true;
    }

    let start_time = Instant::now();
    *state.pressed_at.lock().await = Some(start_time);

    emit_overlay(&app, "recording-started", ());

    let mut capture = state.audio_capture.lock().await;
    if let Err(e) = capture.start(app.clone()) {
        log::error!("Failed to start capture: {}", e);
        emit_overlay(&app, "recording-error", e.to_string());
        *state.is_recording.lock().await = false;
        return;
    }

    // Duration ticker — emits elapsed seconds every second while recording.
    // Also enforces the hard MAX_RECORD_SECS cap.
    let ticker_app = app.clone();
    let ticker_state = state.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            if !*ticker_state.is_recording.lock().await {
                break;
            }
            let elapsed = start_time.elapsed().as_secs();
            emit_overlay(&ticker_app, "recording-tick", elapsed);
            if elapsed >= MAX_RECORD_SECS {
                log::info!("Max recording duration ({} sec) reached, auto-stopping", MAX_RECORD_SECS);
                tauri::async_runtime::spawn(on_release(ticker_app.clone(), ticker_state.clone()));
                break;
            }
        }
    });
}

/// Called on hotkey release. Stops capture, transcribes, injects text.
pub async fn on_release(app: AppHandle, state: AppState) {
    let pressed_at = state.pressed_at.lock().await.take();
    *state.is_recording.lock().await = false;

    let hold_ms = pressed_at
        .map(|t| t.elapsed().as_millis())
        .unwrap_or(0);

    let (samples, sample_rate) = {
        let mut capture = state.audio_capture.lock().await;
        if !capture.is_recording() {
            return;
        }
        match capture.stop() {
            Ok(s) => s,
            Err(e) => {
                log::error!("Failed to stop capture: {}", e);
                emit_overlay(&app, "recording-error", e.to_string());
                return;
            }
        }
    };

    // Tap too short — ignore silently
    if hold_ms < MIN_HOLD_MS {
        emit_overlay(&app, "recording-cancelled", ());
        return;
    }

    if samples.is_empty() {
        emit_overlay(&app, "recording-error", "No audio captured".to_string());
        return;
    }

    emit_overlay(&app, "processing-started", ());

    // Encode WAV
    let wav_bytes = match crate::audio::encoder::encode_wav(&samples, sample_rate) {
        Ok(b) => b,
        Err(e) => {
            log::error!("WAV encode failed: {}", e);
            emit_overlay(&app, "recording-error", e.to_string());
            return;
        }
    };

    log::info!(
        "Stopped recording: {:.1}s, {} samples @ {}Hz, {} WAV bytes",
        hold_ms as f64 / 1000.0,
        samples.len(),
        sample_rate,
        wav_bytes.len()
    );

    // Read API config from AppState (pushed by JS on startup / settings change)
    let config = state.api_config.lock().await.clone();

    match transcribe(wav_bytes, &config).await {
        Ok(text) => {
            let text = text.trim().to_string();
            log::info!("Transcription: \"{}\"", text);

            if text.is_empty() {
                emit_overlay(&app, "recording-error", "No speech detected".to_string());
                return;
            }

            // Smart keywords: check for trailing "rephrase" / "rewrite" trigger
            let smart_enabled = config["smartKeywords"]["enabled"].as_bool().unwrap_or(false);
            let final_text = if smart_enabled {
                if let Some(kw) = crate::keywords::detect_keyword(&text) {
                    log::info!("Keyword detected: action='{}', clean_text='{}'", kw.action, kw.clean_text);
                    emit_overlay(&app, "processing-gpt", ());

                    match rephrase(&kw.clean_text, &kw.action, &config).await {
                        Ok(rephrased) => {
                            let rephrased = rephrased.trim().to_string();
                            if rephrased.is_empty() {
                                log::warn!("GPT returned empty, falling back to clean text");
                                kw.clean_text
                            } else {
                                log::info!("Rephrased: \"{}\"", rephrased);
                                rephrased
                            }
                        }
                        Err(e) => {
                            log::error!("GPT rephrase failed: {}, falling back to clean text", e);
                            kw.clean_text
                        }
                    }
                } else {
                    text
                }
            } else {
                text
            };

            let char_count = final_text.len();

            if let Err(e) = crate::injection::inject_text(&final_text).await {
                log::error!("Text injection failed: {}", e);
                emit_overlay(&app, "recording-error", format!("Injection failed: {e}"));
                return;
            }

            emit_overlay(&app, "recording-success", char_count);
        }
        Err(e) => {
            log::error!("Transcription failed: {}", e);
            emit_overlay(&app, "recording-error", e.to_string());
        }
    }
}

// ── Transcription ─────────────────────────────────────────────────────────────

async fn transcribe(wav_bytes: Vec<u8>, config: &Value) -> Result<String> {
    let provider = config["provider"].as_str().unwrap_or("openai");

    if provider == "azure" {
        transcribe_azure(wav_bytes, config).await
    } else {
        transcribe_openai(wav_bytes, config).await
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

// ── GPT Rephrase ──────────────────────────────────────────────────────────────

fn build_system_prompt(action: &str) -> String {
    match action {
        "rephrase" => concat!(
            "You are a writing assistant. The user will give you text that was transcribed from speech. ",
            "Your job:\n",
            "1. Infer the context from the text itself (email, chat message, code comment, professional note, etc.)\n",
            "2. Rephrase the text to be clear, well-written, and appropriate for the inferred context\n",
            "3. Fix any grammar issues typical of speech-to-text transcription\n",
            "4. Preserve the original meaning and approximate length\n",
            "5. Return ONLY the rephrased text — no explanations, labels, or formatting"
        ).to_string(),
        _ => "Rephrase the following text clearly and concisely. Return ONLY the result.".to_string(),
    }
}

async fn rephrase(text: &str, action: &str, config: &Value) -> Result<String> {
    let provider = config["provider"].as_str().unwrap_or("openai");

    if provider == "azure" {
        rephrase_azure(text, action, config).await
    } else {
        rephrase_openai(text, action, config).await
    }
}

async fn rephrase_openai(text: &str, action: &str, config: &Value) -> Result<String> {
    let api_key = config["openai"]["apiKey"]
        .as_str()
        .map(|s| s.to_string())
        .or_else(|| std::env::var("OPENAI_API_KEY").ok())
        .ok_or_else(|| anyhow!("OpenAI API key not configured"))?;

    let base_url = config["openai"]["baseUrl"]
        .as_str()
        .unwrap_or("https://api.openai.com");

    let model = config["openai"]["gptModel"]
        .as_str()
        .unwrap_or("gpt-4o-mini");

    let url = format!("{}/v1/chat/completions", base_url.trim_end_matches('/'));

    let body = serde_json::json!({
        "model": model,
        "messages": [
            { "role": "system", "content": build_system_prompt(action) },
            { "role": "user", "content": text }
        ],
        "temperature": 0.7,
        "max_tokens": 2048
    });

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;

    let resp = client
        .post(&url)
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| anyhow!("GPT request failed: {}", e))?;

    let status = resp.status();
    let resp_body: Value = resp
        .json()
        .await
        .map_err(|e| anyhow!("Failed to parse GPT response: {}", e))?;

    if !status.is_success() {
        let msg = resp_body["error"]["message"]
            .as_str()
            .unwrap_or("Unknown GPT API error");
        return Err(anyhow!("OpenAI GPT error {}: {}", status, msg));
    }

    resp_body["choices"][0]["message"]["content"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow!("No content in GPT response"))
}

async fn rephrase_azure(text: &str, action: &str, config: &Value) -> Result<String> {
    let endpoint = config["azure"]["endpoint"]
        .as_str()
        .ok_or_else(|| anyhow!("Azure endpoint not configured"))?
        .trim_end_matches('/');

    let api_key = config["azure"]["apiKey"]
        .as_str()
        .ok_or_else(|| anyhow!("Azure API key not configured"))?;

    let deployment = config["azure"]["gptDeployment"]
        .as_str()
        .unwrap_or("gpt-4o-mini");

    let api_version = config["azure"]["apiVersion"]
        .as_str()
        .unwrap_or("2024-02-01");

    let url = format!(
        "{}/openai/deployments/{}/chat/completions?api-version={}",
        endpoint, deployment, api_version
    );

    let body = serde_json::json!({
        "messages": [
            { "role": "system", "content": build_system_prompt(action) },
            { "role": "user", "content": text }
        ],
        "temperature": 0.7,
        "max_tokens": 2048
    });

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;

    let resp = client
        .post(&url)
        .header("api-key", api_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| anyhow!("GPT request failed: {}", e))?;

    let status = resp.status();
    let resp_body: Value = resp
        .json()
        .await
        .map_err(|e| anyhow!("Failed to parse GPT response: {}", e))?;

    if !status.is_success() {
        let msg = resp_body["error"]["message"]
            .as_str()
            .unwrap_or("Unknown GPT API error");
        return Err(anyhow!("Azure GPT error {}: {}", status, msg));
    }

    resp_body["choices"][0]["message"]["content"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow!("No content in Azure GPT response"))
}

// ── Window helpers ─────────────────────────────────────────────────────────────

fn emit_overlay<S: serde::Serialize + Clone>(app: &AppHandle, event: &str, payload: S) {
    if let Some(w) = app.get_webview_window("overlay") {
        match w.emit(event, payload) {
            Ok(_) => log::debug!("emit_overlay: '{}'", event),
            Err(e) => log::error!("emit_overlay '{}' failed: {}", event, e),
        }
    } else {
        log::error!("emit_overlay: overlay window not found for '{}'", event);
    }
}
