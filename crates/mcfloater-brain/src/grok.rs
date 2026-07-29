//! xAI Grok API for real-world / physics / open knowledge questions.
//!
//! Key stays on Thumper only (`XAI_API_KEY` or `MCFLOATER_GROK_API_KEY`).

use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;
use tracing::{info, warn};

const GROK_PERSONA: &str = "\
You are helping Floaty McFloater answer a real-world or science question for a lab assistant.
Rules:
- Be accurate and clear; short enough for text-to-speech (under ~60 words unless the user asked for detail).
- No letter stutters or glitch-speak.
- Do NOT invent Home Assistant devices, rooms, or claim you control hardware.
- If the question is about their lab devices, say you only answer world/science facts here.
";

#[derive(Debug, Error)]
pub enum GrokError {
    #[error("grok not configured (set XAI_API_KEY or MCFLOATER_GROK_API_KEY)")]
    NotConfigured,
    #[error("grok request: {0}")]
    Request(String),
    #[error("grok empty reply")]
    EmptyReply,
}

#[derive(Debug, Clone)]
pub struct GrokConfig {
    pub api_key: String,
    pub base_url: String,
    pub model: String,
    pub timeout: Duration,
}

impl GrokConfig {
    pub fn from_env() -> Option<Self> {
        if matches!(
            std::env::var("MCFLOATER_GROK").as_deref(),
            Ok("0") | Ok("off") | Ok("false")
        ) {
            return None;
        }
        let api_key = std::env::var("MCFLOATER_GROK_API_KEY")
            .or_else(|_| std::env::var("XAI_API_KEY"))
            .ok()
            .filter(|s| !s.trim().is_empty())?;
        let base_url = std::env::var("MCFLOATER_GROK_URL")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "https://api.x.ai/v1".into());
        let model = std::env::var("MCFLOATER_GROK_MODEL")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "grok-3-mini".into());
        let timeout = Duration::from_millis(
            std::env::var("MCFLOATER_GROK_TIMEOUT_MS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(60_000),
        );
        Some(Self {
            api_key: api_key.trim().to_string(),
            base_url: base_url.trim_end_matches('/').to_string(),
            model,
            timeout,
        })
    }

    pub fn status_line(&self) -> String {
        format!("grok model={} url={}", self.model, self.base_url)
    }

    pub fn probe(&self) -> Result<(), GrokError> {
        // Lightweight: models list if available; else skip heavy call.
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .map_err(|e| GrokError::Request(e.to_string()))?;
        let url = format!("{}/models", self.base_url);
        let resp = client
            .get(&url)
            .bearer_auth(&self.api_key)
            .send()
            .map_err(|e| GrokError::Request(e.to_string()))?;
        if resp.status().is_success() || resp.status().as_u16() == 404 {
            // 404 on /models is ok — key may still work for chat
            return Ok(());
        }
        if resp.status().as_u16() == 401 || resp.status().as_u16() == 403 {
            return Err(GrokError::Request(format!("auth {}", resp.status())));
        }
        // Non-auth errors: still mark configured; chat will fail loudly
        Ok(())
    }

    pub fn chat(&self, user_text: &str, world_facts: &str) -> Result<String, GrokError> {
        let client = reqwest::blocking::Client::builder()
            .timeout(self.timeout)
            .build()
            .map_err(|e| GrokError::Request(e.to_string()))?;

        let system = format!(
            "{GROK_PERSONA}\n\nLab context (do not invent devices beyond this):\n{world_facts}"
        );

        let body = ChatCompletionsRequest {
            model: self.model.clone(),
            temperature: 0.4,
            max_tokens: 256,
            messages: vec![
                Msg {
                    role: "system".into(),
                    content: system,
                },
                Msg {
                    role: "user".into(),
                    content: user_text.into(),
                },
            ],
        };

        let url = format!("{}/chat/completions", self.base_url);
        info!(model = %self.model, "grok chat");
        let resp = client
            .post(&url)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .map_err(|e| GrokError::Request(e.to_string()))?;
        let status = resp.status();
        let text = resp.text().map_err(|e| GrokError::Request(e.to_string()))?;
        if !status.is_success() {
            warn!(%status, %text, "grok error body");
            return Err(GrokError::Request(format!("HTTP {status}: {text}")));
        }
        let parsed: ChatCompletionsResponse = serde_json::from_str(&text)
            .map_err(|e| GrokError::Request(format!("json: {e}")))?;
        let reply = parsed
            .choices
            .into_iter()
            .next()
            .and_then(|c| c.message)
            .map(|m| m.content)
            .unwrap_or_default()
            .trim()
            .to_string();
        if reply.is_empty() {
            return Err(GrokError::EmptyReply);
        }
        Ok(truncate_speakable(&reply, 400))
    }
}

/// Real-world knowledge / physics / science — route to Grok.
pub fn looks_like_world_question(text: &str) -> bool {
    let t = text.to_ascii_lowercase();
    const KEYS: &[&str] = &[
        "physics",
        "quantum",
        "relativity",
        "gravity",
        "thermodynamic",
        "entropy",
        "chemistry",
        "biology",
        "astronomy",
        "astrophysic",
        "cosmology",
        "universe",
        "galaxy",
        "black hole",
        "speed of light",
        "planck",
        "newton",
        "einstein",
        "molecule",
        "atom",
        "electron",
        "climate",
        "weather",
        "geology",
        "history of",
        "who invented",
        "who discovered",
        "real world",
        "in the real world",
        "scientifically",
        "scientific",
        "how does the sun",
        "how does earth",
        "why is the sky",
        "what causes",
        "explain the physics",
        "calculate the",
        "formula for",
        "wikipedia",
        "news about",
        "current events",
    ];
    if KEYS.iter().any(|k| t.contains(k)) {
        return true;
    }
    // "what is X" / "how does X work" for non-lab topics — soft heuristic
    if (t.starts_with("what is ") || t.starts_with("how does ") || t.starts_with("why does ") || t.starts_with("why is "))
        && !t.contains("lamp")
        && !t.contains("plug")
        && !t.contains("home assistant")
        && !t.contains("switch")
        && !t.contains("floaty")
    {
        return true;
    }
    false
}

#[derive(Debug, Serialize)]
struct ChatCompletionsRequest {
    model: String,
    temperature: f32,
    max_tokens: u32,
    messages: Vec<Msg>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Msg {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionsResponse {
    #[serde(default)]
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: Option<Msg>,
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
