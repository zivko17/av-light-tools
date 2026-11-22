//! IPv4 subnet calculator. Ports `Get-Subnet` (AVCalc.ps1 lines 24-72).

use std::net::Ipv4Addr;

use serde::Serialize;

use crate::error::{Error, Result};
use crate::net::ipv4::{hosts_in, parse_cidr, prefix_to_mask};

/// Largest assignment table `subnet` will build.
pub const MAX_ASSIGNMENTS: u32 = 65_536;

/// Address structure of a subnet plus an optional device assignment table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SubnetResult {
    /// Network address with prefix, e.g. `192.168.10.0/24`.
    pub cidr: String,
    pub prefix: u8,
    pub network: String,
    pub netmask: String,
    pub wildcard: String,
    pub broadcast: String,
    pub first_usable: String,
    pub last_usable: String,
    /// All addresses in the block (2^(32-prefix)).
    pub total_addresses: u64,
    /// Usable hosts: total - 2 up to /30, 2 for /31 (RFC 3021), 1 for /32.
    pub hosts: u64,
    /// Devices requested for the assignment table.
    pub devices: u32,
    /// Host offset (from the network address) of the first device.
    pub start_host: u32,
    pub assignments: Vec<String>,
    /// The table stopped early because it ran past the last usable address.
    pub assignments_truncated: bool,
}

/// Computes the subnet for `cidr` and assigns `devices` consecutive addresses starting
/// at host offset `start_host` (offset 1 = first host).
pub fn subnet(cidr: &str, devices: u32, start_host: u32) -> Result<SubnetResult> {
    let (ip, prefix) = parse_cidr(cidr)?;
    if devices > MAX_ASSIGNMENTS {
        return Err(Error::InvalidInput(format!(
            "devices must be at most {MAX_ASSIGNMENTS}"
        )));
    }
    let mask = prefix_to_mask(prefix);
    let network = u32::from(ip) & u32::from(mask);
    let wildcard = !u32::from(mask);
    let broadcast = network | wildcard;
    let total_addresses = 1u64 << (32 - u32::from(prefix));
    let hosts = match prefix {
        32 => 1,
        31 => 2,
        _ => total_addresses - 2,
    };

    let mut usable = hosts_in(Ipv4Addr::from(network), prefix);
    let (first, last) = match (usable.next(), usable.next_back()) {
        (Some(first), Some(last)) => (first, last),
        // /32 yields a single address, which both ends of the iterator share.
        (Some(only), None) | (None, Some(only)) => (only, only),
        (None, None) => return Err(Error::Other("empty subnet".into())),
    };

    let last_u64 = u64::from(u32::from(last));
    let first_device = u64::from(network) + u64::from(start_host);
    let mut assignments = Vec::new();
    let mut truncated = false;
    for i in 0..u64::from(devices) {
        let addr = first_device + i;
        match u32::try_from(addr) {
            Ok(a) if addr <= last_u64 => assignments.push(Ipv4Addr::from(a).to_string()),
            _ => {
                truncated = true;
                break;
            }
        }
    }

    Ok(SubnetResult {
        cidr: format!("{}/{prefix}", Ipv4Addr::from(network)),
        prefix,
        network: Ipv4Addr::from(network).to_string(),
        netmask: mask.to_string(),
        wildcard: Ipv4Addr::from(wildcard).to_string(),
        broadcast: Ipv4Addr::from(broadcast).to_string(),
        first_usable: first.to_string(),
        last_usable: last.to_string(),
        total_addresses,
        hosts,
        devices,
        start_host,
        assignments,
        assignments_truncated: truncated,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slash24_with_host_bits_set() {
        let r = subnet("192.168.10.37/24", 0, 1).unwrap();
        assert_eq!(
            (
                r.network.as_str(),
                r.netmask.as_str(),
                r.wildcard.as_str(),
                r.broadcast.as_str()
            ),
            (
                "192.168.10.0",
                "255.255.255.0",
                "0.0.0.255",
                "192.168.10.255"
            )
        );
        assert_eq!(
            (r.first_usable.as_str(), r.last_usable.as_str(), r.hosts),
            ("192.168.10.1", "192.168.10.254", 254)
        );
        assert_eq!(r.cidr, "192.168.10.0/24");
        assert_eq!(r.total_addresses, 256);
        assert!(r.assignments.is_empty());
        assert!(!r.assignments_truncated);
    }

    #[test]
    fn slash30_31_32_and_0() {
        let r = subnet("10.0.0.0/30", 0, 1).unwrap();
        assert_eq!(r.hosts, 2);
        let r = subnet("10.0.0.0/31", 0, 1).unwrap();
        assert_eq!(
            (r.first_usable.as_str(), r.last_usable.as_str(), r.hosts),
            ("10.0.0.0", "10.0.0.1", 2)
        );
        let r = subnet("10.0.0.5/32", 0, 1).unwrap();
        assert_eq!(r.hosts, 1);
        assert_eq!(
            (r.first_usable.as_str(), r.last_usable.as_str()),
            ("10.0.0.5", "10.0.0.5")
        );
        let r = subnet("0.0.0.0/0", 0, 1).unwrap();
        assert_eq!(r.netmask, "0.0.0.0");
        assert_eq!(r.wildcard, "255.255.255.255");
        assert_eq!(r.hosts, 4_294_967_294);
        assert_eq!(r.total_addresses, 4_294_967_296);
        assert_eq!(r.last_usable, "255.255.255.254");
    }

    #[test]
    fn assignments_from_start_host() {
        let r = subnet("192.168.1.0/24", 3, 10).unwrap();
        assert_eq!(
            r.assignments,
            vec!["192.168.1.10", "192.168.1.11", "192.168.1.12"]
        );
        assert!(!r.assignments_truncated);
    }

    #[test]
    fn assignments_stop_at_last_usable() {
        let r = subnet("192.168.1.0/30", 5, 1).unwrap();
        assert_eq!(r.assignments.len(), 2);
        assert!(r.assignments_truncated);
    }

    #[test]
    fn assignments_that_exactly_fit_are_not_truncated() {
        let r = subnet("192.168.1.0/30", 2, 1).unwrap();
        assert_eq!(r.assignments, vec!["192.168.1.1", "192.168.1.2"]);
        assert!(!r.assignments_truncated);
    }

    #[test]
    fn invalid_cidr_is_invalid_input() {
        for bad in [
            "192.168.1.0",
            "192.168.1.0/33",
            "300.1.1.1/24",
            "a.b.c.d/24",
            "",
        ] {
            assert!(
                matches!(subnet(bad, 0, 1), Err(Error::InvalidInput(_))),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn huge_device_count_is_rejected() {
        assert!(matches!(
            subnet("10.0.0.0/8", MAX_ASSIGNMENTS + 1, 1),
            Err(Error::InvalidInput(_))
        ));
    }
}
