//! `sys startup` (Startup.ps1): `Run` / `RunOnce` registry entries and Startup folders.
//! Read-only: nothing is enabled, disabled or removed.

use std::path::Path;

use avlt_core::sys::model::StartupEntry;

use crate::Result;

/// The registry sources of Startup.ps1, in its order: label, hive, key.
pub const STARTUP_KEYS: [(&str, crate::registry::Hive, &str); 5] = {
    use crate::registry::Hive::{CurrentUser, LocalMachine};
    [
        (
            "HKLM Run",
            LocalMachine,
            r"Software\Microsoft\Windows\CurrentVersion\Run",
        ),
        (
            "HKLM RunOnce",
            LocalMachine,
            r"Software\Microsoft\Windows\CurrentVersion\RunOnce",
        ),
        (
            "HKCU Run",
            CurrentUser,
            r"Software\Microsoft\Windows\CurrentVersion\Run",
        ),
        (
            "HKCU RunOnce",
            CurrentUser,
            r"Software\Microsoft\Windows\CurrentVersion\RunOnce",
        ),
        (
            "HKLM x86 Run",
            LocalMachine,
            r"Software\Wow6432Node\Microsoft\Windows\CurrentVersion\Run",
        ),
    ]
};

/// Label of the Startup folder entries.
pub const STARTUP_FOLDER: &str = "Startup folder";

/// Every startup entry: the five registry keys, then the user's and all users'
/// Startup folders (`%APPDATA%`, `%ProgramData%`).
pub fn startup_entries() -> Result<Vec<StartupEntry>> {
    if !cfg!(windows) {
        return Err(crate::Error::Unsupported);
    }
    let mut found = Vec::new();
    for (source, hive, key) in STARTUP_KEYS {
        // A key that cannot be read is skipped, like Get-ItemProperty -ErrorAction
        // SilentlyContinue.
        if let Ok(Some(values)) = crate::registry::values(hive, key) {
            found.extend(values.into_iter().map(|(name, command)| StartupEntry {
                source: source.to_string(),
                name,
                command,
            }));
        }
    }
    let rel = r"Microsoft\Windows\Start Menu\Programs\Startup";
    for var in ["APPDATA", "ProgramData"] {
        if let Some(base) = std::env::var_os(var) {
            found.extend(folder_entries(&Path::new(&base).join(rel)));
        }
    }
    Ok(found)
}

/// Visible files of a Startup folder (Get-ChildItem -File: hidden files such as
/// `desktop.ini` are left out), sorted by name.
pub fn folder_entries(dir: &Path) -> Vec<StartupEntry> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<StartupEntry> = entries
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .filter(|e| e.metadata().is_ok_and(|m| !is_hidden(&m)))
        .map(|e| StartupEntry {
            source: STARTUP_FOLDER.to_string(),
            name: e.file_name().to_string_lossy().into_owned(),
            command: e.path().display().to_string(),
        })
        .collect();
    out.sort_by_key(|e| e.name.to_lowercase());
    out
}

#[cfg(windows)]
fn is_hidden(meta: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
    meta.file_attributes() & FILE_ATTRIBUTE_HIDDEN != 0
}

#[cfg(not(windows))]
fn is_hidden(_meta: &std::fs::Metadata) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folder_lists_visible_files_only() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("Zoom.lnk"), b"x").unwrap();
        std::fs::write(tmp.path().join("arena launcher.lnk"), b"x").unwrap();
        std::fs::create_dir(tmp.path().join("subfolder")).unwrap();
        let ini = tmp.path().join("desktop.ini");
        std::fs::write(&ini, b"[.ShellClassInfo]").unwrap();
        #[cfg(windows)]
        {
            let status = std::process::Command::new("attrib")
                .arg("+h")
                .arg(&ini)
                .status()
                .unwrap();
            assert!(status.success());
        }
        #[cfg(not(windows))]
        std::fs::remove_file(&ini).unwrap();

        let list = folder_entries(tmp.path());
        let names: Vec<&str> = list.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["arena launcher.lnk", "Zoom.lnk"]);
        assert!(list.iter().all(|e| e.source == STARTUP_FOLDER));
        assert!(list[1].command.ends_with("Zoom.lnk"));
        assert!(folder_entries(&tmp.path().join("missing")).is_empty());
    }

    #[test]
    fn keys_match_startup_ps1() {
        let labels: Vec<&str> = STARTUP_KEYS.iter().map(|k| k.0).collect();
        assert_eq!(
            labels,
            [
                "HKLM Run",
                "HKLM RunOnce",
                "HKCU Run",
                "HKCU RunOnce",
                "HKLM x86 Run"
            ]
        );
    }

    #[cfg(windows)]
    #[test]
    fn reads_without_error() {
        let list = startup_entries().unwrap();
        assert!(list.iter().all(|e| !e.source.is_empty()));
    }

    #[cfg(not(windows))]
    #[test]
    fn unsupported_elsewhere() {
        assert!(matches!(startup_entries(), Err(crate::Error::Unsupported)));
    }
}
