//! `sys battery` (Battery.ps1, AVToolbox `Battery`).

use avlt_core::sys::model::BatteryInfo;

use crate::Result;

/// The first battery (`Win32_Battery`) with health from `root\wmi`
/// (`BatteryStaticData.DesignedCapacity`, `BatteryFullChargedCapacity`).
/// `Ok(None)` on a PC without battery.
pub fn battery() -> Result<Option<BatteryInfo>> {
    imp::battery()
}

/// Builds the result from the raw WMI values (Battery.ps1 rules).
#[cfg_attr(not(windows), allow(dead_code))]
fn build(
    charge: u32,
    status: u32,
    runtime_raw: Option<u32>,
    designed: Option<u32>,
    full: Option<u32>,
) -> BatteryInfo {
    use avlt_core::sys::format::{battery_health, battery_runtime, battery_status_text};
    let health = match (designed, full) {
        (Some(d), Some(f)) => battery_health(d, f),
        _ => None,
    };
    BatteryInfo {
        charge_pct: charge.min(100),
        status_code: status,
        status: battery_status_text(status),
        runtime_min: runtime_raw.and_then(battery_runtime),
        designed_capacity_mwh: designed.filter(|&d| d > 0),
        full_charged_capacity_mwh: full.filter(|&f| f > 0),
        health_pct: health,
    }
}

#[cfg(windows)]
mod imp {
    use avlt_core::sys::model::BatteryInfo;
    use serde::Deserialize;

    use super::build;
    use crate::Result;
    use crate::wmi::{connect, query};

    #[derive(Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct Win32Battery {
        estimated_charge_remaining: Option<u32>,
        battery_status: Option<u32>,
        estimated_run_time: Option<u32>,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct StaticData {
        designed_capacity: Option<u32>,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct FullCharged {
        full_charged_capacity: Option<u32>,
    }

    pub fn battery() -> Result<Option<BatteryInfo>> {
        let con = connect("ROOT\\CIMV2")?;
        let rows: Vec<Win32Battery> = query(
            &con,
            "SELECT EstimatedChargeRemaining, BatteryStatus, EstimatedRunTime FROM Win32_Battery",
        )?;
        let Some(b) = rows.into_iter().next() else {
            return Ok(None);
        };
        // Health is best effort, as in Battery.ps1 (try/catch around it).
        let (designed, full) = match connect("root\\wmi") {
            Ok(wmi) => (
                query::<StaticData>(&wmi, "SELECT DesignedCapacity FROM BatteryStaticData")
                    .ok()
                    .and_then(|r| r.into_iter().next())
                    .and_then(|s| s.designed_capacity),
                query::<FullCharged>(
                    &wmi,
                    "SELECT FullChargedCapacity FROM BatteryFullChargedCapacity",
                )
                .ok()
                .and_then(|r| r.into_iter().next())
                .and_then(|f| f.full_charged_capacity),
            ),
            Err(_) => (None, None),
        };
        Ok(Some(build(
            b.estimated_charge_remaining.unwrap_or(0),
            b.battery_status.unwrap_or(0),
            b.estimated_run_time,
            designed,
            full,
        )))
    }
}

#[cfg(not(windows))]
mod imp {
    use avlt_core::sys::model::BatteryInfo;

    use crate::{Error, Result};

    pub fn battery() -> Result<Option<BatteryInfo>> {
        Err(Error::Unsupported)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_from_raw_values() {
        let b = build(87, 2, Some(71_582_788), Some(50_000), Some(42_500));
        assert_eq!(b.charge_pct, 87);
        assert_eq!(b.status, "Plugged in (AC)");
        assert_eq!(b.runtime_min, None);
        assert_eq!(b.health_pct, Some(85.0));
        let b = build(150, 1, Some(125), None, Some(1));
        assert_eq!(b.charge_pct, 100);
        assert_eq!(b.status, "Discharging");
        assert_eq!(b.runtime_min, Some(125));
        assert_eq!(b.health_pct, None);
        assert_eq!(b.designed_capacity_mwh, None);
    }

    #[cfg(windows)]
    #[test]
    fn query_succeeds_with_or_without_battery() {
        if let Some(b) = battery().unwrap() {
            assert!(b.charge_pct <= 100);
        }
    }

    #[cfg(not(windows))]
    #[test]
    fn unsupported_elsewhere() {
        assert!(matches!(battery(), Err(crate::Error::Unsupported)));
    }
}
