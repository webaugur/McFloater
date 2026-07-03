//! Realtime formant speech for Floaty McFloater.
//!
//! Uses the vendored C64 SAM engine — same class of synthesis as 1970s–80s
//! parametric/vocoder hardware. No neural models.

mod sam;

pub use sam::{SamEngine, SamError, SAMAudio};

/// Floaty's SAM voice preset — robotic TV-host timbre.
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
            speed: 78,
            pitch: 70,
            throat: 115,
            mouth: 105,
        }
    }
}

/// Runtime voice configuration.
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

/// Unified speech output.
#[derive(Debug, Clone)]
pub struct SynthesizedSpeech {
    pub samples: Vec<u8>,
    pub sample_rate: u32,
}

impl SynthesizedSpeech {
    pub const SAMPLE_RATE: u32 = 22_050;

    pub fn len(&self) -> usize {
        self.samples.len()
    }

    pub fn duration_secs(&self) -> f64 {
        if self.sample_rate == 0 {
            return 0.0;
        }
        self.len() as f64 / self.sample_rate as f64
    }
}

/// Synthesize speech in realtime (typically milliseconds on CPU).
pub fn synthesize(text: &str, config: &FloatyTtsConfig) -> Result<SynthesizedSpeech, SamError> {
    let samples = SamEngine::speak(text, config.sam_voice)?;
    Ok(SynthesizedSpeech {
        samples,
        sample_rate: SynthesizedSpeech::SAMPLE_RATE,
    })
}

/// A canned intro line for demos.
pub const DEMO_LINE: &str =
    "G-g-greetings! I am Floaty McFloater. Welcome to the future of television!";