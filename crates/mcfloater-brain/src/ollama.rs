//! Ollama HTTP client with multiple model lanes.
//!
//! - **Chat** (`MCFLOATER_OLLAMA_MODEL`): fast banter (default llama3.2:3b)
//! - **Instruct** (`MCFLOATER_OLLAMA_INSTRUCT_MODEL`): direction-following for
//!   macros / schedules / multi-step plans (default mistral)
//!
//! Home Assistant inventory is always injected so models cannot invent devices.

use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;
use tracing::{info, warn};

/// Which Ollama weights to load for this call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LlmLane {
    /// Short spoken chat / personality.
    Chat,
    /// Solid instruction following: macros, schedules, structured plans.
    Instruct,
}

const PERSONA_CHAT: &str = "\
You are Floaty McFloater: a lab AI assistant with a light Max Headroom vibe \
(witty TV-host energy, not a butler).

Hard rules (never break these):
- Speak in short sentences for text-to-speech (usually under 35 words).
- Never use letter stutters (no G-g-great, no hyphen glitch text).
- You only know devices listed under HOME ASSISTANT FACTS. Never invent rooms, \
appliances, or hardware not on that list.
- Never claim you toggled hardware unless a FACT line says a command succeeded.
- If no devices are listed, say so; do not suggest turning anything on.
";

const PERSONA_INSTRUCT: &str = "\
You are Floaty McFloater's PLANNER (instruction-following mode).
You convert user goals into clear, executable steps for a lab Home Assistant.

Hard rules:
- Output ONLY valid JSON (no markdown fences, no prose outside JSON).
- Use ONLY devices listed under HOME ASSISTANT FACTS. Never invent entity_ids.
- If a requested device is missing, set ok=false and explain in summary.
- Prefer HA-native scheduling language in steps (e.g. \"create HA automation\" \
ideas) but only reference real entities.
- Keep summary under 40 words, speakable.
- JSON schema:
  {
    \"ok\": true/false,
    \"summary\": \"short speakable result\",
    \"lane\": \"instruct\",
    \"steps\": [
      {\"action\": \"ha_turn_on|ha_turn_off|ha_toggle|wait|speak|note\",
       \"entity_id\": \"optional domain.name\",
       \"seconds\": 0,
       \"text\": \"optional\"}
    ]
  }
";

#[derive(Debug, Error)]
pub enum OllamaError {
    #[error("ollama not configured")]
    NotConfigured,
    #[error("ollama request: {0}")]
    Request(String),
    #[error("ollama empty reply")]
    EmptyReply,
}

#[derive(Debug, Clone)]
pub struct OllamaConfig {
    pub base_url: String,
    /// Fast chat / banter model.
    pub chat_model: String,
    /// Instruction-following model (macros, schedules, plans).
    pub instruct_model: String,
    pub timeout: Duration,
}

impl OllamaConfig {
    pub fn from_env() -> Option<Self> {
        let base = std::env::var("MCFLOATER_OLLAMA_URL")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "http://127.0.0.1:11434".into());
        if matches!(
            std::env::var("MCFLOATER_OLLAMA").as_deref(),
            Ok("0") | Ok("off") | Ok("false")
        ) {
            return None;
        }
        // General local chat (direction: Llama 3.1).
        let chat_model = std::env::var("MCFLOATER_OLLAMA_MODEL")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "llama3.1:8b".into());
        // Direction-following / macro opinion (Mistral).
        let instruct_model = std::env::var("MCFLOATER_OLLAMA_INSTRUCT_MODEL")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "mistral".into());
        let timeout = Duration::from_millis(
            std::env::var("MCFLOATER_OLLAMA_TIMEOUT_MS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(90_000),
        );
        Some(Self {
            base_url: base.trim_end_matches('/').to_string(),
            chat_model,
            instruct_model,
            timeout,
        })
    }

    pub fn model_for(&self, lane: LlmLane) -> &str {
        match lane {
            LlmLane::Chat => &self.chat_model,
            LlmLane::Instruct => &self.instruct_model,
        }
    }

    pub fn status_line(&self) -> String {
        format!(
            "ollama chat={} instruct={} url={}",
            self.chat_model, self.instruct_model, self.base_url
        )
    }

    pub fn probe(&self) -> Result<(), OllamaError> {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(3))
            .build()
            .map_err(|e| OllamaError::Request(e.to_string()))?;
        let url = format!("{}/api/tags", self.base_url);
        let resp = client
            .get(&url)
            .send()
            .map_err(|e| OllamaError::Request(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(OllamaError::Request(format!("HTTP {}", resp.status())));
        }
        Ok(())
    }

    /// Spoken chat (personality lane).
    pub fn chat(&self, user_text: &str, world_facts: &str) -> Result<String, OllamaError> {
        self.complete(LlmLane::Chat, PERSONA_CHAT, user_text, world_facts, 0.35, 90)
            .map(|s| truncate_speakable(&s, 220))
    }

    /// Instruction / macro / schedule planning (Mistral by default).
    /// Returns raw model text (preferably JSON).
    pub fn instruct(&self, user_text: &str, world_facts: &str) -> Result<String, OllamaError> {
        self.complete(
            LlmLane::Instruct,
            PERSONA_INSTRUCT,
            user_text,
            world_facts,
            0.15,
            400,
        )
    }

    fn complete(
        &self,
        lane: LlmLane,
        persona: &str,
        user_text: &str,
        world_facts: &str,
        temperature: f32,
        num_predict: i32,
    ) -> Result<String, OllamaError> {
        let model = self.model_for(lane).to_string();
        let client = reqwest::blocking::Client::builder()
            .timeout(self.timeout)
            .build()
            .map_err(|e| OllamaError::Request(e.to_string()))?;

        let system = format!(
            "{persona}\n\nHOME ASSISTANT FACTS (authoritative — do not invent beyond this):\n{world_facts}"
        );

        let body = ChatRequest {
            model: model.clone(),
            stream: false,
            options: ChatOptions {
                temperature,
                num_predict,
            },
            messages: vec![
                ChatMessage {
                    role: "system".into(),
                    content: system,
                },
                ChatMessage {
                    role: "user".into(),
                    content: user_text.into(),
                },
            ],
        };

        let url = format!("{}/api/chat", self.base_url);
        info!(%model, ?lane, "ollama complete");
        let resp = client
            .post(&url)
            .json(&body)
            .send()
            .map_err(|e| OllamaError::Request(e.to_string()))?;
        let status = resp.status();
        let text = resp.text().map_err(|e| OllamaError::Request(e.to_string()))?;
        if !status.is_success() {
            // If instruct model missing, fall back to chat model once.
            if lane == LlmLane::Instruct && model != self.chat_model {
                warn!(%status, %text, "instruct model failed — falling back to chat model");
                return self.complete(
                    LlmLane::Chat,
                    persona,
                    user_text,
                    world_facts,
                    temperature,
                    num_predict,
                );
            }
            warn!(%status, %text, "ollama error body");
            return Err(OllamaError::Request(format!("HTTP {status}: {text}")));
        }
        let parsed: ChatResponse = serde_json::from_str(&text)
            .map_err(|e| OllamaError::Request(format!("json: {e}")))?;
        let reply = parsed
            .message
            .map(|m| m.content)
            .unwrap_or_default()
            .trim()
            .to_string();
        if reply.is_empty() {
            return Err(OllamaError::EmptyReply);
        }
        Ok(reply)
    }
}

/// True when the user is asking for multi-step / scheduled / macro-style direction.
pub fn looks_like_instruct_task(text: &str) -> bool {
    let t = text.to_ascii_lowercase();
    const KEYS: &[&str] = &[
        "schedule",
        "every day",
        "every morning",
        "every night",
        "weekdays",
        "weekend",
        "macro",
        "automate",
        "automation",
        "remind me",
        "set a timer",
        "then ",
        " after ",
        "before ",
        "if ",
        "when i",
        "when the",
        "plan ",
        "steps",
        "sequence",
        "routine",
        "at  ",
        " at 0",
        " at 1",
        " at 2",
        " at 3",
        " at 4",
        " at 5",
        " at 6",
        " at 7",
        " at 8",
        " at 9",
        "o'clock",
        "oclock",
    ];
    KEYS.iter().any(|k| t.contains(k))
}

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    stream: bool,
    options: ChatOptions,
    messages: Vec<ChatMessage>,
}

#[derive(Debug, Serialize)]
struct ChatOptions {
    temperature: f32,
    num_predict: i32,
}

#[derive(Debug, Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    message: Option<ChatMessage>,
}

fn truncate_speakable(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut t: String = s.chars().take(max.saturating_sub(1)).collect();
    if let Some(i) = t.rfind(['.', '!', '?']) {
        t.truncate(i + 1);
    } else {
        t.push('…');
    }
    t
}
