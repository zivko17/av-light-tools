//! `avlt util ...`: Utilities.

use avlt_core::Result;
use clap::Subcommand;

use super::output::Out;
use super::{Pending, not_implemented};

/// `avlt util` subcommands.
#[derive(Debug, Subcommand)]
pub enum UtilCmd {
    /// Back up a folder
    Backup(Pending),
    /// Search files
    Search(Pending),
    /// Weather forecast
    Weather(Pending),
    /// Countdown timer
    Timer(Pending),
}

/// Runs an `avlt util` subcommand.
pub fn run(cmd: UtilCmd, out: &Out) -> Result<()> {
    let _ = out;
    match cmd {
        UtilCmd::Backup(_) => not_implemented(),
        UtilCmd::Search(_) => not_implemented(),
        UtilCmd::Weather(_) => not_implemented(),
        UtilCmd::Timer(_) => not_implemented(),
    }
}
