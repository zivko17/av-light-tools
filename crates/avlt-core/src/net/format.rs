//! Number formatting and live statistics shared by the CLI and GUI network tools.

use std::time::Duration;

use serde::Serialize;

use crate::net::model::TrafficSample;

/// Throughput of `bytes_per_sec`, in bits, as NetTools.ps1 `Format-Bps` (8-wide number).
pub fn format_bps(bytes_per_sec: f64) -> String {
    let bits = bytes_per_sec * 8.0;
    if bits > 1e9 {
        format!("{:>8.2} Gbps", bits / 1e9)
    } else if bits > 1e6 {
        format!("{:>8.2} Mbps", bits / 1e6)
    } else if bits > 1e3 {
        format!("{:>8.2} Kbps", bits / 1e3)
    } else {
        format!("{bits:>8.0}  bps")
    }
}

/// A byte total as NetTools.ps1 `Format-Bytes` (decimal units, 8-wide number).
pub fn format_bytes(bytes: u64) -> String {
    let b = bytes as f64;
    if b > 1e9 {
        format!("{:>8.2} GB", b / 1e9)
    } else if b > 1e6 {
        format!("{:>8.2} MB", b / 1e6)
    } else if b > 1e3 {
        format!("{:>8.2} KB", b / 1e3)
    } else {
        format!("{bytes:>8}  B")
    }
}

/// Link speed as `Get-NetAdapter` shows it: `1 Gbps`, `2.5 Gbps`, `100 Mbps`.
pub fn format_link_speed(bps: u64) -> String {
    let (value, unit) = match bps {
        b if b >= 1_000_000_000 => (b as f64 / 1e9, "Gbps"),
        b if b >= 1_000_000 => (b as f64 / 1e6, "Mbps"),
        b => (b as f64 / 1e3, "Kbps"),
    };
    let text = format!("{value:.1}");
    let text = text.strip_suffix(".0").unwrap_or(&text);
    format!("{text} {unit}")
}

/// Traffic-light rating used for link speed and latency colours.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LinkClass {
    /// Green.
    Fast,
    /// Yellow.
    Medium,
    /// Red.
    Slow,
}

/// MyIP.ps1 colours: 1 Gbps and above green, 100 Mbps and above yellow, slower red.
pub fn link_class(bps: u64) -> LinkClass {
    match bps {
        b if b >= 1_000_000_000 => LinkClass::Fast,
        b if b >= 100_000_000 => LinkClass::Medium,
        _ => LinkClass::Slow,
    }
}

/// NetTools.ps1 `latency` colours: under 10 ms green, under 30 ms yellow, else red.
pub fn latency_class(ms: u64) -> LinkClass {
    match ms {
        0..10 => LinkClass::Fast,
        10..30 => LinkClass::Medium,
        _ => LinkClass::Slow,
    }
}

/// Length of the `#` bar of NetTools.ps1 `latency`: one per 2 ms, at most 40.
pub fn latency_bars(ms: u64) -> usize {
    usize::try_from((ms / 2).min(40)).unwrap_or(40)
}

/// Running statistics of a ping monitor.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LatencyStats {
    pub sent: u64,
    pub received: u64,
    pub min_ms: Option<u64>,
    pub max_ms: Option<u64>,
    sum_ms: u64,
    jitter_sum_ms: u64,
    jitter_count: u64,
    last_ms: Option<u64>,
}

impl LatencyStats {
    /// Records one ping: `Some(ms)` for a reply, `None` for a timeout.
    pub fn add(&mut self, reply_ms: Option<u64>) {
        self.sent += 1;
        let Some(ms) = reply_ms else {
            return;
        };
        self.received += 1;
        self.sum_ms += ms;
        self.min_ms = Some(self.min_ms.map_or(ms, |m| m.min(ms)));
        self.max_ms = Some(self.max_ms.map_or(ms, |m| m.max(ms)));
        if let Some(last) = self.last_ms {
            self.jitter_sum_ms += last.abs_diff(ms);
            self.jitter_count += 1;
        }
        self.last_ms = Some(ms);
    }

    /// Percentage of pings without reply.
    pub fn loss_pct(&self) -> f64 {
        if self.sent == 0 {
            return 0.0;
        }
        (self.sent - self.received) as f64 * 100.0 / self.sent as f64
    }

    /// Mean round trip of the replies.
    pub fn avg_ms(&self) -> Option<f64> {
        (self.received > 0).then(|| self.sum_ms as f64 / self.received as f64)
    }

    /// Mean absolute difference between consecutive replies.
    pub fn jitter_ms(&self) -> Option<f64> {
        (self.jitter_count > 0).then(|| self.jitter_sum_ms as f64 / self.jitter_count as f64)
    }

    /// Snapshot for output.
    pub fn summary(&self) -> LatencySummary {
        LatencySummary {
            sent: self.sent,
            received: self.received,
            loss_pct: self.loss_pct(),
            min_ms: self.min_ms,
            avg_ms: self.avg_ms(),
            max_ms: self.max_ms,
            jitter_ms: self.jitter_ms(),
        }
    }
}

/// Serializable snapshot of [`LatencyStats`].
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LatencySummary {
    pub sent: u64,
    pub received: u64,
    pub loss_pct: f64,
    pub min_ms: Option<u64>,
    pub avg_ms: Option<f64>,
    pub max_ms: Option<u64>,
    pub jitter_ms: Option<f64>,
}

/// Download and upload rate in bit/s between two samples `secs` apart.
/// A counter that went backwards (adapter reset) counts as zero.
pub fn traffic_rate(prev: &TrafficSample, cur: &TrafficSample, secs: f64) -> (f64, f64) {
    if secs <= 0.0 {
        return (0.0, 0.0);
    }
    let rate = |a: u64, b: u64| b.saturating_sub(a) as f64 * 8.0 / secs;
    (
        rate(prev.rx_bytes, cur.rx_bytes),
        rate(prev.tx_bytes, cur.tx_bytes),
    )
}

/// `mm:ss`, or `h:mm:ss` from one hour.
pub fn format_elapsed(d: Duration) -> String {
    let s = d.as_secs();
    let (h, m, s) = (s / 3600, s / 60 % 60, s % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m:02}:{s:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bps_thresholds_match_nettools_ps1() {
        assert_eq!(format_bps(100.0), "     800  bps");
        assert_eq!(format_bps(125.0), "    1000  bps");
        assert_eq!(format_bps(126.0), "    1.01 Kbps");
        assert_eq!(format_bps(125_000.0), " 1000.00 Kbps");
        assert_eq!(format_bps(12_500_000.0), "  100.00 Mbps");
        assert_eq!(format_bps(250_000_000.0), "    2.00 Gbps");
        assert_eq!(format_bps(0.0), "       0  bps");
    }

    #[test]
    fn bytes_thresholds_match_nettools_ps1() {
        assert_eq!(format_bytes(999), "     999  B");
        assert_eq!(format_bytes(1000), "    1000  B");
        assert_eq!(format_bytes(1500), "    1.50 KB");
        assert_eq!(format_bytes(2_000_001), "    2.00 MB");
        assert_eq!(format_bytes(3_000_000_001), "    3.00 GB");
    }

    #[test]
    fn link_speed_text_and_class() {
        assert_eq!(format_link_speed(1_000_000_000), "1 Gbps");
        assert_eq!(format_link_speed(2_500_000_000), "2.5 Gbps");
        assert_eq!(format_link_speed(100_000_000), "100 Mbps");
        assert_eq!(format_link_speed(866_700_000), "866.7 Mbps");
        assert_eq!(format_link_speed(64_000), "64 Kbps");
        assert_eq!(link_class(10_000_000_000), LinkClass::Fast);
        assert_eq!(link_class(1_000_000_000), LinkClass::Fast);
        assert_eq!(link_class(100_000_000), LinkClass::Medium);
        assert_eq!(link_class(866_700_000), LinkClass::Medium);
        assert_eq!(link_class(10_000_000), LinkClass::Slow);
    }

    #[test]
    fn latency_graph_and_class() {
        assert_eq!(latency_bars(0), 0);
        assert_eq!(latency_bars(5), 2);
        assert_eq!(latency_bars(80), 40);
        assert_eq!(latency_bars(500), 40);
        assert_eq!(latency_class(9), LinkClass::Fast);
        assert_eq!(latency_class(10), LinkClass::Medium);
        assert_eq!(latency_class(29), LinkClass::Medium);
        assert_eq!(latency_class(30), LinkClass::Slow);
    }

    #[test]
    fn latency_stats_loss_and_jitter() {
        let mut s = LatencyStats::default();
        assert_eq!(s.loss_pct(), 0.0);
        assert_eq!(s.avg_ms(), None);
        s.add(Some(10));
        s.add(Some(20));
        s.add(None);
        s.add(Some(15));
        assert_eq!(s.sent, 4);
        assert_eq!(s.received, 3);
        assert_eq!(s.loss_pct(), 25.0);
        assert_eq!(s.min_ms, Some(10));
        assert_eq!(s.max_ms, Some(20));
        assert_eq!(s.avg_ms(), Some(15.0));
        assert_eq!(s.jitter_ms(), Some(7.5));
        let j = serde_json::to_value(s.summary()).unwrap();
        assert_eq!(j["loss_pct"], 25.0);
        assert_eq!(j["jitter_ms"], 7.5);
        assert_eq!(j["min_ms"], 10);
    }

    #[test]
    fn jitter_needs_two_replies() {
        let mut s = LatencyStats::default();
        s.add(Some(10));
        assert_eq!(s.jitter_ms(), None);
    }

    #[test]
    fn traffic_rate_per_second_in_bits() {
        let a = TrafficSample {
            rx_bytes: 1_000,
            tx_bytes: 500,
        };
        let b = TrafficSample {
            rx_bytes: 3_000,
            tx_bytes: 1_500,
        };
        assert_eq!(traffic_rate(&a, &b, 2.0), (8_000.0, 4_000.0));
        assert_eq!(traffic_rate(&b, &a, 1.0), (0.0, 0.0), "counter reset");
        assert_eq!(traffic_rate(&a, &b, 0.0), (0.0, 0.0));
    }

    #[test]
    fn elapsed_text() {
        assert_eq!(format_elapsed(Duration::from_secs(65)), "01:05");
        assert_eq!(format_elapsed(Duration::from_millis(999)), "00:00");
        assert_eq!(format_elapsed(Duration::from_secs(3_661)), "1:01:01");
    }
}
