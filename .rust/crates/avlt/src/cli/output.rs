//! Presentation for CLI subcommands: JSON or human-readable text.
//!
//! Rules every subcommand relies on:
//! - With `--json`, stdout carries only the serialized result: `title`/`field`/`ok`
//!   print nothing. `warn`/`err` always go to stderr, in both modes.
//! - Colour is decided per stream (stdout and stderr each must be a terminal) and is
//!   off when `NO_COLOR` is set.
//! - A closed pipe (`avlt ... | head`) ends the process quietly with exit code 0.

use std::io::{self, IsTerminal, Write};

use serde::Serialize;

const RESET: &str = "\x1b[0m";
const BOLD_CYAN: &str = "\x1b[1;36m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const RED: &str = "\x1b[31m";
const DIM: &str = "\x1b[2m";

/// Output settings shared by every subcommand.
#[derive(Debug, Clone, Copy)]
pub struct Out {
    /// Print the serialized result instead of text.
    pub json: bool,
    /// Use ANSI colours on stdout (only when stdout is a terminal).
    pub color: bool,
    /// Use ANSI colours on stderr (only when stderr is a terminal).
    pub err_color: bool,
}

/// Where a line goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stream {
    Stdout,
    Stderr,
}

/// Kind of line, which decides stream, colour and whether `--json` suppresses it.
#[derive(Debug, Clone, Copy)]
enum Kind<'a> {
    Title,
    /// `label value` line; the label is padded and dimmed.
    Field(&'a str),
    Ok,
    Warn,
    Err,
}

impl Out {
    /// Detects colour support for stdout and stderr separately; `NO_COLOR` disables both.
    pub fn new(json: bool) -> Out {
        let no_color = std::env::var_os("NO_COLOR").is_some();
        Out {
            json,
            color: !no_color && io::stdout().is_terminal(),
            err_color: !no_color && io::stderr().is_terminal(),
        }
    }

    /// Prints `value` as pretty JSON with `--json`, otherwise runs the text renderer.
    pub fn emit<T: Serialize>(&self, value: &T, text: impl FnOnce(&T, &Out)) {
        if self.json {
            match serde_json::to_string_pretty(value) {
                Ok(s) => write_line(Stream::Stdout, &s),
                Err(e) => self.err(&format!("cannot serialize result: {e}")),
            }
        } else {
            text(value, self);
        }
    }

    /// Section heading (stdout; nothing with `--json`).
    pub fn title(&self, s: &str) {
        self.print(Kind::Title, s);
    }

    /// `label value` line with the label padded for alignment (stdout; nothing with `--json`).
    pub fn field(&self, label: &str, value: &str) {
        self.print(Kind::Field(label), value);
    }

    /// Success line (stdout; nothing with `--json`).
    pub fn ok(&self, s: &str) {
        self.print(Kind::Ok, s);
    }

    /// Warning line, always on stderr.
    pub fn warn(&self, s: &str) {
        self.print(Kind::Warn, s);
    }

    /// Error line, always on stderr.
    pub fn err(&self, s: &str) {
        self.print(Kind::Err, s);
    }

    fn print(&self, kind: Kind<'_>, s: &str) {
        if let Some((stream, line)) = self.render(kind, s) {
            write_line(stream, &line);
        }
    }

    /// The stream and final text for a line, or `None` if it must not be printed.
    fn render(&self, kind: Kind<'_>, s: &str) -> Option<(Stream, String)> {
        let stream = match kind {
            Kind::Title | Kind::Field(_) | Kind::Ok if self.json => return None,
            Kind::Title | Kind::Field(_) | Kind::Ok => Stream::Stdout,
            Kind::Warn | Kind::Err => Stream::Stderr,
        };
        let color = match stream {
            Stream::Stdout => self.color,
            Stream::Stderr => self.err_color,
        };
        let paint = |code: &str, text: &str| {
            if color {
                format!("{code}{text}{RESET}")
            } else {
                text.to_string()
            }
        };
        let line = match kind {
            Kind::Title => paint(BOLD_CYAN, s),
            Kind::Field(label) => format!("  {} {s}", paint(DIM, &format!("{label:<18}"))),
            Kind::Ok => paint(GREEN, s),
            Kind::Warn => paint(YELLOW, &format!("warning: {s}")),
            Kind::Err => paint(RED, &format!("error: {s}")),
        };
        Some((stream, line))
    }
}

/// Writes one line to the locked stream. A broken pipe ends the process with exit 0;
/// other write errors are ignored (there is nowhere left to report them).
fn write_line(stream: Stream, line: &str) {
    let result = match stream {
        Stream::Stdout => writeln!(io::stdout().lock(), "{line}"),
        Stream::Stderr => writeln!(io::stderr().lock(), "{line}"),
    };
    if let Err(e) = result {
        exit_if_broken_pipe(&e);
    }
}

/// Ends the process cleanly (exit 0, no message) if `e` is a closed pipe.
pub fn exit_if_broken_pipe(e: &io::Error) {
    if e.kind() == io::ErrorKind::BrokenPipe {
        std::process::exit(0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLAIN: Out = Out {
        json: false,
        color: false,
        err_color: false,
    };
    const JSON: Out = Out {
        json: true,
        color: false,
        err_color: false,
    };

    fn line(out: &Out, kind: Kind<'_>, s: &str) -> Option<(Stream, String)> {
        out.render(kind, s)
    }

    #[test]
    fn text_mode_routes_lines() {
        assert_eq!(
            line(&PLAIN, Kind::Title, "T"),
            Some((Stream::Stdout, "T".into()))
        );
        assert_eq!(
            line(&PLAIN, Kind::Ok, "done"),
            Some((Stream::Stdout, "done".into()))
        );
        assert_eq!(
            line(&PLAIN, Kind::Warn, "w"),
            Some((Stream::Stderr, "warning: w".into()))
        );
        assert_eq!(
            line(&PLAIN, Kind::Err, "e"),
            Some((Stream::Stderr, "error: e".into()))
        );
        assert_eq!(
            line(&PLAIN, Kind::Field("Mask"), "255.255.255.0"),
            Some((Stream::Stdout, format!("  {:<18} 255.255.255.0", "Mask")))
        );
    }

    #[test]
    fn json_mode_keeps_stdout_for_json_only() {
        assert_eq!(line(&JSON, Kind::Title, "T"), None);
        assert_eq!(line(&JSON, Kind::Field("a"), "b"), None);
        assert_eq!(line(&JSON, Kind::Ok, "done"), None);
        assert_eq!(
            line(&JSON, Kind::Warn, "w"),
            Some((Stream::Stderr, "warning: w".into()))
        );
        assert_eq!(
            line(&JSON, Kind::Err, "e"),
            Some((Stream::Stderr, "error: e".into()))
        );
    }

    #[test]
    fn stdout_colour_does_not_colour_stderr() {
        let out = Out {
            json: false,
            color: true,
            err_color: false,
        };
        assert_eq!(
            line(&out, Kind::Ok, "ok"),
            Some((Stream::Stdout, "\x1b[32mok\x1b[0m".into()))
        );
        assert_eq!(
            line(&out, Kind::Err, "e"),
            Some((Stream::Stderr, "error: e".into()))
        );
    }

    #[test]
    fn stderr_colour_does_not_colour_stdout() {
        let out = Out {
            json: false,
            color: false,
            err_color: true,
        };
        assert_eq!(
            line(&out, Kind::Ok, "ok"),
            Some((Stream::Stdout, "ok".into()))
        );
        assert_eq!(
            line(&out, Kind::Err, "e"),
            Some((Stream::Stderr, "\x1b[31merror: e\x1b[0m".into()))
        );
        let json = Out { json: true, ..out };
        assert_eq!(
            line(&json, Kind::Warn, "w"),
            Some((Stream::Stderr, "\x1b[33mwarning: w\x1b[0m".into()))
        );
    }

    #[test]
    fn emit_text_mode_calls_renderer() {
        let mut seen = None;
        PLAIN.emit(&42u32, |v, _| seen = Some(*v));
        assert_eq!(seen, Some(42));
    }

    #[test]
    fn emit_json_mode_skips_renderer() {
        let mut called = false;
        JSON.emit(&42u32, |_, _| called = true);
        assert!(!called);
    }
}
