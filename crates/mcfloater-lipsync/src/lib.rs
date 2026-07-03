//! Lip sync for Floaty McFloater — viseme timeline from SAM phoneme output.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Viseme {
    Closed,
    Open,
    Wide,
    Round,
    Teeth,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisemeKeyframe {
    pub time_ms: u32,
    pub viseme: Viseme,
}
