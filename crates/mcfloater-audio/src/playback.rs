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

    let stream = match format {
        SampleFormat::F32 => build_stream::<f32>(
            &device,
            &config.into(),
            channels,
            Arc::clone(&playback),
            Arc::clone(&position),
            Arc::clone(&playback_done),
        )?,
        SampleFormat::I16 => build_stream::<i16>(
            &device,
            &config.into(),
            channels,
            Arc::clone(&playback),
            Arc::clone(&position),
            Arc::clone(&playback_done),
        )?,
        SampleFormat::U16 => build_stream::<u16>(
            &device,
            &config.into(),
            channels,
            Arc::clone(&playback),
            Arc::clone(&position),
            Arc::clone(&playback_done),
        )?,
        other => return Err(PlaybackError::UnsupportedFormat(other)),
    };

    stream
        .play()
        .map_err(|e| PlaybackError::Stream(e.to_string()))?;

    let total_samples = playback.lock().unwrap().len();
    let duration = Duration::from_secs_f64(total_samples as f64 / device_rate as f64);
    thread::sleep(duration + Duration::from_millis(100));

    Ok(())
}

fn build_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    channels: usize,
    playback: Arc<Mutex<Vec<f32>>>,
    position: Arc<Mutex<usize>>,
    playback_done: Arc<Mutex<bool>>,
) -> Result<cpal::Stream, PlaybackError>
where
    T: Sample + cpal::SizedSample + FromSample<f32>,
{
    let err_fn = |err| warn!(%err, "audio stream error");

    let stream = device
        .build_output_stream(
            config,
            move |data: &mut [T], _| {
                let buf = playback.lock().unwrap();
                let mut pos = position.lock().unwrap();

                for frame in data.chunks_mut(channels) {
                    let sample = if *pos < buf.len() {
                        buf[*pos]
                    } else {
                        0.0
                    };
                    let converted = T::from_sample(sample);
                    for ch in frame.iter_mut() {
                        *ch = converted;
                    }
                    if *pos < buf.len() {
                        *pos += 1;
                    }
                }

                if *pos >= buf.len() {
                    *playback_done.lock().unwrap() = true;
                }
            },
            err_fn,
            None,
        )
        .map_err(|e| PlaybackError::Stream(e.to_string()))?;

    Ok(stream)
}

fn resample_mono(samples: &[f32], from_rate: u32, to_rate: u32) -> Result<Vec<f32>, PlaybackError> {
    if samples.is_empty() {
        return Ok(Vec::new());
    }

    let mut resampler = FftFixedIn::<f32>::new(
        to_rate as usize,
        from_rate as usize,
        1024,
        1,
        1,
    )
    .map_err(|e| PlaybackError::Resample(e.to_string()))?;

    let mut output = Vec::new();
    let chunk_size = resampler.input_frames_max();

    for chunk in samples.chunks(chunk_size) {
        let mut padded = chunk.to_vec();
        padded.resize(chunk_size, 0.0);
        let resampled = resampler
            .process(&[padded], None)
            .map_err(|e| PlaybackError::Resample(e.to_string()))?;
        output.extend_from_slice(&resampled[0]);
    }

    Ok(output)
}
