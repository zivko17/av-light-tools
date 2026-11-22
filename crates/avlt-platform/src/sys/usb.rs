//! `sys usb` (USB.ps1): present USB devices in state OK.

use avlt_core::sys::model::UsbDevice;

use crate::Result;

/// `Get-PnpDevice -PresentOnly | ? InstanceId -like "USB*" -and Status -eq "OK"`,
/// through `Win32_PnPEntity`, sorted by name.
pub fn usb_devices() -> Result<Vec<UsbDevice>> {
    let mut list = imp::usb_devices()?;
    list.sort_by_key(|d| d.name.to_lowercase());
    Ok(list)
}

/// USB.ps1 filter: instance id starting with `USB` (case-insensitive) and status `OK`.
#[cfg_attr(not(windows), allow(dead_code))]
fn keep(instance_id: &str, status: Option<&str>, present: Option<bool>) -> bool {
    instance_id
        .get(..3)
        .is_some_and(|p| p.eq_ignore_ascii_case("USB"))
        && status == Some("OK")
        && present != Some(false)
}

#[cfg(windows)]
mod imp {
    use avlt_core::sys::model::UsbDevice;
    use serde::Deserialize;

    use super::keep;
    use crate::Result;
    use crate::wmi::{connect, query};

    #[derive(Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct PnpEntity {
        name: Option<String>,
        #[serde(rename = "PNPClass")]
        pnp_class: Option<String>,
        #[serde(rename = "PNPDeviceID")]
        pnp_device_id: Option<String>,
        status: Option<String>,
        present: Option<bool>,
    }

    pub fn usb_devices() -> Result<Vec<UsbDevice>> {
        let con = connect("ROOT\\CIMV2")?;
        let rows: Vec<PnpEntity> = query(
            &con,
            "SELECT Name, PNPClass, PNPDeviceID, Status, Present FROM Win32_PnPEntity \
             WHERE PNPDeviceID LIKE 'USB%'",
        )?;
        Ok(rows
            .into_iter()
            .filter_map(|r| {
                let id = r.pnp_device_id?;
                keep(&id, r.status.as_deref(), r.present).then(|| UsbDevice {
                    class: r.pnp_class.unwrap_or_default(),
                    name: r.name.unwrap_or_default(),
                    instance_id: id,
                })
            })
            .collect())
    }
}

#[cfg(not(windows))]
mod imp {
    use avlt_core::sys::model::UsbDevice;

    use crate::{Error, Result};

    pub fn usb_devices() -> Result<Vec<UsbDevice>> {
        Err(Error::Unsupported)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_follows_usb_ps1() {
        assert!(keep(r"USB\VID_046D&PID_C52B\5&1", Some("OK"), Some(true)));
        assert!(keep(r"usb\ROOT_HUB30\4&1", Some("OK"), None));
        assert!(!keep(r"USB\VID_1&PID_2\1", Some("Error"), Some(true)));
        assert!(!keep(r"USB\VID_1&PID_2\1", Some("OK"), Some(false)));
        assert!(!keep(r"HID\VID_046D&PID_C52B", Some("OK"), Some(true)));
        assert!(!keep("US", Some("OK"), None));
    }

    #[cfg(windows)]
    #[test]
    fn lists_usb_devices() {
        // CI virtual machines may have no USB controller at all: only check the filter.
        let list = usb_devices().unwrap();
        assert!(
            list.iter()
                .all(|d| d.instance_id.to_uppercase().starts_with("USB"))
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn unsupported_elsewhere() {
        assert!(matches!(usb_devices(), Err(crate::Error::Unsupported)));
    }
}
