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
        "FLOATY McFLOATER — Space speak · A ask brain · Esc quit".into(),
    ));

    mcfloater_render::run_face(events_rx, req_tx, lines);
    Ok(())
}

fn brain_poll_loop(tx: Sender<FaceEvent>) {
    loop {
        let detail = match brain_url() {
            None => {
                let _ = tx.send(FaceEvent::BrainStatus {
                    ok: false,
                    detail: "BRAIN: set MCFLOATER_BRAIN_URL".into(),
                });
                thread::sleep(Duration::from_secs(5));
                continue;
            }
            Some(url) => match BrainClient::new(&url).and_then(|c| c.health()) {
                Ok(h) if h.ok && h.ha_ok => {
                    let tts = if h.tts_busy {
                        "TTS:BUSY"
                    } else if h.tts_ok {
                        "TTS:OK"
                    } else {
                        "TTS:OFF"
                    };
                    (
                        true,
                        format!("BRAIN: OK  HA: OK  {tts}  ({url})"),
                    )
                }
                Ok(h) if h.ok => (
                    false,
                    format!(
                        "BRAIN: OK  HA: FAIL  {}",
                        h.ha_message.unwrap_or_default()
                    ),
                ),
                Ok(_) => (false, "BRAIN: degraded".into()),
                Err(err) => (false, format!("BRAIN: offline ({err})")),
            },
        };
        let _ = tx.send(FaceEvent::BrainStatus {
            ok: detail.0,
            detail: detail.1,
        });
        thread::sleep(Duration::from_secs(3));
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

    // Speaking state alone drives lip-sync envelope in the face (do NOT send a
    // constant Mouth(0.8) — that froze the jaw open like a banana the whole line).
    let _ = tx.send(FaceEvent::SetState(RuntimeState::Speaking));
    let _ = tx.send(FaceEvent::Mouth(0.0));

    info!(%text, engine = engine.label(), "face → speech");
    speech::speak_play_only(text, engine, tts)?;

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
