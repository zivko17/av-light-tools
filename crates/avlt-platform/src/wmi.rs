//! Thin helpers over the `wmi` crate (`::wmi`; this module shadows the crate name).
//!
//! The connection is `!Send`: create one per thread, per namespace.

#[cfg(windows)]
use crate::{Error, Result};

/// Connects to a WMI namespace such as `ROOT\CIMV2` or `root\wmi`.
#[cfg(windows)]
pub(crate) fn connect(namespace: &str) -> Result<::wmi::WMIConnection> {
    ::wmi::WMIConnection::with_namespace_path(namespace)
        .map_err(|e| Error::Other(format!("WMI {namespace}: {e}")))
}

/// Runs a WQL query and deserializes every row into `T`
/// (`#[serde(rename_all = "PascalCase")]` structs with optional fields).
#[cfg(windows)]
pub(crate) fn query<T: serde::de::DeserializeOwned>(
    con: &::wmi::WMIConnection,
    wql: &str,
) -> Result<Vec<T>> {
    con.raw_query(wql)
        .map_err(|e| Error::Other(format!("WMI query '{wql}': {e}")))
}

/// A CIM `DATETIME` (`yyyymmddHHMMSS.mmmmmmsUUU`) as `yyyy-MM-dd HH:mm`, in the time
/// zone it was written in. `None` if it is not a datetime.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn cim_datetime(s: &str) -> Option<String> {
    let d = s.get(..12)?;
    if !d.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let (month, day) = (&d[4..6], &d[6..8]);
    if month == "00" || day == "00" {
        return None;
    }
    Some(format!(
        "{}-{month}-{day} {}:{}",
        &d[..4],
        &d[8..10],
        &d[10..12]
    ))
}

/// The date part of [`cim_datetime`] (`yyyy-MM-dd`).
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn cim_date(s: &str) -> Option<String> {
    cim_datetime(s).map(|d| d[..10].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cim_datetimes() {
        assert_eq!(
            cim_datetime("20231015123045.000000+060").as_deref(),
            Some("2023-10-15 12:30")
        );
        assert_eq!(
            cim_date("20240131000000.000000-000").as_deref(),
            Some("2024-01-31")
        );
        assert_eq!(cim_datetime(""), None);
        assert_eq!(cim_datetime("2023101512"), None);
        assert_eq!(cim_datetime("abcd1015123045.000000+060"), None);
        assert_eq!(cim_datetime("20230000000000.000000+000"), None);
    }

    #[cfg(windows)]
    #[test]
    fn queries_the_operating_system() {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "PascalCase")]
        struct Os {
            caption: Option<String>,
            install_date: Option<String>,
        }
        let con = connect("ROOT\\CIMV2").unwrap();
        let rows: Vec<Os> = query(
            &con,
            "SELECT Caption, InstallDate FROM Win32_OperatingSystem",
        )
        .unwrap();
        assert_eq!(rows.len(), 1);
        assert!(
            rows[0]
                .caption
                .as_deref()
                .is_some_and(|c| c.contains("Windows"))
        );
        assert!(
            rows[0]
                .install_date
                .as_deref()
                .and_then(cim_datetime)
                .is_some()
        );
    }
}
