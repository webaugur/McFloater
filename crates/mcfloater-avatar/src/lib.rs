//! Headless avatar renderer for McFloater on Thumper.
//!
//! This crate provides a wgpu-based renderer that can be driven by the brain
//! with SAM lip-sync curves to produce video frames for WebRTC calls.
//!
//! The renderer runs completely headless (no display required).

mod headless;

pub use headless::{AvatarRenderer, LipSyncFrame, AvatarError};

/// Re-export common types
pub type RgbImage = image::ImageBuffer<image::Rgb<u8>, Vec<u8>>;