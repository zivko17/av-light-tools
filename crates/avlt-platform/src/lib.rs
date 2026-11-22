//! Everything that touches Windows. Every function has the same signature on all
//! platforms; outside Windows it returns `Error::Unsupported`. Never prints.

pub mod admin;
pub mod edid_source;
pub mod ndi;
pub mod net;
pub mod process;
pub mod registry;
pub mod sys;
pub mod wmi;
