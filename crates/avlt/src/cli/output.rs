//! Presentation for CLI subcommands: JSON or human-readable text.

use std::io::IsTerminal;

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
    /// Use ANSI colours (only when stdout is a terminal).
    pub color: bool,
}

impl Out {
    /// Colour is enabled only when stdout is a terminal and `NO_COLOR` is not set.
    pub fn new(json: bool) -> Out {
        let color = std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none();
        Out { json, color }
    }

    /// Prints `value` as pretty JSON with `--json`, otherwise runs the text renderer.
    pub fn emit<T: Serialize>(&self, value: &T, text: impl FnOnce(&T, &Out)) {
        if self.json {
            match serde_json::to_string_pretty(value) {
                Ok(s) => println!("{s}"),
                Err(e) => self.err(&format!("cannot serialize result: {e}")),
            }
        } else {
            text(value, self);
        }
    }

    /// Section heading.
    pub fn title(&self, s: &str) {
        println!("{}", self.paint(BOLD_CYAN, s));
    }

    /// `label: value` line with the label padded for alignment.
    pub fn field(&self, label: &str, value: &str) {
        println!("{}", self.format_field(label, value));
    }

    /// Success line.
    pub fn ok(&self, s: &str) {
        println!("{}", self.paint(GREEN, s));
    }

    /// Warning line.
    pub fn warn(&self, s: &str) {
        println!("{}", self.paint(YELLOW, s));
    }

    /// Error line, on stderr.
    pub fn err(&self, s: &str) {
        eprintln!("{}", self.paint(RED, &format!("error: {s}")));
    }

    fn format_field(&self, label: &str, value: &str) -> String {
        let label = format!("{label:<18}");
        format!("  {} {value}", self.paint(DIM, &label))
    }

    fn paint(&self, code: &str, s: &str) -> String {
        if self.color {
            format!("{code}{s}{RESET}")
        } else {
            s.to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLAIN: Out = Out {
        json: false,
        color: false,
    };

    #[test]
    fn plain_text_has_no_escape_codes() {
        assert_eq!(PLAIN.paint(RED, "x"), "x");
        assert_eq!(
            PLAIN.format_field("Mask", "255.255.255.0"),
            format!("  {:<18} 255.255.255.0", "Mask")
        );
    }

    #[test]
    fn colour_wraps_in_ansi() {
        let out = Out {
            json: false,
            color: true,
        };
        assert_eq!(out.paint(GREEN, "ok"), "\x1b[32mok\x1b[0m");
    }

    #[test]
    fn emit_text_mode_calls_renderer() {
        let mut seen = None;
        PLAIN.emit(&42u32, |v, _| seen = Some(*v));
        assert_eq!(seen, Some(42));
    }

    #[test]
    fn emit_json_mode_skips_renderer() {
        let out = Out {
            json: true,
            color: false,
        };
        let mut called = false;
        out.emit(&42u32, |_, _| called = true);
        assert!(!called);
    }
}
