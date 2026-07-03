use std::ffi::{CStr, CString};
use thiserror::Error;
use tracing::debug;

/// Demo line in Max Headroom stutter style.
pub const DEMO_LINE: &str = "G-g-great to see you! Catch the wave!";

/// SAM voice parameters (0–255).
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

/// TTS configuration for Floaty.
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

/// Synthesized speech audio.
#[derive(Debug, Clone)]
pub struct SpeechAudio {
    pub samples: Vec<u8>,
    pub sample_rate: u32,
}

impl SpeechAudio {
    pub fn len(&self) -> usize {
        self.samples.len()
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    pub fn duration_secs(&self) -> f64 {
        if self.sample_rate == 0 {
            return 0.0;
        }
        self.samples.len() as f64 / f64::from(self.sample_rate)
    }
}

#[derive(Debug, Error)]
pub enum SynthesisError {
    #[error("empty input text")]
    EmptyText,
    #[error("text contains invalid UTF-8")]
    InvalidUtf8,
    #[error("SAM synthesis failed")]
    SamFailed,
}

extern "C" {
    fn setupSpeak(speed: i32, pitch: i32, throat: i32, mouth: i32);
    fn speakText(text: *const i8) -> *mut AudioResult;
}

#[repr(C)]
struct AudioResult {
    buf: *mut u8,
    buf_size: i32,
}

/// Synthesize text to unsigned 8-bit mono PCM at 22050 Hz.
pub fn synthesize(text: &str, config: &FloatyTtsConfig) -> Result<SpeechAudio, SynthesisError> {
    let text = text.trim();
    if text.is_empty() {
        return Err(SynthesisError::EmptyText);
    }

    let c_text = CString::new(text).map_err(|_| SynthesisError::InvalidUtf8)?;
    let v = config.sam_voice;

    debug!(speed = v.speed, pitch = v.pitch, throat = v.throat, mouth = v.mouth, "SAM setup");

    unsafe {
        setupSpeak(
            i32::from(v.speed),
            i32::from(v.pitch),
            i32::from(v.throat),
            i32::from(v.mouth),
        );

        let result = speakText(c_text.as_ptr());
        if result.is_null() {
            return Err(SynthesisError::SamFailed);
        }

        let audio = &*result;
        if audio.buf.is_null() || audio.buf_size <= 0 {
            libc::free(result as *mut libc::c_void);
            return Err(SynthesisError::SamFailed);
        }

        let samples = std::slice::from_raw_parts(audio.buf, audio.buf_size as usize).to_vec();
        libc::free(audio.buf as *mut libc::c_void);
        libc::free(result as *mut libc::c_void);

        Ok(SpeechAudio {
            samples,
            sample_rate: 22_050,
        })
    }
}
