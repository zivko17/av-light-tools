//! Network access (adapters, ICMP, neighbours, TCP, traffic, IP config).
//!
//! The async functions need a Tokio runtime with blocking threads (`spawn_blocking`).

pub mod adapters;
pub mod http;
pub mod icmp;
pub mod neighbors;
pub mod setip;
#[cfg(windows)]
mod sockaddr;
pub mod tcp;
pub mod traffic;

use std::collections::HashMap;
use std::net::{Ipv4Addr, ToSocketAddrs};
use std::time::Duration;

use avlt_core::net::model::{ScanHost, ScanResult, VendorSource, WhoIsOn};
use avlt_core::net::{hosts_in, oui_vendor};

use crate::{Error, Result};

pub use adapters::{active_adapters, adapters, default_adapter, find_adapter};
pub use http::{public_ip, vendor_online};
pub use icmp::{ping, sweep};
pub use neighbors::{hostname, neighbors};
pub use setip::{flush_dns, restart_adapter, set_dhcp, set_static};
pub use tcp::{port_scan, tcp_open};
pub use traffic::{connections, traffic};

/// Ping timeout of `net scan` and `net whoison`.
pub const PING_TIMEOUT: Duration = Duration::from_millis(1000);
/// Pings in flight during `net scan`.
pub const SCAN_CONCURRENCY: usize = 64;
/// Reverse DNS timeout per host (NetMaint ScanPage).
pub const HOSTNAME_TIMEOUT: Duration = Duration::from_millis(700);
/// TCP connect timeout of `net portcheck` / `net portscan`.
pub const TCP_TIMEOUT: Duration = Duration::from_millis(1000);

/// IPv4 address of `host`: a literal, or the first IPv4 its name resolves to.
pub fn resolve_ipv4(host: &str) -> Result<Ipv4Addr> {
    let host = host.trim();
    if let Ok(ip) = host.parse() {
        return Ok(ip);
    }
    let unresolved = || Error::InvalidInput(format!("cannot resolve '{host}' to an IPv4 address"));
    if host.is_empty() {
        return Err(unresolved());
    }
    (host, 0)
        .to_socket_addrs()
        .map_err(|_| unresolved())?
        .find_map(|a| match a.ip() {
            std::net::IpAddr::V4(v4) => Some(v4),
            std::net::IpAddr::V6(_) => None,
        })
        .ok_or_else(unresolved)
}

/// Ping sweep of `network/prefix` (64 in flight, 1000 ms timeout), then one read of the
/// neighbour table for MACs, the OUI table for vendors and reverse DNS for names.
pub async fn scan(
    network: Ipv4Addr,
    prefix: u8,
    progress: impl FnMut(usize, usize),
) -> Result<ScanResult> {
    if !cfg!(windows) {
        return Err(Error::Unsupported);
    }
    let targets: Vec<Ipv4Addr> = hosts_in(network, prefix).collect();
    let alive = sweep(&targets, PING_TIMEOUT, SCAN_CONCURRENCY, progress).await;
    let table = neighbors().unwrap_or_default();
    let own: HashMap<Ipv4Addr, Option<String>> = adapters()
        .unwrap_or_default()
        .into_iter()
        .flat_map(|a| {
            let mac = a.mac.clone();
            a.ipv4.into_iter().map(move |c| (c.address, mac.clone()))
        })
        .collect();

    let mut names = tokio::task::JoinSet::new();
    for &(ip, _) in &alive {
        names.spawn(async move { (ip, hostname(ip, HOSTNAME_TIMEOUT).await) });
    }
    let mut hostnames = HashMap::new();
    while let Some(Ok((ip, name))) = names.join_next().await {
        hostnames.insert(ip, name);
    }

    let hosts = alive
        .into_iter()
        .map(|(ip, rtt)| {
            let this_pc = own.contains_key(&ip);
            let mac = table
                .get(&ip)
                .cloned()
                .or_else(|| own.get(&ip).cloned().flatten());
            ScanHost {
                ip,
                vendor: mac.as_deref().and_then(oui_vendor).map(str::to_string),
                mac,
                hostname: hostnames.remove(&ip).flatten(),
                latency_ms: u64::try_from(rtt.as_millis()).unwrap_or(u64::MAX),
                this_pc,
            }
        })
        .collect();
    Ok(ScanResult {
        network: format!("{network}/{prefix}"),
        scanned: targets.len(),
        hosts,
    })
}

/// MAC and vendor of `ip` (NetTools.ps1 `whoison`): ping once to fill the neighbour
/// table, read it, then the embedded OUI table, then api.macvendors.com if `online`.
pub async fn whoison(ip: Ipv4Addr, online: bool) -> Result<WhoIsOn> {
    if !cfg!(windows) {
        return Err(Error::Unsupported);
    }
    let _ = ping(ip, PING_TIMEOUT).await;
    let mac = match neighbors()?.remove(&ip) {
        Some(mac) => Some(mac),
        // The neighbour table never lists this PC's own addresses.
        None => adapters()?
            .into_iter()
            .find(|a| a.ipv4.iter().any(|c| c.address == ip))
            .and_then(|a| a.mac),
    };
    let (vendor, vendor_source) = match mac.as_deref() {
        None => (None, None),
        Some(m) => match oui_vendor(m) {
            Some(v) => (Some(v.to_string()), Some(VendorSource::Oui)),
            None if online => {
                let m = m.to_string();
                let found =
                    tokio::task::spawn_blocking(move || vendor_online(&m, Duration::from_secs(4)))
                        .await
                        .ok()
                        .flatten();
                let source = found.as_ref().map(|_| VendorSource::Api);
                (found, source)
            }
            None => (None, None),
        },
    };
    Ok(WhoIsOn {
        ip,
        mac,
        vendor,
        vendor_source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_literal_and_localhost() {
        assert_eq!(
            resolve_ipv4(" 10.0.0.1 ").unwrap(),
            Ipv4Addr::new(10, 0, 0, 1)
        );
        assert_eq!(resolve_ipv4("localhost").unwrap(), Ipv4Addr::LOCALHOST);
    }

    #[test]
    fn resolve_garbage_is_invalid_input() {
        for bad in ["", "999.1.1.1", "no such host.invalid"] {
            let err = resolve_ipv4(bad).unwrap_err();
            assert!(matches!(err, Error::InvalidInput(_)), "{bad}: {err:?}");
        }
    }

    #[cfg(not(windows))]
    #[tokio::test]
    async fn scan_and_whoison_are_unsupported_off_windows() {
        let r = scan(Ipv4Addr::new(192, 168, 1, 0), 24, |_, _| {}).await;
        assert!(matches!(r, Err(Error::Unsupported)));
        let w = whoison(Ipv4Addr::new(192, 168, 1, 1), false).await;
        assert!(matches!(w, Err(Error::Unsupported)));
    }
}
