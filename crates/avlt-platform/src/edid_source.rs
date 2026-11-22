//! EDID of the displays Windows knows about. Read-only: nothing here writes an EDID or
//! touches the display driver.
//!
//! Port of `Get-EDIDBytesFromRegistry`, `Get-EDIDBytesFromWmi` and `Get-EDIDDevice`
//! (`EDIDTools.ps1` lines 588-685): the attached monitors come from WMI `WmiMonitorID`;
//! each EDID is read from the registry (`Enum\DISPLAY\...\Device Parameters\EDID`, full
//! blob with extensions) and, failing that, block by block from WMI
//! `WmiMonitorDescriptorMethods`.

use serde::{Serialize, Serializer};

use crate::Result;

/// Where a display's EDID came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum EdidSource {
    Registry,
    Wmi,
    /// Neither source returned an EDID.
    Unavailable,
}

/// A display and its raw EDID.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DisplayDevice {
    /// 1-based, in enumeration order: the number `edid info <n>` takes.
    pub index: usize,
    /// Friendly name (WMI, else the EDID monitor name, else the hardware ID).
    pub name: String,
    /// PnP instance, e.g. `DISPLAY\GSM5B09\5&2b6e2b7&0&UID4353`.
    pub instance_id: String,
    /// Attached now; `false` for displays only remembered in the registry.
    pub active: bool,
    pub source: EdidSource,
    /// Raw EDID (empty when unavailable), hex-encoded in JSON.
    #[serde(serialize_with = "hex")]
    pub edid: Vec<u8>,
}

fn hex<S: Serializer>(bytes: &[u8], s: S) -> std::result::Result<S::Ok, S::Error> {
    s.serialize_str(&avlt_core::edid::to_hex(bytes))
}

/// The attached displays.
pub fn displays() -> Result<Vec<DisplayDevice>> {
    list(false)
}

/// The attached displays; with `remembered`, also every display still recorded in the
/// registry (a projector plugged in yesterday). Read errors on one source are not fatal:
/// that display falls back to the next source or is reported `Unavailable`.
pub fn list(remembered: bool) -> Result<Vec<DisplayDevice>> {
    imp::list(remembered)
}

/// `DISPLAY\GSM5B09\5&2b6e2b7&0&UID4353_0` (WMI form) -> PnP instance ID.
#[cfg_attr(not(windows), allow(dead_code))]
fn strip_wmi_suffix(instance_name: &str) -> &str {
    match instance_name.rsplit_once('_') {
        Some((head, tail)) if !tail.is_empty() && tail.bytes().all(|b| b.is_ascii_digit()) => head,
        _ => instance_name,
    }
}

/// Display name when WMI gives none: the EDID monitor name, else the hardware ID.
#[cfg_attr(not(windows), allow(dead_code))]
fn fallback_name(instance_id: &str, edid: &[u8]) -> String {
    avlt_core::edid::decode(edid)
        .base
        .and_then(|b| b.monitor_name)
        .unwrap_or_else(|| {
            instance_id
                .split('\\')
                .nth(1)
                .unwrap_or(instance_id)
                .to_string()
        })
}

#[cfg(not(windows))]
mod imp {
    use super::DisplayDevice;
    use crate::{Error, Result};

    pub fn list(_remembered: bool) -> Result<Vec<DisplayDevice>> {
        Err(Error::Unsupported)
    }
}

#[cfg(windows)]
mod imp {
    use std::collections::{HashMap, HashSet};

    use serde::{Deserialize, Serialize};
    use winreg::RegKey;
    use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ};

    use super::{DisplayDevice, EdidSource, fallback_name, strip_wmi_suffix};
    use crate::Result;

    const ENUM: &str = r"SYSTEM\CurrentControlSet\Enum";

    #[derive(Deserialize)]
    #[serde(rename = "WmiMonitorID")]
    struct WmiMonitorId {
        #[serde(rename = "InstanceName")]
        instance_name: String,
        #[serde(rename = "UserFriendlyName")]
        user_friendly_name: Option<Vec<u16>>,
    }

    #[derive(Deserialize)]
    #[serde(rename = "WmiMonitorDescriptorMethods")]
    struct DescriptorMethods {
        #[serde(rename = "__Path")]
        path: String,
        #[serde(rename = "InstanceName")]
        instance_name: String,
    }

    #[derive(Serialize)]
    struct BlockIn {
        #[serde(rename = "BlockId")]
        block_id: u32,
    }

    #[derive(Deserialize)]
    struct BlockOut {
        #[serde(rename = "BlockContent")]
        block_content: Option<Vec<u8>>,
    }

    pub fn list(remembered: bool) -> Result<Vec<DisplayDevice>> {
        let wmi = ::wmi::WMIConnection::with_namespace_path("root\\wmi").ok();
        let mut ids: Vec<WmiMonitorId> = wmi
            .as_ref()
            .and_then(|c| c.query::<WmiMonitorId>().ok())
            .unwrap_or_default();
        ids.sort_by(|a, b| a.instance_name.cmp(&b.instance_name));

        let mut methods: Option<HashMap<String, String>> = None;
        let mut out = Vec::new();
        let mut seen = HashSet::new();
        for id in ids {
            let instance_id = strip_wmi_suffix(&id.instance_name).to_string();
            let (source, edid) = match registry_edid(&instance_id) {
                Some(bytes) => (EdidSource::Registry, bytes),
                None => {
                    let paths = methods.get_or_insert_with(|| descriptor_paths(wmi.as_ref()));
                    match wmi
                        .as_ref()
                        .zip(paths.get(&id.instance_name))
                        .and_then(|(c, p)| wmi_edid(c, p))
                    {
                        Some(bytes) => (EdidSource::Wmi, bytes),
                        None => (EdidSource::Unavailable, Vec::new()),
                    }
                }
            };
            seen.insert(instance_id.to_uppercase());
            let name = id
                .user_friendly_name
                .as_deref()
                .map(wmi_string)
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| fallback_name(&instance_id, &edid));
            out.push(DisplayDevice {
                index: 0,
                name,
                instance_id,
                active: true,
                source,
                edid,
            });
        }

        if remembered {
            for instance_id in registry_display_instances() {
                if seen.contains(&instance_id.to_uppercase()) {
                    continue;
                }
                let Some(edid) = registry_edid(&instance_id) else {
                    continue;
                };
                seen.insert(instance_id.to_uppercase());
                out.push(DisplayDevice {
                    index: 0,
                    name: fallback_name(&instance_id, &edid),
                    instance_id,
                    active: false,
                    source: EdidSource::Registry,
                    edid,
                });
            }
        }

        for (i, d) in out.iter_mut().enumerate() {
            d.index = i + 1;
        }
        Ok(out)
    }

    /// The full EDID blob (base + extensions) the registry holds for one PnP instance.
    fn registry_edid(instance_id: &str) -> Option<Vec<u8>> {
        let key = RegKey::predef(HKEY_LOCAL_MACHINE)
            .open_subkey_with_flags(format!(r"{ENUM}\{instance_id}\Device Parameters"), KEY_READ)
            .ok()?;
        let v = key.get_raw_value("EDID").ok()?;
        (!v.bytes.is_empty()).then(|| v.bytes.into_owned())
    }

    /// Every `DISPLAY\<model>\<instance>` key in the registry.
    fn registry_display_instances() -> Vec<String> {
        let Ok(root) = RegKey::predef(HKEY_LOCAL_MACHINE)
            .open_subkey_with_flags(format!(r"{ENUM}\DISPLAY"), KEY_READ)
        else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for model in root.enum_keys().filter_map(|k| k.ok()) {
            let Ok(mon) = root.open_subkey_with_flags(&model, KEY_READ) else {
                continue;
            };
            for inst in mon.enum_keys().filter_map(|k| k.ok()) {
                out.push(format!(r"DISPLAY\{model}\{inst}"));
            }
        }
        out
    }

    /// WMI instance name -> object path of its `WmiMonitorDescriptorMethods`.
    fn descriptor_paths(wmi: Option<&::wmi::WMIConnection>) -> HashMap<String, String> {
        wmi.and_then(|c| c.query::<DescriptorMethods>().ok())
            .unwrap_or_default()
            .into_iter()
            .map(|m| (m.instance_name, m.path))
            .collect()
    }

    /// Asks the display driver for each 128-byte block; `None` on any failure.
    fn wmi_edid(wmi: &::wmi::WMIConnection, path: &str) -> Option<Vec<u8>> {
        let block = |block_id: u32| -> Option<Vec<u8>> {
            wmi.exec_instance_method::<DescriptorMethods, BlockOut>(
                path,
                "WmiGetMonitorRawEEdidV1Block",
                BlockIn { block_id },
            )
            .ok()?
            .block_content
            .filter(|b| !b.is_empty())
        };
        let mut out = block(0)?;
        let ext_count = out.get(126).copied().unwrap_or(0);
        for b in 1..=u32::from(ext_count) {
            out.extend(block(b)?);
        }
        Some(out)
    }

    /// WMI `uint16[]` string, NUL-terminated or padded.
    fn wmi_string(chars: &[u16]) -> String {
        let end = chars.iter().position(|&c| c == 0).unwrap_or(chars.len());
        String::from_utf16_lossy(&chars[..end]).trim().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wmi_suffix_is_stripped() {
        assert_eq!(
            strip_wmi_suffix(r"DISPLAY\GSM5B09\5&2b6e2b7&0&UID4353_0"),
            r"DISPLAY\GSM5B09\5&2b6e2b7&0&UID4353"
        );
        assert_eq!(strip_wmi_suffix(r"DISPLAY\A\B_12"), r"DISPLAY\A\B");
        assert_eq!(strip_wmi_suffix(r"DISPLAY\A\B_x"), r"DISPLAY\A\B_x");
        assert_eq!(strip_wmi_suffix(r"DISPLAY\A\B_"), r"DISPLAY\A\B_");
        assert_eq!(strip_wmi_suffix("plain"), "plain");
    }

    #[test]
    fn fallback_name_uses_hardware_id_without_edid() {
        assert_eq!(fallback_name(r"DISPLAY\GSM5B09\5&2b", &[]), "GSM5B09");
        assert_eq!(fallback_name("odd", &[]), "odd");
    }

    #[cfg(not(windows))]
    #[test]
    fn unsupported_off_windows() {
        assert!(matches!(displays(), Err(crate::Error::Unsupported)));
    }

    #[test]
    fn device_serializes_edid_as_hex() {
        let d = DisplayDevice {
            index: 1,
            name: "X".into(),
            instance_id: "DISPLAY\\X\\1".into(),
            active: true,
            source: EdidSource::Registry,
            edid: vec![0x00, 0xFF],
        };
        let v = serde_json::to_value(&d).unwrap();
        assert_eq!(v["edid"], "00 FF");
        assert_eq!(v["source"], "registry");
    }
}
