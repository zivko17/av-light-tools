//! `sys tele` (Tele.ps1): CPU, RAM, GPU, system disk and network rate, sampled live.

use avlt_core::sys::model::Telemetry;

use crate::Result;

/// English PDH path of Tele.ps1's GPU counter (3D engines of every process).
#[cfg_attr(not(windows), allow(dead_code))]
const GPU_COUNTER: &str = r"\GPU Engine(*engtype_3D)\Utilization Percentage";

/// Keeps the state that rates need (CPU times, previous network counters).
pub struct TelemetrySampler {
    inner: imp::Sampler,
}

impl TelemetrySampler {
    /// Takes the first readings; the first [`sample`](Self::sample) measures from here.
    pub fn new() -> Result<TelemetrySampler> {
        Ok(TelemetrySampler {
            inner: imp::Sampler::new()?,
        })
    }

    /// One sample; rates cover the time since the previous call.
    pub fn sample(&mut self) -> Result<Telemetry> {
        self.inner.sample()
    }
}

/// Bytes per second between two counter readings (0 if the counter went back).
#[cfg_attr(not(windows), allow(dead_code))]
fn rate(before: u64, after: u64, secs: f64) -> f64 {
    if secs <= 0.0 {
        return 0.0;
    }
    after.saturating_sub(before) as f64 / secs
}

/// Tele.ps1 sums the positive 3D engine utilisations; clamped to 0..100.
#[cfg_attr(not(windows), allow(dead_code))]
fn gpu_total(values: &[f64]) -> f64 {
    values
        .iter()
        .filter(|v| v.is_finite() && **v > 0.0)
        .sum::<f64>()
        .min(100.0)
}

#[cfg(windows)]
mod imp {
    use std::time::Instant;

    use avlt_core::net::model::{Adapter, TrafficSample};
    use avlt_core::sys::format::percent;
    use avlt_core::sys::model::Telemetry;
    use sysinfo::{CpuRefreshKind, MemoryRefreshKind, RefreshKind, System};
    use windows::Win32::System::Performance::{
        PDH_FMT_COUNTERVALUE_ITEM_W, PDH_FMT_DOUBLE, PDH_HCOUNTER, PDH_HQUERY, PDH_MORE_DATA,
        PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData, PdhGetFormattedCounterArrayW,
        PdhOpenQueryW,
    };
    use windows::core::HSTRING;

    use super::{GPU_COUNTER, gpu_total, rate};
    use crate::net;
    use crate::sys::disks::fixed_volumes;
    use crate::{Error, Result};

    /// A PDH query on the GPU engine counter.
    struct GpuCounter {
        query: PDH_HQUERY,
        counter: PDH_HCOUNTER,
    }

    impl GpuCounter {
        fn open() -> Option<GpuCounter> {
            let mut query = PDH_HQUERY::default();
            let mut counter = PDH_HCOUNTER::default();
            // SAFETY: out-parameters owned by this frame; the query is closed here on
            // failure, otherwise exactly once by Drop.
            unsafe {
                if PdhOpenQueryW(None, 0, &mut query) != 0 {
                    return None;
                }
                if PdhAddEnglishCounterW(query, &HSTRING::from(GPU_COUNTER), 0, &mut counter) != 0 {
                    let _ = PdhCloseQuery(query);
                    return None;
                }
                // Rate counters need a first collection to compare against.
                let _ = PdhCollectQueryData(query);
                Some(GpuCounter { query, counter })
            }
        }

        /// Collects and sums the instances; `None` if the counter cannot be read.
        fn read(&self) -> Option<f64> {
            // SAFETY: the handles are valid until Drop; the buffer is sized as PDH asks
            // (bytes, 8-byte aligned through the Vec<u64> backing store).
            unsafe {
                if PdhCollectQueryData(self.query) != 0 {
                    return None;
                }
                let (mut size, mut count) = (0u32, 0u32);
                let status = PdhGetFormattedCounterArrayW(
                    self.counter,
                    PDH_FMT_DOUBLE,
                    &mut size,
                    &mut count,
                    None,
                );
                if status != PDH_MORE_DATA {
                    // No 3D engine instances at all: nothing is rendering.
                    return (status == 0).then_some(0.0);
                }
                let mut buf = vec![0u64; (size as usize).div_ceil(8)];
                let items = buf.as_mut_ptr().cast::<PDH_FMT_COUNTERVALUE_ITEM_W>();
                if PdhGetFormattedCounterArrayW(
                    self.counter,
                    PDH_FMT_DOUBLE,
                    &mut size,
                    &mut count,
                    Some(items),
                ) != 0
                {
                    return None;
                }
                let values: Vec<f64> = std::slice::from_raw_parts(items, count as usize)
                    .iter()
                    .filter(|i| i.FmtValue.CStatus == 0)
                    .map(|i| i.FmtValue.Anonymous.doubleValue)
                    .collect();
                Some(gpu_total(&values))
            }
        }
    }

    impl Drop for GpuCounter {
        fn drop(&mut self) {
            // SAFETY: the query was opened in `open` and is closed once.
            unsafe {
                let _ = PdhCloseQuery(self.query);
            }
        }
    }

    pub struct Sampler {
        sys: System,
        gpu: Option<GpuCounter>,
        adapter: Option<Adapter>,
        traffic: Option<(TrafficSample, Instant)>,
        system_drive: String,
    }

    impl Sampler {
        pub fn new() -> Result<Sampler> {
            let mut sys = System::new_with_specifics(
                RefreshKind::nothing()
                    .with_cpu(CpuRefreshKind::nothing().with_usage())
                    .with_memory(MemoryRefreshKind::nothing().with_ram()),
            )
            .map_err(|e| Error::Other(format!("cannot read system counters: {e}")))?;
            sys.refresh_cpu_usage();
            let adapter = net::default_adapter(None).ok();
            let traffic = adapter
                .as_ref()
                .and_then(|a| net::traffic(a).ok())
                .map(|t| (t, Instant::now()));
            Ok(Sampler {
                sys,
                gpu: GpuCounter::open(),
                adapter,
                traffic,
                system_drive: std::env::var("SystemDrive")
                    .unwrap_or_else(|_| "C:".into())
                    .to_ascii_uppercase(),
            })
        }

        pub fn sample(&mut self) -> Result<Telemetry> {
            self.sys.refresh_cpu_usage();
            self.sys.refresh_memory();
            let ram_total = self.sys.total_memory();
            let ram_used = self.sys.used_memory().min(ram_total);
            let disk = fixed_volumes()
                .unwrap_or_default()
                .into_iter()
                .find(|v| v.drive == self.system_drive);
            let (disk_total, disk_used) = disk
                .as_ref()
                .map_or((0, 0), |d| (d.total_bytes, d.total_bytes - d.free_bytes));

            let (mut down, mut up) = (0.0, 0.0);
            if let Some(adapter) = &self.adapter
                && let Ok(now) = net::traffic(adapter)
            {
                let at = Instant::now();
                if let Some((prev, prev_at)) = &self.traffic {
                    let secs = (at - *prev_at).as_secs_f64();
                    down = rate(prev.rx_bytes, now.rx_bytes, secs);
                    up = rate(prev.tx_bytes, now.tx_bytes, secs);
                }
                self.traffic = Some((now, at));
            }

            Ok(Telemetry {
                time: chrono::Local::now().format("%H:%M:%S").to_string(),
                cpu_pct: f64::from(self.sys.global_cpu_usage()).clamp(0.0, 100.0),
                ram_used_bytes: ram_used,
                ram_total_bytes: ram_total,
                ram_pct: percent(ram_used, ram_total),
                gpu_pct: self.gpu.as_ref().and_then(GpuCounter::read),
                disk_used_bytes: disk_used,
                disk_total_bytes: disk_total,
                disk_pct: percent(disk_used, disk_total),
                net_adapter: self.adapter.as_ref().map(|a| a.name.clone()),
                net_down_bytes_per_s: down,
                net_up_bytes_per_s: up,
            })
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use avlt_core::sys::model::Telemetry;

    use crate::{Error, Result};

    pub struct Sampler;

    impl Sampler {
        pub fn new() -> Result<Sampler> {
            Err(Error::Unsupported)
        }

        pub fn sample(&mut self) -> Result<Telemetry> {
            Err(Error::Unsupported)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rates_and_gpu_sum() {
        assert_eq!(rate(1000, 3000, 2.0), 1000.0);
        assert_eq!(rate(3000, 1000, 2.0), 0.0);
        assert_eq!(rate(0, 10, 0.0), 0.0);
        assert_eq!(gpu_total(&[10.0, 0.0, -1.0, 5.5, f64::NAN]), 15.5);
        assert_eq!(gpu_total(&[80.0, 70.0]), 100.0);
        assert_eq!(gpu_total(&[]), 0.0);
    }

    #[cfg(windows)]
    #[test]
    fn samples_this_machine() {
        let mut s = TelemetrySampler::new().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(500));
        let t = s.sample().unwrap();
        assert!((0.0..=100.0).contains(&t.cpu_pct));
        assert!(t.ram_total_bytes > 0 && t.ram_used_bytes <= t.ram_total_bytes);
        assert!(t.disk_total_bytes > 0, "{t:?}");
        assert!(t.gpu_pct.is_none_or(|g| (0.0..=100.0).contains(&g)));
        assert!(t.net_down_bytes_per_s >= 0.0);
    }

    #[cfg(not(windows))]
    #[test]
    fn unsupported_elsewhere() {
        assert!(matches!(
            TelemetrySampler::new(),
            Err(crate::Error::Unsupported)
        ));
    }
}
