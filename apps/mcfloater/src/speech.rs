//! Speech routing: Piper (Thumper) default, SAM local backup.
//!
//! Overload / failure handling:
//! - Brain reports `tts_busy` or `tts_ok=false` → use SAM for this line
//! - Piper HTTP error / timeout → SAM for this line + prefer SAM once more
//! - Piper succeeds but slower than `MCFLOATER_TTS_SLOW_MS` → prefer SAM for **next** line
//! - Force engines via `--engine sam|brain` / `MCFLOATER_SPEECH_ENGINE`

use crate::brain_client::BrainClient;
use crate::config::{brain_url, SpeechEngine};
use mcfloater_audio::{
    play_pcm_u8_mono, play_pcm_u8_mono_with_notify, play_wav_bytes, play_wav_bytes_with_notify,
    write_wav_u8_mono, OnAudible,
};
use mcfloater_tts::{synthesize, FloatyTtsConfig};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tracing::{info, warn};

/// After a slow/failed Piper call, prefer SAM for the next utterance only.
static PREFER_SAM_NEXT: AtomicBool = AtomicBool::new(false);

/// Default Piper attempt budget (env: MCFLOATER_TTS_TIMEOUT_MS).
pub fn tts_timeout() -> Duration {
    let ms = std::env::var("MCFLOATER_TTS_TIMEOUT_MS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(8_000u64);
    Duration::from_millis(ms.max(500))
}

/// Latency above this (successful synth) marks Thumper "loaded" for next line.
pub fn tts_slow_threshold() -> Duration {
    let ms = std::env::var("MCFLOATER_TTS_SLOW_MS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(3_500u64);
    Duration::from_millis(ms.max(200))
}

pub fn prefer_sam_next() -> bool {
    PREFER_SAM_NEXT.load(Ordering::SeqCst)
}

fn mark_prefer_sam_next(reason: &str) {
    PREFER_SAM_NEXT.store(true, Ordering::SeqCst);
    warn!(%reason, "will prefer SAM for next speech output");
}

fn take_prefer_sam_next() -> bool {
    PREFER_SAM_NEXT.swap(false, Ordering::SeqCst)
}

/// Resolve effective engine for this utterance.
///
/// - `Sam` → always SAM  
/// - `Brain` → always Piper (no fallback; errors propagate)  
/// - `Auto` (default) → Piper unless fallback conditions fire
pub fn effective_engine(requested: SpeechEngine) -> SpeechEngine {
    match requested {
        SpeechEngine::Sam => SpeechEngine::Sam,
        SpeechEngine::Brain => SpeechEngine::Brain,
        SpeechEngine::Auto => {
            if take_prefer_sam_next() {
                info!("using SAM (deferred fallback after prior Thumper load/error)");
                SpeechEngine::Sam
            } else {
                SpeechEngine::Auto
            }
        }
    }
}

/// Speak text with routing. `Auto` tries Piper then SAM; `Brain` is Piper-only; `Sam` is local.
pub fn speak(
    text: &str,
    requested: SpeechEngine,
    config: &FloatyTtsConfig,
    output: Option<&PathBuf>,
    no_play: bool,
) -> Result<(), String> {
    speak_with_audible(text, requested, config, output, no_play, None)
}

/// Like [`speak`], but `on_audible` fires once when silent preroll ends and speech
/// samples actually hit the device (for lip-sync alignment).
pub fn speak_with_audible(
    text: &str,
    requested: SpeechEngine,
    config: &FloatyTtsConfig,
    output: Option<&PathBuf>,
    no_play: bool,
    on_audible: Option<OnAudible>,
) -> Result<(), String> {
    let engine = effective_engine(requested);
    match engine {
        SpeechEngine::Sam => speak_sam(text, config, output, no_play, on_audible),
        SpeechEngine::Brain => speak_piper(text, config, output, no_play, false, on_audible),
        SpeechEngine::Auto => speak_piper(text, config, output, no_play, true, on_audible),
    }
}

fn speak_sam(
    text: &str,
    config: &FloatyTtsConfig,
    output: Option<&PathBuf>,
    no_play: bool,
    on_audible: Option<OnAudible>,
) -> Result<(), String> {
    let v = config.sam_voice;
    info!(
        text = %text,
        speed = v.speed,
        pitch = v.pitch,
        throat = v.throat,
        mouth = v.mouth,
        "speech → SAM formant (local backup / forced)"
    );

    let speech = synthesize(text, config).map_err(|e| e.to_string())?;
    info!(
        samples = speech.len(),
        sample_rate = speech.sample_rate,
        duration_secs = speech.duration_secs(),
        "SAM synthesis complete"
    );

    if let Some(path) = output {
        write_wav_u8_mono(path, &speech.samples, speech.sample_rate)
            .map_err(|e| e.to_string())?;
        info!(path = %path.display(), "wrote WAV");
    }
    if !no_play {
        if on_audible.is_some() {
            play_pcm_u8_mono_with_notify(&speech.samples, on_audible).map_err(|e| e.to_string())?;
        } else {
            play_pcm_u8_mono(&speech.samples).map_err(|e| e.to_string())?;
        }
    } else if let Some(cb) = on_audible {
        // No audio path — still notify so face does not hang waiting forever.
        cb();
    }
    Ok(())
}

/// Piper path. When `allow_fallback`, overload/error → SAM for **this** line.
fn speak_piper(
    text: &str,
    config: &FloatyTtsConfig,
    output: Option<&PathBuf>,
    no_play: bool,
    allow_fallback: bool,
    on_audible: Option<OnAudible>,
) -> Result<(), String> {
    let Some(url) = brain_url() else {
        if allow_fallback {
            warn!("MCFLOATER_BRAIN_URL unset — falling back to SAM");
            return speak_sam(text, config, output, no_play, on_audible);
        }
        return Err("MCFLOATER_BRAIN_URL not set (needed for Piper)".into());
    };

    let client = match BrainClient::new_with_timeout(&url, tts_timeout()) {
        Ok(c) => c,
        Err(e) if allow_fallback => {
            warn!(%e, "brain client failed — SAM fallback");
            mark_prefer_sam_next("brain client error");
            return speak_sam(text, config, output, no_play, on_audible);
        }
        Err(e) => return Err(e),
    };

    // Probe health for busy / missing TTS
    if allow_fallback {
        match client.health() {
            Ok(h) if h.tts_busy => {
                warn!("Thumper TTS busy — SAM for this line");
                mark_prefer_sam_next("tts_busy");
                return speak_sam(text, config, output, no_play, on_audible);
            }
            Ok(h) if !h.tts_ok => {
                warn!(tts = ?h.tts, "Thumper TTS not ready — SAM for this line");
                return speak_sam(text, config, output, no_play, on_audible);
            }
            Ok(_) => {}
            Err(e) => {
                warn!(%e, "brain health failed — SAM for this line");
                mark_prefer_sam_next("health failed");
                return speak_sam(text, config, output, no_play, on_audible);
            }
        }
    }

    info!(text = %text, brain = %url, "speech → Piper (Thumper)");
    let start = Instant::now();
    match client.tts_wav(text) {
        Ok(wav) => {
            let elapsed = start.elapsed();
            info!(bytes = wav.len(), ?elapsed, "received WAV from brain");
            if elapsed >= tts_slow_threshold() {
                mark_prefer_sam_next(&format!(
                    "piper slow ({:.2}s ≥ {:.2}s)",
                    elapsed.as_secs_f32(),
                    tts_slow_threshold().as_secs_f32()
                ));
            }
            if let Some(path) = output {
                std::fs::write(path, &wav).map_err(|e| e.to_string())?;
                info!(path = %path.display(), "wrote WAV");
            }
            if !no_play {
                if on_audible.is_some() {
                    play_wav_bytes_with_notify(&wav, on_audible).map_err(|e| e.to_string())?;
                } else {
                    play_wav_bytes(&wav).map_err(|e| e.to_string())?;
                }
            } else if let Some(cb) = on_audible {
                cb();
            }
            Ok(())
        }
        Err(e) if allow_fallback => {
            let busy = e.contains("503") || e.to_lowercase().contains("busy");
            warn!(%e, busy, "Piper failed — SAM for this line");
            mark_prefer_sam_next(if busy { "tts_busy_http" } else { "piper error" });
            speak_sam(text, config, output, no_play, on_audible)
        }
        Err(e) => Err(e),
    }
}

/// Face / non-CLI path: play only (no file output), same routing.
#[allow(dead_code)] // used by face host (`--features face`)
pub fn speak_play_only(
    text: &str,
    requested: SpeechEngine,
    config: &FloatyTtsConfig,
) -> Result<(), String> {
    speak(text, requested, config, None, false)
}

/// Face path: do not start lip-sync until audible speech (after audio preroll).
#[allow(dead_code)] // used by face host (`--features face`)
pub fn speak_play_only_with_audible(
    text: &str,
    requested: SpeechEngine,
    config: &FloatyTtsConfig,
    on_audible: OnAudible,
) -> Result<(), String> {
    speak_with_audible(text, requested, config, None, false, Some(on_audible))
}
