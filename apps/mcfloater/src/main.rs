use clap::Parser;
use mcfloater_audio::{play_pcm_u8_mono, write_wav_u8_mono};
use mcfloater_tts::{synthesize, FloatyTtsConfig, SamVoice, DEMO_LINE};
use std::path::PathBuf;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(name = "mcfloater", about = "Floaty McFloater — Max Headroom parody avatar")]
struct Args {
    /// Text for Floaty to speak
    #[arg(default_value_t = DEMO_LINE.to_string())]
    text: String,

    /// SAM speed (0–255)
    #[arg(long, default_value_t = SamVoice::default().speed)]
    speed: u8,

    /// SAM pitch (0–255)
    #[arg(long, default_value_t = SamVoice::default().pitch)]
    pitch: u8,

    /// SAM throat (0–255)
    #[arg(long, default_value_t = SamVoice::default().throat)]
    throat: u8,

    /// SAM mouth (0–255)
    #[arg(long, default_value_t = SamVoice::default().mouth)]
    mouth: u8,

    /// Save synthesized audio to a WAV file
    #[arg(long)]
    output: Option<PathBuf>,

    /// Skip speaker playback
    #[arg(long)]
    no_play: bool,
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("mcfloater=info".parse().unwrap()))
        .init();

    let args = Args::parse();
    let config = FloatyTtsConfig {
        sam_voice: SamVoice {
            speed: args.speed,
            pitch: args.pitch,
            throat: args.throat,
            mouth: args.mouth,
        },
    };

    info!(text = %args.text, "Floaty McFloater — synthesizing speech");

    let speech = match synthesize(&args.text, &config) {
        Ok(speech) => speech,
        Err(err) => {
            error!(%err, "speech synthesis failed");
            std::process::exit(1);
        }
    };

    info!(
        samples = speech.len(),
        sample_rate = speech.sample_rate,
        duration_secs = speech.duration_secs(),
        "synthesis complete"
    );

    if let Some(path) = &args.output {
        if let Err(err) = write_wav_u8_mono(path, &speech.samples, speech.sample_rate) {
            error!(%err, path = %path.display(), "failed to write output WAV");
            std::process::exit(1);
        }
        info!(path = %path.display(), "wrote WAV");
    }

    if !args.no_play {
        info!("playing audio");
        if let Err(err) = play_pcm_u8_mono(&speech.samples) {
            error!(%err, "playback failed");
            std::process::exit(1);
        }
    }

    info!("done");
}