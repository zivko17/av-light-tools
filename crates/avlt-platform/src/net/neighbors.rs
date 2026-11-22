//! IPv4 neighbour (ARP) table and reverse DNS.

use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::time::Duration;

use crate::Result;

/// The IPv4 neighbour table, read once: IP to MAC (`AA-BB-CC-DD-EE-FF`).
///
/// Entries without a resolved address (incomplete / unreachable), broadcast and
/// multicast entries are left out.
pub fn neighbors() -> Result<HashMap<Ipv4Addr, String>> {
    imp::neighbors()
}

/// Reverse DNS name of `addr`, or `None` if none answers within `timeout`.
pub async fn hostname(addr: Ipv4Addr, timeout: Duration) -> Option<String> {
    let lookup = tokio::task::spawn_blocking(move || imp::hostname(addr));
    tokio::time::timeout(timeout, lookup)
        .await
        .ok()
        .and_then(|joined| joined.ok())
        .flatten()
}

/// Whether a neighbour entry is worth reporting.
#[cfg_attr(not(windows), allow(dead_code))]
fn keep(ip: Ipv4Addr, mac: &str) -> bool {
    !ip.is_multicast() && !ip.is_broadcast() && !mac.eq_ignore_ascii_case("FF-FF-FF-FF-FF-FF")
}

#[cfg(windows)]
mod imp {
    use std::collections::HashMap;
    use std::net::Ipv4Addr;
    use std::sync::Once;

    use avlt_core::net::oui::format_mac;
    use windows::Win32::Foundation::NO_ERROR;
    use windows::Win32::NetworkManagement::IpHelper::{
        FreeMibTable, GetIpNetTable2, MIB_IPNET_TABLE2,
    };
    use windows::Win32::Networking::WinSock::{
        AF_INET, GetNameInfoW, NI_MAXHOST, NI_NAMEREQD, NlnsIncomplete, NlnsUnreachable, SOCKADDR,
        SOCKADDR_IN, WSADATA, WSAStartup, socklen_t,
    };

    use crate::net::sockaddr::{inet_ipv4, sockaddr_in};
    use crate::{Error, Result};

    pub fn neighbors() -> Result<HashMap<Ipv4Addr, String>> {
        let mut table: *mut MIB_IPNET_TABLE2 = std::ptr::null_mut();
        // SAFETY: `table` receives a buffer allocated by the API, freed below.
        let ret = unsafe { GetIpNetTable2(AF_INET, &mut table) };
        if ret != NO_ERROR || table.is_null() {
            return Err(Error::Other(format!("GetIpNetTable2 failed ({})", ret.0)));
        }
        let mut map = HashMap::new();
        // SAFETY: on success `table` points to NumEntries rows.
        let rows = unsafe {
            std::slice::from_raw_parts((*table).Table.as_ptr(), (*table).NumEntries as usize)
        };
        for row in rows {
            if row.State == NlnsUnreachable || row.State == NlnsIncomplete {
                continue;
            }
            let len = (row.PhysicalAddressLength as usize).min(row.PhysicalAddress.len());
            if len != 6 {
                continue;
            }
            let (Some(ip), Some(mac)) = (
                inet_ipv4(&row.Address),
                format_mac(&row.PhysicalAddress[..len]),
            ) else {
                continue;
            };
            if super::keep(ip, &mac) {
                map.entry(ip).or_insert(mac);
            }
        }
        // SAFETY: `table` came from GetIpNetTable2 and `rows` is not used afterwards.
        unsafe { FreeMibTable(table.cast()) };
        Ok(map)
    }

    pub fn hostname(addr: Ipv4Addr) -> Option<String> {
        static WSA: Once = Once::new();
        WSA.call_once(|| {
            let mut data = WSADATA::default();
            // SAFETY: `data` is a valid out-parameter; Winsock stays initialised for
            // the life of the process.
            let _ = unsafe { WSAStartup(0x0202, &mut data) };
        });
        let sa = sockaddr_in(addr);
        let mut host = vec![0u16; NI_MAXHOST as usize];
        // SAFETY: `sa` is a SOCKADDR_IN of the length passed; `host` is writable.
        let ret = unsafe {
            GetNameInfoW(
                (&raw const sa).cast::<SOCKADDR>(),
                socklen_t(size_of::<SOCKADDR_IN>() as i32),
                Some(&mut host),
                None,
                NI_NAMEREQD as i32,
            )
        };
        if ret != 0 {
            return None;
        }
        let len = host.iter().position(|c| *c == 0).unwrap_or(host.len());
        let name = String::from_utf16_lossy(&host[..len]);
        (!name.is_empty() && name != addr.to_string()).then_some(name)
    }
}

#[cfg(not(windows))]
mod imp {
    use std::collections::HashMap;
    use std::net::Ipv4Addr;

    use crate::{Error, Result};

    pub fn neighbors() -> Result<HashMap<Ipv4Addr, String>> {
        Err(Error::Unsupported)
    }

    pub fn hostname(_addr: Ipv4Addr) -> Option<String> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn broadcast_and_multicast_are_dropped() {
        assert!(keep(Ipv4Addr::new(192, 168, 1, 10), "7C-2E-0D-12-34-56"));
        assert!(!keep(Ipv4Addr::new(224, 0, 0, 251), "01-00-5E-00-00-FB"));
        assert!(!keep(Ipv4Addr::BROADCAST, "FF-FF-FF-FF-FF-FF"));
        assert!(!keep(Ipv4Addr::new(192, 168, 1, 255), "ff-ff-ff-ff-ff-ff"));
    }

    #[cfg(windows)]
    #[test]
    fn neighbour_table_reads() {
        let table = neighbors().unwrap();
        assert!(table.keys().all(|ip| !ip.is_multicast()));
    }
}
