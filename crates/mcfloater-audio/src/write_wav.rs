use std::io;
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
    let bytes = write_wav_i16_mono_bytes(samples, sample_rate)?;
    std::fs::write(path, bytes)?;
    Ok(())
}

/// Encode mono 16-bit PCM as an in-memory WAV (for STT upload).
pub fn write_wav_i16_mono_bytes(samples: &[i16], sample_rate: u32) -> Result<Vec<u8>, WriteWavError> {
    let data_size = (samples.len() * 2) as u32;
    let channels: u16 = 1;
    let bits_per_sample: u16 = 16;
    let byte_rate = sample_rate * channels as u32 * bits_per_sample as u32 / 8;
    let block_align = channels * bits_per_sample / 8;

    let mut out = Vec::with_capacity(44 + samples.len() * 2);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_size).to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&byte_rate.to_le_bytes());
    out.extend_from_slice(&block_align.to_le_bytes());
    out.extend_from_slice(&bits_per_sample.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_size.to_le_bytes());
    for sample in samples {
        out.extend_from_slice(&sample.to_le_bytes());
    }
    Ok(out)
}