//! Adapter byte counters (`GetIfEntry2`) and established TCP connections
//! (`GetExtendedTcpTable`, no administrator rights needed).

use avlt_core::net::model::{Adapter, Connection, TrafficSample};

use crate::Result;

/// Current received / sent byte counters of `adapter` (`Get-NetAdapterStatistics`).
pub fn traffic(adapter: &Adapter) -> Result<TrafficSample> {
    imp::traffic(adapter.index)
}

/// Established IPv4 and IPv6 TCP connections, sorted by process name then remote address.
pub fn connections() -> Result<Vec<Connection>> {
    let mut list = imp::connections()?;
    sort_connections(&mut list);
    Ok(list)
}

fn sort_connections(list: &mut [Connection]) {
    list.sort_by(|a, b| {
        a.process
            .to_lowercase()
            .cmp(&b.process.to_lowercase())
            .then(a.remote_addr.cmp(&b.remote_addr))
            .then(a.remote_port.cmp(&b.remote_port))
    });
}

/// Process name as `Get-Process` shows it (no `.exe`).
#[cfg_attr(not(windows), allow(dead_code))]
fn process_name(name: &str) -> String {
    match name.len().checked_sub(4) {
        Some(cut) if name[cut..].eq_ignore_ascii_case(".exe") => name[..cut].to_string(),
        _ => name.to_string(),
    }
}

#[cfg(windows)]
mod imp {
    use std::collections::HashMap;
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    use avlt_core::net::model::{Connection, TrafficSample};
    use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System};
    use windows::Win32::Foundation::{ERROR_INSUFFICIENT_BUFFER, NO_ERROR};
    use windows::Win32::NetworkManagement::IpHelper::{
        GetExtendedTcpTable, GetIfEntry2, MIB_IF_ROW2, MIB_TCP_STATE_ESTAB, MIB_TCP6ROW_OWNER_PID,
        MIB_TCPROW_OWNER_PID, TCP_TABLE_OWNER_PID_ALL,
    };
    use windows::Win32::Networking::WinSock::{ADDRESS_FAMILY, AF_INET, AF_INET6};

    use crate::{Error, Result};

    pub fn traffic(index: u32) -> Result<TrafficSample> {
        let mut row = MIB_IF_ROW2 {
            InterfaceIndex: index,
            ..Default::default()
        };
        // SAFETY: `row` is a valid MIB_IF_ROW2 with the index set, as the API requires.
        let ret = unsafe { GetIfEntry2(&mut row) };
        if ret != NO_ERROR {
            return Err(Error::Other(format!(
                "cannot read counters of interface {index} ({})",
                ret.0
            )));
        }
        Ok(TrafficSample {
            rx_bytes: row.InOctets,
            tx_bytes: row.OutOctets,
        })
    }

    /// Port stored in the low 16 bits of a DWORD, in network byte order.
    fn port(dw: u32) -> u16 {
        let b = dw.to_ne_bytes();
        u16::from_be_bytes([b[0], b[1]])
    }

    /// Raw `GetExtendedTcpTable` result: `u32` count followed by rows.
    fn tcp_table(family: ADDRESS_FAMILY) -> Result<Vec<u64>> {
        let mut size: u32 = 64 * 1024;
        for _ in 0..4 {
            let mut buf = vec![0u64; (size as usize).div_ceil(8)];
            // SAFETY: `buf` holds `size` writable, 8-byte aligned bytes.
            let ret = unsafe {
                GetExtendedTcpTable(
                    Some(buf.as_mut_ptr().cast()),
                    &mut size,
                    false,
                    u32::from(family.0),
                    TCP_TABLE_OWNER_PID_ALL,
                    0,
                )
            };
            if ret == NO_ERROR.0 {
                return Ok(buf);
            }
            if ret != ERROR_INSUFFICIENT_BUFFER.0 {
                return Err(Error::Other(format!("GetExtendedTcpTable failed ({ret})")));
            }
        }
        Err(Error::Other(
            "GetExtendedTcpTable: table keeps growing".into(),
        ))
    }

    /// SAFETY: `buf` must hold a table returned by GetExtendedTcpTable whose rows are `R`.
    unsafe fn rows<R>(buf: &[u64]) -> &[R] {
        if buf.is_empty() {
            return &[];
        }
        // SAFETY: the table starts with its u32 row count; `buf` is 8-byte aligned.
        let count = unsafe { buf.as_ptr().cast::<u32>().read() } as usize;
        // Rows start after the u32 count, at the alignment of R.
        let offset = std::mem::align_of::<R>().max(4);
        // SAFETY: the API wrote `count` rows of R right after the count field.
        unsafe {
            let first = buf.as_ptr().cast::<u8>().add(offset).cast::<R>();
            std::slice::from_raw_parts(first, count)
        }
    }

    pub fn connections() -> Result<Vec<Connection>> {
        let mut raw = Vec::new();
        let v4 = tcp_table(AF_INET)?;
        // SAFETY: AF_INET with TCP_TABLE_OWNER_PID_ALL yields MIB_TCPROW_OWNER_PID rows.
        for r in unsafe { rows::<MIB_TCPROW_OWNER_PID>(&v4) } {
            if r.dwState == MIB_TCP_STATE_ESTAB.0 as u32 {
                raw.push((
                    r.dwOwningPid,
                    IpAddr::V4(Ipv4Addr::from(r.dwLocalAddr.to_ne_bytes())),
                    port(r.dwLocalPort),
                    IpAddr::V4(Ipv4Addr::from(r.dwRemoteAddr.to_ne_bytes())),
                    port(r.dwRemotePort),
                ));
            }
        }
        let v6 = tcp_table(AF_INET6)?;
        // SAFETY: AF_INET6 with TCP_TABLE_OWNER_PID_ALL yields MIB_TCP6ROW_OWNER_PID rows.
        for r in unsafe { rows::<MIB_TCP6ROW_OWNER_PID>(&v6) } {
            if r.dwState == MIB_TCP_STATE_ESTAB.0 as u32 {
                raw.push((
                    r.dwOwningPid,
                    IpAddr::V6(Ipv6Addr::from(r.ucLocalAddr)),
                    port(r.dwLocalPort),
                    IpAddr::V6(Ipv6Addr::from(r.ucRemoteAddr)),
                    port(r.dwRemotePort),
                ));
            }
        }

        let names = process_names();
        Ok(raw
            .into_iter()
            .map(
                |(pid, local_addr, local_port, remote_addr, remote_port)| Connection {
                    process: names
                        .get(&pid)
                        .cloned()
                        .unwrap_or_else(|| format!("pid {pid}")),
                    pid,
                    local_addr,
                    local_port,
                    remote_addr,
                    remote_port,
                },
            )
            .collect())
    }

    fn process_names() -> HashMap<u32, String> {
        let Ok(mut sys) = System::new_with_specifics(RefreshKind::nothing()) else {
            return HashMap::new();
        };
        sys.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing(),
        );
        sys.processes()
            .iter()
            .map(|(pid, p): (&Pid, _)| {
                (
                    pid.as_u32(),
                    super::process_name(&p.name().to_string_lossy()),
                )
            })
            .collect()
    }
}

#[cfg(not(windows))]
mod imp {
    use avlt_core::net::model::{Connection, TrafficSample};

    use crate::{Error, Result};

    pub fn traffic(_index: u32) -> Result<TrafficSample> {
        Err(Error::Unsupported)
    }

    pub fn connections() -> Result<Vec<Connection>> {
        Err(Error::Unsupported)
    }
}

#[cfg(test)]
mod tests {
    use std::net::IpAddr;

    use super::*;

    fn conn(process: &str, remote: &str, port: u16) -> Connection {
        Connection {
            process: process.into(),
            pid: 1,
            local_addr: IpAddr::from([127, 0, 0, 1]),
            local_port: 1,
            remote_addr: remote.parse().unwrap(),
            remote_port: port,
        }
    }

    #[test]
    fn sorted_by_process_then_remote() {
        let mut list = vec![
            conn("svchost", "10.0.0.2", 443),
            conn("Code", "10.0.0.9", 443),
            conn("svchost", "10.0.0.1", 80),
            conn("chrome", "1.1.1.1", 443),
        ];
        sort_connections(&mut list);
        let order: Vec<_> = list
            .iter()
            .map(|c| format!("{} {}", c.process, c.remote_addr))
            .collect();
        assert_eq!(
            order,
            [
                "chrome 1.1.1.1",
                "Code 10.0.0.9",
                "svchost 10.0.0.1",
                "svchost 10.0.0.2"
            ]
        );
    }

    #[test]
    fn exe_suffix_is_dropped() {
        assert_eq!(process_name("chrome.exe"), "chrome");
        assert_eq!(process_name("System"), "System");
        assert_eq!(process_name("A.EXE"), "A");
        assert_eq!(process_name("exe"), "exe");
    }

    #[cfg(windows)]
    #[test]
    fn connections_include_a_local_one() {
        use std::net::{Ipv4Addr, TcpListener, TcpStream};
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let _client = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
        let _server = listener.accept().unwrap();
        let list = connections().unwrap();
        let me = std::process::id();
        assert!(
            list.iter().any(|c| c.pid == me && c.remote_port == port),
            "{list:?}"
        );
    }
}
