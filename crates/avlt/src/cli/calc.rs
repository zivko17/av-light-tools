//! `avlt calc ...`: AV / IT calculators.

use avlt_core::Result;
use clap::Subcommand;

use super::output::Out;
use super::{Pending, not_implemented};

/// `avlt calc` subcommands.
#[derive(Debug, Subcommand)]
pub enum CalcCmd {
    /// IPv4 subnet calculator (CIDR)
    Subnet(Pending),
    /// Screen size from diagonal and aspect ratio
    Screen(Pending),
    /// LED wall size, resolution and power
    Ledwall(Pending),
    /// Uncompressed video bitrate
    Bitrate(Pending),
    /// AV-over-IP bandwidth
    Avoip(Pending),
    /// Projector throw distance
    Projector(Pending),
    /// LED pixel pitch and viewing distance
    Pixelpitch(Pending),
    /// Projector / display brightness
    Brightness(Pending),
    /// Viewing angle and distance
    Viewangle(Pending),
    /// Camera lens field of view
    Camera(Pending),
    /// Network audio bandwidth
    Audiobw(Pending),
    /// Speaker SPL and coverage
    Speaker(Pending),
    /// Cable length and signal limits
    Cable(Pending),
    /// DSP latency and delay
    Dsp(Pending),
}

/// Runs an `avlt calc` subcommand.
pub fn run(cmd: CalcCmd, out: &Out) -> Result<()> {
    let _ = out;
    match cmd {
        CalcCmd::Subnet(_) => not_implemented(),
        CalcCmd::Screen(_) => not_implemented(),
        CalcCmd::Ledwall(_) => not_implemented(),
        CalcCmd::Bitrate(_) => not_implemented(),
        CalcCmd::Avoip(_) => not_implemented(),
        CalcCmd::Projector(_) => not_implemented(),
        CalcCmd::Pixelpitch(_) => not_implemented(),
        CalcCmd::Brightness(_) => not_implemented(),
        CalcCmd::Viewangle(_) => not_implemented(),
        CalcCmd::Camera(_) => not_implemented(),
        CalcCmd::Audiobw(_) => not_implemented(),
        CalcCmd::Speaker(_) => not_implemented(),
        CalcCmd::Cable(_) => not_implemented(),
        CalcCmd::Dsp(_) => not_implemented(),
    }
}
