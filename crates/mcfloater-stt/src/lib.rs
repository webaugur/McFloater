//! Speech-to-text for Floaty McFloater (Phase 2).

use thiserror::Error;

#[derive(Debug, Error)]
pub enum SttError {
    #[error("not implemented: {0}")]
    NotImplemented(&'static str),
}

pub type Result<T> = std::result::Result<T, SttError>;