//! Lip sync for Floaty McFloater (Phase 4).

use thiserror::Error;

#[derive(Debug, Error)]
pub enum LipSyncError {
    #[error("not implemented: {0}")]
    NotImplemented(&'static str),
}

pub type Result<T> = std::result::Result<T, LipSyncError>;