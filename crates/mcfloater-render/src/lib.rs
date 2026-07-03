//! 3D rendering for Floaty McFloater (Phase 1).

use thiserror::Error;

#[derive(Debug, Error)]
pub enum RenderError {
    #[error("not implemented: {0}")]
    NotImplemented(&'static str),
}

pub type Result<T> = std::result::Result<T, RenderError>;