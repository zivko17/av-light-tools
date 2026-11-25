//! Network adapters (`GetAdaptersAddresses`) and default-adapter resolution.

use avlt_core::net::model::{Adapter, AdapterKind};

use crate::{Error, Result};

/// Every adapter Windows reports (up or not), with IPv4 addresses, gateways, DNS and DHCP mode.
pub fn adapters() -> Result<Vec<Adapter>> {
    imp::adapters()
}

/// The adapter to act on.
///
/// With `preferred` (from `--iface` or `avlt.toml`), the adapter with that name (any
/// state, case-insensitive). Otherwise, as `Resolve-NetIface`: the up adapter that owns
/// the default route (lowest IPv4 metric); failing that the first up adapter with an IPv4
/// address (Ethernet first), so isolated show networks without a gateway still work.
/// Without any, `Error::Other("no active network adapter")`.
pub fn default_adapter(preferred: Option<&str>) -> Result<Adapter> {
    pick_default(adapters()?, preferred)
}

/// The adapter called `name` (case-insensitive), in any state.
pub fn find_adapter(name: &str) -> Result<Adapter> {
    pick_default(adapters()?, Some(name))
}

/// The adapter `net setip` changes.
///
/// With `preferred` (from `--iface` or `avlt.toml`), that adapter, as [`find_adapter`].
/// Otherwise never a guess that could cut the user off: the adapter named `Ethernet` in
/// any state (SetIP.ps1 default), else the default-route adapter only if it is wired
/// Ethernet, else `InvalidInput` asking for `--iface`. Wi-Fi is never chosen implicitly.
pub fn setip_adapter(preferred: Option<&str>) -> Result<Adapter> {
    pick_setip(adapters()?, preferred)
}

pub(crate) fn pick_setip(mut all: Vec<Adapter>, preferred: Option<&str>) -> Result<Adapter> {
    if preferred.is_some() {
        return pick_default(all, preferred);
    }
    if let Some(i) = all
        .iter()
        .position(|a| a.name.eq_ignore_ascii_case("Ethernet"))
    {
        return Ok(all.swap_remove(i));
    }
    let route = all
        .iter()
        .filter(|a| is_active(a) && !a.gateways.is_empty())
        .min_by_key(|a| a.metric);
    match route {
        Some(a) if a.kind == AdapterKind::Ethernet => Ok(a.clone()),
        _ => {
            let names: Vec<&str> = all.iter().map(|a| a.name.as_str()).collect();
            Err(Error::InvalidInput(format!(
                "no adapter named 'Ethernet' and the default route is not on wired Ethernet; choose one with --iface (available: {})",
                names.join(", ")
            )))
        }
    }
}

/// Up, non-loopback adapters with an IPv4 address, default-route owners first.
pub fn active_adapters() -> Result<Vec<Adapter>> {
    let mut list: Vec<Adapter> = adapters()?.into_iter().filter(is_active).collect();
    list.sort_by_key(|a| {
        (
            a.gateways.is_empty(),
            a.kind != AdapterKind::Ethernet,
            a.metric,
        )
    });
    Ok(list)
}

fn is_active(a: &Adapter) -> bool {
    a.up && a.kind != AdapterKind::Loopback && !a.ipv4.is_empty()
}

pub(crate) fn pick_default(all: Vec<Adapter>, preferred: Option<&str>) -> Result<Adapter> {
    if let Some(name) = preferred {
        let names: Vec<String> = all.iter().map(|a| a.name.clone()).collect();
        return all
            .into_iter()
            .find(|a| a.name.eq_ignore_ascii_case(name.trim()))
            .ok_or_else(|| {
                Error::InvalidInput(format!(
                    "no adapter named '{name}' (available: {})",
                    names.join(", ")
                ))
            });
    }
    let mut active: Vec<Adapter> = all.into_iter().filter(is_active).collect();
    if let Some(i) = active
        .iter()
        .enumerate()
        .filter(|(_, a)| !a.gateways.is_empty())
        .min_by_key(|(_, a)| a.metric)
        .map(|(i, _)| i)
    {
        return Ok(active.swap_remove(i));
    }
    active.sort_by_key(|a| a.kind != AdapterKind::Ethernet);
    active
        .into_iter()
        .next()
        .ok_or_else(|| Error::Other("no active network adapter".into()))
}

#[cfg(windows)]
mod imp {
    use std::net::{IpAddr, Ipv4Addr};

    use avlt_core::net::model::{Adapter, AdapterKind, Ipv4Cidr};
    use avlt_core::net::oui::format_mac;
    use windows::Win32::Foundation::{ERROR_BUFFER_OVERFLOW, ERROR_NO_DATA, NO_ERROR};
    use windows::Win32::NetworkManagement::IpHelper::{
        GAA_FLAG_INCLUDE_GATEWAYS, GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_MULTICAST,
        GetAdaptersAddresses, IF_TYPE_ETHERNET_CSMACD, IF_TYPE_IEEE80211,
        IF_TYPE_SOFTWARE_LOOPBACK, IP_ADAPTER_ADDRESSES_LH, IP_ADAPTER_DHCP_ENABLED,
    };
    use windows::Win32::NetworkManagement::Ndis::IfOperStatusUp;
    use windows::Win32::Networking::WinSock::AF_UNSPEC;

    use crate::net::sockaddr::sockaddr_ip;
    use crate::{Error, Result};

    pub fn adapters() -> Result<Vec<Adapter>> {
        let flags = GAA_FLAG_INCLUDE_GATEWAYS | GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST;
        let mut size: u32 = 16 * 1024;
        for _ in 0..4 {
            // u64 elements keep the buffer aligned for IP_ADAPTER_ADDRESSES_LH.
            let mut buf = vec![0u64; (size as usize).div_ceil(8)];
            let head = buf.as_mut_ptr().cast::<IP_ADAPTER_ADDRESSES_LH>();
            // SAFETY: `buf` holds `size` writable bytes; the API fills it with a linked
            // list whose nodes all live inside `buf`.
            let ret = unsafe {
                GetAdaptersAddresses(u32::from(AF_UNSPEC.0), flags, None, Some(head), &mut size)
            };
            match ret {
                r if r == NO_ERROR.0 => {
                    // SAFETY: on success `head` is the first node of a valid list in `buf`.
                    return Ok(unsafe { parse_list(head) });
                }
                r if r == ERROR_NO_DATA.0 => return Ok(Vec::new()),
                r if r == ERROR_BUFFER_OVERFLOW.0 => continue,
                r => return Err(Error::Other(format!("GetAdaptersAddresses failed ({r})"))),
            }
        }
        Err(Error::Other(
            "GetAdaptersAddresses: adapter list keeps growing".into(),
        ))
    }

    /// SAFETY: `node` must be null or the head of a list returned by GetAdaptersAddresses
    /// whose memory is still alive.
    unsafe fn parse_list(mut node: *const IP_ADAPTER_ADDRESSES_LH) -> Vec<Adapter> {
        let mut out = Vec::new();
        while !node.is_null() {
            // SAFETY: guaranteed by the caller for every node of the list.
            let a = unsafe { &*node };
            // SAFETY: as above; strings and sub-lists point into the same buffer.
            out.push(unsafe { parse_adapter(a) });
            node = a.Next;
        }
        out
    }

    unsafe fn parse_adapter(a: &IP_ADAPTER_ADDRESSES_LH) -> Adapter {
        // SAFETY: FriendlyName/Description are NUL-terminated wide strings or null.
        let name = unsafe { wide(a.FriendlyName.0) };
        let description = unsafe { wide(a.Description.0) };
        let kind = match a.IfType {
            IF_TYPE_ETHERNET_CSMACD => AdapterKind::Ethernet,
            IF_TYPE_IEEE80211 => AdapterKind::Wifi,
            IF_TYPE_SOFTWARE_LOOPBACK => AdapterKind::Loopback,
            _ => AdapterKind::Other,
        };
        let mac_len = (a.PhysicalAddressLength as usize).min(a.PhysicalAddress.len());

        let mut ipv4 = Vec::new();
        let mut u = a.FirstUnicastAddress;
        while !u.is_null() {
            // SAFETY: list node inside the GetAdaptersAddresses buffer.
            let ua = unsafe { &*u };
            // SAFETY: lpSockaddr points to a socket address of the size the API reports.
            if let Some(IpAddr::V4(address)) = unsafe { sockaddr_ip(ua.Address.lpSockaddr) } {
                ipv4.push(Ipv4Cidr {
                    address,
                    prefix: ua.OnLinkPrefixLength.min(32),
                });
            }
            u = ua.Next;
        }

        let mut gateways = Vec::new();
        let mut g = a.FirstGatewayAddress;
        while !g.is_null() {
            // SAFETY: list node inside the GetAdaptersAddresses buffer.
            let ga = unsafe { &*g };
            // SAFETY: as for unicast addresses.
            if let Some(IpAddr::V4(gw)) = unsafe { sockaddr_ip(ga.Address.lpSockaddr) }
                && gw != Ipv4Addr::UNSPECIFIED
            {
                gateways.push(gw);
            }
            g = ga.Next;
        }

        let mut dns = Vec::new();
        let mut d = a.FirstDnsServerAddress;
        while !d.is_null() {
            // SAFETY: list node inside the GetAdaptersAddresses buffer.
            let da = unsafe { &*d };
            // SAFETY: as for unicast addresses.
            match unsafe { sockaddr_ip(da.Address.lpSockaddr) } {
                // fec0:0:0:ffff::1-3 are the placeholder IPv6 resolvers Windows lists
                // when none is configured.
                Some(IpAddr::V6(v6)) if v6.segments()[0] == 0xfec0 => {}
                Some(ip) => dns.push(ip),
                None => {}
            }
            d = da.Next;
        }

        // SAFETY: both union views are plain integers.
        let (index, flags) = unsafe { (a.Anonymous1.Anonymous.IfIndex, a.Anonymous2.Flags) };
        let link = a.ReceiveLinkSpeed;
        Adapter {
            name,
            description,
            index,
            kind,
            up: a.OperStatus == IfOperStatusUp,
            mac: format_mac(&a.PhysicalAddress[..mac_len]),
            ipv4,
            gateways,
            dns,
            dhcp: flags & IP_ADAPTER_DHCP_ENABLED != 0,
            link_speed_bps: (link != 0 && link != u64::MAX).then_some(link),
            metric: a.Ipv4Metric,
        }
    }

    /// SAFETY: `p` must be null or a NUL-terminated UTF-16 string.
    unsafe fn wide(p: *const u16) -> String {
        if p.is_null() {
            return String::new();
        }
        let mut len = 0;
        // SAFETY: the string is NUL-terminated.
        while unsafe { *p.add(len) } != 0 {
            len += 1;
        }
        // SAFETY: `len` elements were just read.
        String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(p, len) })
    }
}

#[cfg(not(windows))]
mod imp {
    use avlt_core::net::model::Adapter;

    use crate::{Error, Result};

    pub fn adapters() -> Result<Vec<Adapter>> {
        Err(Error::Unsupported)
    }
}

#[cfg(test)]
mod tests {
    use avlt_core::net::model::Ipv4Cidr;

    use super::*;

    fn adapter(name: &str, up: bool, ip: Option<&str>, gw: Option<&str>, metric: u32) -> Adapter {
        Adapter {
            name: name.into(),
            description: String::new(),
            index: 0,
            kind: if name.starts_with("Wi") {
                AdapterKind::Wifi
            } else {
                AdapterKind::Ethernet
            },
            up,
            mac: None,
            ipv4: ip
                .map(|a| Ipv4Cidr {
                    address: a.parse().unwrap(),
                    prefix: 24,
                })
                .into_iter()
                .collect(),
            gateways: gw.map(|g| g.parse().unwrap()).into_iter().collect(),
            dns: vec![],
            dhcp: true,
            link_speed_bps: None,
            metric,
        }
    }

    #[test]
    fn default_route_with_lowest_metric_wins() {
        let all = vec![
            adapter("Wi-Fi", true, Some("10.0.0.5"), Some("10.0.0.1"), 50),
            adapter(
                "Ethernet",
                true,
                Some("192.168.1.5"),
                Some("192.168.1.1"),
                25,
            ),
            adapter("Ethernet 2", true, Some("2.0.0.5"), None, 5),
        ];
        assert_eq!(pick_default(all, None).unwrap().name, "Ethernet");
    }

    #[test]
    fn without_gateway_falls_back_to_first_active_ethernet() {
        let all = vec![
            adapter("Wi-Fi", true, Some("10.0.0.5"), None, 50),
            adapter("Ethernet 3", false, Some("172.16.0.5"), None, 1),
            adapter("Ethernet 2", true, Some("2.0.0.5"), None, 5),
        ];
        assert_eq!(pick_default(all, None).unwrap().name, "Ethernet 2");
    }

    #[test]
    fn no_active_adapter_is_other_error() {
        let all = vec![
            adapter("Ethernet", false, Some("192.168.1.5"), None, 5),
            adapter("Ethernet 2", true, None, None, 5),
        ];
        let err = pick_default(all, None).unwrap_err();
        assert!(matches!(&err, Error::Other(m) if m == "no active network adapter"));
        assert_eq!(err.exit_code(), 1);
        assert!(pick_default(vec![], None).is_err());
    }

    #[test]
    fn setip_prefers_ethernet_over_wifi_default_route() {
        let all = vec![
            adapter("Wi-Fi", true, Some("10.0.0.5"), Some("10.0.0.1"), 25),
            adapter("Ethernet", true, Some("192.168.1.99"), None, 5),
        ];
        assert_eq!(pick_setip(all, None).unwrap().name, "Ethernet");
    }

    #[test]
    fn setip_takes_ethernet_even_when_down() {
        let all = vec![
            adapter("Wi-Fi", true, Some("10.0.0.5"), Some("10.0.0.1"), 25),
            adapter("Ethernet", false, None, None, 5),
        ];
        assert_eq!(pick_setip(all, None).unwrap().name, "Ethernet");
    }

    #[test]
    fn setip_never_picks_wifi_implicitly() {
        let all = vec![
            adapter("Wi-Fi", true, Some("10.0.0.5"), Some("10.0.0.1"), 25),
            adapter("Ethernet 2", true, Some("192.168.1.99"), None, 5),
        ];
        let err = pick_setip(all, None).unwrap_err();
        assert!(matches!(err, Error::InvalidInput(_)), "{err:?}");
        assert!(!err.to_string().contains("  "), "{err}");
        let msg = err.to_string();
        assert!(msg.contains("--iface"), "{msg}");
        assert!(msg.contains("Wi-Fi") && msg.contains("Ethernet 2"), "{msg}");
        assert!(pick_setip(vec![], None).is_err());
    }

    #[test]
    fn setip_uses_wired_default_route_without_an_ethernet_name() {
        let all = vec![
            adapter("Wi-Fi", true, Some("10.0.0.5"), None, 25),
            adapter(
                "Ethernet 2",
                true,
                Some("192.168.1.5"),
                Some("192.168.1.1"),
                5,
            ),
        ];
        assert_eq!(pick_setip(all, None).unwrap().name, "Ethernet 2");
    }

    #[test]
    fn setip_explicit_name_wins() {
        let all = vec![
            adapter("Wi-Fi", true, Some("10.0.0.5"), Some("10.0.0.1"), 25),
            adapter("Ethernet", true, None, None, 5),
        ];
        assert_eq!(
            pick_setip(all.clone(), Some("wi-fi")).unwrap().name,
            "Wi-Fi"
        );
        assert!(matches!(
            pick_setip(all, Some("Nope")),
            Err(Error::InvalidInput(_))
        ));
    }

    #[test]
    fn preferred_name_any_state_case_insensitive() {
        let all = vec![
            adapter(
                "Ethernet",
                true,
                Some("192.168.1.5"),
                Some("192.168.1.1"),
                5,
            ),
            adapter("Ethernet 2", false, None, None, 5),
        ];
        assert_eq!(
            pick_default(all.clone(), Some("ethernet 2")).unwrap().name,
            "Ethernet 2"
        );
        let err = pick_default(all, Some("Nope")).unwrap_err();
        assert!(matches!(err, Error::InvalidInput(_)), "{err:?}");
        assert!(err.to_string().contains("Ethernet 2"), "{err}");
    }
}
