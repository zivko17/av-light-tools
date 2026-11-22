//! `sys cleanup` (Cleanup.ps1, NetMaint MaintenancePage): temp folders, browser caches and
//! the Recycle Bin. `C:\Windows\Prefetch` is deliberately not a target.

use std::path::{Path, PathBuf};

use avlt_core::sys::fsops::{clean_dir_contents, dir_size, unsafe_cleanup_reason};
use avlt_core::sys::model::{
    CleanStats, CleanStatus, CleanupAnalysis, CleanupItem, CleanupReport, CleanupTargetSize,
    RecycleBinInfo,
};

use crate::Result;

/// A folder whose contents `sys cleanup` deletes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanupTarget {
    pub name: String,
    pub path: PathBuf,
}

/// Cleanup.ps1 targets without Prefetch: user temp, Windows temp, Chrome and Edge caches.
pub fn cleanup_targets() -> Vec<CleanupTarget> {
    let local = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    let windows =
        std::env::var_os("SystemRoot").map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from);
    let mut targets = vec![
        target("User Temp", std::env::temp_dir()),
        target("Windows Temp", windows.join("Temp")),
    ];
    if let Some(local) = local {
        targets.push(target(
            "Chrome cache",
            local.join(r"Google\Chrome\User Data\Default\Cache"),
        ));
        targets.push(target(
            "Edge cache",
            local.join(r"Microsoft\Edge\User Data\Default\Cache"),
        ));
    }
    targets
}

/// Folders no cleanup target may be or contain: the user profile and the Windows folder.
pub fn protected_paths() -> Vec<PathBuf> {
    ["USERPROFILE", "SystemRoot", "HOME"]
        .iter()
        .filter_map(std::env::var_os)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .collect()
}

fn target(name: &str, path: PathBuf) -> CleanupTarget {
    CleanupTarget {
        name: name.to_string(),
        path,
    }
}

/// Sizes of the targets and of the Recycle Bin, deleting nothing (MaintenancePage
/// "Analyze sizes").
pub fn analyze() -> Result<CleanupAnalysis> {
    if !cfg!(windows) {
        return Err(crate::Error::Unsupported);
    }
    Ok(CleanupAnalysis {
        targets: cleanup_targets()
            .into_iter()
            .map(|t| {
                let (size_bytes, files) = dir_size(&t.path);
                CleanupTargetSize {
                    name: t.name,
                    path: t.path,
                    size_bytes,
                    files,
                }
            })
            .collect(),
        recycle_bin: imp::recycle_bin().unwrap_or_default(),
    })
}

/// Cleans every [`cleanup_targets`] folder and empties the Recycle Bin.
pub fn cleanup() -> Result<CleanupReport> {
    if !cfg!(windows) {
        return Err(crate::Error::Unsupported);
    }
    let mut report = clean(&cleanup_targets(), &protected_paths());
    let bin = imp::recycle_bin().unwrap_or_default();
    report.recycle_bin = bin;
    // Emptying an empty bin fails; that is "already empty", not an error.
    report.recycle_bin_emptied = bin.items > 0 && imp::empty_recycle_bin().is_ok();
    if report.recycle_bin_emptied {
        report.total_freed_bytes += bin.bytes;
    }
    Ok(report)
}

/// Empties each target folder (the folder itself stays). Pure file-system work, also
/// used by tests on temporary folders.
/// Targets that fail [`unsafe_cleanup_reason`] against `protected` are reported as
/// [`CleanStatus::Unsafe`] and left untouched.
pub fn clean(targets: &[CleanupTarget], protected: &[PathBuf]) -> CleanupReport {
    let items: Vec<CleanupItem> = targets
        .iter()
        .map(|t| {
            if let Some(reason) = unsafe_cleanup_reason(&t.path, protected) {
                return CleanupItem {
                    name: t.name.clone(),
                    path: t.path.clone(),
                    status: CleanStatus::Unsafe,
                    stats: CleanStats::default(),
                    skipped_reason: Some(format!("unsafe target: {reason}")),
                };
            }
            let stats = clean_dir_contents(Path::new(&t.path));
            CleanupItem {
                name: t.name.clone(),
                path: t.path.clone(),
                status: stats.status(),
                stats,
                skipped_reason: None,
            }
        })
        .collect();
    CleanupReport {
        total_freed_bytes: items.iter().map(|i| i.stats.freed_bytes).sum(),
        items,
        recycle_bin: RecycleBinInfo::default(),
        recycle_bin_emptied: false,
    }
}

#[cfg(windows)]
mod imp {
    use avlt_core::sys::model::RecycleBinInfo;
    use windows::Win32::UI::Shell::{
        SHERB_NOCONFIRMATION, SHERB_NOPROGRESSUI, SHERB_NOSOUND, SHEmptyRecycleBinW, SHQUERYRBINFO,
        SHQueryRecycleBinW,
    };

    use crate::{Error, Result};

    /// Size and item count of the Recycle Bin on all drives.
    pub fn recycle_bin() -> Result<RecycleBinInfo> {
        let mut info = SHQUERYRBINFO {
            cbSize: size_of::<SHQUERYRBINFO>() as u32,
            ..Default::default()
        };
        // SAFETY: `info` has cbSize set; a null root means every drive.
        unsafe { SHQueryRecycleBinW(None, &mut info) }
            .map_err(|e| Error::Other(format!("Recycle Bin: {}", e.message().trim())))?;
        Ok(RecycleBinInfo {
            bytes: u64::try_from(info.i64Size).unwrap_or(0),
            items: u64::try_from(info.i64NumItems).unwrap_or(0),
        })
    }

    /// Clear-RecycleBin -Force: every drive, no dialog, no sound.
    pub fn empty_recycle_bin() -> Result<()> {
        // SAFETY: no window, null root (all drives).
        unsafe {
            SHEmptyRecycleBinW(
                None,
                None,
                SHERB_NOCONFIRMATION | SHERB_NOPROGRESSUI | SHERB_NOSOUND,
            )
        }
        .map_err(|e| Error::Other(format!("Recycle Bin: {}", e.message().trim())))
    }
}

#[cfg(not(windows))]
mod imp {
    use avlt_core::sys::model::RecycleBinInfo;

    use crate::{Error, Result};

    pub fn recycle_bin() -> Result<RecycleBinInfo> {
        Err(Error::Unsupported)
    }

    pub fn empty_recycle_bin() -> Result<()> {
        Err(Error::Unsupported)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn targets_never_include_prefetch() {
        let targets = cleanup_targets();
        let names: Vec<&str> = targets.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(&names[..2], ["User Temp", "Windows Temp"]);
        assert!(
            targets
                .iter()
                .all(|t| !t.path.to_string_lossy().to_lowercase().contains("prefetch"))
        );
        assert!(!names.iter().any(|n| n.to_lowercase().contains("prefetch")));
    }

    /// Runs only on temporary folders: never the real targets.
    #[test]
    fn clean_reports_each_target() {
        let tmp = tempfile::tempdir().unwrap();
        let full = tmp.path().join("full");
        std::fs::create_dir_all(full.join("sub")).unwrap();
        std::fs::write(full.join("a.tmp"), vec![0u8; 300]).unwrap();
        std::fs::write(full.join("sub/b.tmp"), vec![0u8; 700]).unwrap();
        let empty = tmp.path().join("empty");
        std::fs::create_dir(&empty).unwrap();
        let targets = [
            target("Full", full.clone()),
            target("Empty", empty),
            target("Missing", tmp.path().join("missing")),
        ];
        let report = clean(&targets, &[]);
        assert_eq!(report.total_freed_bytes, 1000);
        assert_eq!(report.items[0].status, CleanStatus::Freed);
        assert_eq!(report.items[0].stats.freed_bytes, 1000);
        assert_eq!(report.items[1].status, CleanStatus::AlreadyClean);
        assert_eq!(report.items[2].status, CleanStatus::AlreadyClean);
        assert!(!report.items[2].stats.existed);
        assert!(full.is_dir());
        assert!(!report.recycle_bin_emptied);
    }

    /// A target that is (or contains) a protected folder, or too short, is never touched.
    #[test]
    fn unsafe_targets_are_skipped_untouched() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("profile");
        let inside = home.join("Temp");
        std::fs::create_dir_all(&inside).unwrap();
        std::fs::write(home.join("precious.doc"), b"keep").unwrap();
        std::fs::write(inside.join("junk.tmp"), b"junk").unwrap();
        let root = tmp.path().ancestors().last().unwrap().to_path_buf();
        let targets = [
            target("Ancestor", tmp.path().to_path_buf()),
            target("Profile", home.clone()),
            target("Root", root),
            target("Relative", PathBuf::from("Temp")),
            target("Inside", inside.clone()),
        ];
        let report = clean(&targets, std::slice::from_ref(&home));
        let status: Vec<CleanStatus> = report.items.iter().map(|i| i.status).collect();
        assert_eq!(
            status,
            [
                CleanStatus::Unsafe,
                CleanStatus::Unsafe,
                CleanStatus::Unsafe,
                CleanStatus::Unsafe,
                CleanStatus::Freed
            ]
        );
        assert!(report.items[0].skipped_reason.is_some());
        assert!(
            home.join("precious.doc").is_file(),
            "protected file deleted"
        );
        assert!(!inside.join("junk.tmp").exists());
        assert_eq!(report.total_freed_bytes, 4);
    }

    #[cfg(windows)]
    #[test]
    fn real_targets_pass_the_safety_check() {
        let protected = protected_paths();
        for t in cleanup_targets() {
            assert_eq!(
                avlt_core::sys::fsops::unsafe_cleanup_reason(&t.path, &protected),
                None,
                "{t:?}"
            );
        }
    }

    #[cfg(windows)]
    #[test]
    fn recycle_bin_can_be_queried() {
        // Read-only: never empties the bin.
        let _ = imp::recycle_bin().unwrap();
    }
}
