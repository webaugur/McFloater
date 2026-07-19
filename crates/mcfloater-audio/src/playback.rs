use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat};
use rubato::{FftFixedIn, Resampler};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use thiserror::Error;
use tracing::{debug, warn};

use crate::SAM_SAMPLE_RATE;

/// Default silent lead-in before speech (device / PipeWire / BT often need ~300 ms to wake).
/// Override with `MCFLOATER_AUDIO_PREROLL_MS` (0 disables).
const DEFAULT_PREROLL_MS: u32 = 320;

/// Small silent tail so the last phoneme is not cut when the stream pauses.
const DEFAULT_POSTROLL_MS: u32 = 40;

#[derive(Debug, Error)]
pub enum PlaybackError {
    #[error("no audio output device available")]
    NoDevice,
    #[error("unsupported sample format: {0:?}")]
    UnsupportedFormat(SampleFormat),
    #[error("audio stream error: {0}")]
    Stream(String),
    #[error("resampling failed: {0}")]
    Resample(String),
}

fn env_ms(name: &str, default: u32) -> u32 {
    std::env::var(name)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(default)
}

fn silence_frames(sample_rate: u32, ms: u32) -> usize {
    if ms == 0 {
        return 0;
    }
    (u64::from(sample_rate) * u64::from(ms) / 1000) as usize
}

/// Play unsigned 8-bit mono PCM (SAM output) through the default output device.
pub fn play_pcm_u8_mono(samples: &[u8]) -> Result<(), PlaybackError> {
    if samples.is_empty() {
        return Ok(());
    }

    let f32_samples = samples
        .iter()
        .map(|&s| (f32::from(s) - 128.0) / 128.0)
        .collect::<Vec<f32>>();

    play_pcm_f32_mono(&f32_samples, SAM_SAMPLE_RATE)
}

/// Play signed 16-bit mono PCM at an arbitrary sample rate.
pub fn play_pcm_i16_mono(samples: &[i16], sample_rate: u32) -> Result<(), PlaybackError> {
    if samples.is_empty() {
        return Ok(());
    }
    let f32_samples: Vec<f32> = samples.iter().map(|&s| s as f32 / 32768.0).collect();
    play_pcm_f32_mono(&f32_samples, sample_rate)
}

/// Parse and play a WAV buffer (PCM 16-bit) from e.g. brain `/v1/tts`.
pub fn play_wav_bytes(wav: &[u8]) -> Result<(), PlaybackError> {
    let pcm = crate::wav_parse::parse_wav(wav)
        .map_err(|e| PlaybackError::Stream(e.to_string()))?;
    let mono = pcm.to_mono_f32();
    play_pcm_f32_mono(&mono, pcm.sample_rate)
}

/// Play 32-bit float mono PCM through the default output device.
///
/// Opens a fresh cpal stream each call. Desktop sinks (PipeWire/Pulse, BT) often
/// drop the first ~300 ms while the path wakes — we prepend silence at the
/// **device** rate so speech syllables are not eaten.
pub fn play_pcm_f32_mono(samples: &[f32], sample_rate: u32) -> Result<(), PlaybackError> {
    if samples.is_empty() {
        return Ok(());
    }

    let host = cpal::default_host();
    let device = host.default_output_device().ok_or(PlaybackError::NoDevice)?;
    let config = device
        .default_output_config()
        .map_err(|e| PlaybackError::Stream(e.to_string()))?;

    let device_rate = config.sample_rate().0;
    let channels = config.channels() as usize;
    let format = config.sample_format();
    let preroll_ms = env_ms("MCFLOATER_AUDIO_PREROLL_MS", DEFAULT_PREROLL_MS);
    let postroll_ms = env_ms("MCFLOATER_AUDIO_POSTROLL_MS", DEFAULT_POSTROLL_MS);

    debug!(
        device = %device.name().unwrap_or_else(|_| "unknown".into()),
        device_rate,
        channels,
        ?format,
        input_samples = samples.len(),
        input_rate = sample_rate,
        preroll_ms,
        postroll_ms,
        "starting PCM playback"
    );

    let resampled = if device_rate == sample_rate {
        samples.to_vec()
    } else {
        resample_mono(samples, sample_rate, device_rate)?
    };

    let pre_n = silence_frames(device_rate, preroll_ms);
    let post_n = silence_frames(device_rate, postroll_ms);
    let mut buffer = Vec::with_capacity(pre_n + resampled.len() + post_n);
    buffer.resize(pre_n, 0.0);
    buffer.extend_from_slice(&resampled);
    buffer.resize(pre_n + resampled.len() + post_n, 0.0);

    let playback = Arc::new(Mutex::new(buffer));
    let position = Arc::new(Mutex::new(0usize));
    let playback_done = Arc::new(Mutex::new(false));

    let playback_cb = playback.clone();
    let position_cb = position.clone();
    let done_cb = playback_done.clone();

    let stream = match format {
        SampleFormat::F32 => device.build_output_stream(
            &config.into(),
            move |output: &mut [f32], _| {
                write_output(output, channels, &playback_cb, &position_cb, &done_cb)
            },
            move |err| warn!(%err, "audio stream error"),
            None,
        ),
        SampleFormat::I16 => device.build_output_stream(
            &config.into(),
            move |output: &mut [i16], _| {
                write_output(output, channels, &playback_cb, &position_cb, &done_cb)
            },
            move |err| warn!(%err, "audio stream error"),
            None,
        ),
        SampleFormat::U16 => device.build_output_stream(
            &config.into(),
            move |output: &mut [u16], _| {
                write_output(output, channels, &playback_cb, &position_cb, &done_cb)
            },
            move |err| warn!(%err, "audio stream error"),
            None,
        ),
        other => return Err(PlaybackError::UnsupportedFormat(other)),
    }
    .map_err(|e| PlaybackError::Stream(e.to_string()))?;

    stream
        .play()
        .map_err(|e| PlaybackError::Stream(e.to_string()))?;

    // Wait until the callback has drained the buffer (includes pre/post silence).
    let total_frames = playback.lock().map_err(lock_err)?.len();
    let expected = Duration::from_secs_f64(total_frames as f64 / device_rate as f64);
    let deadline = Instant::now() + expected + Duration::from_millis(500);
    loop {
        let done = playback_done
            .lock()
            .map(|g| *g)
            .unwrap_or(true);
        if done || Instant::now() >= deadline {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    // Let the last callback buffer reach the DAC.
    thread::sleep(Duration::from_millis(50));

    stream
        .pause()
        .map_err(|e| PlaybackError::Stream(e.to_string()))?;

    Ok(())
}

fn write_output<T>(
    output: &mut [T],
    channels: usize,
    playback: &Arc<Mutex<Vec<f32>>>,
    position: &Arc<Mutex<usize>>,
    done: &Arc<Mutex<bool>>,
) where
    T: Sample + FromSample<f32>,
{
    let samples = match playback.lock() {
        Ok(samples) => samples,
        Err(_) => return,
    };

    let mut pos = match position.lock() {
        Ok(pos) => pos,
        Err(_) => return,
    };

    for frame in output.chunks_mut(channels) {
        let sample = if *pos < samples.len() {
            let value = samples[*pos];
            *pos += 1;
            value
        } else {
            if let Ok(mut finished) = done.lock() {
                *finished = true;
            }
            0.0
        };

        for channel in frame.iter_mut() {
            *channel = T::from_sample(sample);
        }
    }
}

fn resample_mono(input: &[f32], from_rate: u32, to_rate: u32) -> Result<Vec<f32>, PlaybackError> {
    if input.is_empty() {
        return Ok(Vec::new());
    }

    let mut resampler = FftFixedIn::<f32>::new(
        from_rate as usize,
        to_rate as usize,
        1024,
        2,
        1,
    )
    .map_err(|e| PlaybackError::Resample(e.to_string()))?;

    let mut out = Vec::with_capacity(resampler.output_frames_max() * 4);
    let mut chunk_start = 0usize;

    while chunk_start < input.len() {
        let chunk_end = (chunk_start + 1024).min(input.len());
        let chunk = &input[chunk_start..chunk_end];

        let mut padded = chunk.to_vec();
        if padded.len() < 1024 {
            padded.resize(1024, 0.0);
        }

        let resampled = resampler
            .process(&[padded], None)
            .map_err(|e| PlaybackError::Resample(e.to_string()))?;

        let valid_frames = if chunk_end == input.len() {
            let ratio = to_rate as f64 / from_rate as f64;
            ((chunk.len() as f64) * ratio).ceil() as usize
        } else {
            resampled[0].len()
        };

        out.extend_from_slice(&resampled[0][..valid_frames.min(resampled[0].len())]);
        chunk_start = chunk_end;
    }

    Ok(out)
}

fn lock_err<E: std::fmt::Display>(err: E) -> PlaybackError {
    PlaybackError::Stream(err.to_string())
}