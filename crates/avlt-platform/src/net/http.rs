//! Small HTTPS GETs (public IP, online MAC vendor lookup) through the system TLS stack.

use std::time::Duration;

use crate::Result;

/// User-Agent of avlt's own requests.
pub const USER_AGENT: &str = concat!("avlt/", env!("CARGO_PKG_VERSION"));

/// Body of `GET url`, trimmed. Non-2xx answers and timeouts are errors.
pub fn get_text(url: &str, timeout: Duration) -> Result<String> {
    get_text_with_agent(url, timeout, USER_AGENT).map(|b| b.trim().to_string())
}

/// Body of `GET url` exactly as received (not trimmed: leading spaces of text art
/// matter), with a custom User-Agent (wttr.in only sends plain text to console clients
/// it recognises by User-Agent). Non-2xx answers and timeouts are errors.
pub fn get_text_with_agent(url: &str, timeout: Duration, user_agent: &str) -> Result<String> {
    imp::get_text(url, timeout, user_agent)
}

/// Public IPv4 address from api.ipify.org (MyIP.ps1), `None` without internet.
pub fn public_ip(timeout: Duration) -> Option<String> {
    get_text("https://api.ipify.org", timeout)
        .ok()
        .filter(|ip| ip.parse::<std::net::IpAddr>().is_ok())
}

/// Vendor from api.macvendors.com for the OUI of `mac` (NetTools.ps1 `whoison`),
/// `None` if unknown or offline.
pub fn vendor_online(mac: &str, timeout: Duration) -> Option<String> {
    let oui = avlt_core::net::oui::oui_prefix(mac)?;
    get_text(&format!("https://api.macvendors.com/{oui:06X}"), timeout)
        .ok()
        .filter(|v| !v.is_empty() && !v.starts_with('{'))
}

#[cfg(windows)]
mod imp {
    use std::time::Duration;

    use ureq::tls::{RootCerts, TlsConfig, TlsProvider};

    use crate::{Error, Result};

    pub fn get_text(url: &str, timeout: Duration, user_agent: &str) -> Result<String> {
        // rustls is not compiled in: native-tls (SChannel) must be selected explicitly,
        // with the Windows certificate store as roots.
        let tls = TlsConfig::builder()
            .provider(TlsProvider::NativeTls)
            .root_certs(RootCerts::PlatformVerifier)
            .build();
        let agent = ureq::Agent::new_with_config(
            ureq::Agent::config_builder()
                .tls_config(tls)
                .timeout_global(Some(timeout))
                .user_agent(user_agent)
                .build(),
        );
        let fail = |e: ureq::Error| Error::Other(format!("GET {url}: {e}"));
        let mut resp = agent.get(url).call().map_err(fail)?;
        let body = resp.body_mut().read_to_string().map_err(fail)?;
        Ok(body)
    }
}

#[cfg(not(windows))]
mod imp {
    use std::time::Duration;

    use crate::{Error, Result};

    pub fn get_text(_url: &str, _timeout: Duration, _user_agent: &str) -> Result<String> {
        Err(Error::Unsupported)
    }
}
