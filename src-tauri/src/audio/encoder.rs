use anyhow::Result;
use hound::{WavSpec, WavWriter};
use std::io::Cursor;

const SAMPLE_RATE: u32 = 16_000;
const CHANNELS: u16 = 1;
const BITS_PER_SAMPLE: u16 = 16;

/// Encodes f32 PCM samples to WAV bytes (16-bit PCM, 16kHz, mono).
/// Whisper accepts WAV/MP3/MP4/WEBM; WAV is simplest with no external deps.
pub fn encode_wav(samples: &[f32]) -> Result<Vec<u8>> {
    let spec = WavSpec {
        channels: CHANNELS,
        sample_rate: SAMPLE_RATE,
        bits_per_sample: BITS_PER_SAMPLE,
        sample_format: hound::SampleFormat::Int,
    };

    let mut buf = Cursor::new(Vec::new());
    let mut writer = WavWriter::new(&mut buf, spec)?;

    for &sample in samples {
        // Clamp to [-1.0, 1.0] before converting to i16
        let clamped = sample.clamp(-1.0, 1.0);
        let pcm = (clamped * i16::MAX as f32) as i16;
        writer.write_sample(pcm)?;
    }

    writer.finalize()?;
    Ok(buf.into_inner())
}

/// Returns approximate duration of the audio in seconds.
pub fn samples_duration_secs(sample_count: usize) -> f32 {
    sample_count as f32 / SAMPLE_RATE as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_wav_produces_valid_header() {
        let samples: Vec<f32> = (0..16000).map(|i| (i as f32 / 16000.0).sin()).collect();
        let wav_bytes = encode_wav(&samples).unwrap();
        // WAV files start with "RIFF"
        assert_eq!(&wav_bytes[0..4], b"RIFF");
        assert_eq!(&wav_bytes[8..12], b"WAVE");
    }

    #[test]
    fn encode_wav_clamps_out_of_range_samples() {
        let samples = vec![2.0f32, -2.0f32, 0.5f32];
        let result = encode_wav(&samples);
        assert!(result.is_ok());
    }
}
