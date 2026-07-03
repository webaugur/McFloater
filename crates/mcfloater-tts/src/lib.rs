//! SAM (C64 formant) text-to-speech for Floaty McFloater.

mod sam;

pub use sam::{FloatyTtsConfig, SamVoice, SpeechAudio, SynthesisError, DEMO_LINE};

use tracing::info;

/// Synthesize `text` into PCM audio using the SAM engine.
pub fn synthesize(text: &str, config: &FloatyTtsConfig) -> Result<SpeechAudio, SynthesisError> {
    info!(text = %text, "SAM synthesis start");
    sam::synthesize_sam(text, config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synthesize_demo_line() {
        let speech = synthesize(DEMO_LINE, &FloatyTtsConfig::default()).expect("synthesis");
        assert!(!speech.samples.is_empty());
        assert_eq!(speech.sample_rate, 22_050);
    }
}
