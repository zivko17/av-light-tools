//! `avlt ndi ...`: NDI tools.

use avlt_core::Result;
use clap::Subcommand;

use super::output::Out;
use super::{Pending, not_implemented};

/// `avlt ndi` subcommands.
#[derive(Debug, Subcommand)]
pub enum NdiCmd {
    /// Launch an NDI tool
    Launch(Pending),
    /// Check the NDI installation and network
    Check(Pending),
    /// NDI-related services
    Services(Pending),
    /// NDI configuration
    Config(Pending),
}

/// Runs an `avlt ndi` subcommand.
pub fn run(cmd: NdiCmd, out: &Out) -> Result<()> {
    let _ = out;
    match cmd {
        NdiCmd::Launch(_) => not_implemented(),
        NdiCmd::Check(_) => not_implemented(),
        NdiCmd::Services(_) => not_implemented(),
        NdiCmd::Config(_) => not_implemented(),
    }
}
