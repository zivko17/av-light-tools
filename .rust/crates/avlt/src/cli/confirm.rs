//! Confirmation for commands that change the system (`net setip`, `net reset-nic`,
//! `sys killp`, `sys cleanup`, `sys monitors-arrange`...).

use std::io::{self, BufRead, IsTerminal, Write};

use avlt_core::{Error, Result};

/// Asks `question` on stderr unless `yes` (the command's `--yes`).
///
/// Without a terminal on stdin it refuses with [`Error::InvalidInput`] (exit 2) instead
/// of acting, so a script never changes the system without `--yes`. Any answer other
/// than `y` / `yes` is [`Error::Other("cancelled")`](Error::Other) (exit 1).
pub fn confirm(yes: bool, question: &str) -> Result<()> {
    let stdin = io::stdin();
    let interactive = stdin.is_terminal();
    confirm_with(yes, interactive, stdin.lock(), question)
}

/// [`confirm`] with the terminal check and the input made explicit (for tests).
pub fn confirm_with(
    yes: bool,
    interactive: bool,
    reader: impl BufRead,
    question: &str,
) -> Result<()> {
    if yes {
        return Ok(());
    }
    if !interactive {
        return Err(Error::InvalidInput(format!(
            "{question} needs confirmation: run it in a terminal or pass --yes"
        )));
    }
    let mut err = io::stderr().lock();
    let _ = write!(err, "{question} [y/N] ");
    let _ = err.flush();
    drop(err);
    if answered_yes(reader) {
        Ok(())
    } else {
        Err(Error::Other("cancelled".into()))
    }
}

fn answered_yes(mut reader: impl BufRead) -> bool {
    let mut answer = String::new();
    reader.read_line(&mut answer).is_ok()
        && matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confirmation_answers() {
        for (input, expected) in [
            ("y\n", true),
            ("YES\r\n", true),
            (" y ", true),
            ("n\n", false),
            ("\n", false),
            ("", false),
            ("yep\n", false),
        ] {
            assert_eq!(answered_yes(input.as_bytes()), expected, "{input:?}");
        }
    }

    #[test]
    fn confirmation_paths() {
        assert!(confirm_with(true, false, "".as_bytes(), "Q?").is_ok());
        let err = confirm_with(false, false, "y\n".as_bytes(), "Q?").unwrap_err();
        assert!(matches!(err, Error::InvalidInput(_)), "{err:?}");
        assert_eq!(err.exit_code(), 2);
        assert!(err.to_string().contains("--yes"), "{err}");
        assert!(confirm_with(false, true, "y\n".as_bytes(), "Q?").is_ok());
        let err = confirm_with(false, true, "n\n".as_bytes(), "Q?").unwrap_err();
        assert_eq!(err.to_string(), "cancelled");
    }
}
