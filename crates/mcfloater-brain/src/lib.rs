//! McFloater brain HTTP service (runs on Thumper).
//!
//! Tower GUI / CLI talk to this API; HA tokens stay on the brain host.
//! Natural TTS (Piper) runs here; SAM formant voice stays on Tower.

mod intent;
mod protocol;
mod server;
mod tts;

pub use intent::handle_chat;
pub use protocol::*;
pub use server::{serve, BrainState, DEFAULT_BIND};
pub use tts::{
    synthesize_wav, TtsConfig, TtsError, DEFAULT_PIPER_VOICE, OPTIONAL_PIPER_VOICES,
};
