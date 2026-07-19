//! Natural TTS on Thumper via Piper (ONNX neural, offline).
//!
//! **Default voice is locked:** [`DEFAULT_PIPER_VOICE`] (`en_US-ryan-medium`).
//! Other installed `.onnx` models remain selectable via `MCFLOATER_PIPER_MODEL`.
//!
//! Env:
//! - `MCFLOATER_PIPER_BIN`   — path to `piper` binary (default: ~/Data/mcfloater/piper/piper)
//! - `MCFLOATER_PIPER_MODEL` — path to `.onnx` voice model (default: ryan)
//! - `MCFLOATER_TTS_ENGINE`  — `piper` (default if model set) | `none`

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;
use tracing::info;

/// Locked lab default Piper voice file name (young adult male).
pub const DEFAULT_PIPER_VOICE: &str = "en_US-ryan-medium";

/// Optional voices that `install-piper.sh` / docs know about (not required).
pub const OPTIONAL_PIPER_VOICES: &[&str] = &[
    "en_US-joe-medium",
    "en_US-lessac-medium",
    "en_GB-alan-medium",
    "en_GB-northern_english_male-medium",
    "en_GB-alba-medium",
    "en_US-amy-medium",
];

#[derive(Debug, Error)]
pub enum TtsError {
    #[error("TTS disabled (set MCFLOATER_PIPER_MODEL or MCFLOATER_TTS_ENGINE=piper)")]
    Disabled,
    #[error("piper binary not found (set MCFLOATER_PIPER_BIN)")]
    MissingBinary,
    #[error("piper model not found: {0}")]
    MissingModel(String),
    #[error("piper failed: {0}")]
    Piper(String),
    #[error("IO: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone)]
pub struct TtsConfig {
    pub enabled: bool,
    pub piper_bin: PathBuf,
    pub model: Option<PathBuf>,
    pub engine: String,
}

impl TtsConfig {
    pub fn from_env() -> Self {
        let engine = std::env::var("MCFLOATER_TTS_ENGINE")
            .unwrap_or_else(|_| "auto".into())
            .to_lowercase();

        let piper_bin = std::env::var("MCFLOATER_PIPER_BIN")
            .map(PathBuf::from)
            .unwrap_or_else(|_| default_piper_bin());

        let model = std::env::var("MCFLOATER_PIPER_MODEL")
            .ok()
            .map(PathBuf::from)
            .filter(|p| !p.as_os_str().is_empty())
            .or_else(default_piper_model);

        let enabled = match engine.as_str() {
            "none" | "off" | "disabled" => false,
            "piper" => true,
            // auto: on when model file exists
            _ => model.as_ref().is_some_and(|m| m.is_file()),
        };

        Self {
            enabled,
            piper_bin,
            model,
            engine: if enabled { "piper".into() } else { engine },
        }
    }

    pub fn status_line(&self) -> String {
        if !self.enabled {
            return "tts: off".into();
        }
        match &self.model {
            Some(m) if m.is_file() && (self.piper_bin.is_file() || which_ok(&self.piper_bin)) => {
                format!(
                    "tts: piper model={}",
                    m.file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("?")
                )
            }
            Some(m) => format!("tts: piper (model missing: {})", m.display()),
            None => "tts: piper (no model)".into(),
        }
    }

    pub fn ready(&self) -> bool {
        if !self.enabled {
            return false;
        }
        let Some(model) = &self.model else {
            return false;
        };
        model.is_file() && (self.piper_bin.is_file() || which_ok(&self.piper_bin))
    }
}

fn default_piper_bin() -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        let p = PathBuf::from(home).join("Data/mcfloater/piper/piper");
        if p.is_file() {
            return p;
        }
    }
    PathBuf::from("piper")
}

fn default_piper_model() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let dir = PathBuf::from(home).join("Data/mcfloater/piper");

    // 1) Locked default first
    let locked = dir.join(format!("{DEFAULT_PIPER_VOICE}.onnx"));
    if locked.is_file() {
        return Some(locked);
    }

    // 2) Known optional voices (documented alternatives)
    for stem in OPTIONAL_PIPER_VOICES {
        let p = dir.join(format!("{stem}.onnx"));
        if p.is_file() {
            return Some(p);
        }
    }

    // 3) Any other .onnx (last resort)
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for ent in rd.flatten() {
            let p = ent.path();
            if p.extension().and_then(|e| e.to_str()) == Some("onnx") {
                return Some(p);
            }
        }
    }
    None
}

fn which_ok(bin: &Path) -> bool {
    if bin.is_file() {
        return true;
    }
    // bare name on PATH
    Command::new("sh")
        .arg("-c")
        .arg(format!("command -v {}", bin.display()))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Synthesize `text` to a WAV byte buffer (RIFF) via Piper.
pub fn synthesize_wav(cfg: &TtsConfig, text: &str) -> Result<Vec<u8>, TtsError> {
    if !cfg.enabled {
        return Err(TtsError::Disabled);
    }
    let model = cfg
        .model
        .as_ref()
        .ok_or(TtsError::Disabled)?;
    if !model.is_file() {
        return Err(TtsError::MissingModel(model.display().to_string()));
    }
    if !cfg.piper_bin.is_file() && !which_ok(&cfg.piper_bin) {
        return Err(TtsError::MissingBinary);
    }

    let text = text.trim();
    if text.is_empty() {
        return Err(TtsError::Piper("empty text".into()));
    }

    // Piper writes a file more reliably than stdout across versions.
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let out_path = std::env::temp_dir().join(format!("mcfloater-tts-{stamp}.wav"));

    let mut child = Command::new(&cfg.piper_bin)
        .arg("--model")
        .arg(model)
        .arg("--output_file")
        .arg(&out_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                TtsError::MissingBinary
            } else {
                TtsError::Io(e)
            }
        })?;

    {
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| TtsError::Piper("no stdin".into()))?;
        // Piper reads one utterance per line; collapse newlines.
        let line = text.replace(['\n', '\r'], " ");
        stdin.write_all(line.as_bytes())?;
        stdin.write_all(b"\n")?;
    }

    let output = child.wait_with_output()?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        let _ = std::fs::remove_file(&out_path);
        return Err(TtsError::Piper(format!(
            "exit {:?}: {err}",
            output.status.code()
        )));
    }

    let wav = std::fs::read(&out_path)?;
    let _ = std::fs::remove_file(&out_path);

    if wav.len() < 44 || &wav[0..4] != b"RIFF" {
        return Err(TtsError::Piper(format!(
            "piper did not write a WAV ({} bytes)",
            wav.len()
        )));
    }

    info!(
        bytes = wav.len(),
        model = %model.display(),
        "piper synthesis ok"
    );
    Ok(wav)
}


