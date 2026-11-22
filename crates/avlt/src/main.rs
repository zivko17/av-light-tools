//! avlt: AV / lighting / IT field toolbox.
#![forbid(unsafe_code)]

mod cli;
mod gui;

use std::process::ExitCode;

use clap::{CommandFactory, Parser};

use cli::Cli;
use cli::output::Out;

fn main() -> ExitCode {
    let cli = Cli::parse();
    let out = Out::new(cli.json);
    let Some(command) = cli.command else {
        // No arguments from a terminal: show the help (double-click opening the GUI is Task 6).
        return match Cli::command().print_help() {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => ExitCode::SUCCESS,
            Err(_) => ExitCode::FAILURE,
        };
    };
    match cli::run(command, &out) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            out.err(&e.to_string());
            ExitCode::from(u8::try_from(e.exit_code()).unwrap_or(1))
        }
    }
}
