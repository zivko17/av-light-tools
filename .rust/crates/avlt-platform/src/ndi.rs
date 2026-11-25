//! NDI Tools: install detection, app launch, network / service diagnostics and the Access
//! Manager configuration. Port of `NDITools.ps1` (`Resolve-NDIBase`, `Invoke-NDILaunch`,
//! `Test-NDINetwork`, `Get-NDIServices`, `Get-NDIConfig`) and `AVToolbox/NdiTools.cs`.
//!
//! The interactive `ndi` menu is not ported; the GUI covers it.

use std::path::{Path, PathBuf};
use std::str::FromStr;

use avlt_core::net::model::{Adapter, AdapterKind};
use serde::Serialize;

use crate::{Error, Result};

/// Folder NDI Tools installs into, under `Program Files`.
const NDI_FOLDER: &str = "NDI";
/// Used when no `NDI N Tools` folder exists (`Resolve-NDIBase` fallback).
const FALLBACK_TOOLS_FOLDER: &str = "NDI 6 Tools";
/// TCP ports NDI streams use (`Test-NDINetwork`).
pub const NDI_PORT_RANGE: std::ops::RangeInclusive<u16> = 5960..=5990;
/// mDNS discovery port.
pub const MDNS_PORT: u16 = 5353;

/// An app of the NDI Tools suite that `avlt ndi launch` starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NdiApp {
    Monitor,
    Router,
    Test,
    Bridge,
    Manager,
    Screen,
    Launcher,
}

impl NdiApp {
    /// Every app, in the order `ndi-launch help` listed them.
    pub const ALL: [NdiApp; 7] = [
        NdiApp::Monitor,
        NdiApp::Router,
        NdiApp::Test,
        NdiApp::Bridge,
        NdiApp::Manager,
        NdiApp::Screen,
        NdiApp::Launcher,
    ];

    /// The alias used on the command line.
    pub fn name(self) -> &'static str {
        match self {
            NdiApp::Monitor => "monitor",
            NdiApp::Router => "router",
            NdiApp::Test => "test",
            NdiApp::Bridge => "bridge",
            NdiApp::Manager => "manager",
            NdiApp::Screen => "screen",
            NdiApp::Launcher => "launcher",
        }
    }

    /// What the app is for.
    pub fn description(self) -> &'static str {
        match self {
            NdiApp::Monitor => "Studio Monitor (view NDI sources)",
            NdiApp::Router => "NDI Router (route sources)",
            NdiApp::Test => "Test Patterns (generate test signal)",
            NdiApp::Bridge => "NDI Bridge (LAN to WAN tunneling)",
            NdiApp::Manager => "Access Manager (groups + remote servers)",
            NdiApp::Screen => "Screen Capture (desktop to NDI)",
            NdiApp::Launcher => "NDI Launcher (main hub)",
        }
    }

    /// Executable path below the `NDI N Tools` folder, one entry per path segment.
    fn segments(self) -> &'static [&'static str] {
        match self {
            NdiApp::Monitor => &[
                "Studio Monitor",
                "Application.Network.StudioMonitor.x64.exe",
            ],
            NdiApp::Router => &["Router", "Application.NDI.Router.exe"],
            NdiApp::Test => &["Test Patterns", "Application.Network.TestPatterns.exe"],
            NdiApp::Bridge => &["Bridge", "Application.NDI.Bridge.UI.exe"],
            NdiApp::Manager => &["Access Manager", "Application.NdiGroupEditor.exe"],
            NdiApp::Screen => &[
                "Screen Capture",
                "Application.Network.ScanConverter2.x64.exe",
            ],
            NdiApp::Launcher => &["NDI Launcher.exe"],
        }
    }

    /// Full path of the app's executable under `base` (an `NDI N Tools` folder).
    pub fn exe_path(self, base: &Path) -> PathBuf {
        self.segments()
            .iter()
            .fold(base.to_path_buf(), |p, s| p.join(s))
    }
}

impl FromStr for NdiApp {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        let key = s.trim().to_ascii_lowercase();
        NdiApp::ALL
            .into_iter()
            .find(|a| a.name() == key)
            .ok_or_else(|| {
                let all: Vec<&str> = NdiApp::ALL.iter().map(|a| a.name()).collect();
                Error::InvalidInput(format!(
                    "unknown NDI app '{s}' (expected one of: {})",
                    all.join(", ")
                ))
            })
    }
}

/// The NDI Tools installation `avlt` found (or the default location when there is none).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NdiInstall {
    /// The `NDI N Tools` folder.
    pub base: PathBuf,
    /// A matching folder exists (false: `base` is the `NDI 6 Tools` fallback).
    pub installed: bool,
    /// The N of `NDI N Tools`.
    pub version: Option<u32>,
}

/// The number in a `NDI N Tools` folder name, or `None` if the name does not have that shape.
fn tools_folder_version(name: &str) -> Option<u32> {
    let lower = name.to_ascii_lowercase();
    if !(lower.starts_with("ndi ") && lower.ends_with(" tools") && lower.len() > "ndi  tools".len())
    {
        return None;
    }
    // `[int]($_.Name -replace '\D','')`: every digit of the name, as one number.
    let digits: String = name.chars().filter(char::is_ascii_digit).collect();
    Some(digits.parse().unwrap_or(0))
}

/// The newest `NDI * Tools` folder name (highest number) among `names`.
pub fn newest_tools_folder<'a>(names: impl IntoIterator<Item = &'a str>) -> Option<&'a str> {
    names
        .into_iter()
        .filter_map(|n| tools_folder_version(n).map(|v| (v, n)))
        .reduce(|best, cur| if cur.0 > best.0 { cur } else { best })
        .map(|(_, n)| n)
}

/// Environment variable that replaces the folder holding the `NDI N Tools` folders
/// (the `$NDIBase` override of `NDITools.ps1`); used by the tests and for portable installs.
pub const ROOT_ENV: &str = "AVLT_NDI_ROOT";

/// The folder holding the `NDI N Tools` folders: `%AVLT_NDI_ROOT%`, else `Program Files\NDI`.
fn ndi_root() -> PathBuf {
    if let Some(root) = std::env::var_os(ROOT_ENV).filter(|v| !v.is_empty()) {
        return PathBuf::from(root);
    }
    let pf = std::env::var_os("ProgramFiles")
        .filter(|v| !v.is_empty())
        .map_or_else(|| PathBuf::from(r"C:\Program Files"), PathBuf::from);
    pf.join(NDI_FOLDER)
}

/// Finds the newest `NDI N Tools` folder under `root`; falls back to `NDI 6 Tools`.
pub fn install_in(root: &Path) -> NdiInstall {
    let names: Vec<String> = std::fs::read_dir(root)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    match newest_tools_folder(names.iter().map(String::as_str)) {
        Some(name) => NdiInstall {
            base: root.join(name),
            installed: true,
            version: tools_folder_version(name),
        },
        None => NdiInstall {
            base: root.join(FALLBACK_TOOLS_FOLDER),
            installed: false,
            version: None,
        },
    }
}

/// The NDI Tools installation on this PC (`Resolve-NDIBase`).
pub fn install() -> NdiInstall {
    install_in(&ndi_root())
}

/// Starts `app` and returns its executable path. The app keeps running on its own and
/// does not inherit avlt's stdout, so `avlt ndi launch monitor | ...` does not wait for it.
///
/// Missing install: [`Error::MissingTool`] (exit 3). Elsewhere than Windows: `Unsupported`.
pub fn launch(app: NdiApp) -> Result<PathBuf> {
    if !cfg!(windows) {
        return Err(Error::Unsupported);
    }
    let exe = app.exe_path(&install().base);
    if !exe.is_file() {
        return Err(Error::MissingTool {
            tool: exe.display().to_string(),
            hint: "NDI Tools from https://ndi.video/tools/".into(),
        });
    }
    imp::shell_open(&exe)?;
    Ok(exe)
}

/// How good a link is for NDI Full Bandwidth.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LinkGrade {
    /// 1 Gbps or more.
    Good,
    /// Between 100 Mbps and 1 Gbps, or unknown.
    Marginal,
    /// 100 Mbps or less: not enough for NDI Full Bandwidth.
    Poor,
}

/// Grades a link speed in bit/s (`Test-NDINetwork`: green >= 1 Gbps, red at 100 Mbps).
pub fn link_grade(bps: Option<u64>) -> LinkGrade {
    match bps {
        Some(b) if b >= 1_000_000_000 => LinkGrade::Good,
        Some(b) if b <= 100_000_000 => LinkGrade::Poor,
        _ => LinkGrade::Marginal,
    }
}

/// An active adapter and its link speed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NdiAdapter {
    pub name: String,
    pub link_mbps: Option<u64>,
    pub mac: Option<String>,
    pub grade: LinkGrade,
}

/// Up adapters that are not loopback or virtual (`Where-Object { Up -and not Loopback|Virtual }`).
pub fn is_ndi_candidate(a: &Adapter) -> bool {
    let desc = a.description.to_ascii_lowercase();
    a.up && a.kind != AdapterKind::Loopback
        && !desc.contains("loopback")
        && !desc.contains("virtual")
}

/// Windows Firewall rules that mention NDI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct NdiFirewall {
    pub allow: usize,
    pub block: usize,
}

/// A TCP port in the NDI range with a listener.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NdiPort {
    pub port: u16,
    pub process: String,
}

/// Result of `avlt ndi check`. A section whose query failed is `None` and explained in
/// `notes`, so one failing query never hides the rest.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NdiCheck {
    pub install: NdiInstall,
    pub adapters: Vec<NdiAdapter>,
    /// State of the `Dnscache` service (`Running`, `Stopped`...).
    pub dns_client: Option<String>,
    /// Something listens on UDP 5353.
    pub mdns_listening: Option<bool>,
    pub firewall: Option<NdiFirewall>,
    /// TCP listeners in 5960-5990.
    pub ports: Option<Vec<NdiPort>>,
    /// Routes to 224.0.0.0/4.
    pub multicast_routes: Option<usize>,
    pub notes: Vec<String>,
}

/// Network diagnostic for NDI: link speed, mDNS, firewall rules, NDI ports and multicast.
pub fn check() -> Result<NdiCheck> {
    imp::check()
}

/// An NDI Windows service.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NdiService {
    pub display_name: String,
    pub status: String,
    pub running: bool,
}

/// A running NDI process.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NdiProcess {
    pub name: String,
    pub pid: u32,
    pub memory_mb: f64,
}

/// Result of `avlt ndi services`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NdiServices {
    pub services: Vec<NdiService>,
    pub processes: Vec<NdiProcess>,
}

/// Service display names `Get-NDIServices` lists: contains "NDI" and starts with "NDI " or
/// mentions Newtek (case-insensitive).
pub fn is_ndi_service(display_name: &str) -> bool {
    let n = display_name.to_ascii_lowercase();
    n.contains("ndi") && (n.starts_with("ndi ") || n.contains("newtek"))
}

/// Process names `Get-NDIServices` lists. `NDI` already matches Router and Bridge
/// (`Application.NDI.*`), so those generic words are not tokens.
pub fn is_ndi_process(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    [
        "ndi",
        "studiomonitor",
        "scanconverter",
        "testpatterns",
        "groupeditor",
    ]
    .iter()
    .any(|t| n.contains(t))
}

/// NDI services and running NDI processes.
pub fn services() -> Result<NdiServices> {
    imp::services()
}

/// What `avlt ndi config` found in the Access Manager configuration.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NdiConfig {
    /// `%APPDATA%\NDI\ndi-config.v1.json`.
    pub path: PathBuf,
    pub found: bool,
    /// `ndi.groups.send`.
    pub groups_send: Vec<String>,
    /// `ndi.groups.recv`.
    pub groups_recv: Vec<String>,
    /// `ndi.networks.discovery`.
    pub discovery: Vec<String>,
    /// `ndi.networks.ips` (manual sources).
    pub ips: Vec<String>,
    /// The whole file.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw: Option<serde_json::Value>,
}

/// A config field that is a comma-separated string or a JSON array, as a list of trimmed,
/// non-empty entries (`ConvertTo-NDIList`).
pub fn ndi_list(value: Option<&serde_json::Value>) -> Vec<String> {
    use serde_json::Value;
    let items: Vec<String> = match value {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(a)) => a
            .iter()
            .map(|v| match v {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            })
            .collect(),
        Some(Value::String(s)) => s.split(',').map(str::to_string).collect(),
        Some(other) => vec![other.to_string()],
    };
    items
        .into_iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Reads and interprets the config file at `path`. A missing file is `found: false`;
/// unreadable or invalid JSON is an error.
pub fn read_config(path: &Path) -> Result<NdiConfig> {
    let mut cfg = NdiConfig {
        path: path.to_path_buf(),
        found: false,
        groups_send: Vec::new(),
        groups_recv: Vec::new(),
        discovery: Vec::new(),
        ips: Vec::new(),
        raw: None,
    };
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(cfg),
        Err(e) => return Err(e.into()),
    };
    let text = String::from_utf8_lossy(&bytes);
    let text = text.trim_start_matches('\u{feff}');
    let json: serde_json::Value = serde_json::from_str(text)
        .map_err(|e| Error::Other(format!("cannot parse {}: {e}", path.display())))?;
    let at = |a: &str, b: &str, c: &str| json.get(a).and_then(|v| v.get(b)).and_then(|v| v.get(c));
    cfg.found = true;
    cfg.groups_send = ndi_list(at("ndi", "groups", "send"));
    cfg.groups_recv = ndi_list(at("ndi", "groups", "recv"));
    cfg.discovery = ndi_list(at("ndi", "networks", "discovery"));
    cfg.ips = ndi_list(at("ndi", "networks", "ips"));
    cfg.raw = Some(json);
    Ok(cfg)
}

/// The Access Manager configuration (`%APPDATA%\NDI\ndi-config.v1.json`).
pub fn config() -> Result<NdiConfig> {
    let appdata = std::env::var_os("APPDATA")
        .filter(|v| !v.is_empty())
        .ok_or(Error::Unsupported)?;
    read_config(
        &PathBuf::from(appdata)
            .join("NDI")
            .join("ndi-config.v1.json"),
    )
}

#[cfg(windows)]
mod imp {
    use std::collections::{BTreeSet, HashMap};

    use serde::Deserialize;
    use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System};

    use super::*;
    use crate::net::adapters::adapters;

    fn wmi(namespace: &str) -> Result<::wmi::WMIConnection> {
        crate::wmi::connect(namespace)
    }

    fn query<T: serde::de::DeserializeOwned>(namespace: &str, wql: &str) -> Result<Vec<T>> {
        wmi(namespace)?
            .raw_query(wql)
            .map_err(|e| Error::Other(format!("WMI query failed ({wql}): {e}")))
    }

    const CIMV2: &str = "ROOT\\CIMV2";
    const NETWORK: &str = "ROOT\\StandardCimv2";

    #[derive(Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct ServiceRow {
        name: Option<String>,
        display_name: Option<String>,
        state: Option<String>,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct PortRow {
        local_port: u32,
        owning_process: u32,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct FirewallRow {
        display_name: Option<String>,
        enabled: Option<u32>,
        action: Option<u32>,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct RouteRow {
        #[allow(dead_code)]
        destination_prefix: Option<String>,
    }

    fn process_names() -> HashMap<u32, String> {
        let Ok(mut sys) = System::new_with_specifics(RefreshKind::nothing()) else {
            return HashMap::new();
        };
        sys.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing(),
        );
        sys.processes()
            .iter()
            .map(|(pid, p): (&Pid, _)| {
                let name = p.name().to_string_lossy().into_owned();
                let name = name
                    .strip_suffix(".exe")
                    .or_else(|| name.strip_suffix(".EXE"))
                    .map_or(name.clone(), str::to_string);
                (pid.as_u32(), name)
            })
            .collect()
    }

    fn service_state(name: &str) -> Result<Option<String>> {
        let rows: Vec<ServiceRow> = query(
            CIMV2,
            &format!("SELECT Name, DisplayName, State FROM Win32_Service WHERE Name = '{name}'"),
        )?;
        Ok(rows.into_iter().next().and_then(|r| r.state))
    }

    fn mdns_listening() -> Result<bool> {
        let rows: Vec<PortRow> = query(
            NETWORK,
            &format!(
                "SELECT LocalPort, OwningProcess FROM MSFT_NetUDPEndpoint WHERE LocalPort = {MDNS_PORT}"
            ),
        )?;
        Ok(!rows.is_empty())
    }

    fn firewall() -> Result<NdiFirewall> {
        let rows: Vec<FirewallRow> = query(
            NETWORK,
            "SELECT DisplayName, Enabled, Action FROM MSFT_NetFirewallRule",
        )?;
        let ndi = rows.iter().filter(|r| {
            r.enabled == Some(1)
                && r.display_name
                    .as_deref()
                    .is_some_and(|n| n.to_ascii_lowercase().contains("ndi"))
        });
        let (mut allow, mut block) = (0, 0);
        for r in ndi {
            match r.action {
                Some(2) => allow += 1,
                Some(4) => block += 1,
                _ => {}
            }
        }
        Ok(NdiFirewall { allow, block })
    }

    fn ports() -> Result<Vec<NdiPort>> {
        let (lo, hi) = (NDI_PORT_RANGE.start(), NDI_PORT_RANGE.end());
        let rows: Vec<PortRow> = query(
            NETWORK,
            &format!(
                "SELECT LocalPort, OwningProcess FROM MSFT_NetTCPConnection WHERE State = 2 AND LocalPort >= {lo} AND LocalPort <= {hi}"
            ),
        )?;
        let names = process_names();
        let unique: BTreeSet<(u16, u32)> = rows
            .into_iter()
            .filter_map(|r| Some((u16::try_from(r.local_port).ok()?, r.owning_process)))
            .collect();
        Ok(unique
            .into_iter()
            .map(|(port, pid)| NdiPort {
                port,
                process: names
                    .get(&pid)
                    .cloned()
                    .unwrap_or_else(|| format!("pid {pid}")),
            })
            .collect())
    }

    fn multicast_routes() -> Result<usize> {
        let rows: Vec<RouteRow> = query(
            NETWORK,
            "SELECT DestinationPrefix FROM MSFT_NetRoute WHERE DestinationPrefix = '224.0.0.0/4'",
        )?;
        Ok(rows.len())
    }

    /// `Some(value)`, or `None` with the failure recorded in `notes`.
    fn section<T>(notes: &mut Vec<String>, what: &str, r: Result<T>) -> Option<T> {
        match r {
            Ok(v) => Some(v),
            Err(e) => {
                notes.push(format!("{what}: {e}"));
                None
            }
        }
    }

    /// Starts `exe` like `Start-Process` (ShellExecute): a new process that inherits none
    /// of avlt's handles, so a pipe on avlt's stdout is not held open by the app.
    pub fn shell_open(exe: &Path) -> Result<()> {
        use std::os::windows::ffi::OsStrExt;

        use windows::Win32::UI::Shell::ShellExecuteW;
        use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
        use windows::core::{PCWSTR, w};

        let file: Vec<u16> = exe.as_os_str().encode_wide().chain(Some(0)).collect();
        // SAFETY: `file` is a NUL-terminated UTF-16 string that outlives the call; the
        // other string arguments are literals or null, and no window handle is passed.
        let result = unsafe {
            ShellExecuteW(
                None,
                w!("open"),
                PCWSTR(file.as_ptr()),
                PCWSTR::null(),
                PCWSTR::null(),
                SW_SHOWNORMAL,
            )
        };
        // ShellExecute reports success with a value greater than 32.
        if result.0 as usize > 32 {
            Ok(())
        } else {
            Err(Error::Other(format!(
                "cannot start {} (ShellExecute error {})",
                exe.display(),
                result.0 as usize
            )))
        }
    }

    pub fn check() -> Result<NdiCheck> {
        let mut notes = Vec::new();
        let adapters = section(&mut notes, "network adapters", adapters())
            .unwrap_or_default()
            .iter()
            .filter(|a| is_ndi_candidate(a))
            .map(|a| NdiAdapter {
                name: a.name.clone(),
                link_mbps: a.link_speed_bps.map(|b| b / 1_000_000),
                mac: a.mac.clone(),
                grade: link_grade(a.link_speed_bps),
            })
            .collect();
        Ok(NdiCheck {
            install: install(),
            adapters,
            dns_client: section(&mut notes, "DNS Client service", service_state("Dnscache"))
                .flatten(),
            mdns_listening: section(&mut notes, "UDP 5353 listeners", mdns_listening()),
            firewall: section(&mut notes, "firewall rules", firewall()),
            ports: section(&mut notes, "NDI ports", ports()),
            multicast_routes: section(&mut notes, "multicast routes", multicast_routes()),
            notes,
        })
    }

    pub fn services() -> Result<NdiServices> {
        let rows: Vec<ServiceRow> =
            query(CIMV2, "SELECT Name, DisplayName, State FROM Win32_Service")?;
        let mut services: Vec<NdiService> = rows
            .into_iter()
            .filter_map(|r| {
                let display_name = r.display_name.or(r.name)?;
                let status = r.state.unwrap_or_default();
                is_ndi_service(&display_name).then(|| NdiService {
                    running: status.eq_ignore_ascii_case("running"),
                    display_name,
                    status,
                })
            })
            .collect();
        services.sort_by_key(|s| s.display_name.to_lowercase());

        let mut sys = System::new_with_specifics(RefreshKind::nothing())
            .map_err(|e| Error::Other(format!("cannot read processes: {e}")))?;
        sys.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing().with_memory(),
        );
        let names = process_names();
        let mut processes: Vec<NdiProcess> = sys
            .processes()
            .iter()
            .filter_map(|(pid, p)| {
                let name = names.get(&pid.as_u32())?.clone();
                is_ndi_process(&name).then(|| NdiProcess {
                    name,
                    pid: pid.as_u32(),
                    memory_mb: (p.memory() as f64 / (1024.0 * 1024.0) * 10.0).round() / 10.0,
                })
            })
            .collect();
        processes.sort_by_key(|p| (p.name.to_lowercase(), p.pid));
        Ok(NdiServices {
            services,
            processes,
        })
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;

    pub fn shell_open(_exe: &Path) -> Result<()> {
        Err(Error::Unsupported)
    }

    pub fn check() -> Result<NdiCheck> {
        Err(Error::Unsupported)
    }

    pub fn services() -> Result<NdiServices> {
        Err(Error::Unsupported)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn aliases_and_paths_match_invoke_ndilaunch() {
        let base = Path::new("T");
        let rel = |a: NdiApp| {
            a.exe_path(base)
                .strip_prefix(base)
                .unwrap()
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/")
        };
        assert_eq!(
            NdiApp::ALL.map(|a| (a.name(), rel(a))),
            [
                (
                    "monitor",
                    "Studio Monitor/Application.Network.StudioMonitor.x64.exe".to_string()
                ),
                ("router", "Router/Application.NDI.Router.exe".to_string()),
                (
                    "test",
                    "Test Patterns/Application.Network.TestPatterns.exe".to_string()
                ),
                ("bridge", "Bridge/Application.NDI.Bridge.UI.exe".to_string()),
                (
                    "manager",
                    "Access Manager/Application.NdiGroupEditor.exe".to_string()
                ),
                (
                    "screen",
                    "Screen Capture/Application.Network.ScanConverter2.x64.exe".to_string()
                ),
                ("launcher", "NDI Launcher.exe".to_string()),
            ]
        );
    }

    #[test]
    fn app_names_parse() {
        for a in NdiApp::ALL {
            assert_eq!(a.name().parse::<NdiApp>().unwrap(), a);
        }
        assert_eq!(" Monitor ".parse::<NdiApp>().unwrap(), NdiApp::Monitor);
        let err = "help".parse::<NdiApp>().unwrap_err();
        assert_eq!(err.exit_code(), 2);
        assert!(err.to_string().contains("launcher"), "{err}");
    }

    #[test]
    fn newest_tools_folder_is_numeric_not_alphabetic() {
        assert_eq!(
            newest_tools_folder(["NDI 5 Tools", "NDI 10 Tools", "NDI 6 Tools"]),
            Some("NDI 10 Tools")
        );
        assert_eq!(
            newest_tools_folder(["NDI 6 Runtime", "NDI 6 SDK", "Other", "NDI Tools"]),
            None
        );
        assert_eq!(
            newest_tools_folder(["ndi 7 tools", "NDI 6 Runtime"]),
            Some("ndi 7 tools")
        );
        assert_eq!(newest_tools_folder([]), None);
    }

    #[test]
    fn install_detection_picks_newest_or_falls_back() {
        let dir = tempfile::tempdir().unwrap();
        let none = install_in(dir.path());
        assert!(!none.installed);
        assert_eq!(none.base, dir.path().join("NDI 6 Tools"));
        assert_eq!(none.version, None);

        for n in ["NDI 5 Tools", "NDI 6 Tools", "NDI 6 SDK"] {
            std::fs::create_dir(dir.path().join(n)).unwrap();
        }
        std::fs::write(dir.path().join("NDI 9 Tools"), b"a file, not a folder").unwrap();
        let got = install_in(dir.path());
        assert!(got.installed);
        assert_eq!(got.base, dir.path().join("NDI 6 Tools"));
        assert_eq!(got.version, Some(6));

        let missing = install_in(&dir.path().join("nonexistent"));
        assert!(!missing.installed);
    }

    #[test]
    fn link_grades() {
        assert_eq!(link_grade(Some(10_000_000_000)), LinkGrade::Good);
        assert_eq!(link_grade(Some(2_500_000_000)), LinkGrade::Good);
        assert_eq!(link_grade(Some(1_000_000_000)), LinkGrade::Good);
        assert_eq!(link_grade(Some(866_000_000)), LinkGrade::Marginal);
        assert_eq!(link_grade(Some(100_000_000)), LinkGrade::Poor);
        assert_eq!(link_grade(Some(10_000_000)), LinkGrade::Poor);
        assert_eq!(link_grade(None), LinkGrade::Marginal);
    }

    fn adapter(desc: &str, up: bool, kind: AdapterKind) -> Adapter {
        Adapter {
            name: "n".into(),
            description: desc.into(),
            index: 1,
            kind,
            up,
            mac: None,
            ipv4: vec![],
            gateways: vec![],
            dns: vec![],
            dhcp: true,
            link_speed_bps: None,
            metric: 0,
        }
    }

    #[test]
    fn adapter_filter_skips_down_loopback_and_virtual() {
        assert!(is_ndi_candidate(&adapter(
            "Realtek PCIe GbE",
            true,
            AdapterKind::Ethernet
        )));
        assert!(!is_ndi_candidate(&adapter(
            "Realtek",
            false,
            AdapterKind::Ethernet
        )));
        assert!(!is_ndi_candidate(&adapter(
            "Hyper-V Virtual Ethernet Adapter",
            true,
            AdapterKind::Ethernet
        )));
        assert!(!is_ndi_candidate(&adapter(
            "Software Loopback Interface 1",
            true,
            AdapterKind::Other
        )));
        assert!(!is_ndi_candidate(&adapter(
            "x",
            true,
            AdapterKind::Loopback
        )));
    }

    #[test]
    fn service_and_process_filters_follow_get_ndiservices() {
        assert!(is_ndi_service("NDI Discovery Service"));
        assert!(is_ndi_service("Newtek NDI Helper"));
        assert!(is_ndi_service("ndi launcher service"));
        assert!(!is_ndi_service("Newtek Updater")); // no "NDI" in the name
        assert!(!is_ndi_service("Windows Installer"));
        assert!(!is_ndi_service("My NDI thing")); // neither "NDI " prefix nor Newtek

        for p in [
            "Application.NDI.Router",
            "Application.Network.StudioMonitor.x64",
            "Application.Network.ScanConverter2.x64",
            "Application.Network.TestPatterns",
            "Application.NdiGroupEditor",
            "NDI Launcher",
        ] {
            assert!(is_ndi_process(p), "{p}");
        }
        assert!(!is_ndi_process("explorer"));
        assert!(!is_ndi_process("RouterHelper"));
    }

    #[test]
    fn ndi_list_accepts_string_array_or_nothing() {
        assert_eq!(ndi_list(None), Vec::<String>::new());
        assert_eq!(ndi_list(Some(&json!(null))), Vec::<String>::new());
        assert_eq!(
            ndi_list(Some(&json!("public, studio ,,cam 2"))),
            ["public", "studio", "cam 2"]
        );
        assert_eq!(ndi_list(Some(&json!(["a", " b ", 3]))), ["a", "b", "3"]);
    }

    #[test]
    fn config_is_read_with_bom_and_both_list_shapes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ndi-config.v1.json");
        let body = r#"{"ndi":{"groups":{"send":"public,show","recv":["a","b"]},
            "networks":{"discovery":"10.0.0.5","ips":"1.2.3.4, 5.6.7.8"}}}"#;
        std::fs::write(&path, format!("\u{feff}{body}")).unwrap();
        let c = read_config(&path).unwrap();
        assert!(c.found);
        assert_eq!(c.groups_send, ["public", "show"]);
        assert_eq!(c.groups_recv, ["a", "b"]);
        assert_eq!(c.discovery, ["10.0.0.5"]);
        assert_eq!(c.ips, ["1.2.3.4", "5.6.7.8"]);
        assert!(c.raw.is_some());
    }

    #[test]
    fn config_missing_empty_or_invalid() {
        let dir = tempfile::tempdir().unwrap();
        let missing = read_config(&dir.path().join("nope.json")).unwrap();
        assert!(!missing.found && missing.raw.is_none());

        let path = dir.path().join("c.json");
        std::fs::write(&path, "{}").unwrap();
        let empty = read_config(&path).unwrap();
        assert!(empty.found && empty.groups_send.is_empty() && empty.ips.is_empty());

        std::fs::write(&path, "{ not json").unwrap();
        let err = read_config(&path).unwrap_err();
        assert!(err.to_string().contains("cannot parse"), "{err}");
    }

    #[cfg(not(windows))]
    #[test]
    fn windows_only_functions_are_unsupported_elsewhere() {
        assert!(matches!(check(), Err(Error::Unsupported)));
        assert!(matches!(services(), Err(Error::Unsupported)));
        assert!(matches!(launch(NdiApp::Monitor), Err(Error::Unsupported)));
    }
}
