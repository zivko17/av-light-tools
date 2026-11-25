//! `util weather` download (Weather.ps1 `Invoke-RestMethod`, 5 s timeout).
//!
//! wttr.in answers plain text only to console clients, recognised by User-Agent
//! (PowerShell, curl...); avlt's default `avlt/x.y` would receive the HTML page.

use std::time::Duration;

use crate::{Error, Result};

/// Weather.ps1 `-TimeoutSec 5`.
pub const WEATHER_TIMEOUT: Duration = Duration::from_secs(5);

/// User-Agent wttr.in treats as a console client.
const USER_AGENT: &str = concat!("avlt/", env!("CARGO_PKG_VERSION"), " (compatible; curl)");

/// Body of the wttr.in report at `url` (see `avlt_core::sys::weather::weather_url`).
pub fn fetch_weather(url: &str) -> Result<String> {
    fetch(url, WEATHER_TIMEOUT)
}

/// The report's leading spaces are part of the layout: only trailing whitespace goes.
fn clean_body(body: &str) -> String {
    body.trim_end().to_string()
}

fn fetch(url: &str, timeout: Duration) -> Result<String> {
    match crate::net::http::get_text_with_agent(url, timeout, USER_AGENT).map(|b| clean_body(&b)) {
        Err(Error::Other(e)) => Err(Error::Other(format!(
            "could not fetch weather ({e}). Check internet connection."
        ))),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn body_keeps_leading_whitespace_and_drops_trailing() {
        assert_eq!(
            clean_body(
                "  
   Paris: +9C

 "
            ),
            "  
   Paris: +9C"
        );
        assert_eq!(clean_body(""), "");
    }

    #[test]
    fn user_agent_is_a_console_client() {
        assert!(USER_AGENT.starts_with("avlt/"));
        assert!(USER_AGENT.contains("curl"));
    }

    #[cfg(windows)]
    #[test]
    fn unreachable_host_is_a_readable_error() {
        let err = fetch("https://avlt.invalid/", Duration::from_secs(2)).unwrap_err();
        assert!(
            err.to_string().contains("Check internet connection"),
            "{err}"
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn unsupported_elsewhere() {
        assert!(matches!(
            fetch_weather("https://wttr.in/"),
            Err(Error::Unsupported)
        ));
    }
}
