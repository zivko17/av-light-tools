//! Command-line interface: the top-level command tree and dispatch to each group.
//!
//! Each group (`calc`, `edid`, ...) owns its clap enum and dispatch in its own file.

pub mod calc;
pub mod confirm;
pub mod edid;
pub(crate) mod env;
pub(crate) mod live;
pub mod media;
pub mod ndi;
pub mod net;
// The presentation helpers are used by the subcommands that Tasks 1-5 port.
#[allow(dead_code)]
pub mod output;
pub mod sys;
pub mod util;

use std::io::{self, Write};

use avlt_core::Result;
use clap::{Parser, Subcommand};
use serde::Serialize;

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
        Command::Gui => crate::gui::run(),
    }
}

/// A plain stdout line for tables and live monitors (nothing with `--json`).
/// A closed pipe ends the process quietly.
pub(crate) fn line(out: &Out, s: &str) {
    if !out.json {
        raw_line(s);
    }
}

/// One compact JSON document on its own stdout line, for live monitors with `--json`
/// (one line per sample).
pub(crate) fn json_line<T: Serialize>(value: &T) {
    match serde_json::to_string(value) {
        Ok(s) => raw_line(&s),
        Err(e) => out_err(&format!("cannot serialize result: {e}")),
    }
}

fn raw_line(s: &str) {
    if let Err(e) = writeln!(io::stdout().lock(), "{s}") {
        output::exit_if_broken_pipe(&e);
    }
}

fn out_err(msg: &str) {
    let _ = writeln!(io::stderr().lock(), "error: {msg}");
}
