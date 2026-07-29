//! JSON protocol between Tower (face) and Thumper (brain).

use mcfloater_ha::EntityState;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub ok: bool,
    pub service: String,
    /// HA REST reachable with valid token (API up).
    pub ha_ok: bool,
    /// At least one switch/light/scene exists (actual C&C possible).
    #[serde(default)]
    pub ha_control_ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ha_message: Option<String>,
    /// Short device inventory, e.g. "0 sw · 0 lt · 0 sc".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ha_control: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ha: Option<Value>,
    /// Natural TTS (Piper on Thumper) ready.
    #[serde(default)]
    pub tts_ok: bool,
    /// Piper currently synthesizing (clients should use SAM for this line).
    #[serde(default)]
    pub tts_busy: bool,
    /// In-flight Piper jobs (0 or 1 today; reserved for queueing).
    #[serde(default)]
    pub tts_inflight: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tts: Option<String>,
    /// Wyoming Whisper (or other ASR) reachable.
    #[serde(default)]
    pub stt_ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stt: Option<String>,
    /// Ollama dialog reachable.
    #[serde(default)]
    pub llm_ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub llm: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SttResponse {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtsRequest {
    /// Text to speak with natural TTS on Thumper.
    pub text: String,
    /// Optional voice override (reserved; currently uses MCFLOATER_PIPER_MODEL).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub voice: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityIdBody {
    pub entity_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceCallBody {
    pub domain: String,
    pub service: String,
    pub entity_id: String,
    #[serde(default)]
    pub data: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    /// User utterance or typed command.
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    /// Line for SAM TTS on Tower.
    pub reply: String,
    /// High-level state hint for GUI.
    pub state: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<ChatAction>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatAction {
    pub kind: String,
    pub entity_id: String,
    pub service: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_state: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatesResponse {
    pub entities: Vec<EntityState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorBody {
    pub error: String,
}
