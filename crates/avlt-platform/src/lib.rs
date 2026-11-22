//! Everything that touches Windows. Every function has the same signature on all
//! platforms; outside Windows it returns `Error::Unsupported`. Never prints.

pub use avlt_core::{Error, Result};

pub mod admin;
pub mod console;
pub mod edid_source;
pub mod media;
pub mod ndi;
pub mod net;
pub mod paths;
pub mod process;
pub mod registry;
pub mod sys;
pub mod wmi;
