//! Command-line interface: the top-level command tree and dispatch to each group.
//!
//! Each group (`calc`, `edid`, ...) owns its clap enum and dispatch in its own file.

pub mod calc;
pub mod edid;
pub mod media;
pub mod ndi;
pub mod net;
// The presentation helpers are used by the subcommands that Tasks 1-5 port.
#[allow(dead_code)]
pub mod output;
pub mod sys;
pub mod util;

use avlt_core::{Error, Result};
use clap::{Args, Parser, Subcommand};

use self::output::Out;

/// AV / lighting / IT field toolbox in one portable executable.
#[derive(Debug, Parser)]
#[command(name = "avlt", version, about)]
pub struct Cli {
    /// Print the result as JSON instead of text
    #[arg(long, global = true)]
    pub json: bool,

    #[command(subcommand)]
    pub command: Option<Command>,
}

/// Top-level command groups.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// AV / IT calculators
    #[command(subcommand)]
    Calc(calc::CalcCmd),
    /// EDID tools (read-only)
    #[command(subcommand)]
    Edid(edid::EdidCmd),
    /// Network tools
    #[command(subcommand)]
    Net(net::NetCmd),
    /// System tools
    #[command(subcommand)]
    Sys(sys::SysCmd),
    /// Media tools (ffmpeg / ffprobe)
    #[command(subcommand)]
    Media(media::MediaCmd),
    /// NDI tools
    #[command(subcommand)]
    Ndi(ndi::NdiCmd),
    /// Utilities
    #[command(subcommand)]
    Util(util::UtilCmd),
    /// Open the graphical interface
    Gui,
}

/// Placeholder arguments for subcommands whose real arguments a later task defines.
#[derive(Debug, Args)]
pub struct Pending {
    /// Arguments (not defined yet)
    #[arg(hide = true)]
    pub args: Vec<String>,
}

/// Result of a subcommand that is not ported yet.
pub fn not_implemented() -> Result<()> {
    Err(Error::Other("not implemented".into()))
}

/// Dispatches a parsed command to its group.
pub fn run(command: Command, out: &Out) -> Result<()> {
    match command {
        Command::Calc(cmd) => calc::run(cmd, out),
        Command::Edid(cmd) => edid::run(cmd, out),
        Command::Net(cmd) => net::run(cmd, out),
        Command::Sys(cmd) => sys::run(cmd, out),
        Command::Media(cmd) => media::run(cmd, out),
        Command::Ndi(cmd) => ndi::run(cmd, out),
        Command::Util(cmd) => util::run(cmd, out),
        Command::Gui => not_implemented(),
    }
}
