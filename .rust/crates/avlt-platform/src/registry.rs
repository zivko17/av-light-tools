//! Read-only registry access (startup entries, GPU memory size). Never writes.

use crate::Result;

/// Root key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hive {
    /// `HKLM`
    LocalMachine,
    /// `HKCU`
    CurrentUser,
}

/// Every value of `hive\path` as text (strings as stored, numbers in decimal,
/// the unnamed default value as `(default)`), in registry order.
/// `Ok(None)` when the key does not exist.
pub fn values(hive: Hive, path: &str) -> Result<Option<Vec<(String, String)>>> {
    imp::values(hive, path)
}

/// Names of the subkeys of `hive\path`; `Ok(None)` when the key does not exist.
pub fn subkeys(hive: Hive, path: &str) -> Result<Option<Vec<String>>> {
    imp::subkeys(hive, path)
}

/// A numeric value: `REG_DWORD`, `REG_QWORD` or a little-endian `REG_BINARY` of up to
/// 8 bytes. `Ok(None)` when the key or value is missing or not numeric.
pub fn number(hive: Hive, path: &str, name: &str) -> Result<Option<u64>> {
    imp::number(hive, path, name)
}

/// Little-endian bytes as a number (at most 8 bytes).
#[cfg_attr(not(windows), allow(dead_code))]
fn le_number(bytes: &[u8]) -> Option<u64> {
    if bytes.is_empty() || bytes.len() > 8 {
        return None;
    }
    let mut buf = [0u8; 8];
    buf[..bytes.len()].copy_from_slice(bytes);
    Some(u64::from_le_bytes(buf))
}

#[cfg(windows)]
mod imp {
    use std::io::ErrorKind;

    use winreg::RegKey;
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, RegType};

    use super::{Hive, le_number};
    use crate::{Error, Result};

    fn open(hive: Hive, path: &str) -> Result<Option<RegKey>> {
        let root = RegKey::predef(match hive {
            Hive::LocalMachine => HKEY_LOCAL_MACHINE,
            Hive::CurrentUser => HKEY_CURRENT_USER,
        });
        match root.open_subkey_with_flags(path, KEY_READ) {
            Ok(k) => Ok(Some(k)),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
            Err(e) => Err(Error::Other(format!("registry {path}: {e}"))),
        }
    }

    pub fn values(hive: Hive, path: &str) -> Result<Option<Vec<(String, String)>>> {
        let Some(key) = open(hive, path)? else {
            return Ok(None);
        };
        let mut out = Vec::new();
        for item in key.enum_values() {
            let Ok((name, value)) = item else { continue };
            let text = match value.vtype {
                RegType::REG_SZ | RegType::REG_EXPAND_SZ | RegType::REG_MULTI_SZ => {
                    key.get_value::<String, _>(&name).unwrap_or_default()
                }
                RegType::REG_DWORD | RegType::REG_QWORD => le_number(&value.bytes)
                    .map(|n| n.to_string())
                    .unwrap_or_default(),
                _ => value
                    .bytes
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<Vec<_>>()
                    .join(" "),
            };
            let name = if name.is_empty() {
                "(default)".to_string()
            } else {
                name
            };
            out.push((name, text.trim_end_matches('\0').to_string()));
        }
        Ok(Some(out))
    }

    pub fn subkeys(hive: Hive, path: &str) -> Result<Option<Vec<String>>> {
        let Some(key) = open(hive, path)? else {
            return Ok(None);
        };
        Ok(Some(key.enum_keys().filter_map(|k| k.ok()).collect()))
    }

    pub fn number(hive: Hive, path: &str, name: &str) -> Result<Option<u64>> {
        let Some(key) = open(hive, path)? else {
            return Ok(None);
        };
        let Ok(value) = key.get_raw_value(name) else {
            return Ok(None);
        };
        Ok(match value.vtype {
            RegType::REG_DWORD | RegType::REG_QWORD | RegType::REG_BINARY => {
                le_number(&value.bytes)
            }
            _ => None,
        })
    }
}

#[cfg(not(windows))]
mod imp {
    use super::Hive;
    use crate::{Error, Result};

    pub fn values(_hive: Hive, _path: &str) -> Result<Option<Vec<(String, String)>>> {
        Err(Error::Unsupported)
    }

    pub fn subkeys(_hive: Hive, _path: &str) -> Result<Option<Vec<String>>> {
        Err(Error::Unsupported)
    }

    pub fn number(_hive: Hive, _path: &str, _name: &str) -> Result<Option<u64>> {
        Err(Error::Unsupported)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn little_endian_numbers() {
        assert_eq!(le_number(&[0x00, 0x00, 0x00, 0x00, 0x02]), Some(1 << 33));
        assert_eq!(le_number(&[0x2a, 0, 0, 0]), Some(42));
        assert_eq!(le_number(&[]), None);
        assert_eq!(le_number(&[0; 9]), None);
    }

    #[cfg(windows)]
    #[test]
    fn reads_a_key_that_always_exists() {
        let path = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";
        let vals = values(Hive::LocalMachine, path).unwrap().unwrap();
        assert!(
            vals.iter()
                .any(|(n, v)| n == "ProductName" && !v.is_empty())
        );
        assert!(
            number(Hive::LocalMachine, path, "CurrentMajorVersionNumber")
                .unwrap()
                .is_some_and(|v| v >= 10)
        );
        assert!(
            subkeys(Hive::LocalMachine, r"SOFTWARE\Microsoft")
                .unwrap()
                .unwrap()
                .iter()
                .any(|k| k.eq_ignore_ascii_case("Windows"))
        );
        assert_eq!(
            values(Hive::CurrentUser, r"Software\avlt-no-such-key").unwrap(),
            None
        );
        assert_eq!(
            number(Hive::LocalMachine, path, "NoSuchValue").unwrap(),
            None
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn unsupported_elsewhere() {
        assert!(matches!(
            values(Hive::LocalMachine, "x"),
            Err(crate::Error::Unsupported)
        ));
    }
}
