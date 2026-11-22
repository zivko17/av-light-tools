//! `sys monitors` (Monitors.ps1) and `sys monitors-arrange` (MonitorsArrange.ps1).
//!
//! Modes are changed with `ChangeDisplaySettingsExW` (no nircmd): first with `CDS_TEST`,
//! and only if the driver accepts it, for real.

use avlt_core::sys::model::{Display, DisplayMode, ModeChange, MonitorsReport};

use crate::Result;

/// Desktop screens (geometry, work area, current refresh rate) and the monitors WMI
/// reports (manufacturer, model, serial, best listed mode).
pub fn monitors() -> Result<MonitorsReport> {
    imp::monitors()
}

/// Display outputs attached to the desktop, with the modes their driver offers at the
/// current colour depth (highest first).
pub fn displays() -> Result<Vec<Display>> {
    imp::displays()
}

/// Sets `mode` on the display `device_name` (e.g. `\\.\DISPLAY1`). `CDS_TEST` runs first;
/// with `test_only` nothing is changed. A rejected mode is an error.
pub fn set_mode(device_name: &str, mode: DisplayMode, test_only: bool) -> Result<ModeChange> {
    imp::set_mode(device_name, mode, test_only)
}

/// Hardware id of a monitor device path: `MSI3CA4` from `MONITOR\MSI3CA4\{...}\0001`
/// (GDI) or `DISPLAY\MSI3CA4\5&...&UID4353_0` (WMI `InstanceName`).
#[cfg_attr(not(windows), allow(dead_code))]
fn pnp_id(path: &str) -> Option<String> {
    let id = path.split('\\').nth(1)?.trim();
    (!id.is_empty()).then(|| id.to_ascii_uppercase())
}

/// The WMI model name of the only monitor with hardware id `id` (two identical monitors
/// cannot be told apart this way).
#[cfg_attr(not(windows), allow(dead_code))]
fn unique_model(id: &str, models: &[(String, String)]) -> Option<String> {
    let mut found = models.iter().filter(|(i, m)| i == id && !m.is_empty());
    let first = found.next()?;
    found.next().is_none().then(|| first.1.clone())
}

/// Text of a `DISP_CHANGE_*` result.
#[cfg_attr(not(windows), allow(dead_code))]
fn disp_change_text(code: i32) -> &'static str {
    match code {
        0 => "successful",
        1 => "restart required",
        -1 => "the display driver failed the mode",
        -2 => "the mode is not supported",
        -3 => "cannot write the settings to the registry",
        -4 => "invalid flags",
        -5 => "invalid parameter",
        -6 => "the settings would leave the desktop without a primary display",
        -7 => "the video mode cannot be changed now",
        _ => "unknown error",
    }
}

#[cfg(windows)]
mod imp {
    use std::collections::HashMap;

    use avlt_core::sys::display::{SourceMode, best_source_mode};
    use avlt_core::sys::format::wmi_chars;
    use avlt_core::sys::model::{
        Display, DisplayMode, ModeChange, MonitorsReport, PhysicalMonitor, Screen,
    };
    use serde::Deserialize;
    use windows::Win32::Foundation::{LPARAM, RECT};
    use windows::Win32::Graphics::Gdi::{
        CDS_TEST, CDS_TYPE, CDS_UPDATEREGISTRY, ChangeDisplaySettingsExW, DEVMODEW,
        DISP_CHANGE_RESTART, DISP_CHANGE_SUCCESSFUL, DISPLAY_DEVICE_ATTACHED_TO_DESKTOP,
        DISPLAY_DEVICE_PRIMARY_DEVICE, DISPLAY_DEVICEW, DM_BITSPERPEL, DM_DISPLAYFREQUENCY,
        DM_PELSHEIGHT, DM_PELSWIDTH, ENUM_CURRENT_SETTINGS, ENUM_DISPLAY_SETTINGS_FLAGS,
        ENUM_DISPLAY_SETTINGS_MODE, EnumDisplayDevicesW, EnumDisplayMonitors,
        EnumDisplaySettingsExW, EnumDisplaySettingsW, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO,
        MONITORINFOEXW,
    };
    use windows::core::{BOOL, HSTRING, PCWSTR};

    use super::{disp_change_text, pnp_id, unique_model};
    use crate::wmi::{connect, query};
    use crate::{Error, Result};

    const MONITORINFOF_PRIMARY: u32 = 1;

    fn wide_str(buf: &[u16]) -> String {
        let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        String::from_utf16_lossy(&buf[..end])
    }

    fn devmode() -> DEVMODEW {
        DEVMODEW {
            dmSize: size_of::<DEVMODEW>() as u16,
            ..Default::default()
        }
    }

    fn current_settings(device: &str) -> Option<DEVMODEW> {
        let mut dm = devmode();
        // SAFETY: `dm` is a DEVMODEW with dmSize set, as the API requires.
        unsafe { EnumDisplaySettingsW(&HSTRING::from(device), ENUM_CURRENT_SETTINGS, &mut dm) }
            .as_bool()
            .then_some(dm)
    }

    fn mode_of(dm: &DEVMODEW) -> DisplayMode {
        DisplayMode {
            width: dm.dmPelsWidth,
            height: dm.dmPelsHeight,
            refresh_hz: dm.dmDisplayFrequency,
        }
    }

    // ------------------------------------------------------------ screens

    unsafe extern "system" fn collect_monitor(
        monitor: HMONITOR,
        _hdc: HDC,
        _rect: *mut RECT,
        data: LPARAM,
    ) -> BOOL {
        // SAFETY: `data` is the address of the Vec passed by `screens`, alive for the
        // whole EnumDisplayMonitors call.
        let list = unsafe { &mut *(data.0 as *mut Vec<MONITORINFOEXW>) };
        let mut info = MONITORINFOEXW::default();
        info.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;
        // SAFETY: a MONITORINFOEXW with cbSize set may be passed as MONITORINFO.
        if unsafe { GetMonitorInfoW(monitor, (&raw mut info).cast::<MONITORINFO>()) }.as_bool() {
            list.push(info);
        }
        BOOL(1)
    }

    fn screens() -> Vec<Screen> {
        let mut infos: Vec<MONITORINFOEXW> = Vec::new();
        // SAFETY: the callback only touches `infos` through the LPARAM during the call.
        let _ = unsafe {
            EnumDisplayMonitors(
                None,
                None,
                Some(collect_monitor),
                LPARAM(&raw mut infos as isize),
            )
        };
        infos
            .iter()
            .zip(1u32..)
            .map(|(info, index)| {
                let r = info.monitorInfo.rcMonitor;
                let w = info.monitorInfo.rcWork;
                let device_name = wide_str(&info.szDevice);
                let refresh_hz = current_settings(&device_name)
                    .map(|dm| dm.dmDisplayFrequency)
                    .filter(|&hz| hz > 1);
                Screen {
                    index,
                    primary: info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0,
                    width: r.right.saturating_sub(r.left).unsigned_abs(),
                    height: r.bottom.saturating_sub(r.top).unsigned_abs(),
                    x: r.left,
                    y: r.top,
                    work_width: w.right.saturating_sub(w.left).unsigned_abs(),
                    work_height: w.bottom.saturating_sub(w.top).unsigned_abs(),
                    refresh_hz,
                    device_name,
                }
            })
            .collect()
    }

    // ------------------------------------------------------------ WMI monitors

    #[derive(Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct MonitorId {
        instance_name: Option<String>,
        manufacturer_name: Option<Vec<u16>>,
        user_friendly_name: Option<Vec<u16>>,
        #[serde(rename = "SerialNumberID")]
        serial_number_id: Option<Vec<u16>>,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct ListedModes {
        instance_name: Option<String>,
        monitor_source_modes: Option<Vec<SourceModeRow>>,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct SourceModeRow {
        horizontal_active_pixels: Option<u32>,
        vertical_active_pixels: Option<u32>,
        vertical_refresh_rate_numerator: Option<u32>,
        vertical_refresh_rate_denominator: Option<u32>,
    }

    fn physical() -> Vec<PhysicalMonitor> {
        let Ok(con) = connect("root\\wmi") else {
            return Vec::new();
        };
        let ids: Vec<MonitorId> = query(
            &con,
            "SELECT InstanceName, ManufacturerName, UserFriendlyName, SerialNumberID \
             FROM WmiMonitorID",
        )
        .unwrap_or_default();
        let modes: HashMap<String, Vec<SourceMode>> = query::<ListedModes>(
            &con,
            "SELECT InstanceName, MonitorSourceModes FROM WmiMonitorListedSupportedSourceModes",
        )
        .unwrap_or_default()
        .into_iter()
        .filter_map(|m| {
            let list = m
                .monitor_source_modes?
                .into_iter()
                .map(|r| SourceMode {
                    width: r.horizontal_active_pixels.unwrap_or(0),
                    height: r.vertical_active_pixels.unwrap_or(0),
                    refresh_numerator: r.vertical_refresh_rate_numerator.unwrap_or(0),
                    refresh_denominator: r.vertical_refresh_rate_denominator.unwrap_or(0),
                })
                .collect();
            Some((m.instance_name?, list))
        })
        .collect();
        ids.into_iter()
            .map(|m| PhysicalMonitor {
                manufacturer: wmi_chars(&m.manufacturer_name.unwrap_or_default()),
                model: wmi_chars(&m.user_friendly_name.unwrap_or_default()),
                serial: wmi_chars(&m.serial_number_id.unwrap_or_default()),
                best_mode: m
                    .instance_name
                    .and_then(|n| modes.get(&n))
                    .and_then(|list| best_source_mode(list)),
            })
            .collect()
    }

    pub fn monitors() -> Result<MonitorsReport> {
        Ok(MonitorsReport {
            screens: screens(),
            physical: physical(),
        })
    }

    // ------------------------------------------------------------ arrange

    fn display_device(parent: Option<&str>, index: u32) -> Option<DISPLAY_DEVICEW> {
        let mut dd = DISPLAY_DEVICEW {
            cb: size_of::<DISPLAY_DEVICEW>() as u32,
            ..Default::default()
        };
        let name = parent.map(HSTRING::from);
        let ptr = name.as_ref().map_or(PCWSTR::null(), |h| PCWSTR(h.as_ptr()));
        // SAFETY: `dd` has cb set; `ptr` is null or a NUL-terminated string kept alive
        // by `name` for the call.
        unsafe { EnumDisplayDevicesW(ptr, index, &mut dd, 0) }
            .as_bool()
            .then_some(dd)
    }

    /// Every mode the driver lists for `device` at `bits` per pixel (raw `DEVMODEW`s).
    fn raw_modes(device: &str, bits: u32) -> Vec<DEVMODEW> {
        let name = HSTRING::from(device);
        let mut out = Vec::new();
        for i in 0u32.. {
            let mut dm = devmode();
            // SAFETY: `dm` has dmSize set; enumeration ends when the call returns FALSE.
            let ok = unsafe {
                EnumDisplaySettingsExW(
                    &name,
                    ENUM_DISPLAY_SETTINGS_MODE(i),
                    &mut dm,
                    ENUM_DISPLAY_SETTINGS_FLAGS(0),
                )
            };
            if !ok.as_bool() {
                break;
            }
            if dm.dmBitsPerPel == bits && dm.dmPelsWidth > 0 && dm.dmPelsHeight > 0 {
                out.push(dm);
            }
        }
        out
    }

    /// `(hardware id, model)` of every monitor WMI knows.
    fn wmi_models() -> Vec<(String, String)> {
        let Ok(con) = connect("root\\wmi") else {
            return Vec::new();
        };
        // Every field of MonitorId must be selected: the wmi deserializer fails otherwise.
        query::<MonitorId>(
            &con,
            "SELECT InstanceName, ManufacturerName, UserFriendlyName, SerialNumberID \
             FROM WmiMonitorID",
        )
        .unwrap_or_default()
        .into_iter()
        .filter_map(|m| {
            Some((
                pnp_id(&m.instance_name?)?,
                wmi_chars(&m.user_friendly_name.unwrap_or_default()),
            ))
        })
        .collect()
    }

    pub fn displays() -> Result<Vec<Display>> {
        let models = wmi_models();
        let mut list = Vec::new();
        let mut index = 0u32;
        for i in 0u32.. {
            let Some(adapter) = display_device(None, i) else {
                break;
            };
            if adapter.StateFlags & DISPLAY_DEVICE_ATTACHED_TO_DESKTOP
                != DISPLAY_DEVICE_ATTACHED_TO_DESKTOP
            {
                continue;
            }
            index += 1;
            let device_name = wide_str(&adapter.DeviceName);
            let monitor = display_device(Some(&device_name), 0).and_then(|m| {
                pnp_id(&wide_str(&m.DeviceID))
                    .and_then(|id| unique_model(&id, &models))
                    .or_else(|| Some(wide_str(&m.DeviceString)))
                    .filter(|s| !s.trim().is_empty())
            });
            let current = current_settings(&device_name);
            let bits = current.as_ref().map_or(32, |dm| dm.dmBitsPerPel);
            let mut modes: Vec<DisplayMode> =
                raw_modes(&device_name, bits).iter().map(mode_of).collect();
            modes.sort_unstable_by(|a, b| {
                (b.width, b.height, b.refresh_hz).cmp(&(a.width, a.height, a.refresh_hz))
            });
            modes.dedup();
            list.push(Display {
                index,
                name: monitor.unwrap_or_else(|| wide_str(&adapter.DeviceString)),
                primary: adapter.StateFlags & DISPLAY_DEVICE_PRIMARY_DEVICE
                    == DISPLAY_DEVICE_PRIMARY_DEVICE,
                current: current.as_ref().map(mode_of),
                modes,
                device_name,
            });
        }
        Ok(list)
    }

    fn change(device: &HSTRING, dm: &DEVMODEW, flags: CDS_TYPE) -> i32 {
        // SAFETY: `dm` is a complete DEVMODEW from EnumDisplaySettingsExW; `device` is a
        // NUL-terminated display name.
        unsafe { ChangeDisplaySettingsExW(device, Some(dm), None, flags, None) }.0
    }

    pub fn set_mode(device_name: &str, mode: DisplayMode, test_only: bool) -> Result<ModeChange> {
        let bits = current_settings(device_name)
            .map(|dm| dm.dmBitsPerPel)
            .ok_or_else(|| {
                Error::InvalidInput(format!("display '{device_name}' is not attached"))
            })?;
        let mut dm = raw_modes(device_name, bits)
            .into_iter()
            .find(|dm| mode_of(dm) == mode)
            .ok_or_else(|| {
                Error::InvalidInput(format!(
                    "{}x{} @ {} Hz is not offered by {device_name}",
                    mode.width, mode.height, mode.refresh_hz
                ))
            })?;
        dm.dmFields = DM_PELSWIDTH | DM_PELSHEIGHT | DM_DISPLAYFREQUENCY | DM_BITSPERPEL;
        let device = HSTRING::from(device_name);
        let rejected = |code: i32| {
            Error::Other(format!(
                "{}x{} @ {} Hz on {device_name}: {} (DISP_CHANGE {code})",
                mode.width,
                mode.height,
                mode.refresh_hz,
                disp_change_text(code)
            ))
        };
        let test = change(&device, &dm, CDS_TEST);
        if test != DISP_CHANGE_SUCCESSFUL.0 {
            return Err(rejected(test));
        }
        let mut result = ModeChange {
            device_name: device_name.to_string(),
            mode,
            test_only,
            restart_required: false,
        };
        if test_only {
            return Ok(result);
        }
        match change(&device, &dm, CDS_UPDATEREGISTRY) {
            c if c == DISP_CHANGE_SUCCESSFUL.0 => {}
            c if c == DISP_CHANGE_RESTART.0 => result.restart_required = true,
            c => return Err(rejected(c)),
        }
        Ok(result)
    }
}

#[cfg(not(windows))]
mod imp {
    use avlt_core::sys::model::{Display, DisplayMode, ModeChange, MonitorsReport};

    use crate::{Error, Result};

    pub fn monitors() -> Result<MonitorsReport> {
        Err(Error::Unsupported)
    }

    pub fn displays() -> Result<Vec<Display>> {
        Err(Error::Unsupported)
    }

    pub fn set_mode(_device: &str, _mode: DisplayMode, _test_only: bool) -> Result<ModeChange> {
        Err(Error::Unsupported)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monitor_names_by_hardware_id() {
        assert_eq!(
            pnp_id(r"MONITOR\MSI3CA4\{4d36e96e-e325-11ce-bfc1-08002be10318}\0001").as_deref(),
            Some("MSI3CA4")
        );
        assert_eq!(
            pnp_id(r"DISPLAY\msi3ca4\5&2b0c&0&UID4353_0").as_deref(),
            Some("MSI3CA4")
        );
        assert_eq!(pnp_id("nothing"), None);
        let models = [
            ("MSI3CA4".to_string(), "MSI MP243X".to_string()),
            ("AUS24A1".to_string(), "ASUS VA24E".to_string()),
            ("AUS24A1".to_string(), "ASUS VA24E".to_string()),
        ];
        assert_eq!(
            unique_model("MSI3CA4", &models).as_deref(),
            Some("MSI MP243X")
        );
        assert_eq!(unique_model("AUS24A1", &models), None);
        assert_eq!(unique_model("XXX0000", &models), None);
    }

    #[test]
    fn disp_change_texts() {
        assert_eq!(disp_change_text(0), "successful");
        assert_eq!(disp_change_text(-2), "the mode is not supported");
        assert_eq!(disp_change_text(42), "unknown error");
    }

    #[cfg(windows)]
    #[test]
    fn reads_screens_and_displays() {
        let report = monitors().unwrap();
        let shown = displays().unwrap();
        // CI runners may be headless; only check consistency.
        assert!(report.screens.iter().filter(|s| s.primary).count() <= 1);
        for d in &shown {
            assert!(d.device_name.starts_with(r"\\.\"), "{d:?}");
            if let Some(cur) = d.current {
                assert!(cur.width > 0 && cur.height > 0);
            }
        }
    }

    /// Validates the current mode with CDS_TEST only: nothing on screen changes.
    #[cfg(windows)]
    #[test]
    fn current_mode_passes_cds_test() {
        for d in displays().unwrap() {
            let Some(cur) = d.current else { continue };
            if !d.modes.contains(&cur) {
                continue;
            }
            let r = set_mode(&d.device_name, cur, true).unwrap();
            assert!(r.test_only && !r.restart_required);
        }
    }

    #[cfg(windows)]
    #[test]
    fn unknown_mode_is_rejected_before_any_change() {
        let Some(d) = displays().unwrap().into_iter().next() else {
            return;
        };
        let bogus = DisplayMode {
            width: 12345,
            height: 7,
            refresh_hz: 3,
        };
        let err = set_mode(&d.device_name, bogus, false).unwrap_err();
        assert_eq!(err.exit_code(), 2, "{err}");
        let err = set_mode(r"\\.\DISPLAY999", bogus, true).unwrap_err();
        assert_eq!(err.exit_code(), 2, "{err}");
    }

    #[cfg(not(windows))]
    #[test]
    fn unsupported_elsewhere() {
        assert!(matches!(monitors(), Err(crate::Error::Unsupported)));
    }
}
