//! TCP connect probes (`net portcheck`, `net portscan`).

use std::net::Ipv4Addr;
use std::time::Duration;

use avlt_core::net::model::PortStatus;
use tokio::net::TcpStream;
use tokio::task::JoinSet;

/// `true` if a TCP connection to `addr:port` completes within `timeout`.
pub async fn tcp_open(addr: Ipv4Addr, port: u16, timeout: Duration) -> bool {
    matches!(
        tokio::time::timeout(timeout, TcpStream::connect((addr, port))).await,
        Ok(Ok(_))
    )
}

/// Probes every `(port, service)` in parallel; results sorted by port.
pub async fn port_scan(
    addr: Ipv4Addr,
    ports: &[(u16, &str)],
    timeout: Duration,
) -> Vec<PortStatus> {
    let mut set = JoinSet::new();
    for &(port, service) in ports {
        let service = service.to_string();
        set.spawn(async move {
            PortStatus {
                port,
                service,
                open: tcp_open(addr, port, timeout).await,
            }
        });
    }
    let mut out = Vec::with_capacity(ports.len());
    while let Some(res) = set.join_next().await {
        if let Ok(status) = res {
            out.push(status);
        }
    }
    out.sort_by_key(|p| p.port);
    out
}

#[cfg(test)]
mod tests {
    use std::net::TcpListener;

    use super::*;

    const TIMEOUT: Duration = Duration::from_millis(1000);

    #[tokio::test]
    async fn open_then_closed() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        assert!(tcp_open(Ipv4Addr::LOCALHOST, port, TIMEOUT).await);
        drop(listener);
        assert!(!tcp_open(Ipv4Addr::LOCALHOST, port, TIMEOUT).await);
    }

    #[tokio::test]
    async fn scan_marks_listening_port_open() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let open = listener.local_addr().unwrap().port();
        let closed = {
            let l = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
            l.local_addr().unwrap().port()
        };
        let ports = [(closed, "closed"), (open, "open")];
        let result = port_scan(Ipv4Addr::LOCALHOST, &ports, TIMEOUT).await;
        assert_eq!(result.len(), 2);
        for p in result {
            assert_eq!(p.open, p.port == open, "{p:?}");
            assert_eq!(p.service, if p.port == open { "open" } else { "closed" });
        }
    }
}
