//! `avlt sys ...`: System tools.

use avlt_core::Result;
use clap::Subcommand;

use super::output::Out;
use super::{Pending, not_implemented};

/// `avlt sys` subcommands.
#[derive(Debug, Subcommand)]
pub enum SysCmd {
    /// System summary (OS, CPU, RAM, GPU)
    Info(Pending),
    /// Live telemetry (CPU, RAM, disk, network)
    Tele(Pending),
    /// Top processes by CPU usage
    Top(Pending),
    /// Kill processes by name
    Killp(Pending),
    /// Connected USB devices
    Usb(Pending),
    /// Disks and free space
    Disks(Pending),
    /// Connected monitors
    Monitors(Pending),
    /// Arrange the monitor layout
    MonitorsArrange(Pending),
    /// Battery status
    Battery(Pending),
    /// Startup programs
    Startup(Pending),
    /// Audio devices
    Audio(Pending),
    /// Audio-related services
    ServicesAudio(Pending),
    /// Clean temporary files
    Cleanup(Pending),
}

/// Runs an `avlt sys` subcommand.
pub fn run(cmd: SysCmd, out: &Out) -> Result<()> {
    let _ = out;
    match cmd {
        SysCmd::Info(_) => not_implemented(),
        SysCmd::Tele(_) => not_implemented(),
        SysCmd::Top(_) => not_implemented(),
        SysCmd::Killp(_) => not_implemented(),
        SysCmd::Usb(_) => not_implemented(),
        SysCmd::Disks(_) => not_implemented(),
        SysCmd::Monitors(_) => not_implemented(),
        SysCmd::MonitorsArrange(_) => not_implemented(),
        SysCmd::Battery(_) => not_implemented(),
        SysCmd::Startup(_) => not_implemented(),
        SysCmd::Audio(_) => not_implemented(),
        SysCmd::ServicesAudio(_) => not_implemented(),
        SysCmd::Cleanup(_) => not_implemented(),
    }
}
