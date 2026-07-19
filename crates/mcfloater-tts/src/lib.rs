//! Realtime formant speech for Floaty McFloater.
//!
//! Uses the vendored C64 SAM engine — same class of synthesis as 1970s–80s
//! parametric/vocoder hardware. No neural models.
//!
//! **Default voice is locked:** [`SamVoice::floaty`] / preset name `"floaty"`.
//! Other named presets remain available via [`SamVoice::named`] / CLI `--voice`.

mod sam;

pub use sam::{SamEngine, SamError, SAMAudio};

/// Canonical preset name for the lab default voice.
pub const DEFAULT_VOICE_PRESET: &str = "floaty";

/// Floaty's SAM voice — robotic TV-host timbre.
///
/// SAM `pitch` is a period value: **lower = higher pitch**.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SamVoice {
    pub speed: u8,
    pub pitch: u8,
    pub throat: u8,
    pub mouth: u8,
}

impl SamVoice {
    /// **Lab default** — young / neutral-male formant (locked).
    ///
    /// speed 74 · pitch 56 · throat 122 · mouth 118
    pub const fn floaty() -> Self {
        Self {
            speed: 74,
            pitch: 56,
            throat: 122,
            mouth: 118,
        }
    }

    /// Original C64 SAM stock defaults.
    pub const fn classic() -> Self {
        Self {
            speed: 72,
            pitch: 64,
            throat: 128,
            mouth: 128,
        }
    }

    /// Older deeper “TV host” experiment (pre-floaty lock).
    pub const fn deep() -> Self {
        Self {
            speed: 78,
            pitch: 70,
            throat: 115,
            mouth: 105,
        }
    }

    /// High chipmunky novelty.
    pub const fn elf() -> Self {
        Self {
            speed: 72,
            pitch: 40,
            throat: 140,
            mouth: 140,
        }
    }

    /// Slow low news-anchor.
    pub const fn rumble() -> Self {
        Self {
            speed: 92,
            pitch: 90,
            throat: 110,
            mouth: 100,
        }
    }

    /// Resolve a preset name (case-insensitive). Unknown → `None`.
    pub fn named(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "floaty" | "default" | "mcfloater" => Some(Self::floaty()),
            "classic" | "sam" | "stock" => Some(Self::classic()),
            "deep" | "host" | "old" => Some(Self::deep()),
            "elf" | "chipmunk" | "high" => Some(Self::elf()),
            "rumble" | "low" | "bass" => Some(Self::rumble()),
            _ => None,
        }
    }

    /// All named presets for CLI help / `voices` command.
    pub fn preset_table() -> &'static [(&'static str, &'static str, SamVoice)] {
        &VOICE_PRESETS
    }
}

/// Named SAM presets: (name, description, voice). First entry is the locked default.
pub static VOICE_PRESETS: [(&str, &str, SamVoice); 5] = [
    (
        "floaty",
        "lab default — young/neutral male formant (LOCKED)",
        SamVoice::floaty(),
    ),
    ("classic", "stock C64 SAM", SamVoice::classic()),
    (
        "deep",
        "older deeper TV-host experiment",
        SamVoice::deep(),
    ),
    ("elf", "high novelty / chipmunk", SamVoice::elf()),
    ("rumble", "slow low news-anchor", SamVoice::rumble()),
];

impl Default for SamVoice {
    fn default() -> Self {
        Self::floaty()
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
            sam_voice: SamVoice::floaty(),
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

/// Canned intro for Space / `speak` demos.
/// Written for TTS: no Max-Headroom letter stutters (they sound like "G G G" out loud).
pub const DEMO_LINE: &str =
    "Hello! I'm Floaty McFloater. Catch the wave — and welcome to the future!";

/// Default line the face **A** key sends to the brain (not spoken as-is — brain replies).
pub const DEFAULT_ASK_LINE: &str = "Hello!";
