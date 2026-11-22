//! `avlt edid ...`: EDID tools (read-only).

use avlt_core::Result;
use clap::Subcommand;

use super::output::Out;
use super::{Pending, not_implemented};

/// `avlt edid` subcommands.
#[derive(Debug, Subcommand)]
pub enum EdidCmd {
    /// List connected monitors and their EDID
    List(Pending),
    /// Decode an EDID
    Info(Pending),
    /// Validate an EDID (header, checksums, extensions)
    Check(Pending),
    /// Compare two EDIDs
    Diff(Pending),
    /// Save the EDID of connected monitors to files
    Dump(Pending),
}

/// Runs an `avlt edid` subcommand.
pub fn run(cmd: EdidCmd, out: &Out) -> Result<()> {
    let _ = out;
    match cmd {
        EdidCmd::List(_) => not_implemented(),
        EdidCmd::Info(_) => not_implemented(),
        EdidCmd::Check(_) => not_implemented(),
        EdidCmd::Diff(_) => not_implemented(),
        EdidCmd::Dump(_) => not_implemented(),
    }
}
