//! `sys info` (SysInfo.ps1, AVToolbox `SysInfo`): identity, OS, CPU, RAM, GPU, storage.

use avlt_core::sys::model::SystemInfo;

use crate::Result;

/// Registry class key of display adapters (SysInfo.ps1 VRAM lookup).
#[cfg_attr(not(windows), allow(dead_code))]
const DISPLAY_CLASS_KEY: &str =
    r"SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}";

/// Collects the system summary. Missing WMI values come back empty, not as errors.
pub fn info() -> Result<SystemInfo> {
    imp::info()
}

/// SysInfo.ps1 skips virtual adapters: `Name -notmatch "Basic|Remote"` (case-insensitive).
#[cfg_attr(not(windows), allow(dead_code))]
fn is_real_gpu(name: &str) -> bool {
    let lower = name.to_lowercase();
    !lower.contains("basic") && !lower.contains("remote")
}

/// VRAM of the GPU `name` at position `index`: the registry size of the adapter key whose
/// `DriverDesc` is `name`; else SysInfo.ps1's rule (the positive registry size at the
/// GPU's index, which mismatches on hybrid iGPU + dGPU PCs); else `AdapterRAM`.
/// `registry` holds `(DriverDesc, qwMemorySize)` in key order.
#[cfg_attr(not(windows), allow(dead_code))]
fn vram_for(
    index: usize,
    name: &str,
    registry: &[(String, u64)],
    adapter_ram: Option<u64>,
) -> Option<u64> {
    let by_name = registry
        .iter()
        .find(|(desc, size)| *size > 0 && desc.trim().eq_ignore_ascii_case(name.trim()))
        .map(|(_, size)| *size);
    let sizes: Vec<u64> = registry.iter().map(|r| r.1).filter(|&s| s > 0).collect();
    by_name
        .or_else(|| sizes.get(index).copied())
        .or(adapter_ram.filter(|&r| r > 0))
}

#[cfg(windows)]
mod imp {
    use avlt_core::sys::model::{CpuInfo, GpuInfo, OsInfo, RamInfo, RamModule, SystemInfo};
    use serde::Deserialize;

    use super::{DISPLAY_CLASS_KEY, is_real_gpu, vram_for};
    use crate::Result;
    use crate::registry::{Hive, number, subkeys, values};
    use crate::sys::disks::fixed_volumes;
    use crate::wmi::{cim_date, cim_datetime, connect, query};

    #[derive(Deserialize, Default)]
    #[serde(rename_all = "PascalCase")]
    struct ComputerSystem {
        manufacturer: Option<String>,
        model: Option<String>,
        total_physical_memory: Option<u64>,
    }

    #[derive(Deserialize, Default)]
    #[serde(rename_all = "PascalCase")]
    struct Bios {
        serial_number: Option<String>,
        #[serde(rename = "SMBIOSBIOSVersion")]
        smbios_version: Option<String>,
    }

    #[derive(Deserialize, Default)]
    #[serde(rename_all = "PascalCase")]
    struct OperatingSystem {
        caption: Option<String>,
        version: Option<String>,
        build_number: Option<String>,
        #[serde(rename = "OSArchitecture")]
        os_architecture: Option<String>,
        install_date: Option<String>,
    }

    #[derive(Deserialize, Default)]
    #[serde(rename_all = "PascalCase")]
    struct Processor {
        name: Option<String>,
        number_of_cores: Option<u32>,
        number_of_logical_processors: Option<u32>,
        max_clock_speed: Option<u32>,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct PhysicalMemory {
        capacity: Option<u64>,
        speed: Option<u32>,
        manufacturer: Option<String>,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct VideoController {
        name: Option<String>,
        #[serde(rename = "AdapterRAM")]
        adapter_ram: Option<u32>,
        driver_version: Option<String>,
        driver_date: Option<String>,
    }

    fn text(v: Option<String>) -> String {
        v.map(|s| s.trim().to_string()).unwrap_or_default()
    }

    fn first<T: Default>(rows: Result<Vec<T>>) -> T {
        rows.ok()
            .and_then(|r| r.into_iter().next())
            .unwrap_or_default()
    }

    /// `(DriverDesc, HardwareInformation.qwMemorySize)` of each `0*` adapter subkey that
    /// has a size, in key order (SysInfo.ps1 `$gpuRegMem`, plus the adapter name).
    fn registry_vram() -> Vec<(String, u64)> {
        let Ok(Some(mut keys)) = subkeys(Hive::LocalMachine, DISPLAY_CLASS_KEY) else {
            return Vec::new();
        };
        keys.retain(|k| k.starts_with('0'));
        keys.sort();
        keys.iter()
            .filter_map(|k| {
                let path = format!(r"{DISPLAY_CLASS_KEY}\{k}");
                let size = number(
                    Hive::LocalMachine,
                    &path,
                    "HardwareInformation.qwMemorySize",
                )
                .ok()
                .flatten()?;
                let desc = values(Hive::LocalMachine, &path)
                    .ok()
                    .flatten()
                    .and_then(|v| v.into_iter().find(|(n, _)| n == "DriverDesc"))
                    .map(|(_, d)| d)
                    .unwrap_or_default();
                Some((desc, size))
            })
            .collect()
    }

    pub fn info() -> Result<SystemInfo> {
        let con = connect("ROOT\\CIMV2")?;
        let cs: ComputerSystem = first(query(
            &con,
            "SELECT Manufacturer, Model, TotalPhysicalMemory FROM Win32_ComputerSystem",
        ));
        let bios: Bios = first(query(
            &con,
            "SELECT SerialNumber, SMBIOSBIOSVersion FROM Win32_BIOS",
        ));
        let os: OperatingSystem = first(query(
            &con,
            "SELECT Caption, Version, BuildNumber, OSArchitecture, InstallDate \
             FROM Win32_OperatingSystem",
        ));
        let cpu: Processor = first(query(
            &con,
            "SELECT Name, NumberOfCores, NumberOfLogicalProcessors, MaxClockSpeed \
             FROM Win32_Processor",
        ));
        let modules: Vec<PhysicalMemory> = query(
            &con,
            "SELECT Capacity, Speed, Manufacturer FROM Win32_PhysicalMemory",
        )
        .unwrap_or_default();
        let gpus: Vec<VideoController> = query(
            &con,
            "SELECT Name, AdapterRAM, DriverVersion, DriverDate FROM Win32_VideoController",
        )
        .unwrap_or_default();
        let reg_vram = registry_vram();

        Ok(SystemInfo {
            hostname: std::env::var("COMPUTERNAME").unwrap_or_default(),
            manufacturer: text(cs.manufacturer),
            model: text(cs.model),
            serial_number: text(bios.serial_number),
            bios_version: text(bios.smbios_version),
            os: OsInfo {
                caption: text(os.caption),
                version: text(os.version),
                build: text(os.build_number),
                architecture: text(os.os_architecture),
                install_date: os.install_date.as_deref().and_then(cim_datetime),
                uptime_s: sysinfo::System::uptime().unwrap_or(0),
            },
            cpu: CpuInfo {
                name: text(cpu.name),
                cores: cpu.number_of_cores.unwrap_or(0),
                logical_processors: cpu.number_of_logical_processors.unwrap_or(0),
                base_mhz: cpu.max_clock_speed.unwrap_or(0),
            },
            ram: RamInfo {
                total_bytes: cs.total_physical_memory.unwrap_or(0),
                modules: modules
                    .into_iter()
                    .map(|m| RamModule {
                        capacity_bytes: m.capacity.unwrap_or(0),
                        speed_mhz: m.speed.filter(|&s| s > 0),
                        manufacturer: text(m.manufacturer),
                    })
                    .collect(),
            },
            gpus: gpus
                .into_iter()
                .filter(|g| g.name.as_deref().is_some_and(is_real_gpu))
                .enumerate()
                .map(|(i, g)| {
                    let name = text(g.name);
                    GpuInfo {
                        vram_bytes: vram_for(i, &name, &reg_vram, g.adapter_ram.map(u64::from)),
                        name,
                        driver_version: text(g.driver_version),
                        driver_date: g.driver_date.as_deref().and_then(cim_date),
                    }
                })
                .collect(),
            volumes: fixed_volumes().unwrap_or_default(),
        })
    }
}

#[cfg(not(windows))]
mod imp {
    use avlt_core::sys::model::SystemInfo;

    use crate::{Error, Result};

    pub fn info() -> Result<SystemInfo> {
        Err(Error::Unsupported)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpu_filter_and_vram_choice() {
        assert!(is_real_gpu("NVIDIA GeForce RTX 4070"));
        assert!(!is_real_gpu("Microsoft Basic Display Adapter"));
        assert!(!is_real_gpu("Microsoft Remote Display Adapter"));
        let reg = |d: &str, s: u64| (d.to_string(), s);
        // Matched by adapter name first: WMI and registry orders differ on hybrid PCs.
        let both = [
            reg("NVIDIA GeForce RTX 5070 Ti", 16 << 30),
            reg("AMD Radeon(TM) Graphics", 2 << 30),
        ];
        assert_eq!(
            vram_for(0, "AMD Radeon(TM) Graphics", &both, Some(1 << 30)),
            Some(2 << 30)
        );
        assert_eq!(
            vram_for(1, "nvidia geforce rtx 5070 ti", &both, None),
            Some(16 << 30)
        );
        // No name match: SysInfo.ps1's index rule, then AdapterRAM.
        let unnamed = [reg("", 12 << 30)];
        assert_eq!(vram_for(0, "GPU", &unnamed, Some(4 << 30)), Some(12 << 30));
        assert_eq!(vram_for(1, "GPU", &unnamed, Some(1 << 30)), Some(1 << 30));
        assert_eq!(vram_for(0, "GPU", &[reg("", 0)], None), None);
    }

    #[cfg(windows)]
    #[test]
    fn reads_this_machine() {
        let i = info().unwrap();
        assert!(!i.hostname.is_empty());
        assert!(i.os.caption.contains("Windows"), "{:?}", i.os);
        assert!(i.cpu.logical_processors >= i.cpu.cores && i.cpu.cores > 0);
        assert!(i.ram.total_bytes > 0);
        assert!(i.os.uptime_s > 0);
        assert!(i.volumes.iter().any(|v| v.drive == "C:"));
    }

    #[cfg(not(windows))]
    #[test]
    fn unsupported_elsewhere() {
        assert!(matches!(info(), Err(crate::Error::Unsupported)));
    }
}
