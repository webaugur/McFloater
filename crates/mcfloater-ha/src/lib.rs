//! Home Assistant REST client for McFloater brain (runs on Thumper).

use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Duration;
use thiserror::Error;
use tracing::debug;
use url::Url;

/// Configuration for the HA REST API.
#[derive(Debug, Clone)]
pub struct HaConfig {
    pub url: String,
    pub token: String,
}

impl HaConfig {
    /// Build from `HA_URL` and `HA_TOKEN` environment variables.
    pub fn from_env() -> Result<Self, HaError> {
        let url = std::env::var("HA_URL").map_err(|_| HaError::MissingEnv("HA_URL"))?;
        let token = std::env::var("HA_TOKEN").map_err(|_| HaError::MissingEnv("HA_TOKEN"))?;
        if token.trim().is_empty() {
            return Err(HaError::MissingEnv("HA_TOKEN"));
        }
        Ok(Self {
            url: url.trim_end_matches('/').to_string(),
            token,
        })
    }
}

#[derive(Debug, Error)]
pub enum HaError {
    #[error("missing environment variable {0}")]
    MissingEnv(&'static str),
    #[error("invalid HA_URL: {0}")]
    InvalidUrl(String),
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("HA API {status}: {body}")]
    Api { status: u16, body: String },
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Minimal entity state from `GET /api/states`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityState {
    pub entity_id: String,
    pub state: String,
    #[serde(default)]
    pub attributes: Value,
}

/// Blocking REST client (CLI + simple brain handlers).
#[derive(Debug, Clone)]
pub struct HaClient {
    base: Url,
    http: Client,
}

impl HaClient {
    pub fn new(config: &HaConfig) -> Result<Self, HaError> {
        let base = Url::parse(&format!("{}/", config.url.trim_end_matches('/')))
            .map_err(|e| HaError::InvalidUrl(e.to_string()))?;

        let mut headers = HeaderMap::new();
        let auth = format!("Bearer {}", config.token.trim());
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&auth).map_err(|e| HaError::InvalidUrl(e.to_string()))?,
        );
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        let http = Client::builder()
            .default_headers(headers)
            .timeout(Duration::from_secs(15))
            .build()?;

        Ok(Self { base, http })
    }

    fn api(&self, path: &str) -> Result<Url, HaError> {
        self.base
            .join(path.trim_start_matches('/'))
            .map_err(|e| HaError::InvalidUrl(e.to_string()))
    }

    /// `GET /api/` — confirms token and HA is up.
    pub fn check(&self) -> Result<Value, HaError> {
        let url = self.api("api/")?;
        debug!(%url, "HA GET");
        let resp = self.http.get(url).send()?;
        Self::json_response(resp)
    }

    /// `GET /api/states` — optionally filter by domain prefix (`switch`, `light`, …).
    pub fn states(&self, domain: Option<&str>) -> Result<Vec<EntityState>, HaError> {
        let url = self.api("api/states")?;
        debug!(%url, "HA GET states");
        let resp = self.http.get(url).send()?;
        let all: Vec<EntityState> = Self::json_response(resp)?;
        Ok(match domain {
            Some(d) => {
                let prefix = format!("{d}.");
                all.into_iter()
                    .filter(|e| e.entity_id.starts_with(&prefix))
                    .collect()
            }
            None => all,
        })
    }

    /// `GET /api/states/{entity_id}`
    pub fn state(&self, entity_id: &str) -> Result<EntityState, HaError> {
        let path = format!("api/states/{entity_id}");
        let url = self.api(&path)?;
        debug!(%url, "HA GET state");
        let resp = self.http.get(url).send()?;
        Self::json_response(resp)
    }

    /// `POST /api/services/{domain}/{service}`
    pub fn call_service(
        &self,
        domain: &str,
        service: &str,
        entity_id: &str,
        extra: Option<Value>,
    ) -> Result<Value, HaError> {
        let path = format!("api/services/{domain}/{service}");
        let url = self.api(&path)?;
        let mut body = json!({ "entity_id": entity_id });
        if let Some(Value::Object(map)) = extra {
            if let Value::Object(ref mut dest) = body {
                for (k, v) in map {
                    dest.insert(k, v);
                }
            }
        }
        debug!(%url, %body, "HA POST service");
        let resp = self.http.post(url).json(&body).send()?;
        // HA often returns a list of changed states (or empty).
        let status = resp.status();
        let text = resp.text()?;
        if !status.is_success() {
            return Err(HaError::Api {
                status: status.as_u16(),
                body: text,
            });
        }
        if text.trim().is_empty() {
            return Ok(Value::Null);
        }
        Ok(serde_json::from_str(&text)?)
    }

    pub fn turn_on(&self, entity_id: &str) -> Result<Value, HaError> {
        let domain = domain_of(entity_id);
        self.call_service(domain, "turn_on", entity_id, None)
    }

    pub fn turn_off(&self, entity_id: &str) -> Result<Value, HaError> {
        let domain = domain_of(entity_id);
        self.call_service(domain, "turn_off", entity_id, None)
    }

    pub fn toggle(&self, entity_id: &str) -> Result<Value, HaError> {
        let domain = domain_of(entity_id);
        self.call_service(domain, "toggle", entity_id, None)
    }

    fn json_response<T: for<'de> Deserialize<'de>>(
        resp: reqwest::blocking::Response,
    ) -> Result<T, HaError> {
        let status = resp.status();
        let text = resp.text()?;
        if !status.is_success() {
            return Err(HaError::Api {
                status: status.as_u16(),
                body: text,
            });
        }
        Ok(serde_json::from_str(&text)?)
    }
}

/// Domain part of `switch.desk_lamp` → `switch`.
pub fn domain_of(entity_id: &str) -> &str {
    entity_id.split('.').next().unwrap_or(entity_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domain_split() {
        assert_eq!(domain_of("switch.desk"), "switch");
        assert_eq!(domain_of("light.kitchen"), "light");
    }
}
