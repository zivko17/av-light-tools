//! Thresholds, bars and small conversions shared by the CLI and GUI presentation of the
//! system tools. The thresholds are the colours of the original scripts.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::sys::model::UsbDevice;

/// Traffic-light level of a value (green / yellow / red in the scripts).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Good,
    Warn,
    Bad,
}

fn above(value: f64, warn: f64, bad: f64) -> Level {
    if value > bad {
        Level::Bad
    } else if value > warn {
        Level::Warn
    } else {
        Level::Good
    }
}

fn below(value: f64, warn: f64, bad: f64) -> Level {
    if value < bad {
        Level::Bad
    } else if value < warn {
        Level::Warn
    } else {
        Level::Good
    }
}

/// CPU, RAM and GPU load (Tele.ps1): above 80 % red, above 50 % yellow.
pub fn load_level(pct: f64) -> Level {
    above(pct, 50.0, 80.0)
}

/// Disk usage (Tele.ps1, Disks.ps1): above 85 % red, above 70 % yellow.
pub fn disk_level(pct: f64) -> Level {
    above(pct, 70.0, 85.0)
}

/// Share of total RAM used by one process (Top.ps1): above 10 % red, above 5 % yellow.
pub fn process_memory_level(pct_of_total: f64) -> Level {
    above(pct_of_total, 5.0, 10.0)
}

/// Battery charge (Battery.ps1): below 20 % red, below 50 % yellow.
pub fn charge_level(pct: f64) -> Level {
    below(pct, 50.0, 20.0)
}

/// Battery health (Battery.ps1): below 70 % red, below 85 % yellow.
pub fn battery_health_level(pct: f64) -> Level {
    below(pct, 85.0, 70.0)
}

/// Timer countdown (Timer.ps1): under 10 s red, under 60 s yellow.
pub fn countdown_level(remaining_s: f64) -> Level {
    below(remaining_s, 60.0, 10.0)
}

/// Disk health colour of Disks.ps1.
pub fn health_level(health: &str) -> Option<Level> {
    match health {
        "Healthy" => Some(Level::Good),
        "Warning" => Some(Level::Warn),
        "Unhealthy" => Some(Level::Bad),
        _ => None,
    }
}

/// 20-character bar of Tele.ps1 / Battery.ps1: one `#` per 5 %, clamped to 0..100.
pub fn bar(pct: f64) -> String {
    let pct = if pct.is_nan() {
        0.0
    } else {
        pct.clamp(0.0, 100.0)
    };
    let filled = (pct / 5.0).floor() as usize;
    format!("[{}{}]", "#".repeat(filled), ".".repeat(20 - filled))
}

/// `part` in percent of `total` (0 when `total` is 0).
pub fn percent(part: u64, total: u64) -> f64 {
    if total == 0 {
        0.0
    } else {
        part as f64 / total as f64 * 100.0
    }
}

/// Rounds to `decimals` places, as `[math]::Round` does for display.
pub fn round_to(value: f64, decimals: i32) -> f64 {
    let f = 10f64.powi(decimals);
    (value * f).round() / f
}

/// Bytes in GiB (the scripts' `/ 1GB`).
pub fn gib(bytes: u64) -> f64 {
    bytes as f64 / (1u64 << 30) as f64
}

/// Bytes in MiB (the scripts' `/ 1MB`).
pub fn mib(bytes: u64) -> f64 {
    bytes as f64 / (1u64 << 20) as f64
}

/// Cleanup.ps1 total: `N GB` from 1 GB up (2 decimals), else `N MB` (1 decimal).
pub fn freed_text(bytes: u64) -> String {
    let gb = round_to(gib(bytes), 2);
    if gb >= 1.0 {
        format!("{gb} GB")
    } else {
        format!("{} MB", round_to(mib(bytes), 1))
    }
}

/// `s` cut to `max` characters plus `ellipsis` when longer (Top.ps1, Disks.ps1, Startup.ps1).
pub fn truncate(s: &str, max: usize, ellipsis: &str) -> String {
    if s.chars().count() > max {
        let cut: String = s.chars().take(max).collect();
        format!("{cut}{ellipsis}")
    } else {
        s.to_string()
    }
}

/// `Nd Nh Nm` uptime of SysInfo.ps1.
pub fn uptime_text(secs: u64) -> String {
    format!(
        "{}d {}h {}m",
        secs / 86_400,
        secs % 86_400 / 3600,
        secs % 3600 / 60
    )
}

/// Text of `Win32_Battery.BatteryStatus` (Battery.ps1 `$statusMap`).
pub fn battery_status_text(code: u32) -> String {
    let text = match code {
        1 => "Discharging",
        2 => "Plugged in (AC)",
        3 => "Fully charged",
        4 => "Low",
        5 => "Critical",
        6 => "Charging",
        7 => "Charging and High",
        8 => "Charging and Low",
        9 => "Charging and Critical",
        10 => "Undefined",
        11 => "Partially Charged",
        _ => return format!("Unknown ({code})"),
    };
    text.to_string()
}

/// `EstimatedRunTime` in minutes when it is a real estimate: Windows reports 71582788
/// (0x4444444) while charging or calculating (Battery.ps1).
pub fn battery_runtime(raw: u32) -> Option<u32> {
    (raw > 0 && raw < 71_582_788).then_some(raw)
}

/// Full charge capacity in percent of the designed one, one decimal (Battery.ps1).
pub fn battery_health(designed_mwh: u32, full_mwh: u32) -> Option<f64> {
    (designed_mwh > 0).then(|| round_to(f64::from(full_mwh) / f64::from(designed_mwh) * 100.0, 1))
}

/// Characters of a WMI `uint16[]` string (`WmiMonitorID`), NUL padding dropped.
pub fn wmi_chars(codes: &[u16]) -> String {
    let codes: Vec<u16> = codes.iter().copied().filter(|&c| c != 0).collect();
    String::from_utf16_lossy(&codes).trim().to_string()
}

/// Name of a service state as `Get-Service` prints it (`SERVICE_STATUS.dwCurrentState`).
pub fn service_state_name(state: u32) -> &'static str {
    match state {
        1 => "Stopped",
        2 => "StartPending",
        3 => "StopPending",
        4 => "Running",
        5 => "ContinuePending",
        6 => "PausePending",
        7 => "Paused",
        _ => "Unknown",
    }
}

/// USB.ps1 grouping: by class (`Other` when empty), classes and names sorted.
pub fn group_usb(devices: &[UsbDevice]) -> Vec<(String, Vec<String>)> {
    let mut groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for d in devices {
        let class = if d.class.trim().is_empty() {
            "Other".to_string()
        } else {
            d.class.clone()
        };
        let name = if d.name.trim().is_empty() {
            "(no name)".to_string()
        } else {
            d.name.clone()
        };
        groups.entry(class).or_default().push(name);
    }
    groups
        .into_iter()
        .map(|(class, mut names)| {
            names.sort_by_key(|n| n.to_lowercase());
            (class, names)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_follow_the_scripts() {
        assert_eq!(load_level(50.0), Level::Good);
        assert_eq!(load_level(50.1), Level::Warn);
        assert_eq!(load_level(80.0), Level::Warn);
        assert_eq!(load_level(80.1), Level::Bad);
        assert_eq!(disk_level(70.0), Level::Good);
        assert_eq!(disk_level(71.0), Level::Warn);
        assert_eq!(disk_level(86.0), Level::Bad);
        assert_eq!(process_memory_level(5.0), Level::Good);
        assert_eq!(process_memory_level(6.0), Level::Warn);
        assert_eq!(process_memory_level(11.0), Level::Bad);
        assert_eq!(charge_level(19.0), Level::Bad);
        assert_eq!(charge_level(20.0), Level::Warn);
        assert_eq!(charge_level(50.0), Level::Good);
        assert_eq!(battery_health_level(69.9), Level::Bad);
        assert_eq!(battery_health_level(84.9), Level::Warn);
        assert_eq!(battery_health_level(85.0), Level::Good);
        assert_eq!(countdown_level(9.0), Level::Bad);
        assert_eq!(countdown_level(59.0), Level::Warn);
        assert_eq!(countdown_level(60.0), Level::Good);
        assert_eq!(health_level("Healthy"), Some(Level::Good));
        assert_eq!(health_level("Unhealthy"), Some(Level::Bad));
        assert_eq!(health_level("Unknown"), None);
    }

    #[test]
    fn bar_is_twenty_wide_and_clamped() {
        assert_eq!(bar(0.0), format!("[{}]", ".".repeat(20)));
        assert_eq!(bar(54.9), format!("[{}{}]", "#".repeat(10), ".".repeat(10)));
        assert_eq!(bar(100.0), format!("[{}]", "#".repeat(20)));
        assert_eq!(bar(250.0), bar(100.0));
        assert_eq!(bar(-3.0), bar(0.0));
        assert_eq!(bar(f64::NAN), bar(0.0));
    }

    #[test]
    fn numbers() {
        assert_eq!(percent(1, 4), 25.0);
        assert_eq!(percent(1, 0), 0.0);
        assert_eq!(round_to(12.345, 1), 12.3);
        assert_eq!(round_to(12.36, 1), 12.4);
        assert_eq!(gib(3 << 30), 3.0);
        assert_eq!(mib(1 << 20), 1.0);
        assert_eq!(freed_text(0), "0 MB");
        assert_eq!(freed_text(1_572_864), "1.5 MB");
        assert_eq!(freed_text(3 << 29), "1.5 GB");
        assert_eq!(uptime_text(90_061), "1d 1h 1m");
    }

    #[test]
    fn truncation_is_char_safe() {
        assert_eq!(truncate("short", 28, ".."), "short");
        assert_eq!(truncate("abcdef", 3, ".."), "abc..");
        assert_eq!(truncate("ñandú música", 5, "..."), "ñandú...");
    }

    #[test]
    fn battery_helpers() {
        assert_eq!(battery_status_text(2), "Plugged in (AC)");
        assert_eq!(battery_status_text(11), "Partially Charged");
        assert_eq!(battery_status_text(42), "Unknown (42)");
        assert_eq!(battery_runtime(0), None);
        assert_eq!(battery_runtime(71_582_788), None);
        assert_eq!(battery_runtime(95), Some(95));
        assert_eq!(battery_health(50_000, 42_500), Some(85.0));
        assert_eq!(battery_health(0, 42_500), None);
    }

    #[test]
    fn wmi_strings_drop_padding() {
        let codes: Vec<u16> = "DEL".encode_utf16().chain([0, 0, 0]).collect();
        assert_eq!(wmi_chars(&codes), "DEL");
        assert_eq!(wmi_chars(&[0, 0]), "");
    }

    #[test]
    fn service_states() {
        assert_eq!(service_state_name(4), "Running");
        assert_eq!(service_state_name(1), "Stopped");
        assert_eq!(service_state_name(99), "Unknown");
    }

    #[test]
    fn usb_grouping() {
        let d = |class: &str, name: &str| UsbDevice {
            class: class.into(),
            name: name.into(),
            instance_id: String::new(),
        };
        let groups = group_usb(&[
            d("USB", "Root hub"),
            d("", "Mystery"),
            d("HIDClass", "keyboard"),
            d("HIDClass", "Audio control"),
            d("MEDIA", ""),
        ]);
        let classes: Vec<&str> = groups.iter().map(|(c, _)| c.as_str()).collect();
        assert_eq!(classes, ["HIDClass", "MEDIA", "Other", "USB"]);
        assert_eq!(groups[0].1, ["Audio control", "keyboard"]);
        assert_eq!(groups[1].1, ["(no name)"]);
    }
}
