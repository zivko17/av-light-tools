//! Process matching for `sys killp` / `sys top` (KillP.ps1, Top.ps1, SystemTools.cs).

use crate::sys::model::ProcessInfo;

/// Processes `sys killp` flags as critical (KillP.ps1 `$critical`).
pub const CRITICAL_PROCESSES: [&str; 13] = [
    "svchost",
    "csrss",
    "wininit",
    "services",
    "lsass",
    "winlogon",
    "smss",
    "dwm",
    "explorer",
    "System",
    "Idle",
    "fontdrvhost",
    "ctfmon",
];

/// Process name without a trailing `.exe` (Get-Process `ProcessName`).
pub fn process_base_name(name: &str) -> &str {
    let n = name.len();
    if n > 4 && name.is_char_boundary(n - 4) && name[n - 4..].eq_ignore_ascii_case(".exe") {
        &name[..n - 4]
    } else {
        name
    }
}

/// `true` for the system processes of [`CRITICAL_PROCESSES`] (case-insensitive, `.exe` optional).
pub fn is_critical_process(name: &str) -> bool {
    let base = process_base_name(name);
    CRITICAL_PROCESSES
        .iter()
        .any(|c| c.eq_ignore_ascii_case(base))
}

/// KillP.ps1 matching: exact, case-insensitive name by default; substring with `wildcard`.
pub fn matches_process(pattern: &str, name: &str, wildcard: bool) -> bool {
    let pattern = process_base_name(pattern.trim()).to_lowercase();
    if pattern.is_empty() {
        return false;
    }
    let name = process_base_name(name).to_lowercase();
    if wildcard {
        name.contains(&pattern)
    } else {
        name == pattern
    }
}

/// CPU use in percent of the whole machine: `delta_proc` CPU seconds used during
/// `delta_wall` seconds on `cores` logical processors (SystemTools.cs `Top`).
pub fn cpu_percent(delta_proc: f64, delta_wall: f64, cores: usize) -> f64 {
    if cores == 0
        || delta_wall.is_nan()
        || delta_wall <= 0.0
        || delta_proc.is_nan()
        || delta_proc <= 0.0
    {
        return 0.0;
    }
    delta_proc / delta_wall / cores as f64 * 100.0
}

/// Kernel-level processes the OS reports under names KillP.ps1's list does not contain
/// (sysinfo / Toolhelp names): never offered for killing.
pub const KERNEL_PROCESSES: [&str; 5] = [
    "[System Process]",
    "System Idle Process",
    "Secure System",
    "Registry",
    "Memory Compression",
];

/// Critical by name ([`is_critical_process`], [`KERNEL_PROCESSES`]) or by PID
/// (0 = idle, 4 = System).
pub fn is_protected_process(pid: u32, name: &str) -> bool {
    pid == 0
        || pid == 4
        || is_critical_process(name)
        || KERNEL_PROCESSES
            .iter()
            .any(|k| k.eq_ignore_ascii_case(name))
}

/// `sys killp` split: `(to_kill, skipped)`. Critical processes are always skipped unless
/// `include_critical` (which the CLI only honours after an interactive answer).
pub fn kill_plan(
    matched: &[ProcessInfo],
    include_critical: bool,
) -> (Vec<&ProcessInfo>, Vec<&ProcessInfo>) {
    matched
        .iter()
        .partition(|p| include_critical || !(p.critical || is_protected_process(p.pid, &p.name)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(name: &str, pid: u32) -> ProcessInfo {
        ProcessInfo {
            name: name.into(),
            pid,
            memory_bytes: 0,
            cpu_pct: 0.0,
            critical: is_critical_process(name),
        }
    }

    #[test]
    fn critical_processes_are_skipped_by_default() {
        let procs = [
            p("svchost", 4),
            p("chrome", 8),
            p("System", 12),
            p("arena", 16),
        ];
        let (kill, skip) = kill_plan(&procs, false);
        let pids = |v: &[&ProcessInfo]| v.iter().map(|p| p.pid).collect::<Vec<_>>();
        assert_eq!(pids(&kill), [8, 16]);
        assert_eq!(pids(&skip), [4, 12]);
        let (kill, skip) = kill_plan(&procs, true);
        assert_eq!(pids(&kill), [4, 8, 12, 16]);
        assert!(skip.is_empty());
        // Kernel processes count as critical even though KillP.ps1's list lacks them.
        let kernel = [
            p("[System Process]", 0),
            p("Secure System", 332),
            p("Registry", 9),
        ];
        let (kill, skip) = kill_plan(&kernel, false);
        assert!(kill.is_empty());
        assert_eq!(pids(&skip), [0, 332, 9]);
        assert!(is_protected_process(4, "anything"));
        assert!(!is_protected_process(1234, "SystemSettings"));
        let one = [p("ctfmon", 1)];
        let (kill, skip) = kill_plan(&one, false);
        assert!(kill.is_empty());
        assert_eq!(pids(&skip), [1]);
    }

    #[test]
    fn brief_examples() {
        assert!(is_critical_process("svchost"));
        assert!(!is_critical_process("resolume"));
        assert!(matches_process("chrome", "chrome", false));
        assert!(!matches_process("svc", "svchost", false));
        assert!(matches_process("svc", "svchost", true));
        assert_eq!(cpu_percent(1.0, 2.0, 4), 12.5);
    }

    #[test]
    fn names_ignore_case_and_exe() {
        assert_eq!(process_base_name("svchost.exe"), "svchost");
        assert_eq!(process_base_name("Arena.EXE"), "Arena");
        assert_eq!(process_base_name("System"), "System");
        assert_eq!(process_base_name(".exe"), ".exe");
        assert!(is_critical_process("SVCHOST.exe"));
        assert!(is_critical_process("system"));
        assert!(is_critical_process("Explorer.EXE"));
        assert!(!is_critical_process("svchost2"));
        assert!(matches_process("Chrome", "chrome.exe", false));
        assert!(matches_process("chrome.exe", "chrome.exe", false));
        assert!(!matches_process("chrome", "chrome_proxy.exe", false));
        assert!(matches_process("CHROME", "chrome_proxy.exe", true));
        assert!(!matches_process("", "chrome.exe", true));
        assert!(!matches_process("  ", "chrome.exe", false));
    }

    #[test]
    fn cpu_percent_edges() {
        assert_eq!(cpu_percent(0.0, 2.0, 8), 0.0);
        assert_eq!(cpu_percent(2.0, 2.0, 1), 100.0);
        assert_eq!(cpu_percent(1.0, 0.0, 4), 0.0);
        assert_eq!(cpu_percent(1.0, 2.0, 0), 0.0);
        assert_eq!(cpu_percent(-1.0, 2.0, 4), 0.0);
        assert_eq!(cpu_percent(f64::NAN, 2.0, 4), 0.0);
    }
}
