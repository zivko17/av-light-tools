//! IPv4 helpers: CIDR parsing, prefix masks and host enumeration.

use std::net::Ipv4Addr;

use crate::error::{Error, Result};

/// Parses `"a.b.c.d/p"` (surrounding whitespace ignored) into the address and prefix length.
///
/// Fails with [`Error::InvalidInput`] if the `/` is missing, an octet is not 0-255,
/// or the prefix is not a number in 0-32.
pub fn parse_cidr(s: &str) -> Result<(Ipv4Addr, u8)> {
    let s = s.trim();
    let usage = || {
        Error::InvalidInput(format!(
            "'{s}' is not CIDR notation (expected a.b.c.d/0-32)"
        ))
    };
    let (addr, prefix) = s.split_once('/').ok_or_else(usage)?;
    let addr: Ipv4Addr = addr.parse().map_err(|_| usage())?;
    if prefix.is_empty() || !prefix.bytes().all(|b| b.is_ascii_digit()) {
        return Err(usage());
    }
    let prefix: u8 = prefix.parse().map_err(|_| usage())?;
    if prefix > 32 {
        return Err(Error::InvalidInput(format!(
            "invalid prefix /{prefix} in '{s}' (must be 0-32)"
        )));
    }
    Ok((addr, prefix))
}

/// Dotted netmask for a prefix length; prefixes above 32 are treated as 32.
pub fn prefix_to_mask(prefix: u8) -> Ipv4Addr {
    Ipv4Addr::from(mask_bits(prefix))
}

fn mask_bits(prefix: u8) -> u32 {
    match prefix {
        0 => 0,
        p => u32::MAX << (32 - u32::from(p.min(32))),
    }
}

/// Host addresses of the network that contains `net` with `prefix`.
///
/// The network is aligned first (`192.168.1.77/24` enumerates `192.168.1.0/24`).
/// For prefixes up to /30 the network and broadcast addresses are excluded;
/// /31 yields both addresses (RFC 3021) and /32 the single address.
/// Prefixes above 32 are treated as 32.
pub fn hosts_in(net: Ipv4Addr, prefix: u8) -> impl DoubleEndedIterator<Item = Ipv4Addr> {
    let mask = mask_bits(prefix);
    let first = u32::from(net) & mask;
    let last = first | !mask;
    let range = if prefix >= 31 {
        first..=last
    } else {
        first + 1..=last - 1
    };
    range.map(Ipv4Addr::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(s: &str) -> Ipv4Addr {
        s.parse().unwrap()
    }

    fn hosts(net: &str, prefix: u8) -> Vec<String> {
        hosts_in(ip(net), prefix).map(|a| a.to_string()).collect()
    }

    #[test]
    fn parse_cidr_valid() {
        assert_eq!(
            parse_cidr("192.168.1.25/24").unwrap(),
            (ip("192.168.1.25"), 24)
        );
        assert_eq!(parse_cidr("0.0.0.0/0").unwrap(), (ip("0.0.0.0"), 0));
        assert_eq!(parse_cidr("10.0.0.1/32").unwrap(), (ip("10.0.0.1"), 32));
    }

    #[test]
    fn parse_cidr_without_prefix_is_error() {
        let err = parse_cidr("192.168.1.25").unwrap_err();
        assert!(matches!(err, Error::InvalidInput(_)), "{err:?}");
    }

    #[test]
    fn parse_cidr_invalid_inputs() {
        for bad in [
            "192.168.1.0/33",
            "300.1.1.1/24",
            "a.b.c.d/24",
            "",
            "/24",
            "192.168.1.0/",
            "192.168.1.0/-1",
            "192.168.1/24",
            "192.168.1.0/24/8",
        ] {
            match parse_cidr(bad) {
                Err(Error::InvalidInput(_)) => {}
                other => panic!("{bad:?} should be InvalidInput, got {other:?}"),
            }
        }
    }

    #[test]
    fn masks() {
        assert_eq!(prefix_to_mask(24).to_string(), "255.255.255.0");
        assert_eq!(prefix_to_mask(0).to_string(), "0.0.0.0");
        assert_eq!(prefix_to_mask(32).to_string(), "255.255.255.255");
        assert_eq!(prefix_to_mask(20).to_string(), "255.255.240.0");
    }

    #[test]
    fn hosts_slash24() {
        assert_eq!(hosts_in(ip("192.168.1.0"), 24).count(), 254);
        let h = hosts("192.168.1.0", 24);
        assert_eq!(h.first().map(String::as_str), Some("192.168.1.1"));
        assert_eq!(h.last().map(String::as_str), Some("192.168.1.254"));
    }

    #[test]
    fn hosts_are_network_aligned() {
        assert_eq!(hosts("192.168.1.77", 30), ["192.168.1.77", "192.168.1.78"]);
        assert_eq!(hosts_in(ip("192.168.1.25"), 24).count(), 254);
    }

    #[test]
    fn hosts_slash30() {
        assert_eq!(hosts("10.0.0.0", 30), ["10.0.0.1", "10.0.0.2"]);
    }

    #[test]
    fn hosts_slash31_yields_both() {
        assert_eq!(hosts("10.0.0.0", 31), ["10.0.0.0", "10.0.0.1"]);
        assert_eq!(hosts("10.0.0.1", 31), ["10.0.0.0", "10.0.0.1"]);
    }

    #[test]
    fn hosts_slash32_yields_single() {
        assert_eq!(hosts("10.0.0.7", 32), ["10.0.0.7"]);
    }

    #[test]
    fn hosts_slash0_excludes_ends() {
        let mut it = hosts_in(ip("1.2.3.4"), 0);
        assert_eq!(it.next(), Some(ip("0.0.0.1")));
        assert_eq!(it.next_back(), Some(ip("255.255.255.254")));
    }
}
