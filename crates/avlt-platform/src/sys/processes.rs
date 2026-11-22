//! `sys top` and `sys killp` (Top.ps1, KillP.ps1, AVToolbox `Top`).

use avlt_core::sys::model::{ProcessInfo, TopSample};

use crate::Result;

/// Samples per-process CPU use between two calls (instantaneous CPU, not accumulated
/// seconds as Top.ps1 did).
pub struct TopSampler {
    inner: imp::Sampler,
}

impl TopSampler {
    /// Takes the first snapshot; the first [`sample`](Self::sample) measures from here.
    pub fn new() -> Result<TopSampler> {
        Ok(TopSampler {
            inner: imp::Sampler::new()?,
        })
    }

    /// The `count` processes with the highest CPU use since the previous call.
    pub fn sample(&mut self, count: usize) -> Result<TopSample> {
        self.inner.sample(count)
    }
}

/// Running processes whose name matches `pattern` (KillP.ps1: exact by default,
/// substring with `wildcard`), sorted by name then PID. `cpu_pct` is 0.
pub fn find_processes(pattern: &str, wildcard: bool) -> Result<Vec<ProcessInfo>> {
    imp::find_processes(pattern, wildcard)
}

/// Terminates process `pid` (Stop-Process -Force).
pub fn kill(pid: u32) -> Result<()> {
    imp::kill(pid)
}

/// Sorts by CPU (highest first; ties by memory) and keeps `count`.
#[cfg_attr(not(windows), allow(dead_code))]
fn top_n(mut procs: Vec<ProcessInfo>, count: usize) -> Vec<ProcessInfo> {
    procs.sort_by(|a, b| {
        b.cpu_pct
            .total_cmp(&a.cpu_pct)
            .then(b.memory_bytes.cmp(&a.memory_bytes))
    });
    procs.truncate(count);
    procs
}

#[cfg(windows)]
mod imp {
    use std::collections::HashMap;
    use std::time::Instant;

    use avlt_core::sys::model::{ProcessInfo, TopSample};
    use avlt_core::sys::process::{is_protected_process, process_base_name};
    use avlt_core::sys::{cpu_percent, matches_process};
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_TERMINATE, TerminateProcess};

    use super::top_n;
    use crate::{Error, Result};

    fn system() -> Result<System> {
        System::new().map_err(|e| Error::Other(format!("cannot read processes: {e}")))
    }

    fn refresh(sys: &mut System) {
        sys.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing().with_memory().with_cpu(),
        );
    }

    fn info(p: &sysinfo::Process, cpu_pct: f64) -> ProcessInfo {
        let name = process_base_name(&p.name().to_string_lossy()).to_string();
        ProcessInfo {
            critical: is_protected_process(p.pid().as_u32(), &name),
            name,
            pid: p.pid().as_u32(),
            memory_bytes: p.memory(),
            cpu_pct,
        }
    }

    pub struct Sampler {
        sys: System,
        cpu_ms: HashMap<u32, u64>,
        last: Instant,
        cores: usize,
    }

    impl Sampler {
        pub fn new() -> Result<Sampler> {
            let mut sys = system()?;
            refresh(&mut sys);
            sys.refresh_memory();
            let cores = std::thread::available_parallelism().map_or(1, |n| n.get());
            let cpu_ms = sys
                .processes()
                .values()
                .map(|p| (p.pid().as_u32(), p.accumulated_cpu_time()))
                .collect();
            Ok(Sampler {
                sys,
                cpu_ms,
                last: Instant::now(),
                cores,
            })
        }

        pub fn sample(&mut self, count: usize) -> Result<TopSample> {
            refresh(&mut self.sys);
            let now = Instant::now();
            let wall = (now - self.last).as_secs_f64();
            let mut next = HashMap::with_capacity(self.cpu_ms.len());
            let mut procs = Vec::with_capacity(self.sys.processes().len());
            for p in self.sys.processes().values() {
                let pid = p.pid().as_u32();
                if pid == 0 {
                    // System Idle Process: its "CPU" is the idle time.
                    continue;
                }
                let cpu = p.accumulated_cpu_time();
                next.insert(pid, cpu);
                // A process born during the interval is measured from its start.
                let before = self.cpu_ms.get(&pid).copied().unwrap_or(0);
                let delta_s = cpu.saturating_sub(before) as f64 / 1000.0;
                procs.push(info(p, cpu_percent(delta_s, wall, self.cores)));
            }
            self.cpu_ms = next;
            self.last = now;
            Ok(TopSample {
                time: chrono::Local::now().format("%H:%M:%S").to_string(),
                interval_s: wall,
                total_memory_bytes: self.sys.total_memory(),
                processes: top_n(procs, count),
            })
        }
    }

    pub fn find_processes(pattern: &str, wildcard: bool) -> Result<Vec<ProcessInfo>> {
        let mut sys = system()?;
        refresh(&mut sys);
        let mut found: Vec<ProcessInfo> = sys
            .processes()
            .values()
            .filter(|p| matches_process(pattern, &p.name().to_string_lossy(), wildcard))
            .map(|p| info(p, 0.0))
            .collect();
        found.sort_by(|a, b| {
            a.name
                .to_lowercase()
                .cmp(&b.name.to_lowercase())
                .then(a.pid.cmp(&b.pid))
        });
        Ok(found)
    }

    pub fn kill(pid: u32) -> Result<()> {
        let fail = |e: windows::core::Error| Error::Other(e.message().trim().to_string());
        // SAFETY: plain Win32 calls; the handle is checked and closed exactly once.
        unsafe {
            let handle = OpenProcess(PROCESS_TERMINATE, false, pid).map_err(fail)?;
            let result = TerminateProcess(handle, 1).map_err(fail);
            let _ = CloseHandle(handle);
            result
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use avlt_core::sys::model::{ProcessInfo, TopSample};

    use crate::{Error, Result};

    pub struct Sampler;

    impl Sampler {
        pub fn new() -> Result<Sampler> {
            Err(Error::Unsupported)
        }

        pub fn sample(&mut self, _count: usize) -> Result<TopSample> {
            Err(Error::Unsupported)
        }
    }

    pub fn find_processes(_pattern: &str, _wildcard: bool) -> Result<Vec<ProcessInfo>> {
        Err(Error::Unsupported)
    }

    pub fn kill(_pid: u32) -> Result<()> {
        Err(Error::Unsupported)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(name: &str, cpu: f64, mem: u64) -> ProcessInfo {
        ProcessInfo {
            name: name.into(),
            pid: 1,
            memory_bytes: mem,
            cpu_pct: cpu,
            critical: false,
        }
    }

    #[test]
    fn top_sorts_by_instant_cpu() {
        let top = top_n(
            vec![
                p("a", 1.0, 10),
                p("b", 30.0, 1),
                p("c", 1.0, 99),
                p("d", 0.0, 5),
            ],
            3,
        );
        let names: Vec<&str> = top.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["b", "c", "a"]);
    }

    #[cfg(windows)]
    #[test]
    fn finds_and_kills_a_process_we_spawned() {
        /// Kills the child on drop, so a failing assertion leaves nothing running.
        struct Guard(std::process::Child);
        impl Drop for Guard {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }

        // A private copy of a harmless program, so no other process shares its name.
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("avltkilltest.exe");
        let system = std::env::var("SystemRoot").unwrap();
        std::fs::copy(format!(r"{system}\System32\ping.exe"), &exe).unwrap();
        let mut child = Guard(
            std::process::Command::new(&exe)
                .args(["-n", "30", "127.0.0.1"])
                .stdout(std::process::Stdio::null())
                .spawn()
                .unwrap(),
        );
        let pid = child.0.id();

        let found = find_processes("AVLTKILLTEST", false).unwrap();
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].pid, pid);
        assert_eq!(found[0].name, "avltkilltest");
        assert!(!found[0].critical);
        assert!(find_processes("avltkill", false).unwrap().is_empty());
        assert_eq!(find_processes("avltkill", true).unwrap().len(), 1);

        kill(pid).unwrap();
        let status = child.0.wait().unwrap();
        assert!(!status.success());
    }

    #[cfg(windows)]
    #[test]
    fn kill_of_a_missing_pid_is_an_error() {
        // PIDs are multiples of 4; this one is never valid.
        assert!(kill(0xFFFF_FFF3).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn sampler_reports_processes() {
        let mut s = TopSampler::new().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(300));
        let sample = s.sample(5).unwrap();
        assert_eq!(sample.processes.len(), 5);
        assert!(sample.total_memory_bytes > 0);
        assert!(sample.interval_s > 0.2);
        assert!(
            sample
                .processes
                .iter()
                .all(|p| (0.0..=100.5).contains(&p.cpu_pct))
        );
        assert_eq!(sample.time.len(), 8);
    }

    #[cfg(not(windows))]
    #[test]
    fn unsupported_elsewhere() {
        assert!(matches!(TopSampler::new(), Err(crate::Error::Unsupported)));
        assert!(matches!(kill(1), Err(crate::Error::Unsupported)));
    }
}
