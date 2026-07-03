//! Core orchestration for Floaty McFloater.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum McFloaterError {
    #[error("not implemented: {0}")]
    NotImplemented(&'static str),
}

pub type Result<T> = std::result::Result<T, McFloaterError>;