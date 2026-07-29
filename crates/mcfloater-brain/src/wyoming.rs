//! Minimal [Wyoming](https://github.com/rhasspy/wyoming) TCP client for STT.
//!
//! Protocol: JSONL header line + optional binary payload (PCM).
//! Used by HA Assist and McFloater brain against `rhasspy/wyoming-whisper`.

use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;
use thiserror::Error;
use tracing::debug;

#[derive(Debug, Error)]
pub enum WyomingError {
    #[error("wyoming not configured (set MCFLOATER_WYOMING_STT=host:port)")]
    NotConfigured,
    #[error("wyoming connect {0}")]
    Connect(String),
    #[error("wyoming i/o: {0}")]
    Io(#[from] std::io::Error),
    #[error("wyoming protocol: {0}")]
    Protocol(String),
    #[error("wyoming: no transcript returned")]
    NoTranscript,
}

/// Config for Wyoming ASR (Whisper).
#[derive(Debug, Clone)]
pub struct WyomingSttConfig {
    pub host: String,
    pub port: u16,
    pub language: Option<String>,
    pub timeout: Duration,
}

impl WyomingSttConfig {
    pub fn from_env() -> Option<Self> {
        let raw = std::env::var("MCFLOATER_WYOMING_STT").ok()?;
        let raw = raw.trim();
        if raw.is_empty() {
            return None;
        }
        let (host, port) = parse_host_port(raw)?;
        let language = std::env::var("MCFLOATER_WHISPER_LANG")
            .ok()
            .filter(|s| !s.is_empty());
        let timeout = Duration::from_millis(
            std::env::var("MCFLOATER_WYOMING_TIMEOUT_MS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(120_000),
        );
        Some(Self {
            host,
            port,
            language,
            timeout,
        })
    }

    pub fn addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    /// TCP connect + describe probe.
    pub fn probe(&self) -> Result<(), WyomingError> {
        let mut stream = connect(self)?;
        write_event(&mut stream, "describe", None, None)?;
        let (ty, _) = read_event(&mut stream)?;
        if ty != "info" {
            return Err(WyomingError::Protocol(format!(
                "expected info, got {ty}"
            )));
        }
        Ok(())
    }
}

fn parse_host_port(s: &str) -> Option<(String, u16)> {
    if let Some((h, p)) = s.rsplit_once(':') {
        let port: u16 = p.parse().ok()?;
        Some((h.to_string(), port))
    } else {
        Some((s.to_string(), 10300))
    }
}

fn connect(cfg: &WyomingSttConfig) -> Result<TcpStream, WyomingError> {
    let addr_s = cfg.addr();
    let mut addrs = addr_s
        .to_socket_addrs()
        .map_err(|e| WyomingError::Connect(format!("{addr_s}: {e}")))?;
    let sock = addrs
        .next()
        .ok_or_else(|| WyomingError::Connect(format!("no address for {addr_s}")))?;
    let stream = TcpStream::connect_timeout(&sock, cfg.timeout.min(Duration::from_secs(5)))
        .map_err(|e| WyomingError::Connect(format!("{addr_s}: {e}")))?;
    stream.set_read_timeout(Some(cfg.timeout))?;
    stream.set_write_timeout(Some(cfg.timeout))?;
    Ok(stream)
}

fn write_event(
    stream: &mut TcpStream,
    event_type: &str,
    data: Option<Value>,
    payload: Option<&[u8]>,
) -> Result<(), WyomingError> {
    let mut header = json!({ "type": event_type });
    if let Some(d) = data {
        header["data"] = d;
    }
    if let Some(p) = payload {
        header["payload_length"] = json!(p.len());
    }
    let line = serde_json::to_string(&header)
        .map_err(|e| WyomingError::Protocol(e.to_string()))?;
    stream.write_all(line.as_bytes())?;
    stream.write_all(b"\n")?;
    if let Some(p) = payload {
        stream.write_all(p)?;
    }
    Ok(())
}

fn read_event(stream: &mut TcpStream) -> Result<(String, Value), WyomingError> {
    let mut line = Vec::new();
    loop {
        let mut b = [0u8; 1];
        stream.read_exact(&mut b)?;
        if b[0] == b'\n' {
            break;
        }
        line.push(b[0]);
        if line.len() > 1_000_000 {
            return Err(WyomingError::Protocol("header too large".into()));
        }
    }
    let header: Value = serde_json::from_slice(&line)
        .map_err(|e| WyomingError::Protocol(format!("bad header json: {e}")))?;
    let ty = header
        .get("type")
        .and_then(|t| t.as_str())
        .ok_or_else(|| WyomingError::Protocol("missing type".into()))?
        .to_string();

    let mut data = header.get("data").cloned().unwrap_or_else(|| json!({}));
    if let Some(len) = header.get("data_length").and_then(|v| v.as_u64()) {
        let mut extra = vec![0u8; len as usize];
        stream.read_exact(&mut extra)?;
        if let Ok(more) = serde_json::from_slice::<Value>(&extra) {
            if let (Some(base), Some(add)) = (data.as_object_mut(), more.as_object()) {
                for (k, v) in add {
                    base.insert(k.clone(), v.clone());
                }
            }
        }
    }
    if let Some(len) = header.get("payload_length").and_then(|v| v.as_u64()) {
        let mut payload = vec![0u8; len as usize];
        stream.read_exact(&mut payload)?;
        // STT client ignores audio payloads from server.
        let _ = payload;
    }
    Ok((ty, data))
}

/// Transcribe mono PCM (16-bit little-endian) via Wyoming ASR.
pub fn transcribe_pcm16(
    cfg: &WyomingSttConfig,
    pcm: &[u8],
    sample_rate: u32,
) -> Result<String, WyomingError> {
    let mut stream = connect(cfg)?;
    let mut data = json!({});
    if let Some(lang) = &cfg.language {
        data["language"] = json!(lang);
    }
    write_event(&mut stream, "transcribe", Some(data), None)?;
    write_event(
        &mut stream,
        "audio-start",
        Some(json!({
            "rate": sample_rate,
            "width": 2,
            "channels": 1,
        })),
        None,
    )?;

    // Chunk PCM to keep headers small.
    const CHUNK: usize = 8192;
    for chunk in pcm.chunks(CHUNK) {
        write_event(
            &mut stream,
            "audio-chunk",
            Some(json!({
                "rate": sample_rate,
                "width": 2,
                "channels": 1,
            })),
            Some(chunk),
        )?;
    }
    write_event(&mut stream, "audio-stop", None, None)?;

    // Read until transcript (skip intermediate events).
    for _ in 0..64 {
        let (ty, data) = read_event(&mut stream)?;
        debug!(%ty, "wyoming event");
        if ty == "transcript" {
            let text = data
                .get("text")
                .and_then(|t| t.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            if text.is_empty() {
                return Err(WyomingError::NoTranscript);
            }
            return Ok(text);
        }
        if ty == "error" {
            return Err(WyomingError::Protocol(format!("{data}")));
        }
    }
    Err(WyomingError::NoTranscript)
}

/// Transcribe a WAV buffer (PCM 16-bit mono or stereo → mono).
pub fn transcribe_wav(cfg: &WyomingSttConfig, wav: &[u8]) -> Result<String, WyomingError> {
    let (rate, pcm) = decode_wav_pcm16_mono(wav)?;
    transcribe_pcm16(cfg, &pcm, rate)
}

fn decode_wav_pcm16_mono(wav: &[u8]) -> Result<(u32, Vec<u8>), WyomingError> {
    // Minimal RIFF/WAVE PCM reader (avoids new crate dep).
    if wav.len() < 44 || &wav[0..4] != b"RIFF" || &wav[8..12] != b"WAVE" {
        return Err(WyomingError::Protocol("not a RIFF/WAVE file".into()));
    }
    let mut pos = 12usize;
    let mut fmt_channels = 1u16;
    let mut sample_rate = 16_000u32;
    let mut data_pcm: Option<&[u8]> = None;

    while pos + 8 <= wav.len() {
        let id = &wav[pos..pos + 4];
        let size = u32::from_le_bytes(wav[pos + 4..pos + 8].try_into().unwrap()) as usize;
        let start = pos + 8;
        let end = start.saturating_add(size).min(wav.len());
        if id == b"fmt " && size >= 16 {
            let audio_format = u16::from_le_bytes(wav[start..start + 2].try_into().unwrap());
            if audio_format != 1 {
                return Err(WyomingError::Protocol(
                    "only uncompressed PCM WAV supported".into(),
                ));
            }
            fmt_channels = u16::from_le_bytes(wav[start + 2..start + 4].try_into().unwrap());
            sample_rate = u32::from_le_bytes(wav[start + 4..start + 8].try_into().unwrap());
            let bits = u16::from_le_bytes(wav[start + 14..start + 16].try_into().unwrap());
            if bits != 16 {
                return Err(WyomingError::Protocol("only 16-bit PCM WAV supported".into()));
            }
        } else if id == b"data" {
            data_pcm = Some(&wav[start..end]);
        }
        pos = end + (size % 2); // word align
    }

    let pcm = data_pcm.ok_or_else(|| WyomingError::Protocol("WAV missing data chunk".into()))?;
    if fmt_channels == 1 {
        return Ok((sample_rate, pcm.to_vec()));
    }
    if fmt_channels != 2 {
        return Err(WyomingError::Protocol(format!(
            "unsupported channel count {fmt_channels}"
        )));
    }
    // Downmix stereo i16 → mono
    let mut mono = Vec::with_capacity(pcm.len() / 2);
    for frame in pcm.chunks_exact(4) {
        let l = i16::from_le_bytes([frame[0], frame[1]]) as i32;
        let r = i16::from_le_bytes([frame[2], frame[3]]) as i32;
        let m = ((l + r) / 2) as i16;
        mono.extend_from_slice(&m.to_le_bytes());
    }
    Ok((sample_rate, mono))
}
