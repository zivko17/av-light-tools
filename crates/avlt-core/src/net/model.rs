//! Results of the network tools. Serializable for `--json`; presented by the CLI and GUI.

use std::net::{IpAddr, Ipv4Addr};

use serde::Serialize;

/// An IPv4 address with its prefix length.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Ipv4Cidr {
    pub address: Ipv4Addr,
    pub prefix: u8,
}

/// Physical medium of an adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AdapterKind {
    Ethernet,
    Wifi,
    Loopback,
    Other,
}

/// A network adapter (`Get-NetIPConfiguration` / `Get-NetAdapter` fields avlt uses).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Adapter {
    /// Interface alias, e.g. `Ethernet 2` (what netsh and `--iface` take).
    pub name: String,
    /// Driver description, e.g. `Realtek PCIe GbE Family Controller`.
    pub description: String,
    /// IPv4 interface index.
    pub index: u32,
    pub kind: AdapterKind,
    /// Operational status is Up.
    pub up: bool,
    /// `AA-BB-CC-DD-EE-FF`, if the adapter has one.
    pub mac: Option<String>,
    pub ipv4: Vec<Ipv4Cidr>,
    /// IPv4 default gateways.
    pub gateways: Vec<Ipv4Addr>,
    pub dns: Vec<IpAddr>,
    /// IPv4 address from DHCP (`true`) or static (`false`).
    pub dhcp: bool,
    /// Receive link speed in bit/s, if known.
    pub link_speed_bps: Option<u64>,
    /// IPv4 interface metric (lower wins for the default route).
    pub metric: u32,
}

impl Adapter {
    /// First IPv4 address that is not link-local (169.254/16), else the first one.
    pub fn primary_ipv4(&self) -> Option<Ipv4Cidr> {
        self.ipv4
            .iter()
            .find(|a| !a.address.is_link_local())
            .or_else(|| self.ipv4.first())
            .copied()
    }
}

/// Byte counters of one adapter at one instant.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct TrafficSample {
    pub rx_bytes: u64,
    pub tx_bytes: u64,
}

/// An established TCP connection with its owning process.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Connection {
    /// Process name without `.exe`, or `pid N` if it cannot be resolved.
    pub process: String,
    pub pid: u32,
    pub local_addr: IpAddr,
    pub local_port: u16,
    pub remote_addr: IpAddr,
    pub remote_port: u16,
}

/// Where a vendor name came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum VendorSource {
    /// The embedded OUI table.
    Oui,
    /// The api.macvendors.com fallback.
    Api,
}

/// A host that answered the ping sweep of `net scan`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ScanHost {
    pub ip: Ipv4Addr,
    pub mac: Option<String>,
    /// From the embedded OUI table.
    pub vendor: Option<String>,
    /// Reverse DNS name, if any answered in time.
    pub hostname: Option<String>,
    pub latency_ms: u64,
    /// The address belongs to this PC.
    pub this_pc: bool,
}

/// Result of `net scan`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ScanResult {
    /// Network swept, as CIDR.
    pub network: String,
    /// Number of addresses pinged.
    pub scanned: usize,
    /// Hosts that answered, in address order.
    pub hosts: Vec<ScanHost>,
}

/// One port of `net portscan`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PortStatus {
    pub port: u16,
    pub service: String,
    pub open: bool,
}

/// Result of `net portscan`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PortScan {
    pub host: String,
    pub ip: Ipv4Addr,
    pub ports: Vec<PortStatus>,
}

/// Result of `net portcheck`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PortCheck {
    pub host: String,
    pub ip: Ipv4Addr,
    pub port: u16,
    pub open: bool,
}

/// Result of `net whoison`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WhoIsOn {
    pub ip: Ipv4Addr,
    /// `None`: no ARP entry (host offline or outside the local subnet).
    pub mac: Option<String>,
    pub vendor: Option<String>,
    pub vendor_source: Option<VendorSource>,
}

/// Result of `net myip`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MyIp {
    /// Up adapters with an IPv4 address.
    pub adapters: Vec<Adapter>,
    /// `None` when it could not be retrieved (no internet).
    pub public_ip: Option<String>,
}

/// One host in a `net pingmulti` round.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PingHost {
    /// As typed by the user.
    pub host: String,
    /// `None` if the name did not resolve.
    pub ip: Option<Ipv4Addr>,
    pub online: bool,
    pub latency_ms: Option<u64>,
    /// Failed pings since the monitor started.
    pub failures: u64,
}

/// One refresh of `net pingmulti`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PingRound {
    pub elapsed_s: f64,
    pub hosts: Vec<PingHost>,
}

/// One sample of `net latency`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LatencySample {
    pub elapsed_s: f64,
    pub host: String,
    pub ip: Ipv4Addr,
    /// `None`: timeout.
    pub latency_ms: Option<u64>,
    pub stats: crate::net::format::LatencySummary,
}

/// One sample of `net speed`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SpeedSample {
    pub elapsed_s: f64,
    pub adapter: String,
    pub down_bps: f64,
    pub up_bps: f64,
    pub total_down_bytes: u64,
    pub total_up_bytes: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn adapter(addrs: &[&str]) -> Adapter {
        Adapter {
            name: "Ethernet".into(),
            description: String::new(),
            index: 1,
            kind: AdapterKind::Ethernet,
            up: true,
            mac: None,
            ipv4: addrs
                .iter()
                .map(|a| Ipv4Cidr {
                    address: a.parse().unwrap(),
                    prefix: 24,
                })
                .collect(),
            gateways: vec![],
            dns: vec![],
            dhcp: false,
            link_speed_bps: None,
            metric: 0,
        }
    }

    #[test]
    fn primary_ipv4_skips_link_local() {
        let a = adapter(&["169.254.3.4", "192.168.1.10"]);
        assert_eq!(
            a.primary_ipv4().map(|c| c.address.to_string()).as_deref(),
            Some("192.168.1.10")
        );
        let b = adapter(&["169.254.3.4"]);
        assert_eq!(
            b.primary_ipv4().map(|c| c.address.to_string()).as_deref(),
            Some("169.254.3.4")
        );
        assert_eq!(adapter(&[]).primary_ipv4(), None);
    }

    #[test]
    fn json_shape() {
        let v = serde_json::to_value(PortCheck {
            host: "localhost".into(),
            ip: Ipv4Addr::LOCALHOST,
            port: 80,
            open: true,
        })
        .unwrap();
        assert_eq!(v["open"], true);
        assert_eq!(v["ip"], "127.0.0.1");
        let k = serde_json::to_value(AdapterKind::Wifi).unwrap();
        assert_eq!(k, "wifi");
    }
}
