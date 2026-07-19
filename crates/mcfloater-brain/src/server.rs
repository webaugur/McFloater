//! Axum HTTP server for the brain control plane.

use crate::intent::handle_chat;
use crate::protocol::*;
use crate::tts::{self, TtsConfig};
use axum::extract::{Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use mcfloater_ha::{HaClient, HaConfig};
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
    /// In-flight Piper synthesizers (Tower uses this for SAM fallback).
    pub tts_inflight: Arc<AtomicU32>,
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
    let state = BrainState {
        ha: Arc::new(ha),
        tts: Arc::new(tts),
        tts_inflight: Arc::new(AtomicU32::new(0)),
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
    match state.ha.check() {
        Ok(v) => Json(HealthResponse {
            ok: true,
            service: "mcfloater-brain".into(),
            ha_ok: true,
            ha_message: None,
            ha: Some(v),
            tts_ok,
            tts_busy,
            tts_inflight,
            tts: tts_detail,
        }),
        Err(err) => Json(HealthResponse {
            ok: true,
            service: "mcfloater-brain".into(),
            ha_ok: false,
            ha_message: Some(err.to_string()),
            ha: None,
            tts_ok,
            tts_busy,
            tts_inflight,
            tts: tts_detail,
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
    let resp = tokio::task::spawn_blocking(move || handle_chat(&ha, &body))
        .await
        .unwrap_or_else(|e| ChatResponse {
            reply: "Brain task failed.".into(),
            state: "idle".into(),
            actions: vec![],
            error: Some(e.to_string()),
        });
    Json(resp)
}

/// Natural TTS: returns `audio/wav` (Piper on Thumper).
/// Concurrent requests get **503 tts_busy** so Tower can fall back to SAM.
async fn tts_synth(
    State(state): State<BrainState>,
    Json(body): Json<TtsRequest>,
) -> Result<Response, ApiError> {
    // Single-flight Piper: second client should use local SAM.
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
            tts::TtsError::Disabled | tts::TtsError::MissingBinary | tts::TtsError::MissingModel(_) => {
                StatusCode::SERVICE_UNAVAILABLE
            }
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
