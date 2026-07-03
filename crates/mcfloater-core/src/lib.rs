//! Core orchestration for Floaty McFloater.
//!
//! Phase 0: placeholder types. Dialog loop and LLM IPC arrive in Phase 3.

/// High-level runtime state for the avatar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeState {
    Idle,
    Listening,
    Thinking,
    Speaking,
}

/// A single turn in the conversation history.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ConversationTurn {
    pub role: String,
    pub content: String,
}