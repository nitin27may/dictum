use anyhow::{anyhow, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, SampleFormat, Stream, StreamConfig};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};

const SAMPLE_RATE: u32 = 16_000;
const CHANNELS: u16 = 1;
const LEVEL_EMIT_INTERVAL_MS: u64 = 50;

/// Wraps cpal::Stream to make it Send.
///
/// cpal::Stream is not Send on macOS (CoreAudio handles thread affinity).
/// Safety: we only start/stop the stream while holding the AppState Mutex,
/// so exclusive access is guaranteed — no concurrent access occurs.
struct SendStream(Stream);
unsafe impl Send for SendStream {}

pub struct AudioCapture {
    buffer: Arc<Mutex<Vec<f32>>>,
    stream: Option<SendStream>,
    device_name: Option<String>,
    actual_sample_rate: u32,
}

impl AudioCapture {
    pub fn new(device_name: Option<String>) -> Self {
        Self {
            buffer: Arc::new(Mutex::new(Vec::new())),
            stream: None,
            device_name,
            actual_sample_rate: SAMPLE_RATE,
        }
    }

    pub fn start(&mut self, app_handle: AppHandle) -> Result<()> {
        let host = cpal::default_host();
        let device = self.find_device(&host)?;
        log::info!(
            "Recording from device: \"{}\"",
            device.name().unwrap_or_else(|_| "unknown".into())
        );
        let (config, sample_rate) = self.find_config(&device)?;
        self.actual_sample_rate = sample_rate;

        let buffer = Arc::clone(&self.buffer);
        let buffer_for_level = Arc::clone(&self.buffer);

        // Clone handle for level emitter; original for silence detector
        let app_for_level = app_handle.clone();

        let mut last_level_emit = std::time::Instant::now();
        let emit_interval = std::time::Duration::from_millis(LEVEL_EMIT_INTERVAL_MS);

        let stream = device.build_input_stream(
            &config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                if let Ok(mut buf) = buffer.lock() {
                    buf.extend_from_slice(data);
                }
                let now = std::time::Instant::now();
                if now.duration_since(last_level_emit) >= emit_interval {
                    last_level_emit = now;
                    let level = compute_rms(data);
                    let _ = app_for_level.emit("audio-level", level);
                }
            },
            move |err| {
                log::error!("Audio capture error: {}", err);
            },
            None,
        )?;

        stream.play()?;
        self.stream = Some(SendStream(stream));

        // Silence detector — fires after first 2s if RMS is near-zero
        let buf_check = Arc::clone(&buffer_for_level);
        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
            if let Ok(buf) = buf_check.lock() {
                if !buf.is_empty() {
                    let rms = compute_rms(&buf);
                    log::info!("Audio RMS at 2s: {:.6}", rms);
                    if rms < 0.001 {
                        let _ = app_handle.emit("audio-silence-detected", ());
                        log::warn!("Silence detected (RMS {:.6}) — mic may be muted or permission denied", rms);
                    }
                }
            }
        });

        Ok(())
    }

    /// Stops capture and returns (samples, sample_rate).
    pub fn stop(&mut self) -> Result<(Vec<f32>, u32)> {
        self.stream.take(); // drop stream to stop capture

        let samples = self
            .buffer
            .lock()
            .map_err(|_| anyhow!("Buffer lock poisoned"))?
            .drain(..)
            .collect();

        Ok((samples, self.actual_sample_rate))
    }

    pub fn is_recording(&self) -> bool {
        self.stream.is_some()
    }

    fn find_device(&self, host: &cpal::Host) -> Result<Device> {
        if let Some(name) = &self.device_name {
            host.input_devices()?
                .find(|d| d.name().map(|n| &n == name).unwrap_or(false))
                .ok_or_else(|| anyhow!("Audio device '{}' not found", name))
        } else {
            host.default_input_device()
                .ok_or_else(|| anyhow!("No default input device available"))
        }
    }

    fn find_config(&self, device: &Device) -> Result<(StreamConfig, u32)> {
        let supported = device.supported_input_configs()?;
        for range in supported {
            if range.channels() == CHANNELS
                && range.sample_format() == SampleFormat::F32
                && range.min_sample_rate().0 <= SAMPLE_RATE
                && range.max_sample_rate().0 >= SAMPLE_RATE
            {
                return Ok((
                    StreamConfig {
                        channels: CHANNELS,
                        sample_rate: cpal::SampleRate(SAMPLE_RATE),
                        buffer_size: cpal::BufferSize::Default,
                    },
                    SAMPLE_RATE,
                ));
            }
        }

        let default = device.default_input_config()?;
        let actual_rate = default.sample_rate().0;
        log::warn!(
            "Could not find exact 16kHz mono f32 config; using default: {:?} ({}Hz)",
            default,
            actual_rate
        );
        Ok((default.into(), actual_rate))
    }
}

fn compute_rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum_sq: f32 = samples.iter().map(|s| s * s).sum();
    (sum_sq / samples.len() as f32).sqrt()
}

pub fn list_input_devices() -> Result<Vec<String>> {
    let host = cpal::default_host();
    let devices = host
        .input_devices()?
        .filter_map(|d| d.name().ok())
        .collect();
    Ok(devices)
}
