use std::fs::File;
use std::io::{self, Write};
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum WriteWavError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
}

/// Write mono 32-bit float PCM as 16-bit WAV.
pub fn write_wav_f32_mono(path: &Path, samples: &[f32], sample_rate: u32) -> Result<(), WriteWavError> {
    let pcm: Vec<i16> = samples
        .iter()
        .map(|&sample| {
            let clamped = sample.clamp(-1.0, 1.0);
            (clamped * i16::MAX as f32) as i16
        })
        .collect();
    write_wav_i16_mono(path, &pcm, sample_rate)
}

/// Write mono unsigned 8-bit PCM as 16-bit WAV.
pub fn write_wav_u8_mono(path: &Path, samples: &[u8], sample_rate: u32) -> Result<(), WriteWavError> {
    let pcm: Vec<i16> = samples
        .iter()
        .map(|&sample| {
            let centered = (sample as i32) - 128;
            (centered << 8) as i16
        })
        .collect();
    write_wav_i16_mono(path, &pcm, sample_rate)
}

fn write_wav_i16_mono(path: &Path, samples: &[i16], sample_rate: u32) -> Result<(), WriteWavError> {
    let mut file = File::create(path)?;
    let data_size = (samples.len() * 2) as u32;
    let channels: u16 = 1;
    let bits_per_sample: u16 = 16;
    let byte_rate = sample_rate * channels as u32 * bits_per_sample as u32 / 8;
    let block_align = channels * bits_per_sample / 8;

    file.write_all(b"RIFF")?;
    file.write_all(&(36 + data_size).to_le_bytes())?;
    file.write_all(b"WAVE")?;
    file.write_all(b"fmt ")?;
    file.write_all(&16u32.to_le_bytes())?;
    file.write_all(&1u16.to_le_bytes())?; // PCM
    file.write_all(&channels.to_le_bytes())?;
    file.write_all(&sample_rate.to_le_bytes())?;
    file.write_all(&byte_rate.to_le_bytes())?;
    file.write_all(&block_align.to_le_bytes())?;
    file.write_all(&bits_per_sample.to_le_bytes())?;
    file.write_all(b"data")?;
    file.write_all(&data_size.to_le_bytes())?;

    for sample in samples {
        file.write_all(&sample.to_le_bytes())?;
    }

    Ok(())
}