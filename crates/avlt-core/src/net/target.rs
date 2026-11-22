//! Parsing of the targets that `net scan` and `net setip` accept.

use std::collections::BTreeMap;
use std::fmt;
use std::net::Ipv4Addr;

use crate::config::IpProfile;
use crate::error::{Error, Result};
use crate::net::ipv4::{parse_cidr, prefix_to_mask};

/// Smallest prefix `net scan` accepts (/20 = 4094 hosts).
pub const MIN_SCAN_PREFIX: u8 = 20;

/// Network to sweep for `net scan [target]`, aligned to its prefix.
///
/// Accepts three octets (`192.168.1`, as NetTools.ps1 `scan`), a CIDR
/// (`192.168.1.0/24`, /20 to /32) or a full address (scans its /24).
pub fn parse_scan_target(s: &str) -> Result<(Ipv4Addr, u8)> {
    let s = s.trim();
    let usage = || {
        Error::InvalidInput(format!(
            "'{s}' is not a subnet (use 192.168.1, 192.168.1.0/24 or 192.168.1.10)"
        ))
    };
    let (addr, prefix) = if s.contains('/') {
        parse_cidr(s)?
    } else {
        let trimmed = s.strip_suffix('.').unwrap_or(s);
        match trimmed.split('.').count() {
            3 => (format!("{trimmed}.0").parse().map_err(|_| usage())?, 24),
            4 => (trimmed.parse().map_err(|_| usage())?, 24),
            _ => return Err(usage()),
        }
    };
    if prefix < MIN_SCAN_PREFIX {
        return Err(Error::InvalidInput(format!(
            "/{prefix} is too large to scan (use /{MIN_SCAN_PREFIX} to /32)"
        )));
    }
    let mask = u32::from(prefix_to_mask(prefix));
    Ok((Ipv4Addr::from(u32::from(addr) & mask), prefix))
}

/// What `net setip` applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "mode", rename_all = "lowercase")]
pub enum IpTarget {
    /// Address and DNS from DHCP.
    Dhcp,
    /// Static address with optional default gateway (DNS goes back to DHCP, as SetIP.ps1).
    Static {
        address: Ipv4Addr,
        prefix: u8,
        gateway: Option<Ipv4Addr>,
    },
}

impl fmt::Display for IpTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IpTarget::Dhcp => f.write_str("DHCP (automatic)"),
            IpTarget::Static {
                address,
                prefix,
                gateway,
            } => {
                write!(f, "{address}/{prefix} ({})", prefix_to_mask(*prefix))?;
                match gateway {
                    Some(gw) => write!(f, " GW {gw}"),
                    None => f.write_str(" (no gateway)"),
                }
            }
        }
    }
}

const SETIP_USAGE: &str = "usage:
  avlt net setip 0                              # profile 0
  avlt net setip 1                              # profile 1
  avlt net setip dhcp                           # automatic
  avlt net setip 192.168.1.25/24                # custom IP, no gateway
  avlt net setip 192.168.1.25/24 192.168.1.1    # custom IP + gateway";

fn setip_error(msg: String) -> Error {
    Error::InvalidInput(format!(
        "{msg}
{SETIP_USAGE}"
    ))
}

/// Parses the arguments of `net setip <target> [gateway]` (SetIP.ps1 modes):
/// `dhcp`, a profile name from `profiles`, or `<ip>/<prefix>` with an optional gateway.
pub fn parse_ip_target(
    target: &str,
    gateway: Option<&str>,
    profiles: &BTreeMap<String, IpProfile>,
) -> Result<IpTarget> {
    let target = target.trim();
    if target.eq_ignore_ascii_case("dhcp") {
        if gateway.is_some() {
            return Err(setip_error("dhcp takes no gateway".into()));
        }
        return Ok(IpTarget::Dhcp);
    }
    if let Some(profile) = profiles.get(target) {
        if gateway.is_some() {
            return Err(setip_error(format!(
                "profile '{target}' already defines its gateway"
            )));
        }
        return static_target(&profile.address, profile.gateway.as_deref())
            .map_err(|e| setip_error(format!("profile '{target}' in avlt.toml: {e}")));
    }
    if !target.contains('/') {
        let names: Vec<&str> = profiles.keys().map(String::as_str).collect();
        return Err(setip_error(format!(
            "invalid input '{target}' (profiles: {}; or dhcp, or ip/prefix)",
            names.join(", ")
        )));
    }
    static_target(target, gateway).map_err(|e| setip_error(e.to_string()))
}

fn static_target(cidr: &str, gateway: Option<&str>) -> Result<IpTarget> {
    let (address, prefix) = parse_cidr(cidr)?;
    let gateway = match gateway.map(str::trim) {
        None => None,
        Some(gw) => Some(
            gw.parse::<Ipv4Addr>()
                .map_err(|_| Error::InvalidInput(format!("invalid gateway '{gw}'")))?,
        ),
    };
    Ok(IpTarget::Static {
        address,
        prefix,
        gateway,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    fn ip(s: &str) -> Ipv4Addr {
        s.parse().unwrap()
    }

    #[test]
    fn scan_three_octets() {
        assert_eq!(
            parse_scan_target("192.168.1").unwrap(),
            (ip("192.168.1.0"), 24)
        );
        assert_eq!(
            parse_scan_target(" 10.0.5. ").unwrap(),
            (ip("10.0.5.0"), 24)
        );
    }

    #[test]
    fn scan_cidr_and_full_address() {
        assert_eq!(
            parse_scan_target("192.168.1.0/24").unwrap(),
            (ip("192.168.1.0"), 24)
        );
        assert_eq!(
            parse_scan_target("192.168.1.77/30").unwrap(),
            (ip("192.168.1.76"), 30)
        );
        assert_eq!(
            parse_scan_target("192.168.1.77").unwrap(),
            (ip("192.168.1.0"), 24)
        );
        assert_eq!(
            parse_scan_target("10.1.0.0/20").unwrap(),
            (ip("10.1.0.0"), 20)
        );
    }

    #[test]
    fn scan_rejects_bad_input() {
        for bad in [
            "",
            "192.168",
            "192.168.300",
            "a.b.c",
            "192.168.1.0/19",
            "0.0.0.0/0",
            "192.168.1.0/33",
            "192.168.1.1.1",
        ] {
            match parse_scan_target(bad) {
                Err(Error::InvalidInput(_)) => {}
                other => panic!("{bad:?} should be InvalidInput, got {other:?}"),
            }
        }
    }

    fn profiles() -> BTreeMap<String, IpProfile> {
        Config::default().ip_profiles
    }

    fn static_ip(a: &str, prefix: u8, gw: Option<&str>) -> IpTarget {
        IpTarget::Static {
            address: ip(a),
            prefix,
            gateway: gw.map(ip),
        }
    }

    #[test]
    fn setip_keywords_and_profiles() {
        let p = profiles();
        assert_eq!(parse_ip_target("dhcp", None, &p).unwrap(), IpTarget::Dhcp);
        assert_eq!(parse_ip_target("DHCP", None, &p).unwrap(), IpTarget::Dhcp);
        assert_eq!(
            parse_ip_target("0", None, &p).unwrap(),
            static_ip("192.168.0.99", 24, Some("192.168.0.1"))
        );
        assert_eq!(
            parse_ip_target("1", None, &p).unwrap(),
            static_ip("192.168.1.99", 24, Some("192.168.1.1"))
        );
    }

    #[test]
    fn setip_custom() {
        let p = profiles();
        assert_eq!(
            parse_ip_target("192.168.1.25/24", None, &p).unwrap(),
            static_ip("192.168.1.25", 24, None)
        );
        assert_eq!(
            parse_ip_target("192.168.1.25/24", Some("192.168.1.1"), &p).unwrap(),
            static_ip("192.168.1.25", 24, Some("192.168.1.1"))
        );
    }

    #[test]
    fn setip_rejects_bad_input() {
        let p = profiles();
        let cases: [(&str, Option<&str>); 8] = [
            ("2", None),
            ("192.168.1.25", None),
            ("192.168.1.256/24", None),
            ("192.168.1.25/33", None),
            ("192.168.1.25/24", Some("192.168.1.300")),
            ("dhcp", Some("192.168.1.1")),
            ("0", Some("192.168.0.254")),
            ("", None),
        ];
        for (target, gw) in cases {
            match parse_ip_target(target, gw, &p) {
                Err(Error::InvalidInput(msg)) => {
                    assert!(msg.contains("setip"), "{target:?}: usage missing in {msg}")
                }
                other => panic!("{target:?} {gw:?} should be InvalidInput, got {other:?}"),
            }
        }
    }

    #[test]
    fn setip_bad_profile_in_config_is_invalid_input() {
        let mut p = BTreeMap::new();
        p.insert(
            "lab".to_string(),
            IpProfile {
                address: "10.0.0.5".into(),
                gateway: None,
            },
        );
        assert!(matches!(
            parse_ip_target("lab", None, &p),
            Err(Error::InvalidInput(_))
        ));
    }

    #[test]
    fn describe_target() {
        assert_eq!(IpTarget::Dhcp.to_string(), "DHCP (automatic)");
        assert_eq!(
            static_ip("192.168.1.25", 24, Some("192.168.1.1")).to_string(),
            "192.168.1.25/24 (255.255.255.0) GW 192.168.1.1"
        );
        assert_eq!(
            static_ip("192.168.1.25", 24, None).to_string(),
            "192.168.1.25/24 (255.255.255.0) (no gateway)"
        );
    }
}
