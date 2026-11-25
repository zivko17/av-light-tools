//! The common TCP ports that `net portscan` probes.

/// The 14 ports of `portscan` in NetTools.ps1, sorted by port number.
pub const COMMON_PORTS: &[(u16, &str)] = &[
    (21, "FTP"),
    (22, "SSH"),
    (23, "Telnet"),
    (53, "DNS"),
    (80, "HTTP"),
    (123, "NTP"),
    (161, "SNMP"),
    (443, "HTTPS"),
    (445, "SMB"),
    (3389, "RDP"),
    (5353, "mDNS/Bonjour"),
    (8000, "HTTP-Alt"),
    (8080, "HTTP-Proxy"),
    (8443, "HTTPS-Alt"),
];

/// Service name of a port in [`COMMON_PORTS`].
pub fn service_name(port: u16) -> Option<&'static str> {
    COMMON_PORTS
        .iter()
        .find(|(p, _)| *p == port)
        .map(|(_, name)| *name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn common_ports_match_nettools_ps1() {
        assert_eq!(COMMON_PORTS.len(), 14);
        assert!(COMMON_PORTS.contains(&(3389, "RDP")));
        assert!(COMMON_PORTS.contains(&(5353, "mDNS/Bonjour")));
        assert!(
            COMMON_PORTS.windows(2).all(|w| w[0].0 < w[1].0),
            "sorted, unique"
        );
    }

    #[test]
    fn service_lookup() {
        assert_eq!(service_name(22), Some("SSH"));
        assert_eq!(service_name(8443), Some("HTTPS-Alt"));
        assert_eq!(service_name(9), None);
    }
}
