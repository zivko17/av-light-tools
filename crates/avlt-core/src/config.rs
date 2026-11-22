//! `avlt.toml` configuration, read from the directory that holds the executable.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::{Error, Result};

/// Name of the config file next to the executable.
pub const CONFIG_FILE: &str = "avlt.toml";
/// Portable output folder next to the executable.
pub const PORTABLE_OUTPUT_DIR: &str = "_OUTPUT";
/// City used by `util weather` when none is configured.
pub const DEFAULT_WEATHER_CITY: &str = "Madrid";

/// User configuration. Every field is optional.
#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Network interface used by `net setip` and friends.
    pub iface: Option<String>,
    /// City for `util weather` (default: [`DEFAULT_WEATHER_CITY`]).
    pub weather_city: Option<String>,
    /// Output folder override; relative paths are resolved against the exe folder.
    pub output_dir: Option<PathBuf>,
    /// Named IP profiles for `net setip <name>`.
    pub ip_profiles: BTreeMap<String, IpProfile>,
}

/// A static IPv4 profile.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IpProfile {
    /// Address in CIDR form, e.g. `"192.168.0.99/24"`.
    pub address: String,
    /// Optional default gateway.
    pub gateway: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            iface: None,
            weather_city: None,
            output_dir: None,
            ip_profiles: default_ip_profiles(),
        }
    }
}

/// The predefined profiles of `SetIP.ps1` (`$profiles`), used when the config defines none.
/// `dhcp` is not a profile here: `net setip dhcp` is a keyword of its own.
fn default_ip_profiles() -> BTreeMap<String, IpProfile> {
    [
        ("0", "192.168.0.99/24", "192.168.0.1"),
        ("1", "192.168.1.99/24", "192.168.1.1"),
    ]
    .into_iter()
    .map(|(name, address, gateway)| {
        (
            name.to_string(),
            IpProfile {
                address: address.to_string(),
                gateway: Some(gateway.to_string()),
            },
        )
    })
    .collect()
}

impl Config {
    /// Loads `exe_dir/avlt.toml`; returns the default config if the file does not exist.
    pub fn load(exe_dir: &Path) -> Result<Config> {
        let path = exe_dir.join(CONFIG_FILE);
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Config::default()),
            Err(e) => return Err(e.into()),
        };
        toml::from_str(&text).map_err(|e| Error::InvalidInput(format!("{}: {e}", path.display())))
    }

    /// City for `util weather`.
    pub fn weather_city(&self) -> &str {
        self.weather_city.as_deref().unwrap_or(DEFAULT_WEATHER_CITY)
    }

    /// Folder where avlt writes its output files.
    ///
    /// Order: the configured `output_dir` (relative to `exe_dir`), then
    /// `<exe_dir>/_OUTPUT` in portable mode, then `Documents\AVLT` in the user profile.
    pub fn output_dir(&self, exe_dir: &Path, portable: bool) -> PathBuf {
        if let Some(dir) = &self.output_dir {
            return exe_dir.join(dir);
        }
        if portable {
            return exe_dir.join(PORTABLE_OUTPUT_DIR);
        }
        match home_dir() {
            Some(home) => home.join("Documents").join("AVLT"),
            None => exe_dir.join(PORTABLE_OUTPUT_DIR),
        }
    }
}

/// Portable mode: `avlt.toml` or an `_OUTPUT` folder exists next to the executable.
pub fn is_portable(exe_dir: &Path) -> bool {
    exe_dir.join(CONFIG_FILE).is_file() || exe_dir.join(PORTABLE_OUTPUT_DIR).is_dir()
}

/// The user's profile folder (`%USERPROFILE%`, or `$HOME` elsewhere).
fn home_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(address: &str, gateway: &str) -> IpProfile {
        IpProfile {
            address: address.into(),
            gateway: Some(gateway.into()),
        }
    }

    #[test]
    fn load_without_file_returns_default() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = Config::load(dir.path()).unwrap();
        assert_eq!(cfg.iface, None);
        assert_eq!(cfg.weather_city, None);
        assert_eq!(cfg.weather_city(), "Madrid");
        assert_eq!(cfg.output_dir, None);
        assert_eq!(cfg.ip_profiles, Config::default().ip_profiles);
    }

    #[test]
    fn default_profiles_match_setip_ps1() {
        let cfg = Config::default();
        assert_eq!(cfg.ip_profiles.len(), 2);
        assert_eq!(
            cfg.ip_profiles["0"],
            profile("192.168.0.99/24", "192.168.0.1")
        );
        assert_eq!(
            cfg.ip_profiles["1"],
            profile("192.168.1.99/24", "192.168.1.1")
        );
    }

    #[test]
    fn load_reads_weather_city_and_profile() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(CONFIG_FILE),
            r#"
weather_city = "Ibiza"

[ip_profiles."0"]
address = "10.0.0.5/16"
gateway = "10.0.0.1"
"#,
        )
        .unwrap();
        let cfg = Config::load(dir.path()).unwrap();
        assert_eq!(cfg.weather_city.as_deref(), Some("Ibiza"));
        assert_eq!(cfg.weather_city(), "Ibiza");
        assert_eq!(cfg.ip_profiles.len(), 1);
        assert_eq!(cfg.ip_profiles["0"], profile("10.0.0.5/16", "10.0.0.1"));
    }

    #[test]
    fn load_rejects_malformed_toml() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(CONFIG_FILE), "weather_city = ").unwrap();
        let err = Config::load(dir.path()).unwrap_err();
        assert_eq!(err.exit_code(), 2);
        assert!(err.to_string().contains("avlt.toml"), "{err}");
    }

    #[test]
    fn portable_detection() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!is_portable(dir.path()));
        std::fs::create_dir(dir.path().join(PORTABLE_OUTPUT_DIR)).unwrap();
        assert!(is_portable(dir.path()));

        let dir2 = tempfile::tempdir().unwrap();
        std::fs::write(dir2.path().join(CONFIG_FILE), "").unwrap();
        assert!(is_portable(dir2.path()));
    }

    #[test]
    fn output_dir_portable_and_override() {
        let exe = Path::new("exe_dir");
        let cfg = Config::default();
        assert_eq!(cfg.output_dir(exe, true), exe.join("_OUTPUT"));
        assert!(
            cfg.output_dir(exe, false)
                .ends_with(Path::new("Documents").join("AVLT"))
        );

        let rel = Config {
            output_dir: Some(PathBuf::from("out")),
            ..Config::default()
        };
        assert_eq!(rel.output_dir(exe, false), exe.join("out"));
        assert_eq!(rel.output_dir(exe, true), exe.join("out"));
    }
}
