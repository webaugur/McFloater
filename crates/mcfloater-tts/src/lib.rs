//! SAM formant TTS for Floaty McFloater.

mod sam;

pub use sam::{
    synthesize, FloatyTtsConfig, SamVoice, SpeechAudio, SynthesisError, DEMO_LINE,
};
