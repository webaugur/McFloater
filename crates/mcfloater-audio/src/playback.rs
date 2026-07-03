use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat};
use rubato::{FftFixedIn, Resampler};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use thiserror::Error;
use tracing::{debug, warn};

use crate::SAM_SAMPLE_RATE;

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

/// Play 32-bit float mono PCM through the default output device.
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

    debug!(
        device = %device.name().unwrap_or_else(|_| "unknown".into()),
        device_rate,
        channels,
        ?format,
        input_samples = samples.len(),
        input_rate = sample_rate,
        "starting PCM playback"
    );

    let resampled = if device_rate == sample_rate {
        samples.to_vec()
    } else {
        resample_mono(samples, sample_rate, device_rate)?
    };

    let playback = Arc::new(Mutex::new(resampled));
    let position = Arc::new(Mutex::new(0usize));
    let playback_done = Arc::new(Mutex::new(false));

    let playback_cb = playback.clone();
    let position_cb = position.clone();
    let done_cb = playback_done.clone();

    let stream = match format {
        SampleFormat::F32 => device.build_output_stream(
            &config.into(),
            move |output: &mut [f32], _| write_output(output, channels, &playback_cb, &position_cb, &done_cb),
            move |err| warn!(%err, "audio stream error"),
            None,
        ),
        SampleFormat::I16 => device.build_output_stream(
            &config.into(),
            move |output: &mut [i16], _| write_output(output, channels, &playback_cb, &position_cb, &done_cb),
            move |err| warn!(%err, "audio stream error"),
            None,
        ),
        SampleFormat::U16 => device.build_output_stream(
            &config.into(),
            move |output: &mut [u16], _| write_output(output, channels, &playback_cb, &position_cb, &done_cb),
            move |err| warn!(%err, "audio stream error"),
            None,
        ),
        other => return Err(PlaybackError::UnsupportedFormat(other)),
    }
    .map_err(|e| PlaybackError::Stream(e.to_string()))?;

    stream
        .play()
        .map_err(|e| PlaybackError::Stream(e.to_string()))?;

    let total_frames = playback.lock().map_err(lock_err)?.len();
    let duration = Duration::from_secs_f64(total_frames as f64 / device_rate as f64);
    thread::sleep(duration + Duration::from_millis(100));

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