//! Lightweight intent router until Ollama (Phase 3) lands.

use crate::protocol::{ChatAction, ChatRequest, ChatResponse};
use mcfloater_ha::HaClient;
use tracing::info;

/// Handle a user chat line: simple lamp C&C phrases, else a witty stub reply.
pub fn handle_chat(ha: &HaClient, req: &ChatRequest) -> ChatResponse {
    let text = req.text.trim();
    if text.is_empty() {
        return ChatResponse {
            reply: "I didn't catch that, meatbag.".into(),
            state: "idle".into(),
            actions: vec![],
            error: None,
        };
    }

    if let Some(parsed) = parse_device_command(text) {
        info!(?parsed, "intent: device command");
        return run_device_command(ha, parsed);
    }

    // Greetings (face A default is "Hello!") — speakable reply, same energy as Space demo.
    if is_greeting(text) {
        return ChatResponse {
            reply: "Hello! I'm Floaty McFloater. Catch the wave — and welcome to the future!"
                .into(),
            state: "speaking".into(),
            actions: vec![],
            error: None,
        };
    }

    // Stub dialog until Ollama sidecar is up — keep TTS-friendly (no letter stutters, no meta).
    ChatResponse {
        reply: format!(
            "I heard you. {}. I'm online — try turn on desk lamp when you want a light.",
            truncate(text, 60)
        ),
        state: "speaking".into(),
        actions: vec![],
        error: None,
    }
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
    let lower = lower.trim().trim_end_matches('.').trim();

    // "toggle switch.desk_lamp" / "turn on light.kitchen"
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

    // Prefer explicit entity_id; else slugify words → guess switch.<slug>
    let entity_id = if rest.contains('.') {
        rest.replace(' ', "")
    } else {
        let slug = rest
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .collect::<String>()
            .trim_matches('_')
            .to_string();
        if slug.is_empty() {
            return None;
        }
        format!("switch.{slug}")
    };

    Some(DeviceCommand { service, entity_id })
}

fn run_device_command(ha: &HaClient, cmd: DeviceCommand) -> ChatResponse {
    // HA may return HTTP 200 for unknown entities; require the entity to exist.
    if let Err(err) = ha.state(&cmd.entity_id) {
        return ChatResponse {
            reply: format!(
                "I can't find {} in Home Assistant.",
                humanize(&cmd.entity_id)
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
}
