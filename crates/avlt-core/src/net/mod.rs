//! Network models and pure helpers.

pub mod ipv4;
pub mod model;
pub mod oui;
pub mod ports;

pub use ipv4::{hosts_in, parse_cidr, prefix_to_mask};
