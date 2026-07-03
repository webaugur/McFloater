//! Audio I/O for Floaty McFloater.

mod playback;
mod write_wav;

pub use playback::{play_pcm_f32_mono, play_pcm_u8_mono, PlaybackError};
pub use write_wav::{write_wav_f32_mono, write_wav_u8_mono, WriteWavError};

/// SAM outputs unsigned 8-bit mono PCM at 22050 Hz.
pub const SAM_SAMPLE_RATE: u32 = 22_050;