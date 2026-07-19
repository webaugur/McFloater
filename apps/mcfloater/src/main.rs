//! Floaty McFloater CLI — speak locally; talk to Thumper brain for HA; optional Bevy face.

mod brain_client;
mod config;
#[cfg(feature = "face")]
mod face_host;
mod speech;

use brain_client::BrainClient;
use clap::{Parser, Subcommand};
use config::{brain_bind, brain_url, load_env_files, SpeechEngine};
use mcfloater_brain::{serve, DEFAULT_BIND, DEFAULT_PIPER_VOICE, OPTIONAL_PIPER_VOICES};
use mcfloater_ha::{HaClient, HaConfig};
use mcfloater_tts::{
    FloatyTtsConfig, SamVoice, DEFAULT_ASK_LINE, DEFAULT_VOICE_PRESET, DEMO_LINE,
};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::ExitCode;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(
    name = "mcfloater",
    about = "Floaty McFloater — Max Headroom parody avatar (Tower face + Thumper brain)"
)]
struct Args {
    #[command(subcommand)]
    command: Option<Command>,

    /// Text for Floaty to speak (when no subcommand)
    #[arg(default_value_t = DEMO_LINE.to_string())]
    text: String,

    /// SAM voice preset (default: floaty — lab locked). See `mcfloater voices`.
    #[arg(long, default_value = DEFAULT_VOICE_PRESET, global = true)]
    voice: String,

    /// Override SAM speed (0–255); default comes from --voice preset
    #[arg(long, global = true)]
    speed: Option<u8>,

    /// Override SAM pitch period (0–255; lower = higher voice)
    #[arg(long, global = true)]
    pitch: Option<u8>,

    /// Override SAM throat (0–255)
    #[arg(long, global = true)]
    throat: Option<u8>,

    /// Override SAM mouth (0–255)
    #[arg(long, global = true)]
    mouth: Option<u8>,

    /// Save synthesized audio to a WAV file
    #[arg(long, global = true)]
    output: Option<PathBuf>,

    /// Skip speaker playback
    #[arg(long, global = true)]
    no_play: bool,

    /// Speech engine: `auto` (Piper→SAM, default), `brain` (Piper only), `sam` (local only).
    /// Env: MCFLOATER_SPEECH_ENGINE. Overload/slow Thumper → SAM backup (auto).
    #[arg(long, global = true)]
    engine: Option<String>,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Speak text (default: Piper on Thumper, SAM backup)
    Speak {
        #[arg(default_value_t = DEMO_LINE.to_string())]
        text: String,
    },

    /// Bevy face window on local GPUs (Tower5810). Requires `--features face`.
    /// Space=speak, A=ask brain, Esc=quit
    #[cfg(feature = "face")]
    Face {
        /// Demo line for Space
        #[arg(long)]
        line: Option<String>,
    },

    /// Run McFloater brain HTTP service (Thumper — owns HA_TOKEN)
    Brain {
        /// Bind address (default MCFLOATER_BRAIN_BIND or 0.0.0.0:8750)
        #[arg(long)]
        bind: Option<String>,
    },

    /// Brain + HA health (via MCFLOATER_BRAIN_URL, or local HA if configured)
    Health,

    /// List HA entity states (optional domain filter)
    States {
        #[arg(long)]
        domain: Option<String>,
    },

    /// Turn an entity on
    On {
        entity_id: String,
    },

    /// Turn an entity off
    Off {
        entity_id: String,
    },

    /// Toggle an entity
    Toggle {
        entity_id: String,
    },

    /// Send text to brain intents; speak the reply (auto Piper→SAM)
    Ask {
        /// Words to send (joined)
        text: Vec<String>,
    },

    /// Speak (same routing as default — Piper with SAM backup)
    Say {
        /// Words to speak (joined)
        text: Vec<String>,
    },

    /// List locked defaults + optional SAM / Piper voices
    Voices,
}

fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env().add_directive("mcfloater=info".parse().unwrap()),
        )
        .init();

    load_env_files();

    let args = Args::parse();
    let sam_voice = match resolve_sam_voice(&args) {
        Ok(v) => v,
        Err(err) => {
            error!(%err, "command failed");
            return ExitCode::FAILURE;
        }
    };
    let tts = FloatyTtsConfig { sam_voice };

    let engine_flag = args
        .engine
        .as_deref()
        .and_then(SpeechEngine::parse);

    // Lab default: Piper on Thumper with SAM backup (`auto`).
    let default_engine = engine_flag.unwrap_or_else(|| SpeechEngine::from_env_or(SpeechEngine::DEFAULT));

    let result = match args.command {
        None => speech::speak(
            &args.text,
            default_engine,
            &tts,
            args.output.as_ref(),
            args.no_play,
        ),
        Some(Command::Speak { text }) => {
            speech::speak(&text, default_engine, &tts, args.output.as_ref(), args.no_play)
        }
        #[cfg(feature = "face")]
        Some(Command::Face { line }) => {
            face_host::run_face_host(tts, line.unwrap_or_else(|| DEMO_LINE.to_string()))
        }
        Some(Command::Brain { bind }) => run_brain(bind),
        Some(Command::Health) => cmd_health(),
        Some(Command::States { domain }) => cmd_states(domain.as_deref()),
        Some(Command::On { entity_id }) => cmd_entity("turn_on", &entity_id),
        Some(Command::Off { entity_id }) => cmd_entity("turn_off", &entity_id),
        Some(Command::Toggle { entity_id }) => cmd_entity("toggle", &entity_id),
        Some(Command::Ask { text }) => {
            // Ask text is the *question* to the brain — not the Space demo monologue.
            let line = if text.is_empty() {
                DEFAULT_ASK_LINE.to_string()
            } else {
                text.join(" ")
            };
            cmd_ask(&line, default_engine, &tts, args.output.as_ref(), args.no_play)
        }
        Some(Command::Say { text }) => {
            let line = if text.is_empty() {
                DEMO_LINE.to_string()
            } else {
                text.join(" ")
            };
            speech::speak(
                &line,
                default_engine,
                &tts,
                args.output.as_ref(),
                args.no_play,
            )
        }
        Some(Command::Voices) => cmd_voices(),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            error!(%err, "command failed");
            ExitCode::FAILURE
        }
    }
}

fn resolve_sam_voice(args: &Args) -> Result<SamVoice, String> {
    let mut v = SamVoice::named(&args.voice).ok_or_else(|| {
        let names: Vec<_> = SamVoice::preset_table().iter().map(|(n, _, _)| *n).collect();
        format!(
            "unknown SAM voice preset {:?}; try: {} (or `mcfloater voices`)",
            args.voice,
            names.join(", ")
        )
    })?;
    if let Some(s) = args.speed {
        v.speed = s;
    }
    if let Some(p) = args.pitch {
        v.pitch = p;
    }
    if let Some(t) = args.throat {
        v.throat = t;
    }
    if let Some(m) = args.mouth {
        v.mouth = m;
    }
    Ok(v)
}

fn cmd_voices() -> Result<(), String> {
    println!("SAM formant presets (local; --voice NAME)");
    println!("  default / locked: {DEFAULT_VOICE_PRESET}");
    println!();
    for (name, note, v) in SamVoice::preset_table() {
        let mark = if *name == DEFAULT_VOICE_PRESET {
            "  * "
        } else {
            "    "
        };
        println!(
            "{mark}{name:<10}  speed={:<3} pitch={:<3} throat={:<3} mouth={:<3}  {note}",
            v.speed, v.pitch, v.throat, v.mouth
        );
    }
    println!();
    println!("Piper natural voices (Thumper; MCFLOATER_PIPER_MODEL=…/*.onnx)");
    println!("  default / locked: {DEFAULT_PIPER_VOICE}");
    println!("  * {DEFAULT_PIPER_VOICE}  young adult male (lab default)");
    for alt in OPTIONAL_PIPER_VOICES {
        println!("    {alt}");
    }
    println!();
    println!("Engines: --engine auto|brain|sam   (env MCFLOATER_SPEECH_ENGINE)");
    println!("  * auto   Piper on Thumper → SAM local if busy/slow/offline  [DEFAULT]");
    println!("    brain  Piper only (no SAM backup)");
    println!("    sam    local formant only ({DEFAULT_VOICE_PRESET})");
    println!("  Piper default model: {DEFAULT_PIPER_VOICE}");
    println!(
        "  Overload: concurrent TTS → 503 tts_busy; slow >{}ms → next line uses SAM",
        speech::tts_slow_threshold().as_millis()
    );
    println!(
        "  Timeouts: MCFLOATER_TTS_TIMEOUT_MS (default {})  MCFLOATER_TTS_SLOW_MS (default {})",
        speech::tts_timeout().as_millis(),
        speech::tts_slow_threshold().as_millis()
    );
    Ok(())
}

fn run_brain(bind: Option<String>) -> Result<(), String> {
    let bind_str = bind.unwrap_or_else(brain_bind);
    let addr: SocketAddr = bind_str
        .parse()
        .map_err(|e| format!("invalid bind {bind_str:?}: {e}"))?;
    let ha = HaConfig::from_env().map_err(|e| {
        format!("{e} (set HA_URL and HA_TOKEN on Thumper, e.g. in ~/Data/mcfloater/mcfloater.env)")
    })?;

    info!(%addr, ha_url = %ha.url, "starting McFloater brain");
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    rt.block_on(async move { serve(addr, ha).await.map_err(|e| e.to_string()) })
}

fn cmd_health() -> Result<(), String> {
    if let Some(url) = brain_url() {
        let client = BrainClient::new(&url)?;
        let h = client.health()?;
        println!(
            "brain: ok={} ha_ok={} tts_ok={} tts_busy={} inflight={} msg={:?}",
            h.ok, h.ha_ok, h.tts_ok, h.tts_busy, h.tts_inflight, h.ha_message
        );
        if let Some(tts) = &h.tts {
            println!("tts: {tts}");
        }
        if speech::prefer_sam_next() {
            println!("client: prefer SAM for next speech (prior overload/error)");
        }
        if let Some(ha) = h.ha {
            println!("ha: {ha}");
        }
        if !h.ha_ok {
            return Err(h.ha_message.unwrap_or_else(|| "HA not ok".into()));
        }
        return Ok(());
    }

    let ha = HaClient::new(&HaConfig::from_env().map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let v = ha.check().map_err(|e| e.to_string())?;
    println!("ha (direct): {v}");
    Ok(())
}

fn cmd_states(domain: Option<&str>) -> Result<(), String> {
    if let Some(url) = brain_url() {
        let client = BrainClient::new(&url)?;
        let states = client.states(domain)?;
        for e in states {
            println!("{}\t{}", e.entity_id, e.state);
        }
        return Ok(());
    }

    let ha = HaClient::new(&HaConfig::from_env().map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let states = ha.states(domain).map_err(|e| e.to_string())?;
    for e in states {
        println!("{}\t{}", e.entity_id, e.state);
    }
    Ok(())
}

fn cmd_entity(service: &str, entity_id: &str) -> Result<(), String> {
    if let Some(url) = brain_url() {
        let client = BrainClient::new(&url)?;
        let v = match service {
            "turn_on" => client.turn_on(entity_id)?,
            "turn_off" => client.turn_off(entity_id)?,
            "toggle" => client.toggle(entity_id)?,
            other => return Err(format!("unknown service {other}")),
        };
        println!("{v}");
        return Ok(());
    }

    let ha = HaClient::new(&HaConfig::from_env().map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let v = match service {
        "turn_on" => ha.turn_on(entity_id),
        "turn_off" => ha.turn_off(entity_id),
        "toggle" => ha.toggle(entity_id),
        other => return Err(format!("unknown service {other}")),
    }
    .map_err(|e| e.to_string())?;
    println!("{v}");
    Ok(())
}

fn cmd_ask(
    text: &str,
    engine: SpeechEngine,
    tts: &FloatyTtsConfig,
    output: Option<&PathBuf>,
    no_play: bool,
) -> Result<(), String> {
    let url = brain_url().ok_or_else(|| {
        format!(
            "MCFLOATER_BRAIN_URL not set (e.g. http://thumper.local:8750). \
             Brain default bind is {DEFAULT_BIND}"
        )
    })?;
    let client = BrainClient::new(&url)?;
    info!(%text, brain = %url, "ask brain");
    let resp = client.chat(text)?;
    if let Some(err) = &resp.error {
        error!(%err, "brain reported error (still speaking reply)");
    }
    for a in &resp.actions {
        info!(
            entity = %a.entity_id,
            service = %a.service,
            state = ?a.result_state,
            "action"
        );
    }
    println!("state: {}", resp.state);
    println!("reply: {}", resp.reply);
    speech::speak(resp.reply.as_str(), engine, tts, output, no_play)
}
