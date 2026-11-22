//! Process matching for `sys killp` / `sys top` (KillP.ps1, Top.ps1, SystemTools.cs).

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

#[cfg(test)]
mod tests {
    use super::*;

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
