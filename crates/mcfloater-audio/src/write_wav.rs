use std::fs::File;
use std::io::Write;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WriteWavError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Write unsigned 8-bit mono PCM as a WAV file.
pub fn write_wav_u8_mono(path: &Path, samples: &[u8], sample_rate: u32) -> Result<(), WriteWavError> {
    let f32_samples: Vec<f32> = samples
        .iter()
        .map(|&s| (f32::from(s) - 128.0) / 128.0)
        .collect();
    write_wav_f32_mono(path, &f32_samples, sample_rate)
}

/// Write 32-bit float mono PCM as a WAV file.
pub fn write_wav_f32_mono(path: &Path, samples: &[f32], sample_rate: u32) -> Result<(), WriteWavError> {
    let mut file = File::create(path)?;

    let num_samples = samples.len() as u32;
    let byte_rate = sample_rate * 2;
    let data_size = num_samples * 2;
    let file_size = 36 + data_size;

    file.write_all(b"RIFF")?;
    file.write_all(&file_size.to_le_bytes())?;
    file.write_all(b"WAVE")?;
    file.write_all(b"fmt ")?;
    file.write_all(&16u32.to_le_bytes())?;
    file.write_all(&1u16.to_le_bytes())?; // PCM
    file.write_all(&1u16.to_le_bytes())?; // mono
    file.write_all(&sample_rate.to_le_bytes())?;
    file.write_all(&byte_rate.to_le_bytes())?;
    file.write_all(&2u16.to_le_bytes())?; // block align
    file.write_all(&16u16.to_le_bytes())?; // 16-bit
    file.write_all(b"data")?;
    file.write_all(&data_size.to_le_bytes())?;

    for &sample in samples {
        let clamped = sample.clamp(-1.0, 1.0);
        let int_sample = (clamped * 32767.0) as i16;
        file.write_all(&int_sample.to_le_bytes())?;
    }

    Ok(())
}
