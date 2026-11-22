//! ICMP echo (`IcmpSendEcho`, no administrator rights needed) and the concurrent ping sweep.

use std::net::Ipv4Addr;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Semaphore;
use tokio::task::JoinSet;

/// Round trip of one echo request, or `None` on timeout / unreachable.
/// Always `None` outside Windows.
pub async fn ping(addr: Ipv4Addr, timeout: Duration) -> Option<Duration> {
    tokio::task::spawn_blocking(move || imp::ping(addr, timeout))
        .await
        .ok()
        .flatten()
}

/// Pings every address with at most `concurrency` requests in flight.
///
/// Calls `progress(done, total)` after each reply or timeout and returns the hosts that
/// answered with their round trip, in address order.
pub async fn sweep(
    targets: &[Ipv4Addr],
    timeout: Duration,
    concurrency: usize,
    mut progress: impl FnMut(usize, usize),
) -> Vec<(Ipv4Addr, Duration)> {
    let limit = Arc::new(Semaphore::new(concurrency.max(1)));
    let mut set = JoinSet::new();
    for &addr in targets {
        let limit = Arc::clone(&limit);
        set.spawn(async move {
            let _permit = limit.acquire_owned().await.ok()?;
            ping(addr, timeout).await.map(|rtt| (addr, rtt))
        });
    }
    let total = targets.len();
    let mut done = 0;
    let mut alive = Vec::new();
    while let Some(res) = set.join_next().await {
        done += 1;
        progress(done, total);
        if let Ok(Some(hit)) = res {
            alive.push(hit);
        }
    }
    alive.sort_by_key(|(a, _)| *a);
    alive
}

#[cfg(windows)]
mod imp {
    use std::net::Ipv4Addr;
    use std::time::Duration;

    use windows::Win32::NetworkManagement::IpHelper::{
        ICMP_ECHO_REPLY, IP_SUCCESS, IcmpCloseHandle, IcmpCreateFile, IcmpSendEcho,
    };

    const PAYLOAD: &[u8; 32] = b"avlt-ping-avlt-ping-avlt-ping-av";

    pub fn ping(addr: Ipv4Addr, timeout: Duration) -> Option<Duration> {
        // SAFETY: no preconditions; the handle is closed below.
        let handle = unsafe { IcmpCreateFile() }.ok()?;
        // Reply buffer: one ICMP_ECHO_REPLY plus the echoed payload and 8 bytes for an
        // ICMP error, as the documentation requires; u64 elements keep it aligned.
        let reply_size = size_of::<ICMP_ECHO_REPLY>() + PAYLOAD.len() + 8 + 64;
        let mut buf = vec![0u64; reply_size.div_ceil(8)];
        let timeout_ms = u32::try_from(timeout.as_millis())
            .unwrap_or(u32::MAX)
            .max(1);
        // SAFETY: the handle is valid, the payload and reply buffers have the sizes passed.
        let count = unsafe {
            IcmpSendEcho(
                handle,
                u32::from_ne_bytes(addr.octets()),
                PAYLOAD.as_ptr().cast(),
                PAYLOAD.len() as u16,
                None,
                buf.as_mut_ptr().cast(),
                (buf.len() * 8) as u32,
                timeout_ms,
            )
        };
        // SAFETY: the buffer starts with an ICMP_ECHO_REPLY when `count` > 0.
        let reply = (count > 0).then(|| unsafe { *buf.as_ptr().cast::<ICMP_ECHO_REPLY>() });
        // SAFETY: `handle` came from IcmpCreateFile and is not used afterwards.
        let _ = unsafe { IcmpCloseHandle(handle) };
        let reply = reply?;
        (reply.Status == IP_SUCCESS).then(|| Duration::from_millis(u64::from(reply.RoundTripTime)))
    }
}

#[cfg(not(windows))]
mod imp {
    use std::net::Ipv4Addr;
    use std::time::Duration;

    pub fn ping(_addr: Ipv4Addr, _timeout: Duration) -> Option<Duration> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn sweep_reports_progress_for_every_target() {
        // TEST-NET-1 (RFC 5737) never answers; outside Windows ping is always None.
        let targets: Vec<Ipv4Addr> = (1..=5).map(|i| Ipv4Addr::new(192, 0, 2, i)).collect();
        let mut calls = Vec::new();
        let alive = sweep(&targets, Duration::from_millis(200), 2, |d, t| {
            calls.push((d, t))
        })
        .await;
        assert!(alive.is_empty(), "{alive:?}");
        assert_eq!(calls.len(), 5);
        assert_eq!(calls.last(), Some(&(5, 5)));
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn loopback_answers() {
        let rtt = ping(Ipv4Addr::LOCALHOST, Duration::from_millis(1000)).await;
        assert!(rtt.is_some());
    }
}
