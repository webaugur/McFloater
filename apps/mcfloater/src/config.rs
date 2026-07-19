//! Load lab env files and resolve brain bind/URL.

use std::path::PathBuf;
use tracing::debug;

/// Load the first existing env file (does not override already-set vars).
pub fn load_env_files() {
    if let Ok(path) = std::env::var("MCFLOATER_ENV") {
        let _ = dotenvy::from_path(path);
        return;
    }

    let mut candidates = Vec::new();
    if let Ok(home) = std::env::var("HOME") {
        candidates.push(PathBuf::from(home).join("Data/mcfloater/mcfloater.env"));
    }
    candidates.push(PathBuf::from("/home/user/Data/mcfloater/mcfloater.env"));
    // Repo-local convenience for Tower clients
    candidates.push(PathBuf::from("mcfloater.env"));
    candidates.push(PathBuf::from("deploy/thumper/mcfloater.env"));

    for path in candidates {
        if path.is_file() {
            match dotenvy::from_path(&path) {
                Ok(()) => {
                    debug!(path = %path.display(), "loaded env file");
                    return;
                }
                Err(err) => {
                    debug!(path = %path.display(), %err, "env file present but not loaded");
                }
            }
        }
    }
}

pub fn brain_bind() -> String {
    std::env::var("MCFLOATER_BRAIN_BIND").unwrap_or_else(|_| "0.0.0.0:8750".into())
}

/// Base URL for Tower → Thumper brain (no trailing slash), e.g. `http://thumper.local:8750`.
pub fn brain_url() -> Option<String> {
    std::env::var("MCFLOATER_BRAIN_URL")
        .ok()
        .map(|s| s.trim().trim_end_matches('/').to_string())
        .filter(|s| !s.is_empty())
}

/// Preferred speech engine for CLI/face.
///
/// Lab default is [`SpeechEngine::Auto`]: **Piper on Thumper**, with **SAM**
/// local formant as backup when Thumper is busy/slow/offline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpeechEngine {
    /// Piper first; SAM on overload / error / missing brain.
    Auto,
    /// Local formant only (no network).
    Sam,
    /// Piper only (errors if brain TTS fails; no SAM backup).
    Brain,
}

impl SpeechEngine {
    /// Lab default when neither flag nor env is set.
    pub const DEFAULT: Self = Self::Auto;

    pub fn from_env_or(default: Self) -> Self {
        let raw = std::env::var("MCFLOATER_SPEECH_ENGINE").unwrap_or_default();
        let key = raw.trim().to_lowercase();
        if key.is_empty() {
            return default;
        }
        match key.as_str() {
            "auto" | "default" => Self::Auto,
            "brain" | "piper" | "natural" | "thumper" => Self::Brain,
            "sam" | "local" | "formant" => Self::Sam,
            _ => default,
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "auto" | "default" => Some(Self::Auto),
            "sam" | "local" | "formant" => Some(Self::Sam),
            "brain" | "piper" | "natural" | "thumper" => Some(Self::Brain),
            _ => None,
        }
    }

    #[allow(dead_code)] // used by face host (`--features face`)
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "auto (piper→sam)",
            Self::Sam => "sam",
            Self::Brain => "brain/piper",
        }
    }
}
