//! Microphone capture for push-to-talk / listen windows (Tower).

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, StreamConfig};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use thiserror::Error;
use tracing::{debug, info, warn};

use crate::write_wav::write_wav_i16_mono_bytes;

#[derive(Debug, Error)]
pub enum CaptureError {
    #[error("no audio input device available")]
    NoDevice,
    #[error("unsupported input format: {0}")]
    Unsupported(String),
    #[error("capture failed: {0}")]
    Stream(String),
    #[error("wav encode: {0}")]
    Wav(String),
}

/// Default listen window (seconds). Override with `MCFLOATER_LISTEN_SECS`.
pub fn listen_secs() -> f32 {
    let s: f32 = std::env::var("MCFLOATER_LISTEN_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(5.0);
    s.clamp(1.0, 30.0)
}

/// Record mono audio and return a 16-bit PCM WAV (16 kHz preferred for Whisper).
pub fn record_wav_mono(duration: Duration) -> Result<Vec<u8>, CaptureError> {
    let host = cpal::default_host();
    let device = select_input_device(&host)?;
    let name = device.name().unwrap_or_else(|_| "unknown".into());
    let config = device
        .default_input_config()
        .map_err(|e| CaptureError::Stream(e.to_string()))?;

    info!(
        device = %name,
        sample_format = ?config.sample_format(),
        sample_rate = config.sample_rate().0,
        channels = config.channels(),
        secs = duration.as_secs_f32(),
        "recording mic"
    );

    let sample_rate = config.sample_rate().0;
    let channels = config.channels() as usize;
    let samples: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));
    let samples_cb = samples.clone();

    let stream_config: StreamConfig = config.clone().into();
    let err_fn = |e| warn!(%e, "input stream error");

    let stream = match config.sample_format() {
        SampleFormat::F32 => device
            .build_input_stream(
                &stream_config,
                move |data: &[f32], _| {
                    if let Ok(mut buf) = samples_cb.lock() {
                        push_frames(&mut buf, data, channels);
                    }
                },
                err_fn,
                None,
            )
            .map_err(|e| CaptureError::Stream(e.to_string()))?,
        SampleFormat::I16 => device
            .build_input_stream(
                &stream_config,
                move |data: &[i16], _| {
                    if let Ok(mut buf) = samples_cb.lock() {
                        let f: Vec<f32> = data.iter().map(|&s| s as f32 / 32768.0).collect();
                        push_frames(&mut buf, &f, channels);
                    }
                },
                err_fn,
                None,
            )
            .map_err(|e| CaptureError::Stream(e.to_string()))?,
        SampleFormat::U16 => device
            .build_input_stream(
                &stream_config,
                move |data: &[u16], _| {
                    if let Ok(mut buf) = samples_cb.lock() {
                        let f: Vec<f32> = data
                            .iter()
                            .map(|&s| (s as f32 / 32768.0) - 1.0)
                            .collect();
                        push_frames(&mut buf, &f, channels);
                    }
                },
                err_fn,
                None,
            )
            .map_err(|e| CaptureError::Stream(e.to_string()))?,
        other => {
            return Err(CaptureError::Unsupported(format!("{other:?}")));
        }
    };

    stream
        .play()
        .map_err(|e| CaptureError::Stream(e.to_string()))?;
    thread::sleep(duration);
    drop(stream);

    let mono = samples.lock().map_err(|e| CaptureError::Stream(e.to_string()))?;
    if mono.is_empty() {
        return Err(CaptureError::Stream("no samples captured".into()));
    }

    // Resample to 16 kHz for Whisper if needed (linear — good enough for STT).
    let target_rate = 16_000u32;
    let mono16 = if sample_rate == target_rate {
        mono.clone()
    } else {
        resample_linear(&mono, sample_rate, target_rate)
    };

    let i16s: Vec<i16> = mono16
        .iter()
        .map(|&s| (s.clamp(-1.0, 1.0) * 32767.0) as i16)
        .collect();

    debug!(frames = i16s.len(), rate = target_rate, "capture complete");
    write_wav_i16_mono_bytes(&i16s, target_rate).map_err(|e| CaptureError::Wav(e.to_string()))
}

fn select_input_device(host: &cpal::Host) -> Result<cpal::Device, CaptureError> {
    if let Ok(name) = std::env::var("MCFLOATER_MIC_DEVICE") {
        let want = name.trim().to_ascii_lowercase();
        if !want.is_empty() {
            if let Ok(devices) = host.input_devices() {
                for d in devices {
                    if let Ok(n) = d.name() {
                        if n.to_ascii_lowercase().contains(&want) {
                            return Ok(d);
                        }
                    }
                }
            }
            warn!(%name, "MCFLOATER_MIC_DEVICE not found — using default");
        }
    }
    host.default_input_device().ok_or(CaptureError::NoDevice)
}

fn push_frames(out: &mut Vec<f32>, interleaved: &[f32], channels: usize) {
    if channels <= 1 {
        out.extend_from_slice(interleaved);
        return;
    }
    for frame in interleaved.chunks(channels) {
        let sum: f32 = frame.iter().sum();
        out.push(sum / channels as f32);
    }
}

fn resample_linear(input: &[f32], from: u32, to: u32) -> Vec<f32> {
    if input.is_empty() || from == 0 {
        return Vec::new();
    }
    let ratio = to as f64 / from as f64;
    let out_len = ((input.len() as f64) * ratio).round().max(1.0) as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let src = i as f64 / ratio;
        let i0 = src.floor() as usize;
        let i1 = (i0 + 1).min(input.len() - 1);
        let t = (src - i0 as f64) as f32;
        out.push(input[i0] * (1.0 - t) + input[i1] * t);
    }
    out
}

