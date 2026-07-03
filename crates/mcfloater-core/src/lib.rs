//! Core state machine and dialog loop for Floaty McFloater.

use serde::{Deserialize, Serialize};

/// High-level avatar state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AvatarState {
    Idle,
    Listening,
    Thinking,
    Speaking,
}

/// Placeholder dialog event (Phase 3 will wire Ollama).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DialogEvent {
    pub user_text: String,
    pub reply_text: String,
}
