//! Phoneme-driven lip sync for Floaty McFloater.
//!
//! Phase 4: SAM phoneme timeline or Rhubarb fallback.

/// A viseme weight at a point in time (milliseconds from speech start).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VisemeKeyframe {
    pub time_ms: u32,
    pub viseme: String,
    pub weight: f32,
}