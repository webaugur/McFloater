use crate::{FloatyTtsConfig, SpeechAudio, SynthesisError};
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::time::Instant;
use tracing::debug;

const SAM_SAMPLE_RATE: u32 = 22_050;

extern "C" {
    fn speakText(input: *mut c_char) -> *mut AudioResult;
    fn setupSpeak(pitch: u8, speed: u8, throat: u8, mouth: u8);
}

#[repr(C)]
pub struct AudioResult {
    pub res: c_int,
    pub buf: *mut c_char,
    pub buf_size: c_int,
}

/// Run SAM on `text` and return 8-bit mono PCM at 22050 Hz.
pub fn synthesize_sam(text: &str, config: &FloatyTtsConfig) -> Result<SpeechAudio, SynthesisError> {
    let start = Instant::now();

    let c_text = CString::new(text).map_err(|_| SynthesisError::InvalidInput)?;

    unsafe {
        setupSpeak(
            config.sam_voice.pitch,
            config.sam_voice.speed,
            config.sam_voice.throat,
            config.sam_voice.mouth,
        );

        let result = speakText(c_text.into_raw());
        if result.is_null() {
            return Err(SynthesisError::EngineFailed("null result".into()));
        }

        let audio = &*result;
        if audio.res == 0 {
            return Err(SynthesisError::EngineFailed("SAMMain returned 0".into()));
        }

        let len = audio.buf_size as usize;
        if audio.buf.is_null() || len == 0 {
            return Err(SynthesisError::EngineFailed("empty buffer".into()));
        }

        let slice = std::slice::from_raw_parts(audio.buf as *const u8, len);
        let samples = slice.to_vec();

        let elapsed = start.elapsed();
        debug!(
            samples = samples.len(),
            elapsed_ms = elapsed.as_millis(),
            "SAM synthesis done"
        );

        Ok(SpeechAudio {
            samples,
            sample_rate: SAM_SAMPLE_RATE,
        })
    }
}

/// Floaty McFloater's signature greeting.
pub const DEMO_LINE: &str = "G-g-great to see you! Catch the wave — I'm Floaty McFloater!";

/// SAM voice parameters (C64-style).
#[derive(Debug, Clone, Copy)]
pub struct SamVoice {
    pub speed: u8,
    pub pitch: u8,
    pub throat: u8,
    pub mouth: u8,
}

impl Default for SamVoice {
    fn default() -> Self {
        Self {
            speed: 72,
            pitch: 64,
            throat: 128,
            mouth: 128,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FloatyTtsConfig {
    pub sam_voice: SamVoice,
}

impl Default for FloatyTtsConfig {
    fn default() -> Self {
        Self {
            sam_voice: SamVoice::default(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SynthesisError {
    #[error("invalid input text")]
    InvalidInput,
    #[error("SAM engine failed: {0}")]
    EngineFailed(String),
}

/// Synthesized speech buffer.
#[derive(Debug, Clone)]
pub struct SpeechAudio {
    pub samples: Vec<u8>,
    pub sample_rate: u32,
}

impl SpeechAudio {
    pub fn len(&self) -> usize {
        self.samples.len()
    }

    pub fn duration_secs(&self) -> f64 {
        self.samples.len() as f64 / self.sample_rate as f64
    }
}
