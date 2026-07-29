//! Thin HTTP client for the McFloater brain (Tower → Thumper).

use mcfloater_brain::{
    ChatRequest, ChatResponse, HealthResponse, StatesResponse, SttResponse, TtsRequest,
};
use mcfloater_ha::EntityState;
use reqwest::blocking::Client;
use serde_json::{json, Value};
use std::time::Duration;

pub struct BrainClient {
    base: String,
    http: Client,
}

impl BrainClient {
    pub fn new(base_url: &str) -> Result<Self, String> {
        Self::new_with_timeout(base_url, Duration::from_secs(120))
    }

    pub fn new_with_timeout(base_url: &str, timeout: Duration) -> Result<Self, String> {
        let base = base_url.trim().trim_end_matches('/').to_string();
        let http = Client::builder()
            .timeout(timeout)
            .connect_timeout(Duration::from_secs(3).min(timeout))
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self { base, http })
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base, path)
    }

    pub fn health(&self) -> Result<HealthResponse, String> {
        self.get_json("/health")
    }

    pub fn states(&self, domain: Option<&str>) -> Result<Vec<EntityState>, String> {
        let path = match domain {
            Some(d) => format!("/v1/ha/states?domain={d}"),
            None => "/v1/ha/states".into(),
        };
        let resp: StatesResponse = self.get_json(&path)?;
        Ok(resp.entities)
    }

    pub fn turn_on(&self, entity_id: &str) -> Result<Value, String> {
        self.post_json("/v1/ha/turn_on", &json!({ "entity_id": entity_id }))
    }

    pub fn turn_off(&self, entity_id: &str) -> Result<Value, String> {
        self.post_json("/v1/ha/turn_off", &json!({ "entity_id": entity_id }))
    }

    pub fn toggle(&self, entity_id: &str) -> Result<Value, String> {
        self.post_json("/v1/ha/toggle", &json!({ "entity_id": entity_id }))
    }

    pub fn chat(&self, text: &str) -> Result<ChatResponse, String> {
        self.post_json("/v1/chat", &ChatRequest { text: text.into() })
    }

    /// Speech-to-text on Thumper (Wyoming Whisper). Body is WAV bytes.
    pub fn stt_wav(&self, wav: &[u8]) -> Result<String, String> {
        let url = self.url("/v1/stt");
        let resp = self
            .http
            .post(&url)
            .header("Content-Type", "audio/wav")
            .body(wav.to_vec())
            .send()
            .map_err(|e| e.to_string())?;
        let status = resp.status();
        let text = resp.text().map_err(|e| e.to_string())?;
        if !status.is_success() {
            return Err(format!("brain STT {status}: {text}"));
        }
        let parsed: SttResponse =
            serde_json::from_str(&text).map_err(|e| format!("STT JSON: {e}; body={text}"))?;
        if let Some(err) = parsed.error {
            return Err(err);
        }
        Ok(parsed.text)
    }

    /// Natural TTS on Thumper (Piper). Returns raw WAV bytes.
    pub fn tts_wav(&self, text: &str) -> Result<Vec<u8>, String> {
        let url = self.url("/v1/tts");
        let resp = self
            .http
            .post(&url)
            .json(&TtsRequest {
                text: text.into(),
                voice: None,
            })
            .send()
            .map_err(|e| e.to_string())?;
        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().unwrap_or_default();
            return Err(format!("brain TTS {status}: {text}"));
        }
        let bytes = resp.bytes().map_err(|e| e.to_string())?;
        Ok(bytes.to_vec())
    }

    fn get_json<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T, String> {
        let url = self.url(path);
        let resp = self.http.get(&url).send().map_err(|e| e.to_string())?;
        let status = resp.status();
        let text = resp.text().map_err(|e| e.to_string())?;
        if !status.is_success() {
            return Err(format!("brain {status}: {text}"));
        }
        serde_json::from_str(&text).map_err(|e| format!("brain JSON: {e}; body={text}"))
    }

    fn post_json<B: serde::Serialize, T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T, String> {
        let url = self.url(path);
        let resp = self
            .http
            .post(&url)
            .json(body)
            .send()
            .map_err(|e| e.to_string())?;
        let status = resp.status();
        let text = resp.text().map_err(|e| e.to_string())?;
        if !status.is_success() {
            return Err(format!("brain {status}: {text}"));
        }
        if text.trim().is_empty() {
            return serde_json::from_str("null").map_err(|e| e.to_string());
        }
        serde_json::from_str(&text).map_err(|e| format!("brain JSON: {e}; body={text}"))
    }
}
