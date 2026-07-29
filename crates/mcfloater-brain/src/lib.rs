//! McFloater brain HTTP service (runs on Thumper).
//!
//! Tower GUI / CLI talk to this API; HA tokens stay on the brain host.
//! Natural TTS (Piper) runs here; SAM formant voice stays on Tower.
//! STT via Wyoming Whisper; dialog via Llama 3.1 / Mistral (Ollama) + optional Grok API.

mod grok;
mod intent;
mod ollama;
mod protocol;
mod server;
mod tts;
mod wyoming;

pub use grok::GrokConfig;
pub use intent::handle_chat;
pub use ollama::OllamaConfig;
pub use protocol::*;
pub use server::{serve, BrainState, DEFAULT_BIND};
pub use tts::{
    synthesize_wav, TtsConfig, TtsError, DEFAULT_PIPER_VOICE, OPTIONAL_PIPER_VOICES,
};
pub use wyoming::WyomingSttConfig;
