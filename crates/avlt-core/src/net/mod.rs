//! Network models and pure helpers.

pub mod format;
pub mod ipv4;
pub mod model;
pub mod oui;
mod oui_table;
pub mod ports;
pub mod target;

pub use ipv4::{hosts_in, parse_cidr, prefix_to_mask};
pub use oui::oui_vendor;
pub use ports::COMMON_PORTS;
