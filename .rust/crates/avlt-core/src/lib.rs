//! Pure, platform-independent logic for avlt. Never prints.
#![forbid(unsafe_code)]

pub mod calc;
pub mod config;
pub mod edid;
pub mod error;
pub mod media;
pub mod net;
pub mod sys;

pub use error::{Error, Result};
