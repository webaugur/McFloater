//! Intent router: HA first, then Llama 3.1 / Mistral / Grok by lane.

use crate::grok::{looks_like_world_question, GrokConfig};
use crate::ollama::{looks_like_instruct_task, OllamaConfig};
use crate::protocol::{ChatAction, ChatRequest, ChatResponse};
use mcfloater_ha::HaClient;
use tracing::{info, warn};

/// Handle a user chat line.
///
/// Lanes (after HA / greetings):
/// 1. **Instruct (Mistral)** — macros, schedules, multi-step direction following  
/// 2. **Grok API** — real-world / physics / open knowledge  
/// 3. **Chat (Llama 3.1)** — general local banter  
pub fn handle_chat(
    ha: &HaClient,
    ollama: Option<&OllamaConfig>,
    grok: Option<&GrokConfig>,
    req: &ChatRequest,
) -> ChatResponse {
    let text = req.text.trim();
    if text.is_empty() {
        return ChatResponse {
            reply: "I didn't catch that.".into(),
            state: "idle".into(),
            actions: vec![],
            error: None,
        };
    }

    if let Some(parsed) = parse_device_command(text) {
        info!(?parsed, "intent: device command");
        return run_device_command(ha, parsed);
    }

    if is_greeting(text) {
        return ChatResponse {
            reply: "Hello! I'm Floaty McFloater. Catch the wave — and welcome to the future!"
                .into(),
            state: "speaking".into(),
            actions: vec![],
            error: None,
        };
    }

    if looks_like_home_control(text) {
        if let Ok(inv) = ha.control_inventory() {
            if !inv.control_ok() {
                return ChatResponse {
                    reply: "I can't control any lights or plugs yet. Home Assistant has no \
switch or light entities — add a plug first, then I can help."
                        .into(),
                    state: "speaking".into(),
                    actions: vec![],
                    error: None,
                };
            }
        }
    }

    let world = ha_world_facts(ha);

    // --- Lane A: Mistral opinion / direction following (local) ---
    if looks_like_instruct_task(text) {
        if let Some(cfg) = ollama {
            info!("llm lane=instruct (mistral — direction following)");
            match cfg.instruct(text, &world) {
                Ok(raw) => {
                    let reply = speakable_from_plan_json(&raw)
                        .unwrap_or_else(|| truncate(raw.trim(), 220));
                    return ChatResponse {
                        reply,
                        state: "speaking".into(),
                        actions: vec![],
                        error: None,
                    };
                }
                Err(err) => {
                    warn!(%err, "instruct lane failed — trying chat fallback");
                    if let Ok(reply) = cfg.chat(text, &world) {
                        return ChatResponse {
                            reply,
                            state: "speaking".into(),
                            actions: vec![],
                            error: Some(err.to_string()),
                        };
                    }
                }
            }
        }
    }

    // --- Lane B: Grok (cloud) for real-world / physics ---
    if looks_like_world_question(text) {
        if let Some(cfg) = grok {
            info!("llm lane=grok (world/physics)");
            match cfg.chat(text, &world) {
                Ok(reply) => {
                    return ChatResponse {
                        reply,
                        state: "speaking".into(),
                        actions: vec![],
                        error: None,
                    };
                }
                Err(err) => {
                    warn!(%err, "grok failed — falling back to local chat");
                    if let Some(o) = ollama {
                        if let Ok(reply) = o.chat(text, &world) {
                            return ChatResponse {
                                reply: format!("{reply} (answered locally; Grok was unavailable)"),
                                state: "speaking".into(),
                                actions: vec![],
                                error: Some(err.to_string()),
                            };
                        }
                    }
                    return ChatResponse {
                        reply: "I can't reach Grok for a real-world answer right now, and local models are offline too.".into(),
                        state: "idle".into(),
                        actions: vec![],
                        error: Some(err.to_string()),
                    };
                }
            }
        } else {
            info!("llm lane=grok skipped (no API key) — local chat");
        }
    }

    // --- Lane C: Llama 3.1 general local chat ---
    if let Some(cfg) = ollama {
        info!("llm lane=chat (llama local)");
        match cfg.chat(text, &world) {
            Ok(reply) => {
                return ChatResponse {
                    reply,
                    state: "speaking".into(),
                    actions: vec![],
                    error: None,
                };
            }
            Err(err) => {
                warn!(%err, "chat lane failed");
                return ChatResponse {
                    reply: format!(
                        "I heard you say {}. Local chat is offline right now.",
                        truncate(text, 50)
                    ),
                    state: "speaking".into(),
                    actions: vec![],
                    error: Some(err.to_string()),
                };
            }
        }
    }

    ChatResponse {
        reply: format!(
            "I heard you. {}. No language model is configured on the brain.",
            truncate(text, 60)
        ),
        state: "speaking".into(),
        actions: vec![],
        error: None,
    }
}

/// Live HA inventory for the LLM — the only hardware it is allowed to know about.
fn ha_world_facts(ha: &HaClient) -> String {
    let mut lines = Vec::new();
    match ha.control_inventory() {
        Ok(inv) => {
            lines.push(format!("Summary: {}.", inv.summary()));
            if !inv.control_ok() {
                lines.push(
                    "NO controllable devices. No switches, lights, or scenes. \
Do not invent any. Do not suggest turning anything on."
                        .into(),
                );
            }
        }
        Err(e) => lines.push(format!("Could not read HA inventory: {e}")),
    }

    for domain in ["switch", "light", "scene"] {
        match ha.states(Some(domain)) {
            Ok(list) if list.is_empty() => {
                lines.push(format!("{domain}: (none)"));
            }
            Ok(list) => {
                let ids: Vec<_> = list
                    .into_iter()
                    .map(|e| format!("{}={}", e.entity_id, e.state))
                    .take(40)
                    .collect();
                lines.push(format!("{domain}: {}", ids.join(", ")));
            }
            Err(e) => lines.push(format!("{domain}: error ({e})")),
        }
    }

    lines.push(
        "Location context: small lab / workshop setup on Thumper — not a mansion. \
No kitchen, bedrooms, or fridge unless listed above."
            .into(),
    );
    lines.join("\n")
}

/// Pull `summary` from planner JSON if present.
fn speakable_from_plan_json(raw: &str) -> Option<String> {
    let s = raw.trim();
    let start = s.find('{')?;
    let end = s.rfind('}')?;
    if end <= start {
        return None;
    }
    let slice = &s[start..=end];
    let v: serde_json::Value = serde_json::from_str(slice).ok()?;
    let summary = v.get("summary")?.as_str()?.trim();
    if summary.is_empty() {
        return None;
    }
    Some(summary.to_string())
}

fn looks_like_home_control(text: &str) -> bool {
    let t = text.to_ascii_lowercase();
    const KEYS: &[&str] = &[
        "turn on",
        "turn off",
        "switch on",
        "switch off",
        "toggle",
        "lights",
        "light",
        "lamp",
        "plug",
        "fridge",
        "thermostat",
        "ac ",
        "heater",
        "open the",
        "close the",
        "dim ",
        "brighten",
        "unlock",
        "lock the",
    ];
    KEYS.iter().any(|k| t.contains(k))
}

fn is_greeting(text: &str) -> bool {
    let t = text
        .trim()
        .trim_end_matches(|c: char| c.is_ascii_punctuation())
        .to_ascii_lowercase();
    matches!(
        t.as_str(),
        "hello"
            | "hi"
            | "hey"
            | "howdy"
            | "greetings"
            | "good morning"
            | "good afternoon"
            | "good evening"
    ) || t.starts_with("hello ")
        || t.starts_with("hi ")
        || t.starts_with("hey ")
}

#[derive(Debug)]
struct DeviceCommand {
    service: &'static str,
    entity_id: String,
}

fn parse_device_command(text: &str) -> Option<DeviceCommand> {
    let lower = text.to_lowercase();
    let lower = lower.trim().trim_end_matches(|c: char| c == '.' || c == '!').trim();

    let service = if lower.starts_with("toggle ") {
        "toggle"
    } else if lower.starts_with("turn on ") || lower.starts_with("switch on ") {
        "turn_on"
    } else if lower.starts_with("turn off ") || lower.starts_with("switch off ") {
        "turn_off"
    } else {
        return None;
    };

    let rest = match service {
        "toggle" => lower.strip_prefix("toggle ")?,
        "turn_on" => lower
            .strip_prefix("turn on ")
            .or_else(|| lower.strip_prefix("switch on "))?,
        "turn_off" => lower
            .strip_prefix("turn off ")
            .or_else(|| lower.strip_prefix("switch off "))?,
        _ => return None,
    };
    let rest = rest.trim();
    if rest.is_empty() {
        return None;
    }

    // Only short, concrete targets — not "kitchen lights and open the fridge…".
    // Reject multi-clause or snack/room fantasy before we invent switch.the_kitchen…
    if rest.contains(" and ")
        || rest.contains(',')
        || rest.contains('?')
        || rest.split_whitespace().count() > 4
    {
        return None;
    }
    const REJECT: &[&str] = &[
        "fridge", "oven", "kitchen", "bedroom", "bathroom", "living", "mansion",
        "house", "room", "snack", "thermostat", "camera", "tv", "television",
    ];
    if REJECT.iter().any(|w| rest.contains(w)) {
        return None;
    }

    let entity_id = if rest.contains('.') {
        // Explicit entity_id only if it looks like domain.name
        let cleaned = rest.replace(' ', "");
        if cleaned.matches('.').count() != 1 {
            return None;
        }
        cleaned
    } else {
        let slug = rest
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .collect::<String>()
            .trim_matches('_')
            .to_string();
        if slug.is_empty() || slug.len() > 40 {
            return None;
        }
        format!("switch.{slug}")
    };

    Some(DeviceCommand { service, entity_id })
}

fn run_device_command(ha: &HaClient, cmd: DeviceCommand) -> ChatResponse {
    if let Err(err) = ha.state(&cmd.entity_id) {
        let hint = match ha.control_inventory() {
            Ok(inv) if !inv.control_ok() => {
                " Home Assistant has no switches or lights yet — add Tuya or KMC plugs first."
                    .to_string()
            }
            Ok(inv) => format!(" Available right now: {}.", inv.summary()),
            Err(_) => String::new(),
        };
        return ChatResponse {
            reply: format!(
                "I can't find {} in Home Assistant.{}",
                humanize(&cmd.entity_id),
                hint
            ),
            state: "idle".into(),
            actions: vec![],
            error: Some(err.to_string()),
        };
    }

    let result = match cmd.service {
        "turn_on" => ha.turn_on(&cmd.entity_id),
        "turn_off" => ha.turn_off(&cmd.entity_id),
        "toggle" => ha.toggle(&cmd.entity_id),
        other => {
            return ChatResponse {
                reply: format!("Unknown service {other}."),
                state: "idle".into(),
                actions: vec![],
                error: Some(format!("unknown service {other}")),
            };
        }
    };

    match result {
        Ok(_) => {
            let result_state = ha.state(&cmd.entity_id).ok().map(|s| s.state);
            let spoken = match cmd.service {
                "turn_on" => format!("Turning on {}.", humanize(&cmd.entity_id)),
                "turn_off" => format!("Turning off {}.", humanize(&cmd.entity_id)),
                "toggle" => format!(
                    "Toggling {}. Now {}.",
                    humanize(&cmd.entity_id),
                    result_state.as_deref().unwrap_or("unknown")
                ),
                _ => format!("Done with {}.", humanize(&cmd.entity_id)),
            };
            ChatResponse {
                reply: spoken,
                state: "speaking".into(),
                actions: vec![ChatAction {
                    kind: "ha".into(),
                    entity_id: cmd.entity_id,
                    service: cmd.service.into(),
                    result_state,
                }],
                error: None,
            }
        }
        Err(err) => ChatResponse {
            reply: format!(
                "Could not control {}. Home Assistant said no.",
                humanize(&cmd.entity_id)
            ),
            state: "idle".into(),
            actions: vec![],
            error: Some(err.to_string()),
        },
    }
}

fn humanize(entity_id: &str) -> String {
    entity_id
        .split_once('.')
        .map(|(_, name)| name.replace('_', " "))
        .unwrap_or_else(|| entity_id.replace('_', " "))
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let t: String = s.chars().take(max).collect();
        format!("{t}…")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_toggle_entity() {
        let c = parse_device_command("toggle switch.desk_lamp").unwrap();
        assert_eq!(c.service, "toggle");
        assert_eq!(c.entity_id, "switch.desk_lamp");
    }

    #[test]
    fn parses_turn_on_words() {
        let c = parse_device_command("turn on desk lamp").unwrap();
        assert_eq!(c.service, "turn_on");
        assert_eq!(c.entity_id, "switch.desk_lamp");
    }

    #[test]
    fn rejects_mansion_fantasy() {
        assert!(parse_device_command(
            "turn on the kitchen lights and open the fridge for a snack"
        )
        .is_none());
        assert!(parse_device_command("turn on the fridge").is_none());
    }
}
