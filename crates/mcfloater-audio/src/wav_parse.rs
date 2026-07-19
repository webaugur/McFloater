//! Minimal PCM WAV reader (16-bit mono/stereo → mono f32).

use thiserror::Error;

#[derive(Debug, Error)]
pub enum WavParseError {
    #[error("WAV too short")]
    TooShort,
    #[error("not a RIFF/WAVE file")]
    NotRiff,
    #[error("unsupported WAV format (need PCM 16-bit)")]
    Unsupported,
    #[error("missing fmt or data chunk")]
    MissingChunk,
}

#[derive(Debug, Clone)]
pub struct PcmWav {
    pub sample_rate: u32,
    pub channels: u16,
    /// Interleaved i16 samples as stored in the file.
    pub samples_i16: Vec<i16>,
}

impl PcmWav {
    pub fn duration_secs(&self) -> f64 {
        if self.sample_rate == 0 || self.channels == 0 {
            return 0.0;
        }
        self.samples_i16.len() as f64 / (self.sample_rate as f64 * self.channels as f64)
    }

    /// Downmix to mono f32 in [-1, 1].
    pub fn to_mono_f32(&self) -> Vec<f32> {
        let ch = self.channels.max(1) as usize;
        if ch == 1 {
            return self
                .samples_i16
                .iter()
                .map(|&s| s as f32 / 32768.0)
                .collect();
        }
        let frames = self.samples_i16.len() / ch;
        let mut out = Vec::with_capacity(frames);
        for i in 0..frames {
            let mut acc = 0.0f32;
            for c in 0..ch {
                acc += self.samples_i16[i * ch + c] as f32 / 32768.0;
            }
            out.push(acc / ch as f32);
        }
        out
    }
}

/// Parse a standard PCM WAV buffer (little-endian).
pub fn parse_wav(bytes: &[u8]) -> Result<PcmWav, WavParseError> {
    if bytes.len() < 44 {
        return Err(WavParseError::TooShort);
    }
    if &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err(WavParseError::NotRiff);
    }

    let mut pos = 12usize;
    let mut sample_rate = 0u32;
    let mut channels = 0u16;
    let mut bits_per_sample = 0u16;
    let mut audio_format = 0u16;
    let mut data: Option<&[u8]> = None;

    while pos + 8 <= bytes.len() {
        let id = &bytes[pos..pos + 4];
        let size = u32::from_le_bytes(bytes[pos + 4..pos + 8].try_into().unwrap()) as usize;
        let chunk_start = pos + 8;
        let chunk_end = chunk_start
            .checked_add(size)
            .ok_or(WavParseError::TooShort)?;
        if chunk_end > bytes.len() {
            return Err(WavParseError::TooShort);
        }
        let chunk = &bytes[chunk_start..chunk_end];

        if id == b"fmt " {
            if chunk.len() < 16 {
                return Err(WavParseError::Unsupported);
            }
            audio_format = u16::from_le_bytes(chunk[0..2].try_into().unwrap());
            channels = u16::from_le_bytes(chunk[2..4].try_into().unwrap());
            sample_rate = u32::from_le_bytes(chunk[4..8].try_into().unwrap());
            bits_per_sample = u16::from_le_bytes(chunk[14..16].try_into().unwrap());
        } else if id == b"data" {
            data = Some(chunk);
        }

        // chunks are word-aligned
        pos = chunk_end + (size % 2);
    }

    let data = data.ok_or(WavParseError::MissingChunk)?;
    if audio_format != 1 || bits_per_sample != 16 || channels == 0 || sample_rate == 0 {
        return Err(WavParseError::Unsupported);
    }

    let mut samples_i16 = Vec::with_capacity(data.len() / 2);
    for pair in data.chunks_exact(2) {
        samples_i16.push(i16::from_le_bytes([pair[0], pair[1]]));
    }

    Ok(PcmWav {
        sample_rate,
        channels,
        samples_i16,
    })
}
