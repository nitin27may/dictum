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

/// Called on hotkey press.
/// Waits for MIN_HOLD_MS before starting mic capture. If the key is released
/// before the threshold, the keypress is replayed (tap-through) and no
/// recording occurs. This prevents single-key hotkeys like Space from
/// blocking normal typing.
pub async fn on_press(app: AppHandle, state: AppState) {
    // Ignore events from our own key replay
    if state.is_replaying.load(std::sync::atomic::Ordering::SeqCst) {
        return;
    }

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

    // Wait for hold threshold before starting the mic
    tokio::time::sleep(Duration::from_millis(MIN_HOLD_MS as u64)).await;

    // Check if key was already released during the wait (tap)
    if !*state.is_recording.lock().await {
        return; // on_release already ran and handled the tap-through
    }

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
    let was_recording = {
        let mut r = state.is_recording.lock().await;
        let v = *r;
        *r = false;
        v
    };

    if !was_recording {
        return; // Already handled
    }

    let hold_ms = pressed_at
        .map(|t| t.elapsed().as_millis())
        .unwrap_or(0);

    // Tap too short — mic never started, replay the consumed keypress
    if hold_ms < MIN_HOLD_MS {
        let hotkey = state.current_hotkey.lock().await.clone();
        crate::injection::replay_shortcut_key(&hotkey);
        log::debug!("Short tap ({}ms), replayed key: {}", hold_ms, hotkey);
        return;
    }

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

            // Smart keywords: check for trigger phrases anywhere in the text
            let smart_enabled = config["smartKeywords"]["enabled"].as_bool().unwrap_or(false);
            log::info!("Smart keywords enabled: {} (raw: {})", smart_enabled, config["smartKeywords"]["enabled"]);
            let final_text = if smart_enabled {
                if let Some(kw) = crate::keywords::detect_keyword(&text) {
                    log::info!(
                        "Keyword detected: action='{}', format={:?}, clean_text='{}'",
                        kw.action, kw.format, kw.clean_text
                    );
                    emit_overlay(&app, "processing-gpt", ());

                    match rephrase(&kw.clean_text, &kw.action, kw.format.as_deref(), &config).await {
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

fn build_system_prompt(action: &str, format: Option<&str>) -> String {
    match (action, format) {
        ("rephrase", Some(fmt)) => format!(
            "You are a writing assistant that rewrites speech-to-text transcriptions.\n\n\
            TASK: Rewrite the user's transcribed text as a properly formatted {fmt}.\n\n\
            CRITICAL RULES:\n\
            - Actually REWRITE the text — don't just clean it up. Improve word choice, sentence structure, and flow.\n\
            - Use REAL LINE BREAKS (newline characters) to separate paragraphs, greeting, body, and sign-off.\n\
            - Apply the full structure and layout expected for a {fmt}.\n\
            - Fix grammar issues from speech-to-text.\n\
            - Preserve the original meaning and intent.\n\
            - Return ONLY the formatted result. No explanations, no labels, no \"Here's the email:\" prefix.\n\n\
            FORMAT RULES FOR {fmt_upper}:\n{guidelines}",
            fmt = fmt,
            fmt_upper = fmt.to_uppercase(),
            guidelines = format_guidelines(fmt),
        ),
        ("rephrase", None) => concat!(
            "You are a writing assistant that rewrites speech-to-text transcriptions.\n\n",
            "TASK: Rewrite the user's transcribed text to be clear, professional, and well-written.\n\n",
            "CRITICAL RULES:\n",
            "- Actually REWRITE the text — don't just clean it up. Improve word choice, sentence structure, and flow.\n",
            "- Infer the context (email, chat, note, etc.) and apply appropriate formatting.\n",
            "- Use REAL LINE BREAKS (newline characters) to separate paragraphs where appropriate.\n",
            "- Fix grammar issues from speech-to-text.\n",
            "- Preserve the original meaning.\n",
            "- Return ONLY the rewritten text. No explanations, no labels, no meta-commentary."
        ).to_string(),
        _ => "Rewrite the following text clearly and concisely. Use proper formatting with line breaks. Return ONLY the result.".to_string(),
    }
}

fn format_guidelines(fmt: &str) -> &'static str {
    match fmt {
        "email" | "formal email" | "professional email" => {
            "- Line 1: Greeting (e.g. \"Hi Ryan,\")\n\
             - Line 2: BLANK LINE\n\
             - Lines 3+: Body paragraphs, each separated by a blank line\n\
             - Then: BLANK LINE\n\
             - Then: Sign-off (e.g. \"Best regards,\")\n\
             - Then: Your name on its own line\n\
             - Use professional, clear language — rewrite awkward phrasing\n\
             - Do NOT include a subject line\n\
             - If no name is mentioned, use \"Hi,\" or \"Hello,\""
        }
        "casual message" | "message" | "text message" => {
            "- Keep it short — 1-3 sentences max\n\
             - Conversational and friendly tone\n\
             - No formal greeting/sign-off\n\
             - Rewrite to sound natural, not robotic"
        }
        "slack message" | "teams message" | "chat message" => {
            "- Keep it concise and direct — one short paragraph\n\
             - Semi-casual workplace tone\n\
             - Lead with the key point\n\
             - No greeting/sign-off needed\n\
             - Rewrite to be clear and scannable"
        }
        "professional" | "formal" => {
            "- Use professional, polished language\n\
             - Separate paragraphs with blank lines\n\
             - Be clear, direct, and well-organized\n\
             - Actually rewrite — don't just fix typos"
        }
        "casual" | "friendly" => {
            "- Warm, conversational tone\n\
             - Natural and approachable\n\
             - Contractions and informal phrasing OK"
        }
        "bullet points" | "bullets" => {
            "- Convert into clear bullet points, one per line\n\
             - Use '- ' prefix for each bullet\n\
             - Each bullet = one concise, complete thought\n\
             - Group related points together\n\
             - Use a blank line between groups if needed"
        }
        "summary" => {
            "- Condense into 1-3 well-written sentences\n\
             - Lead with the most important information\n\
             - Cut filler and repetition"
        }
        "code comment" | "comment" => {
            "- Concise and technical\n\
             - Explain the 'why', not the 'what'\n\
             - 1-2 lines max"
        }
        _ => {
            "- Apply proper formatting with line breaks between sections\n\
             - Use the appropriate tone and structure\n\
             - Actually rewrite — don't just clean up"
        }
    }
}

async fn rephrase(text: &str, action: &str, format: Option<&str>, config: &Value) -> Result<String> {
    let provider = config["provider"].as_str().unwrap_or("openai");

    if provider == "azure" {
        rephrase_azure(text, action, format, config).await
    } else {
        rephrase_openai(text, action, format, config).await
    }
}

async fn rephrase_openai(text: &str, action: &str, format: Option<&str>, config: &Value) -> Result<String> {
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
            { "role": "system", "content": build_system_prompt(action, format) },
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

async fn rephrase_azure(text: &str, action: &str, format: Option<&str>, config: &Value) -> Result<String> {
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
            { "role": "system", "content": build_system_prompt(action, format) },
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
