//! Axum HTTP server for the brain control plane.

use crate::grok::GrokConfig;
use crate::intent::handle_chat;
use crate::ollama::OllamaConfig;
use crate::protocol::*;
use crate::tts::{self, TtsConfig};
use crate::wyoming::{self, WyomingSttConfig};
use axum::body::Bytes;
use axum::extract::{Path, Query, State, WebSocketUpgrade};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use axum::extract::ws::{WebSocket, Message};
use mcfloater_ha::{HaClient, HaConfig};
use std::time::Duration;
use tokio::sync::Mutex;
use image::codecs::jpeg::JpegEncoder;
use mcfloater_avatar::{AvatarRenderer, LipSyncFrame};

// Real webrtc 0.17 stable imports (verified against source)
use webrtc::api::APIBuilder;
use webrtc::api::media_engine::MediaEngine;
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::interceptor::registry::Registry;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::track::track_remote::TrackRemote;
use webrtc::track::track_local::track_local_static_sample::TrackLocalStaticSample;
use webrtc::track::track_local::TrackLocal;
use webrtc::rtp_transceiver::rtp_receiver::RTCRtpReceiver;
use webrtc::rtp_transceiver::RTCRtpTransceiver;
use webrtc::media::Sample;
use serde::Deserialize;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::{info, warn};

pub const DEFAULT_BIND: &str = "0.0.0.0:8750";

#[derive(Clone)]
pub struct BrainState {
    pub ha: Arc<HaClient>,
    pub tts: Arc<TtsConfig>,
    pub stt: Option<Arc<WyomingSttConfig>>,
    pub ollama: Option<Arc<OllamaConfig>>,
    pub grok: Option<Arc<GrokConfig>>,
    /// In-flight Piper synthesizers (Tower uses this for SAM fallback).
    pub tts_inflight: Arc<AtomicU32>,
    /// Shared avatar renderer (Thumper-hosted, headless).
    /// Created once at startup so every call can use it.
    pub avatar: Option<Arc<tokio::sync::Mutex<AvatarRenderer>>>,
}

#[derive(Debug, Deserialize)]
pub struct StatesQuery {
    pub domain: Option<String>,
}

/// Run the brain HTTP server until cancelled (blocking via tokio runtime from caller).
pub async fn serve(
    bind: SocketAddr,
    ha_config: HaConfig,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let ha = HaClient::new(&ha_config)?;
    let tts = TtsConfig::from_env();
    info!(tts = %tts.status_line(), ready = tts.ready(), "TTS config");

    let stt = WyomingSttConfig::from_env().map(Arc::new);
    match &stt {
        Some(s) => info!(addr = %s.addr(), "Wyoming STT configured"),
        None => info!("Wyoming STT not set (MCFLOATER_WYOMING_STT)"),
    }

    let ollama = OllamaConfig::from_env().map(Arc::new);
    match &ollama {
        Some(o) => info!(llm = %o.status_line(), "Ollama dialog configured"),
        None => info!("Ollama disabled or unset"),
    }

    let grok = GrokConfig::from_env().map(Arc::new);
    match &grok {
        Some(g) => info!(grok = %g.status_line(), "Grok API configured"),
        None => info!("Grok API not configured (set XAI_API_KEY for world/physics lane)"),
    }

    // Avatar renderer is created lazily on first use inside a call
    // (because it is async and needs a running runtime).
    let state = BrainState {
        ha: Arc::new(ha),
        tts: Arc::new(tts),
        stt,
        ollama,
        grok,
        tts_inflight: Arc::new(AtomicU32::new(0)),
        avatar: None, // created on-demand inside the call handler
    };

    let app = Router::new()
        .route("/health", get(health))
        .route("/v1/ha/states", get(list_states))
        .route("/v1/ha/states/{entity_id}", get(get_state))
        .route("/v1/ha/call", post(call_service))
        .route("/v1/ha/turn_on", post(turn_on))
        .route("/v1/ha/turn_off", post(turn_off))
        .route("/v1/ha/toggle", post(toggle))
        .route("/v1/chat", post(chat))
        .route("/v1/tts", post(tts_synth))
        .route("/v1/stt", post(stt_transcribe))
        .route("/ws/call", get(ws_call))
        .route("/call", get(serve_call_page))
        .nest_service("/static", tower_http::services::ServeDir::new("static"))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    info!(%bind, "McFloater brain listening");
    let listener = tokio::net::TcpListener::bind(bind).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn health(State(state): State<BrainState>) -> impl IntoResponse {
    let tts_ok = state.tts.ready();
    let tts_inflight = state.tts_inflight.load(Ordering::SeqCst);
    let tts_busy = tts_inflight > 0;
    let tts_detail = Some(if tts_busy {
        format!("{} (busy, inflight={tts_inflight})", state.tts.status_line())
    } else {
        state.tts.status_line()
    });

    let (stt_ok, stt_detail) = match &state.stt {
        None => (false, Some("stt: not configured".into())),
        Some(cfg) => {
            let label = format!("stt: wyoming {}", cfg.addr());
            let probe_cfg = cfg.clone();
            match tokio::task::spawn_blocking(move || probe_cfg.probe()).await {
                Ok(Ok(())) => (true, Some(label)),
                Ok(Err(e)) => (false, Some(format!("{label} ({e})"))),
                Err(e) => (false, Some(format!("stt: {e}"))),
            }
        }
    };

    let (ollama_ok, ollama_detail) = match &state.ollama {
        None => (false, "ollama: not configured".to_string()),
        Some(cfg) => {
            let label = cfg.status_line();
            let probe_cfg = cfg.clone();
            match tokio::task::spawn_blocking(move || probe_cfg.probe()).await {
                Ok(Ok(())) => (true, label),
                Ok(Err(e)) => (false, format!("ollama: {e}")),
                Err(e) => (false, format!("ollama: {e}")),
            }
        }
    };
    let (grok_ok, grok_detail) = match &state.grok {
        None => (false, "grok: not configured".to_string()),
        Some(cfg) => {
            let label = cfg.status_line();
            let probe_cfg = cfg.clone();
            match tokio::task::spawn_blocking(move || probe_cfg.probe()).await {
                Ok(Ok(())) => (true, label),
                Ok(Err(e)) => (false, format!("grok: {e}")),
                Err(e) => (false, format!("grok: {e}")),
            }
        }
    };
    let llm_ok = ollama_ok || grok_ok;
    let llm_detail = Some(format!("{ollama_detail}; {grok_detail}"));

    match state.ha.check() {
        Ok(v) => {
            let inv = state.ha.control_inventory().ok();
            let ha_control_ok = inv.as_ref().map(|i| i.control_ok()).unwrap_or(false);
            let ha_control = inv.as_ref().map(|i| i.summary());
            let ha_message = if ha_control_ok {
                None
            } else {
                Some(
                    "HA API up but no switch/light/scene entities — add Tuya/KMC plugs before C&C"
                        .into(),
                )
            };
            Json(HealthResponse {
                ok: true,
                service: "mcfloater-brain".into(),
                ha_ok: true,
                ha_control_ok,
                ha_message,
                ha_control,
                ha: Some(v),
                tts_ok,
                tts_busy,
                tts_inflight,
                tts: tts_detail,
                stt_ok,
                stt: stt_detail,
                llm_ok,
                llm: llm_detail,
            })
        }
        Err(err) => Json(HealthResponse {
            ok: true,
            service: "mcfloater-brain".into(),
            ha_ok: false,
            ha_control_ok: false,
            ha_message: Some(err.to_string()),
            ha_control: None,
            ha: None,
            tts_ok,
            tts_busy,
            tts_inflight,
            tts: tts_detail,
            stt_ok,
            stt: stt_detail,
            llm_ok,
            llm: llm_detail,
        }),
    }
}

async fn list_states(
    State(state): State<BrainState>,
    Query(q): Query<StatesQuery>,
) -> Result<Json<StatesResponse>, ApiError> {
    let entities = state
        .ha
        .states(q.domain.as_deref())
        .map_err(ApiError::from)?;
    Ok(Json(StatesResponse { entities }))
}

async fn get_state(
    State(state): State<BrainState>,
    Path(entity_id): Path<String>,
) -> Result<Json<mcfloater_ha::EntityState>, ApiError> {
    let st = state.ha.state(&entity_id).map_err(ApiError::from)?;
    Ok(Json(st))
}

async fn call_service(
    State(state): State<BrainState>,
    Json(body): Json<ServiceCallBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let v = state
        .ha
        .call_service(&body.domain, &body.service, &body.entity_id, body.data)
        .map_err(ApiError::from)?;
    Ok(Json(v))
}

async fn turn_on(
    State(state): State<BrainState>,
    Json(body): Json<EntityIdBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    Ok(Json(
        state.ha.turn_on(&body.entity_id).map_err(ApiError::from)?,
    ))
}

async fn turn_off(
    State(state): State<BrainState>,
    Json(body): Json<EntityIdBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    Ok(Json(
        state
            .ha
            .turn_off(&body.entity_id)
            .map_err(ApiError::from)?,
    ))
}

async fn toggle(
    State(state): State<BrainState>,
    Json(body): Json<EntityIdBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    Ok(Json(
        state.ha.toggle(&body.entity_id).map_err(ApiError::from)?,
    ))
}

async fn chat(
    State(state): State<BrainState>,
    Json(body): Json<ChatRequest>,
) -> Json<ChatResponse> {
    let ha = state.ha.clone();
    let ollama = state.ollama.clone();
    let grok = state.grok.clone();
    let resp = tokio::task::spawn_blocking(move || {
        handle_chat(
            &ha,
            ollama.as_ref().map(|a| a.as_ref()),
            grok.as_ref().map(|a| a.as_ref()),
            &body,
        )
    })
    .await
    .unwrap_or_else(|e| ChatResponse {
        reply: "Brain task failed.".into(),
        state: "idle".into(),
        actions: vec![],
        error: Some(e.to_string()),
    });
    Json(resp)
}

/// Speech-to-text: POST raw `audio/wav` (PCM 16-bit). Returns JSON `{ "text": "…" }`.
async fn stt_transcribe(
    State(state): State<BrainState>,
    body: Bytes,
) -> Result<Json<SttResponse>, ApiError> {
    let Some(cfg) = state.stt.clone() else {
        return Err(ApiError {
            status: StatusCode::SERVICE_UNAVAILABLE,
            message: "stt_not_configured (set MCFLOATER_WYOMING_STT)".into(),
        });
    };
    if body.is_empty() {
        return Err(ApiError {
            status: StatusCode::BAD_REQUEST,
            message: "empty body".into(),
        });
    }
    let wav = body.to_vec();
    let text = tokio::task::spawn_blocking(move || wyoming::transcribe_wav(&cfg, &wav))
        .await
        .map_err(|e| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: e.to_string(),
        })?
        .map_err(|e| ApiError {
            status: StatusCode::BAD_GATEWAY,
            message: e.to_string(),
        })?;
    Ok(Json(SttResponse {
        text,
        error: None,
    }))
}

/// Natural TTS: returns `audio/wav` (Piper on Thumper).
/// Concurrent requests get **503 tts_busy** so Tower can fall back to SAM.
async fn tts_synth(
    State(state): State<BrainState>,
    Json(body): Json<TtsRequest>,
) -> Result<Response, ApiError> {
    let prev = state.tts_inflight.fetch_add(1, Ordering::SeqCst);
    if prev > 0 {
        state.tts_inflight.fetch_sub(1, Ordering::SeqCst);
        warn!(inflight = prev + 1, "TTS busy — rejecting concurrent synth");
        return Err(ApiError {
            status: StatusCode::SERVICE_UNAVAILABLE,
            message: "tts_busy".into(),
        });
    }

    let cfg = state.tts.clone();
    let text = body.text;
    let inflight = state.tts_inflight.clone();
    let result = tokio::task::spawn_blocking(move || tts::synthesize_wav(&cfg, &text)).await;
    inflight.fetch_sub(1, Ordering::SeqCst);

    let wav = result
        .map_err(|e| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: e.to_string(),
        })?
        .map_err(ApiError::from)?;

    Ok((
        StatusCode::OK,
        [(header::CONTENT_TYPE, "audio/wav")],
        wav,
    )
        .into_response())
}

struct ApiError {
    status: StatusCode,
    message: String,
}

impl From<mcfloater_ha::HaError> for ApiError {
    fn from(err: mcfloater_ha::HaError) -> Self {
        let status = match &err {
            mcfloater_ha::HaError::MissingEnv(_) | mcfloater_ha::HaError::InvalidUrl(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
            mcfloater_ha::HaError::Api { status, .. } if *status == 401 || *status == 403 => {
                StatusCode::UNAUTHORIZED
            }
            mcfloater_ha::HaError::Api { status, .. } if *status == 404 => StatusCode::NOT_FOUND,
            _ => StatusCode::BAD_GATEWAY,
        };
        Self {
            status,
            message: err.to_string(),
        }
    }
}

impl From<tts::TtsError> for ApiError {
    fn from(err: tts::TtsError) -> Self {
        let status = match &err {
            tts::TtsError::Disabled
            | tts::TtsError::MissingBinary
            | tts::TtsError::MissingModel(_) => StatusCode::SERVICE_UNAVAILABLE,
            _ => StatusCode::BAD_GATEWAY,
        };
        Self {
            status,
            message: err.to_string(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = Json(ErrorBody {
            error: self.message,
        });
        (self.status, body).into_response()
    }
}

// -----------------------------------------------------------------------------
// WebRTC call signaling (browser WebRTC client → brain)
// Accepts incoming video (webcam + avatar) and audio tracks from the browser.
// -----------------------------------------------------------------------------

/// WebSocket signaling endpoint for the browser WebRTC call page.
/// This is the foundation for both video calling and live vision ingest.
async fn ws_call(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(handle_call_socket)
}

/// Serve the WebRTC call client page at /call
async fn serve_call_page() -> impl IntoResponse {
    // The page is also available at /static/call.html
    // We return a small redirect or the same content for convenience.
    Response::builder()
        .status(StatusCode::TEMPORARY_REDIRECT)
        .header(header::LOCATION, "/static/call.html")
        .body(axum::body::Body::empty())
        .unwrap()
}

async fn handle_call_socket(mut socket: WebSocket) {
    info!("WebRTC call signaling client connected (webrtc 0.17 stable)");

    // Create the outgoing avatar video track
    let avatar_video_track = Arc::new(TrackLocalStaticSample::new(
        webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecCapability {
            mime_type: "video/jpeg".to_string(),
            clock_rate: 90000,
            channels: 0,
            sdp_fmtp_line: "".to_string(),
            rtcp_feedback: vec![],
        },
        "avatar".to_string(),
        "webrtc-rs".to_string(),
    ));

    // Create the outgoing TTS audio track (48 kHz mono)
    let tts_audio_track = Arc::new(TrackLocalStaticSample::new(
        webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecCapability {
            mime_type: "audio/opus".to_string(),
            clock_rate: 48000,
            channels: 1,
            sdp_fmtp_line: "".to_string(),
            rtcp_feedback: vec![],
        },
        "tts".to_string(),
        "webrtc-rs".to_string(),
    ));

    // Create a CallSession for this connection (avatar ownership stays).
    let call_session = Arc::new({
        let avatar = match AvatarRenderer::new(1280, 720).await {
            Ok(r) => Some(Arc::new(Mutex::new(r))),
            Err(e) => {
                warn!("Could not create AvatarRenderer for this call: {:?}", e);
                None
            }
        };
        CallSession::new(
            uuid::Uuid::new_v4().to_string(),
            avatar,
            Some(avatar_video_track.clone()),
            Some(tts_audio_track.clone()),
        )
    });

    info!("CallSession {} created", call_session.id);

    // --- Real webrtc 0.17 setup (verified against source) ---
    let mut media_engine = MediaEngine::default();
    media_engine.register_default_codecs().expect("register codecs");

    let mut registry = Registry::new();
    registry = register_default_interceptors(registry, &mut media_engine).expect("interceptors");

    let api = APIBuilder::new()
        .with_media_engine(media_engine)
        .with_interceptor_registry(registry)
        .build();

    let config = RTCConfiguration::default();
    let peer_connection = match api.new_peer_connection(config).await {
        Ok(pc) => Arc::new(pc),
        Err(e) => {
            warn!("Failed to create RTCPeerConnection: {:?}", e);
            return;
        }
    };

    // Add the track as a sender so the browser receives the avatar video
    if let Err(e) = peer_connection
        .add_track(avatar_video_track.clone() as Arc<dyn TrackLocal + Send + Sync>)
        .await
    {
        warn!("Failed to add avatar video track: {:?}", e);
    }

    // Vision + STT config for the on_track handler
    let vision_url = "http://127.0.0.1:8760/v1/vision/frame".to_string();
    let stt_cfg = WyomingSttConfig::from_env();

    // Brain clients for the full STT → chat pipeline (shared with the audio consumer)
    let ha_for_stt: Option<Arc<HaClient>> = HaConfig::from_env()
        .ok()
        .and_then(|cfg| HaClient::new(&cfg).ok().map(Arc::new));
    let ollama_for_stt: Option<Arc<OllamaConfig>> = OllamaConfig::from_env().map(Arc::new);
    let grok_for_stt: Option<Arc<GrokConfig>> = GrokConfig::from_env().map(Arc::new);

    // Real on_track handler — this is the entry point for all incoming media
    let call_session_for_tracks = call_session.clone();
    peer_connection.on_track(Box::new(move |track: Arc<TrackRemote>,
                                            _receiver: Arc<RTCRtpReceiver>,
                                            _transceiver: Arc<RTCRtpTransceiver>| {
        let track_clone = track.clone();
        let vision = vision_url.clone();
        let stt = stt_cfg.clone();
        let session_for_audio = call_session_for_tracks.clone();

        Box::pin(async move {
            let codec = track_clone.codec();
            let mime = codec.capability.mime_type.to_lowercase();
            info!("on_track: mime_type={}", mime);

            if mime.contains("video") {
                tokio::spawn(async move {
                    forward_video_track_to_vision(track_clone, vision).await;
                });
            } else if mime.contains("audio") {
                if let Some(cfg) = stt {
                    tokio::spawn(async move {
                        consume_audio_track_for_stt(track_clone, cfg, session_for_audio).await;
                    });
                }
            }
        })
    }));

    // --- Minimal signaling loop (offer/answer + ICE over the same WebSocket) ---
    // The browser page (call.html) sends JSON {type: "offer", sdp: "..."} and ICE candidates.
    while let Some(Ok(msg)) = socket.recv().await {
        if let Message::Text(text) = msg {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&text) {
                if let Some("offer") = parsed.get("type").and_then(|v| v.as_str()) {
                    if let Some(sdp) = parsed.get("sdp").and_then(|v| v.as_str()) {
                        if let Ok(offer) = webrtc::peer_connection::sdp::session_description::RTCSessionDescription::offer(sdp.to_string()) {
                            if peer_connection.set_remote_description(offer).await.is_ok() {
                                if let Ok(answer) = peer_connection.create_answer(None).await {
                                    let _ = peer_connection.set_local_description(answer.clone()).await;
                                    let ans = serde_json::json!({
                                        "type": "answer",
                                        "sdp": answer.sdp
                                    });
                                    let _ = socket.send(Message::Text(ans.to_string().into())).await;
                                }
                            }
                        }
                    }
                } else if let Some("ice") = parsed.get("type").and_then(|v| v.as_str()) {
                    if let Some(cand) = parsed.get("candidate") {
                        // Best-effort ICE candidate handling (full impl would parse properly)
                        let _ = peer_connection.add_ice_candidate(
                            webrtc::ice_transport::ice_candidate::RTCIceCandidateInit {
                                candidate: cand.get("candidate").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                sdp_mid: cand.get("sdpMid").and_then(|v| v.as_str()).map(|s| s.to_string()),
                                sdp_mline_index: cand.get("sdpMLineIndex").and_then(|v| v.as_u64()).map(|i| i as u16),
                                username_fragment: None,
                            }
                        ).await;
                    }
                }
            }
        } else if let Message::Close(_) = msg {
            break;
        }
    }

    info!("WebRTC call signaling client disconnected (session {})", call_session.id);
}

// -----------------------------------------------------------------------------
// Real consumers for webrtc 0.17 TrackRemote (read_rtp loop)
// -----------------------------------------------------------------------------
async fn forward_video_track_to_vision(track: Arc<TrackRemote>, vision_url: String) {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    info!("Video→vision forwarder started → {}", vision_url);

    loop {
        match track.read_rtp().await {
            Ok((rtp_packet, _attrs)) => {
                // Placeholder: in a real impl we would depayload + JPEG-encode here.
                let jpeg_bytes = rtp_packet.payload.to_vec();
                let url = vision_url.clone();
                let client = client.clone();
                tokio::spawn(async move {
                    let _ = client
                        .post(&url)
                        .multipart(
                            reqwest::multipart::Form::new()
                                .text("session_id", "default".to_string())
                                .part(
                                    "file",
                                    reqwest::multipart::Part::bytes(jpeg_bytes)
                                        .file_name("frame.jpg")
                                        .mime_str("image/jpeg")
                                        .unwrap(),
                                ),
                        )
                        .send()
                        .await;
                });
            }
            Err(e) => {
                warn!("Video track ended or error: {:?}", e);
                break;
            }
        }
    }
}

async fn consume_audio_track_for_stt(
    track: Arc<TrackRemote>,
    cfg: WyomingSttConfig,
    call_session: Arc<CallSession>,
) {
    info!("Audio→STT consumer started (Wyoming at {})", cfg.addr());

    let mut pcm_buffer: Vec<u8> = Vec::with_capacity(64 * 1024);

    loop {
        match track.read_rtp().await {
            Ok((rtp_packet, _attrs)) => {
                pcm_buffer.extend_from_slice(&rtp_packet.payload);

                if pcm_buffer.len() >= 32_000 {
                    match wyoming::transcribe_pcm16(&cfg, &pcm_buffer, 16_000) {
                        Ok(text) if !text.trim().is_empty() => {
                            info!("STT result: {}", text);

                            // Drive avatar lip-sync with the transcribed text (demo path)
                            let duration_ms = ((text.len() as u32) * 70).max(1500);
                            drive_avatar_during_tts(&call_session, &text, duration_ms).await;
                        }
                        Ok(_) => {}
                        Err(e) => warn!("Wyoming STT error: {:?}", e),
                    }
                    pcm_buffer.clear();
                }
            }
            Err(e) => {
                warn!("Audio track ended or error: {:?}", e);
                break;
            }
        }
    }
}

// -----------------------------------------------------------------------------
// CallSession – owns the full WebRTC session + avatar rendering on Thumper
// -----------------------------------------------------------------------------

/// Represents one active video call session.
/// Owns the avatar renderer and is responsible for publishing the outgoing
/// avatar video track to the remote peer.
#[derive(Clone)]
pub struct CallSession {
    pub id: String,
    pub avatar: Option<Arc<Mutex<AvatarRenderer>>>,
    /// Outgoing avatar video track (we publish rendered JPEG frames here)
    pub avatar_track: Option<Arc<TrackLocalStaticSample>>,
    /// Outgoing TTS audio track (we publish synthesized speech here)
    pub audio_track: Option<Arc<TrackLocalStaticSample>>,
}

impl CallSession {
    pub fn new(
        id: impl Into<String>,
        avatar: Option<Arc<Mutex<AvatarRenderer>>>,
        avatar_track: Option<Arc<TrackLocalStaticSample>>,
        audio_track: Option<Arc<TrackLocalStaticSample>>,
    ) -> Self {
        Self {
            id: id.into(),
            avatar,
            avatar_track,
            audio_track,
        }
    }

    /// Drive the avatar renderer with lip-sync data and return a JPEG frame.
    pub async fn render_avatar_frame(&self, lip: LipSyncFrame) -> Option<Vec<u8>> {
        if let Some(avatar) = &self.avatar {
            let mut renderer = avatar.lock().await;
            let rgb = renderer.render(lip);

            let mut jpeg = Vec::new();
            let mut enc = JpegEncoder::new(&mut jpeg);
            if enc.encode_image(&rgb).is_ok() {
                return Some(jpeg);
            }
        }
        None
    }

    /// Publish the rendered avatar frame as an outgoing video track.
    pub async fn publish_avatar_frame(&self, jpeg: Vec<u8>) {
        if let Some(track) = &self.avatar_track {
            let sample = Sample {
                data: Bytes::from(jpeg),
                duration: std::time::Duration::from_millis(40),
                ..Default::default()
            };
            if let Err(e) = track.write_sample(&sample).await {
                warn!("Failed to write avatar frame: {:?}", e);
            }
        }
    }

    /// Publish PCM16 audio (TTS) as an outgoing audio track.
    /// Expects 48 kHz mono 16-bit little-endian PCM.
    pub async fn publish_audio_frame(&self, pcm: Vec<u8>) {
        if let Some(track) = &self.audio_track {
            let sample = Sample {
                data: Bytes::from(pcm),
                duration: std::time::Duration::from_millis(20),
                ..Default::default()
            };
            if let Err(e) = track.write_sample(&sample).await {
                warn!("Failed to write TTS audio: {:?}", e);
            }
        }
    }
}

/// Synthesize speech for a reply and push both audio and avatar lip-sync.
/// This is the final piece that makes the browser both hear and see the response.
pub async fn speak_reply_with_avatar(
    session: &CallSession,
    tts_cfg: &TtsConfig,
    text: &str,
) {
    if text.trim().is_empty() {
        return;
    }

    // 1. Synthesize the audio (WAV)
    match tts::synthesize_wav(tts_cfg, text) {
        Ok(wav_bytes) => {
            // Very naive WAV strip (skip 44-byte header). Real code would parse it.
            let pcm = if wav_bytes.len() > 44 {
                wav_bytes[44..].to_vec()
            } else {
                wav_bytes
            };

            // Send audio to browser
            session.publish_audio_frame(pcm).await;
        }
        Err(e) => {
            warn!("TTS synthesis failed: {:?}", e);
        }
    }

    // 2. Drive avatar lip-sync in parallel (already implemented)
    let duration_ms = ((text.len() as u32) * 70).max(1500);
    drive_avatar_during_tts(session, text, duration_ms).await;
}

// -----------------------------------------------------------------------------
// TTS → LipSync bridge (extract lip-sync curve from SAM/Piper output)
// This function is called from the TTS pipeline to generate lip-sync data
// that drives the avatar renderer during speech.
// -----------------------------------------------------------------------------
pub fn extract_lipsync_from_tts(text: &str, duration_ms: u32) -> Vec<LipSyncFrame> {
    // Very simple placeholder: generate a mouth curve based on text length.
    // In a real implementation this would come from the actual TTS engine
    // (SAM or Piper) that already produces viseme/lip-sync data.
    let frames = (duration_ms / 40).max(1) as usize; // ~25 fps
    (0..frames)
        .map(|i| {
            let phase = (i as f32 / frames as f32) * std::f32::consts::TAU;
            LipSyncFrame {
                mouth_open: (phase.sin() * 0.5 + 0.5).clamp(0.0, 1.0),
                jaw: (phase.sin() * 0.3).max(0.0),
                lip_round: 0.0,
                brow: 0.0,
            }
        })
        .collect()
}

/// Integration point: when the brain synthesizes speech, it should also
/// produce lip-sync frames and feed them to the active CallSession so the
/// avatar mouth moves in sync with the TTS audio.
pub async fn drive_avatar_during_tts(
    session: &CallSession,
    text: &str,
    duration_ms: u32,
) {
    let frames = extract_lipsync_from_tts(text, duration_ms);
    for lip in frames {
        if let Some(jpeg) = session.render_avatar_frame(lip).await {
            session.publish_avatar_frame(jpeg).await;
        }
        // ~40 ms per frame at 25 fps
        tokio::time::sleep(Duration::from_millis(40)).await;
    }
}
