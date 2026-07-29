//! Host side for Bevy face: brain poll + speak/ask workers (Piper→SAM).

use crate::brain_client::BrainClient;
use crate::config::{brain_url, SpeechEngine};
use crate::speech;
use crossbeam_channel::{Receiver, Sender};
use mcfloater_render::{FaceEvent, FaceLines, FaceRequest, RuntimeState};
use mcfloater_tts::FloatyTtsConfig;
use std::thread;
use std::time::Duration;
use tracing::{error, info, warn};

/// Run face UI (blocking on main thread) with background host workers.
pub fn run_face_host(tts: FloatyTtsConfig, demo_line: String) -> Result<(), String> {
    let (events_tx, events_rx) = crossbeam_channel::unbounded::<FaceEvent>();
    let (req_tx, req_rx) = crossbeam_channel::unbounded::<FaceRequest>();

    let poll_tx = events_tx.clone();
    thread::spawn(move || brain_poll_loop(poll_tx));

    let worker_tx = events_tx.clone();
    let tts_worker = tts.clone();
    thread::spawn(move || host_request_loop(req_rx, worker_tx, tts_worker));

    let lines = FaceLines {
        demo: demo_line,
        // A sends this to the brain; spoken audio is the brain *reply* (not this string).
        ask: std::env::var("MCFLOATER_FACE_ASK")
            .unwrap_or_else(|_| mcfloater_tts::DEFAULT_ASK_LINE.into()),
    };

    let _ = events_tx.send(FaceEvent::SetCaption(
        "FLOATY McFLOATER — Space speak · A ask · L listen · Esc quit".into(),
    ));

    mcfloater_render::run_face(events_rx, req_tx, lines);
    Ok(())
}

fn brain_poll_loop(tx: Sender<FaceEvent>) {
    loop {
        let status = match brain_url() {
            None => FaceEvent::BrainStatus {
                ok: false,
                ha_control: false,
                detail: "BRAIN: set MCFLOATER_BRAIN_URL".into(),
            },
            Some(url) => match BrainClient::new(&url).and_then(|c| c.health()) {
                Ok(h) if h.ok && h.ha_ok && h.ha_control_ok => {
                    let tts = tts_tag(&h);
                    let inv = h.ha_control.as_deref().unwrap_or("devices ok");
                    FaceEvent::BrainStatus {
                        ok: true,
                        ha_control: true,
                        detail: format!("BRAIN OK · HA C&C {inv} · {tts}"),
                    }
                }
                Ok(h) if h.ok && h.ha_ok => {
                    // API token works but no switch/light/scene — do not claim C&C.
                    let tts = tts_tag(&h);
                    let inv = h
                        .ha_control
                        .as_deref()
                        .unwrap_or("0 sw · 0 lt · 0 sc");
                    FaceEvent::BrainStatus {
                        ok: true,
                        ha_control: false,
                        detail: format!("BRAIN OK · HA API · NO DEVICES ({inv}) · {tts}"),
                    }
                }
                Ok(h) if h.ok => FaceEvent::BrainStatus {
                    ok: true,
                    ha_control: false,
                    detail: format!(
                        "BRAIN OK · HA FAIL {}",
                        h.ha_message.unwrap_or_default()
                    ),
                },
                Ok(_) => FaceEvent::BrainStatus {
                    ok: false,
                    ha_control: false,
                    detail: "BRAIN: degraded".into(),
                },
                Err(err) => FaceEvent::BrainStatus {
                    ok: false,
                    ha_control: false,
                    detail: format!("BRAIN: offline ({err})"),
                },
            },
        };
        let _ = tx.send(status);
        thread::sleep(Duration::from_secs(3));
    }
}

fn tts_tag(h: &mcfloater_brain::HealthResponse) -> &'static str {
    if h.tts_busy {
        "TTS:BUSY"
    } else if h.tts_ok {
        "TTS:OK"
    } else {
        "TTS:OFF"
    }
}

fn host_request_loop(
    rx: Receiver<FaceRequest>,
    tx: Sender<FaceEvent>,
    tts: FloatyTtsConfig,
) {
    while let Ok(req) = rx.recv() {
        match req {
            FaceRequest::Quit => {
                let _ = tx.send(FaceEvent::Quit);
                break;
            }
            FaceRequest::Speak(text) => {
                if let Err(err) = speak_with_face(&tx, &tts, &text) {
                    error!(%err, "face speak failed");
                    let _ = tx.send(FaceEvent::SetCaption(format!("speak error: {err}")));
                    let _ = tx.send(FaceEvent::SetState(RuntimeState::Idle));
                }
            }
            FaceRequest::Ask(text) => {
                if let Err(err) = ask_with_face(&tx, &tts, &text) {
                    error!(%err, "face ask failed");
                    let _ = tx.send(FaceEvent::SetCaption(format!("ask error: {err}")));
                    let _ = tx.send(FaceEvent::SetState(RuntimeState::Idle));
                }
            }
            FaceRequest::Listen => {
                if let Err(err) = listen_with_face(&tx, &tts) {
                    error!(%err, "face listen failed");
                    let _ = tx.send(FaceEvent::SetCaption(format!("listen error: {err}")));
                    let _ = tx.send(FaceEvent::SetState(RuntimeState::Idle));
                }
            }
        }
    }
}

fn speak_with_face(
    tx: &Sender<FaceEvent>,
    tts: &FloatyTtsConfig,
    text: &str,
) -> Result<(), String> {
    let engine = SpeechEngine::from_env_or(SpeechEngine::DEFAULT);
    let _ = tx.send(FaceEvent::SetState(RuntimeState::Thinking));
    let _ = tx.send(FaceEvent::SetCaption(text.to_string()));
    let _ = tx.send(FaceEvent::Mouth(0.0));

    // Do NOT enter Speaking before audio: playback prepends ~320 ms silence so the
    // sink can wake. Lip-sync starts only when the first speech sample is written
    // (on_audible). End still lines up because we Idle right after play returns.
    info!(%text, engine = engine.label(), "face → speech");
    let tx_go = tx.clone();
    speech::speak_play_only_with_audible(
        text,
        engine,
        tts,
        Box::new(move || {
            let _ = tx_go.send(FaceEvent::SetState(RuntimeState::Speaking));
            let _ = tx_go.send(FaceEvent::Mouth(0.0));
        }),
    )?;

    let _ = tx.send(FaceEvent::Mouth(0.0));
    let _ = tx.send(FaceEvent::SetState(RuntimeState::Idle));
    let note = if speech::prefer_sam_next() {
        format!("said: {text}  (next: SAM if Thumper still loaded)")
    } else {
        format!("said: {text}")
    };
    let _ = tx.send(FaceEvent::SetCaption(note));
    Ok(())
}

fn ask_with_face(
    tx: &Sender<FaceEvent>,
    tts: &FloatyTtsConfig,
    text: &str,
) -> Result<(), String> {
    let url = brain_url().ok_or_else(|| "MCFLOATER_BRAIN_URL not set".to_string())?;
    info!(%text, %url, "face → brain ask");

    let _ = tx.send(FaceEvent::SetState(RuntimeState::Listening));
    let _ = tx.send(FaceEvent::SetCaption(format!("you: {text}")));

    let _ = tx.send(FaceEvent::SetState(RuntimeState::Thinking));
    let client = BrainClient::new(&url)?;
    let resp = client.chat(text)?;
    if let Some(err) = &resp.error {
        warn!(%err, "brain error on ask");
    }
    for a in &resp.actions {
        info!(
            entity = %a.entity_id,
            service = %a.service,
            "face action"
        );
    }

    speak_with_face(tx, tts, &resp.reply)
}

/// Record mic → Wyoming STT on Thumper → chat → speak.
fn listen_with_face(tx: &Sender<FaceEvent>, tts: &FloatyTtsConfig) -> Result<(), String> {
    use mcfloater_audio::{listen_secs, record_wav_mono};
    use std::time::Duration;

    let url = brain_url().ok_or_else(|| "MCFLOATER_BRAIN_URL not set".to_string())?;
    let secs = listen_secs();
    let _ = tx.send(FaceEvent::SetState(RuntimeState::Listening));
    let _ = tx.send(FaceEvent::SetCaption(format!(
        "listening… ({secs:.0}s) — speak now"
    )));

    info!(secs, "face → mic capture");
    let wav = record_wav_mono(Duration::from_secs_f32(secs)).map_err(|e| e.to_string())?;

    let _ = tx.send(FaceEvent::SetState(RuntimeState::Thinking));
    let _ = tx.send(FaceEvent::SetCaption("transcribing…".into()));
    let client = BrainClient::new_with_timeout(&url, Duration::from_secs(120))?;
    let transcript = client.stt_wav(&wav)?;
    info!(%transcript, "STT transcript");
    let _ = tx.send(FaceEvent::SetCaption(format!("you: {transcript}")));

    let _ = tx.send(FaceEvent::SetState(RuntimeState::Thinking));
    let resp = client.chat(&transcript)?;
    if let Some(err) = &resp.error {
        warn!(%err, "brain error after listen");
    }
    for a in &resp.actions {
        info!(entity = %a.entity_id, service = %a.service, "face action");
    }
    speak_with_face(tx, tts, &resp.reply)
}
