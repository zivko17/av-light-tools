//! `sys disks` (Disks.ps1, AVToolbox `Disks`): physical disks and local fixed volumes.

use avlt_core::sys::model::{DisksReport, Volume};

use crate::Result;

/// Physical disks (`MSFT_PhysicalDisk`, else `Win32_DiskDrive`) and fixed volumes.
pub fn disks() -> Result<DisksReport> {
    imp::disks()
}

/// Local fixed volumes with a drive letter, sorted by letter (`sysinfo`).
pub fn fixed_volumes() -> Result<Vec<Volume>> {
    imp::fixed_volumes()
}

/// `MSFT_PhysicalDisk.MediaType` as Disks.ps1 prints it.
#[cfg_attr(not(windows), allow(dead_code))]
fn media_type_name(code: u16) -> &'static str {
    match code {
        3 => "HDD",
        4 => "SSD",
        5 => "SCM",
        _ => "Unspecified",
    }
}

/// `MSFT_PhysicalDisk.HealthStatus` as Disks.ps1 prints it.
#[cfg_attr(not(windows), allow(dead_code))]
fn health_name(code: u16) -> &'static str {
    match code {
        0 => "Healthy",
        1 => "Warning",
        2 => "Unhealthy",
        _ => "Unknown",
    }
}

/// `C:` from a mount point `C:\`; `None` for folder mount points.
#[cfg_attr(not(windows), allow(dead_code))]
fn drive_of(mount: &str) -> Option<String> {
    let b = mount.as_bytes();
    let root = b.len() == 2 || (b.len() == 3 && (b[2] == b'\\' || b[2] == b'/'));
    (root && b[0].is_ascii_alphabetic() && b[1] == b':')
        .then(|| format!("{}:", char::from(b[0].to_ascii_uppercase())))
}

#[cfg(windows)]
mod imp {
    use std::collections::HashMap;

    use avlt_core::sys::format::percent;
    use avlt_core::sys::model::{DisksReport, PhysicalDisk, Volume};
    use serde::Deserialize;

    use super::{drive_of, health_name, media_type_name};
    use crate::Result;
    use crate::wmi::{connect, query};

    #[derive(Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct MsftPhysicalDisk {
        device_id: Option<String>,
        friendly_name: Option<String>,
        media_type: Option<u16>,
        size: Option<u64>,
        health_status: Option<u16>,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct Reliability {
        device_id: Option<String>,
        temperature: Option<u32>,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct DiskDrive {
        model: Option<String>,
        size: Option<u64>,
        interface_type: Option<String>,
    }

    pub fn disks() -> Result<DisksReport> {
        Ok(DisksReport {
            physical: physical().unwrap_or_default(),
            volumes: fixed_volumes()?,
        })
    }

    fn physical() -> Result<Vec<PhysicalDisk>> {
        match storage_disks() {
            Ok(d) if !d.is_empty() => Ok(d),
            _ => disk_drives(),
        }
    }

    fn storage_disks() -> Result<Vec<PhysicalDisk>> {
        let con = connect("ROOT\\Microsoft\\Windows\\Storage")?;
        let rows: Vec<MsftPhysicalDisk> = query(
            &con,
            "SELECT DeviceId, FriendlyName, MediaType, Size, HealthStatus FROM MSFT_PhysicalDisk",
        )?;
        // Usually needs administrator rights; without them there is no temperature.
        let temps: HashMap<String, u32> = query::<Reliability>(
            &con,
            "SELECT DeviceId, Temperature FROM MSFT_StorageReliabilityCounter",
        )
        .unwrap_or_default()
        .into_iter()
        .filter_map(|r| Some((r.device_id?, r.temperature.filter(|&t| t > 0)?)))
        .collect();
        Ok(rows
            .into_iter()
            .map(|d| PhysicalDisk {
                model: d
                    .friendly_name
                    .filter(|n| !n.trim().is_empty())
                    .unwrap_or_else(|| "(unknown)".into()),
                media_type: media_type_name(d.media_type.unwrap_or(0)).into(),
                size_bytes: d.size.unwrap_or(0),
                health: health_name(d.health_status.unwrap_or(5)).into(),
                temperature_c: d.device_id.and_then(|id| temps.get(&id).copied()),
            })
            .collect())
    }

    /// AVToolbox fallback: `Win32_DiskDrive`.
    fn disk_drives() -> Result<Vec<PhysicalDisk>> {
        let con = connect("ROOT\\CIMV2")?;
        let rows: Vec<DiskDrive> = query(
            &con,
            "SELECT Model, Size, InterfaceType FROM Win32_DiskDrive",
        )?;
        Ok(rows
            .into_iter()
            .map(|d| PhysicalDisk {
                model: d.model.unwrap_or_else(|| "(unknown)".into()),
                media_type: d.interface_type.unwrap_or_else(|| "Unspecified".into()),
                size_bytes: d.size.unwrap_or(0),
                health: "Unknown".into(),
                temperature_c: None,
            })
            .collect())
    }

    pub fn fixed_volumes() -> Result<Vec<Volume>> {
        let disks = sysinfo::Disks::new_with_refreshed_list()
            .map_err(|e| crate::Error::Other(format!("cannot list volumes: {e}")))?;
        let mut out: Vec<Volume> = disks
            .iter()
            .filter(|d| !d.is_removable())
            .filter_map(|d| {
                let drive = drive_of(&d.mount_point().to_string_lossy())?;
                let total = d.total_space();
                let free = d.available_space().min(total);
                Some(Volume {
                    drive,
                    label: d.name().to_string_lossy().trim().to_string(),
                    total_bytes: total,
                    free_bytes: free,
                    used_pct: percent(total - free, total),
                })
            })
            .collect();
        out.sort_by(|a, b| a.drive.cmp(&b.drive));
        out.dedup_by(|a, b| a.drive == b.drive);
        Ok(out)
    }
}

#[cfg(not(windows))]
mod imp {
    use avlt_core::sys::model::{DisksReport, Volume};

    use crate::{Error, Result};

    pub fn disks() -> Result<DisksReport> {
        Err(Error::Unsupported)
    }

    pub fn fixed_volumes() -> Result<Vec<Volume>> {
        Err(Error::Unsupported)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_and_drive_letters() {
        assert_eq!(media_type_name(4), "SSD");
        assert_eq!(media_type_name(0), "Unspecified");
        assert_eq!(health_name(0), "Healthy");
        assert_eq!(health_name(5), "Unknown");
        assert_eq!(drive_of("C:\\").as_deref(), Some("C:"));
        assert_eq!(drive_of("d:").as_deref(), Some("D:"));
        assert_eq!(drive_of("C:\\Mount\\Data"), None);
        assert_eq!(drive_of("/"), None);
    }

    #[cfg(windows)]
    #[test]
    fn lists_the_system_volume() {
        let report = disks().unwrap();
        let c = report.volumes.iter().find(|v| v.drive == "C:").unwrap();
        assert!(c.total_bytes > 0 && c.free_bytes <= c.total_bytes);
        assert!((0.0..=100.0).contains(&c.used_pct));
        assert!(!report.physical.is_empty());
        assert!(report.physical.iter().all(|d| d.size_bytes > 0));
    }

    #[cfg(not(windows))]
    #[test]
    fn unsupported_elsewhere() {
        assert!(matches!(disks(), Err(crate::Error::Unsupported)));
    }
}
