//! Audio I/O for Floaty McFloater.

mod boop;
mod playback;
mod wav_parse;
mod write_wav;

pub use boop::{
    find_boop, list_boop_patterns, random_boop, seed_boop_rng, seed_boop_rng_from_time,
    select_boop_for_statement, split_statements, synthesize_boop, synthesize_boop_for_statement,
    synthesize_random_boop, trailing_punctuation, BoopPattern, PATTERNS, BOOP_SAMPLE_RATE,
};
pub use playback::{
    audio_preroll_ms, play_pcm_f32_mono, play_pcm_f32_mono_with_notify, play_pcm_i16_mono,
    play_pcm_u8_mono, play_pcm_u8_mono_with_notify, play_wav_bytes, play_wav_bytes_with_notify,
    OnAudible, PlaybackError, DEFAULT_POSTROLL_MS, DEFAULT_PREROLL_MS,
};
pub use wav_parse::{parse_wav, PcmWav, WavParseError};
pub use write_wav::{write_wav_f32_mono, write_wav_u8_mono, WriteWavError};

/// SAM outputs unsigned 8-bit mono PCM at 22050 Hz.
pub const SAM_SAMPLE_RATE: u32 = 22_050;