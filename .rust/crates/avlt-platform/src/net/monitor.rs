//! The live monitors' sampling steps (`net pingmulti`, `net speed`), shared by the CLI
//! loop and the GUI: one call = one sample.

use std::net::Ipv4Addr;
use std::time::{Duration, Instant};

use avlt_core::net::format::traffic_rate;
use avlt_core::net::model::{Adapter, PingHost, PingRound, SpeedSample, TrafficSample};

use super::{PING_TIMEOUT, ping, resolve_ipv4, traffic};
use crate::Result;

fn millis(d: Duration) -> u64 {
    u64::try_from(d.as_millis()).unwrap_or(u64::MAX)
}

/// The hosts of a multi-ping, each with its address (`None`: the name did not resolve).
pub fn ping_targets(hosts: &[String]) -> Vec<(String, Option<Ipv4Addr>)> {
    hosts
        .iter()
        .map(|h| (h.clone(), resolve_ipv4(h).ok()))
        .collect()
}

/// Pings every target at once. `failures` (one counter per target, kept by the caller
/// between rounds) counts the missed replies; `start` is when monitoring began.
pub async fn ping_round(
    targets: &[(String, Option<Ipv4Addr>)],
    failures: &mut [u64],
    start: Instant,
) -> PingRound {
    let mut set = tokio::task::JoinSet::new();
    for (i, (_, ip)) in targets.iter().enumerate() {
        let ip = *ip;
        set.spawn(async move {
            let rtt = match ip {
                Some(ip) => ping(ip, PING_TIMEOUT).await,
                None => None,
            };
            (i, rtt)
        });
    }
    let mut rtts = vec![None; targets.len()];
    while let Some(Ok((i, rtt))) = set.join_next().await {
        rtts[i] = rtt;
    }
    let hosts = targets
        .iter()
        .zip(rtts)
        .zip(failures.iter_mut())
        .map(|(((host, ip), rtt), fails)| {
            if rtt.is_none() {
                *fails += 1;
            }
            PingHost {
                host: host.clone(),
                ip: *ip,
                online: rtt.is_some(),
                latency_ms: rtt.map(millis),
                failures: *fails,
            }
        })
        .collect();
    PingRound {
        elapsed_s: start.elapsed().as_secs_f64(),
        hosts,
    }
}

/// One latency probe in milliseconds (`None`: timeout).
pub async fn latency_ms(ip: Ipv4Addr) -> Option<u64> {
    ping(ip, PING_TIMEOUT).await.map(millis)
}

/// Throughput of one adapter between successive calls to [`TrafficMeter::sample`].
pub struct TrafficMeter {
    adapter: Adapter,
    base: TrafficSample,
    prev: TrafficSample,
    start: Instant,
    last: Instant,
}

impl TrafficMeter {
    /// Reads the adapter's counters now; rates are measured from here.
    pub fn new(adapter: Adapter) -> Result<TrafficMeter> {
        let base = traffic(&adapter)?;
        let now = Instant::now();
        Ok(TrafficMeter {
            adapter,
            base,
            prev: base,
            start: now,
            last: now,
        })
    }

    pub fn adapter(&self) -> &Adapter {
        &self.adapter
    }

    /// Rates since the previous sample and totals since [`TrafficMeter::new`].
    pub fn sample(&mut self) -> Result<SpeedSample> {
        let cur = traffic(&self.adapter)?;
        let now = Instant::now();
        let (down_bps, up_bps) = traffic_rate(&self.prev, &cur, (now - self.last).as_secs_f64());
        let sample = SpeedSample {
            elapsed_s: (now - self.start).as_secs_f64(),
            adapter: self.adapter.name.clone(),
            down_bps,
            up_bps,
            total_down_bytes: cur.rx_bytes.saturating_sub(self.base.rx_bytes),
            total_up_bytes: cur.tx_bytes.saturating_sub(self.base.tx_bytes),
        };
        (self.prev, self.last) = (cur, now);
        Ok(sample)
    }

    /// Time since [`TrafficMeter::new`].
    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn unresolved_targets_count_failures() {
        let targets = vec![("no-such-host.invalid".to_string(), None)];
        let mut failures = vec![0u64; 1];
        let start = Instant::now();
        let r = ping_round(&targets, &mut failures, start).await;
        let r2 = ping_round(&targets, &mut failures, start).await;
        assert!(!r.hosts[0].online);
        assert_eq!(r2.hosts[0].failures, 2);
    }
}
