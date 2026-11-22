//! `util weather` download (Weather.ps1 `Invoke-RestMethod`, 5 s timeout).
//!
//! wttr.in answers plain text only to console clients, recognised by User-Agent
//! (PowerShell, curl...). `net::http::get_text` sends `avlt/x.y` and would receive the
//! HTML page, so this request names itself as curl-compatible.

use std::time::Duration;

use crate::Result;

/// Weather.ps1 `-TimeoutSec 5`.
pub const WEATHER_TIMEOUT: Duration = Duration::from_secs(5);

/// User-Agent wttr.in treats as a console client.
#[cfg_attr(not(windows), allow(dead_code))]
const USER_AGENT: &str = concat!("avlt/", env!("CARGO_PKG_VERSION"), " (compatible; curl)");

/// Body of the wttr.in report at `url` (see `avlt_core::sys::weather::weather_url`).
pub fn fetch_weather(url: &str) -> Result<String> {
    imp::fetch(url, WEATHER_TIMEOUT)
}

#[cfg(windows)]
mod imp {
    use std::time::Duration;

    use ureq::tls::{RootCerts, TlsConfig, TlsProvider};

    use super::USER_AGENT;
    use crate::{Error, Result};

    pub fn fetch(url: &str, timeout: Duration) -> Result<String> {
        let tls = TlsConfig::builder()
            .provider(TlsProvider::NativeTls)
            .root_certs(RootCerts::PlatformVerifier)
            .build();
        let agent = ureq::Agent::new_with_config(
            ureq::Agent::config_builder()
                .tls_config(tls)
                .timeout_global(Some(timeout))
                .user_agent(USER_AGENT)
                .build(),
        );
        let fail = |e: ureq::Error| {
            Error::Other(format!(
                "could not fetch weather ({e}). Check internet connection."
            ))
        };
        let mut resp = agent.get(url).call().map_err(fail)?;
        let body = resp.body_mut().read_to_string().map_err(fail)?;
        Ok(body.trim_end().to_string())
    }
}

#[cfg(not(windows))]
mod imp {
    use std::time::Duration;

    use crate::{Error, Result};

    pub fn fetch(_url: &str, _timeout: Duration) -> Result<String> {
        Err(Error::Unsupported)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_agent_is_a_console_client() {
        assert!(USER_AGENT.starts_with("avlt/"));
        assert!(USER_AGENT.contains("curl"));
    }

    #[cfg(windows)]
    #[test]
    fn unreachable_host_is_a_readable_error() {
        let err = imp::fetch("https://avlt.invalid/", Duration::from_secs(2)).unwrap_err();
        assert!(
            err.to_string().contains("Check internet connection"),
            "{err}"
        );
    }
}
