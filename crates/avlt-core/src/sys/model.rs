//! Results of the system tools. Serializable for `--json`; presented by the CLI and GUI.
//!
//! The fields are the ones the original scripts show (SysInfo.ps1, Tele.ps1, Top.ps1,
//! KillP.ps1, USB.ps1, Disks.ps1, Monitors.ps1, Battery.ps1, Startup.ps1, Audio.ps1,
//! ServicesAudio.ps1, Cleanup.ps1, Backup.ps1, Search.ps1).

use std::path::PathBuf;

use serde::Serialize;

// ---------------------------------------------------------------- sys info

/// `sys info` (SysInfo.ps1, AVToolbox `SysInfo`).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SystemInfo {
    pub hostname: String,
    pub manufacturer: String,
    pub model: String,
    pub serial_number: String,
    pub bios_version: String,
    pub os: OsInfo,
    pub cpu: CpuInfo,
    pub ram: RamInfo,
    pub gpus: Vec<GpuInfo>,
    /// Local fixed volumes ("Storage Summary").
    pub volumes: Vec<Volume>,
}

/// Operating system part of [`SystemInfo`] (`Win32_OperatingSystem`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OsInfo {
    /// e.g. `Microsoft Windows 11 Pro`.
    pub caption: String,
    pub version: String,
    pub build: String,
    pub architecture: String,
    /// Local install date, `yyyy-MM-dd HH:mm`.
    pub install_date: Option<String>,
    pub uptime_s: u64,
}

/// CPU part of [`SystemInfo`] (`Win32_Processor`, first one).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CpuInfo {
    pub name: String,
    pub cores: u32,
    pub logical_processors: u32,
    /// `MaxClockSpeed` (base speed), MHz.
    pub base_mhz: u32,
}

/// RAM part of [`SystemInfo`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RamInfo {
    pub total_bytes: u64,
    /// `Win32_PhysicalMemory`, one per module.
    pub modules: Vec<RamModule>,
}

/// One memory module.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RamModule {
    pub capacity_bytes: u64,
    pub speed_mhz: Option<u32>,
    pub manufacturer: String,
}

/// One graphics adapter (`Win32_VideoController`, without Basic / Remote adapters).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GpuInfo {
    pub name: String,
    /// Registry `HardwareInformation.qwMemorySize`, else `AdapterRAM` (caps at 4 GB).
    pub vram_bytes: Option<u64>,
    pub driver_version: String,
    /// `yyyy-MM-dd`.
    pub driver_date: Option<String>,
}

/// A local fixed volume (`sys info` storage summary, `sys disks` logical volumes).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Volume {
    /// e.g. `C:`.
    pub drive: String,
    pub label: String,
    pub total_bytes: u64,
    pub free_bytes: u64,
    /// Used space in percent of the total.
    pub used_pct: f64,
}

// ---------------------------------------------------------------- telemetry / processes

/// One `sys tele` sample (Tele.ps1).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Telemetry {
    /// Local time of the sample, `HH:mm:ss`.
    pub time: String,
    pub cpu_pct: f64,
    pub ram_used_bytes: u64,
    pub ram_total_bytes: u64,
    pub ram_pct: f64,
    /// Sum of the 3D engine utilisation counters; `None` when unavailable.
    pub gpu_pct: Option<f64>,
    /// System drive (`C:`).
    pub disk_used_bytes: u64,
    pub disk_total_bytes: u64,
    pub disk_pct: f64,
    /// Adapter of the default route; `None` without network.
    pub net_adapter: Option<String>,
    pub net_down_bytes_per_s: f64,
    pub net_up_bytes_per_s: f64,
}

/// A running process (`sys top`, `sys killp`).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProcessInfo {
    /// Name without `.exe` (Get-Process `ProcessName`).
    pub name: String,
    pub pid: u32,
    /// Working set.
    pub memory_bytes: u64,
    /// CPU use over the last sampling interval, percent of the whole machine.
    pub cpu_pct: f64,
    /// In KillP.ps1's critical process list.
    pub critical: bool,
}

/// One `sys top` sample.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TopSample {
    /// Local time of the sample, `HH:mm:ss`.
    pub time: String,
    pub interval_s: f64,
    pub total_memory_bytes: u64,
    /// Highest instantaneous CPU first.
    pub processes: Vec<ProcessInfo>,
}

/// Result of killing one process.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct KillResult {
    pub name: String,
    pub pid: u32,
    pub killed: bool,
    /// Critical process left alone (no `--include-critical`).
    pub skipped: bool,
    pub error: Option<String>,
}

/// `sys killp` result.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct KillReport {
    pub pattern: String,
    pub wildcard: bool,
    pub matched: Vec<ProcessInfo>,
    pub results: Vec<KillResult>,
}

// ---------------------------------------------------------------- devices

/// A present USB device in state OK (USB.ps1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UsbDevice {
    /// PnP class (`Other` when the device has none).
    pub class: String,
    pub name: String,
    pub instance_id: String,
}

/// A physical disk (Disks.ps1 `Get-PhysicalDisk`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PhysicalDisk {
    pub model: String,
    /// `HDD`, `SSD`, `SCM` or `Unspecified`.
    pub media_type: String,
    pub size_bytes: u64,
    /// `Healthy`, `Warning`, `Unhealthy` or `Unknown`.
    pub health: String,
    /// Storage reliability counter (usually needs administrator rights).
    pub temperature_c: Option<u32>,
}

/// `sys disks` result.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DisksReport {
    pub physical: Vec<PhysicalDisk>,
    pub volumes: Vec<Volume>,
}

/// A desktop screen (Monitors.ps1 `Screen.AllScreens`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Screen {
    /// 1-based, in enumeration order.
    pub index: u32,
    /// GDI device name, e.g. `\\.\DISPLAY1`.
    pub device_name: String,
    pub primary: bool,
    pub width: u32,
    pub height: u32,
    pub x: i32,
    pub y: i32,
    pub work_width: u32,
    pub work_height: u32,
    /// Current refresh rate.
    pub refresh_hz: Option<u32>,
}

/// A monitor as WMI reports it (`WmiMonitorID`, `WmiMonitorListedSupportedSourceModes`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PhysicalMonitor {
    pub manufacturer: String,
    pub model: String,
    pub serial: String,
    /// The listed source mode with the highest refresh rate, larger resolution on a tie
    /// (Monitors.ps1 "REFRESH RATE").
    pub best_mode: Option<DisplayMode>,
}

/// `sys monitors` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MonitorsReport {
    pub screens: Vec<Screen>,
    pub physical: Vec<PhysicalMonitor>,
}

/// A display mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub struct DisplayMode {
    pub width: u32,
    pub height: u32,
    pub refresh_hz: u32,
}

/// A display adapter output that `sys monitors-arrange` can change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Display {
    /// 1-based, as `--display` takes it.
    pub index: u32,
    /// GDI device name, e.g. `\\.\DISPLAY1`.
    pub device_name: String,
    /// Monitor name, e.g. `DELL U2720Q` (or the adapter name).
    pub name: String,
    pub primary: bool,
    pub current: Option<DisplayMode>,
    /// Modes the driver offers at the current colour depth.
    pub modes: Vec<DisplayMode>,
}

/// `sys monitors-arrange` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ModeChange {
    pub device_name: String,
    pub mode: DisplayMode,
    /// Only validated with `CDS_TEST`, nothing changed.
    pub test_only: bool,
    /// The change needs a restart to take effect.
    pub restart_required: bool,
}

/// `sys battery` (Battery.ps1, AVToolbox `Battery`).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BatteryInfo {
    pub charge_pct: u32,
    pub status_code: u32,
    pub status: String,
    /// Minutes left, when Windows has an estimate.
    pub runtime_min: Option<u32>,
    pub designed_capacity_mwh: Option<u32>,
    pub full_charged_capacity_mwh: Option<u32>,
    /// Full charge capacity in percent of the designed capacity.
    pub health_pct: Option<f64>,
}

/// A startup entry (Startup.ps1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StartupEntry {
    /// `HKLM Run`, `HKCU RunOnce`, `Startup folder`...
    pub source: String,
    pub name: String,
    pub command: String,
}

/// Audio endpoint direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AudioFlow {
    Playback,
    Recording,
}

/// An active audio endpoint (Audio.ps1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AudioDevice {
    pub name: String,
    pub flow: AudioFlow,
    /// Default device of its direction (console role).
    pub default: bool,
    pub id: String,
}

/// `sys audio` result.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AudioReport {
    pub playback: Vec<AudioDevice>,
    pub recording: Vec<AudioDevice>,
    /// Master volume of the default playback device, percent.
    pub default_volume_pct: Option<f64>,
    pub default_muted: Option<bool>,
}

/// An audio-related Windows service (ServicesAudio.ps1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ServiceStatus {
    pub name: String,
    /// `Running`, `Stopped`, `StartPending`...
    pub status: String,
    pub description: String,
}

// ---------------------------------------------------------------- cleanup / backup / search

/// Outcome of emptying one folder (`sys cleanup`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct CleanStats {
    /// The folder existed.
    pub existed: bool,
    /// Bytes of the files actually deleted.
    pub freed_bytes: u64,
    pub deleted_files: u64,
    /// Files left behind (in use, locked, no permission).
    pub failed_files: u64,
}

/// What happened to one cleanup target (Cleanup.ps1 messages).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CleanStatus {
    /// Something was deleted.
    Freed,
    /// Nothing could be deleted: everything is in use or locked.
    Locked,
    /// Nothing to delete.
    AlreadyClean,
    /// Not touched: the path failed the safety check (root, profile, Windows folder...).
    Unsafe,
}

impl CleanStats {
    /// Cleanup.ps1: freed > 0, else "in use / locked (skipped)", else "already clean".
    pub fn status(&self) -> CleanStatus {
        if self.freed_bytes > 0 || self.deleted_files > 0 {
            CleanStatus::Freed
        } else if self.failed_files > 0 {
            CleanStatus::Locked
        } else {
            CleanStatus::AlreadyClean
        }
    }
}

/// One cleaned folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CleanupItem {
    pub name: String,
    pub path: PathBuf,
    pub status: CleanStatus,
    pub stats: CleanStats,
    /// Why an [`CleanStatus::Unsafe`] target was skipped.
    pub skipped_reason: Option<String>,
}

/// Size of one cleanup target before cleaning (`sys cleanup --analyze`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CleanupTargetSize {
    pub name: String,
    pub path: PathBuf,
    pub size_bytes: u64,
    pub files: u64,
}

/// Recycle Bin contents (all drives).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct RecycleBinInfo {
    pub bytes: u64,
    pub items: u64,
}

/// `sys cleanup` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CleanupReport {
    pub items: Vec<CleanupItem>,
    /// Contents before emptying it.
    pub recycle_bin: RecycleBinInfo,
    pub recycle_bin_emptied: bool,
    /// Folder bytes deleted plus the Recycle Bin size when it was emptied.
    pub total_freed_bytes: u64,
}

/// `sys cleanup --analyze` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CleanupAnalysis {
    pub targets: Vec<CleanupTargetSize>,
    pub recycle_bin: RecycleBinInfo,
}

/// `util backup` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BackupReport {
    pub source: PathBuf,
    /// The new `<name>_backup_<stamp>` folder.
    pub destination: PathBuf,
    /// The source is a single file.
    pub is_file: bool,
    /// Files copied.
    pub files: u64,
    /// Bytes copied.
    pub bytes: u64,
    /// Files that could not be copied, with the reason.
    pub failed: Vec<String>,
}

/// One `util search` match.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SearchHit {
    pub path: PathBuf,
    pub is_dir: bool,
    /// File size (0 for folders).
    pub size_bytes: u64,
    /// Last write time, seconds since the Unix epoch.
    pub modified_unix: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_status_follows_cleanup_ps1() {
        let s = |freed, deleted, failed| CleanStats {
            existed: true,
            freed_bytes: freed,
            deleted_files: deleted,
            failed_files: failed,
        };
        assert_eq!(s(10, 1, 3).status(), CleanStatus::Freed);
        assert_eq!(s(0, 1, 0).status(), CleanStatus::Freed);
        assert_eq!(s(0, 0, 2).status(), CleanStatus::Locked);
        assert_eq!(s(0, 0, 0).status(), CleanStatus::AlreadyClean);
        assert_eq!(CleanStats::default().status(), CleanStatus::AlreadyClean);
    }

    #[test]
    fn enums_serialize_lowercase() {
        assert_eq!(
            serde_json::to_string(&CleanStatus::AlreadyClean).unwrap(),
            "\"already_clean\""
        );
        assert_eq!(
            serde_json::to_string(&AudioFlow::Playback).unwrap(),
            "\"playback\""
        );
    }
}
