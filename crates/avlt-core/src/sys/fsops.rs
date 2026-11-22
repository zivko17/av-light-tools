//! File-system work behind `sys cleanup`, `util backup` and `util search`
//! (Cleanup.ps1, Backup.ps1, Search.ps1, NetMaint MaintenancePage.cs).
//!
//! Plain `std::fs`: testable on any platform in a temporary folder.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::sys::model::{BackupReport, CleanStats, SearchHit};

/// Deletes everything inside `dir` (the folder itself stays) in one walk, adding up the
/// size of each file that was actually deleted. Files in use or without permission are
/// skipped and counted in `failed_files`; a missing `dir` is not an error.
pub fn clean_dir_contents(dir: &Path) -> CleanStats {
    let mut stats = CleanStats {
        existed: dir.is_dir(),
        ..CleanStats::default()
    };
    if stats.existed {
        clean_into(dir, &mut stats);
    }
    stats
}

fn clean_into(dir: &Path, stats: &mut CleanStats) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(kind) = entry.file_type() else {
            stats.failed_files += 1;
            continue;
        };
        if kind.is_dir() {
            clean_into(&path, stats);
            // Fails (and the folder stays) while something inside could not be deleted.
            let _ = fs::remove_dir(&path);
        } else if kind.is_symlink() {
            // Links (and Windows junctions) are removed, never followed.
            if fs::remove_file(&path)
                .or_else(|_| fs::remove_dir(&path))
                .is_ok()
            {
                stats.deleted_files += 1;
            } else {
                stats.failed_files += 1;
            }
        } else {
            let len = entry.metadata().map(|m| m.len()).unwrap_or(0);
            if remove_file(&path) {
                stats.deleted_files += 1;
                stats.freed_bytes += len;
            } else {
                stats.failed_files += 1;
            }
        }
    }
}

/// Deletes a file; on Windows a read-only file is made writable first
/// (MaintenancePage `File.SetAttributes(f, FileAttributes.Normal)`).
fn remove_file(path: &Path) -> bool {
    match fs::remove_file(path) {
        Ok(()) => true,
        #[cfg(windows)]
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            let Ok(meta) = fs::metadata(path) else {
                return false;
            };
            let mut perm = meta.permissions();
            if !perm.readonly() {
                return false;
            }
            // Windows only: clears FILE_ATTRIBUTE_READONLY (no Unix mode bits involved).
            #[allow(clippy::permissions_set_readonly_false)]
            perm.set_readonly(false);
            fs::set_permissions(path, perm).is_ok() && fs::remove_file(path).is_ok()
        }
        Err(_) => false,
    }
}

/// Total size and file count under `dir` (MaintenancePage `DirSize`); unreadable entries
/// are skipped, a missing folder is `(0, 0)`.
pub fn dir_size(dir: &Path) -> (u64, u64) {
    let (mut bytes, mut files) = (0, 0);
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = fs::read_dir(&d) else {
            continue;
        };
        for entry in entries.flatten() {
            match entry.file_type() {
                Ok(t) if t.is_dir() => stack.push(entry.path()),
                Ok(t) if t.is_file() => {
                    files += 1;
                    bytes += entry.metadata().map(|m| m.len()).unwrap_or(0);
                }
                _ => {}
            }
        }
    }
    (bytes, files)
}

/// Why `target` must not be emptied by `sys cleanup`, or `None` when it is safe:
/// relative paths and `..`, drive / share roots, paths with fewer than 3 components
/// (`C:\Temp` is the shortest allowed), and any path equal to or containing one of
/// `protected` (the user profile, the Windows folder). Case-insensitive.
pub fn unsafe_cleanup_reason(target: &Path, protected: &[PathBuf]) -> Option<String> {
    if let Some(reason) = lexical_reason(target, protected) {
        return Some(reason);
    }
    // Links and junctions must not smuggle a protected folder in: judge the real path.
    if fs::symlink_metadata(target).is_ok() {
        return match fs::canonicalize(target) {
            Ok(real) => lexical_reason(&strip_verbatim(real), protected),
            Err(e) => Some(format!("cannot resolve the real path ({e})")),
        };
    }
    None
}

fn lexical_reason(target: &Path, protected: &[PathBuf]) -> Option<String> {
    if target.as_os_str().is_empty() || !target.is_absolute() {
        return Some("not an absolute path".into());
    }
    let Some(parts) = normalized(target) else {
        return Some("contains '..'".into());
    };
    if parts.len() < 3 {
        return Some("too close to the drive root".into());
    }
    for p in protected {
        let real = fs::canonicalize(p).map(strip_verbatim).ok();
        for candidate in std::iter::once(p).chain(real.as_ref()) {
            if let Some(prot) = normalized(candidate)
                && prot.starts_with(&parts)
            {
                return Some(format!("is or contains {}", p.display()));
            }
        }
    }
    None
}

/// Drops the verbatim prefix `canonicalize` adds on Windows (`\\?\C:\x` -> `C:\x`,
/// `\\?\UNC\s\x` -> `\\s\x`).
fn strip_verbatim(p: PathBuf) -> PathBuf {
    let text = p.to_string_lossy();
    if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        PathBuf::from(format!(r"\\{rest}"))
    } else if let Some(rest) = text.strip_prefix(r"\\?\") {
        PathBuf::from(rest)
    } else {
        p
    }
}

/// Lower-cased path components (`None` if the path has `..`).
fn normalized(p: &Path) -> Option<Vec<String>> {
    use std::path::Component;
    let mut out = Vec::new();
    for c in p.components() {
        match c {
            Component::Prefix(x) => out.push(x.as_os_str().to_string_lossy().to_lowercase()),
            Component::RootDir => out.push(std::path::MAIN_SEPARATOR.to_string()),
            Component::Normal(s) => out.push(s.to_string_lossy().to_lowercase()),
            Component::CurDir => {}
            Component::ParentDir => return None,
        }
    }
    Some(out)
}

/// Backup folder name of Backup.ps1 / MaintenancePage: `<name>_backup_<stamp>`, where
/// `stamp` is the local time as `yyyy-MM-dd_HH-mm`.
pub fn backup_name(source_name: &str, stamp: &str) -> String {
    format!("{source_name}_backup_{stamp}")
}

/// `path` if nothing exists there, else `path_1`, `path_2`... (never overwrite).
pub fn unique_path(path: &Path) -> PathBuf {
    if fs::symlink_metadata(path).is_err() {
        return path.to_path_buf();
    }
    let base = path.as_os_str().to_os_string();
    (1u64..)
        .map(|n| {
            let mut candidate = base.clone();
            candidate.push(format!("_{n}"));
            PathBuf::from(candidate)
        })
        .find(|p| fs::symlink_metadata(p).is_err())
        .unwrap_or_else(|| path.to_path_buf())
}

/// Copies `source` (file or folder tree) into a new folder
/// `<dest_root>/<name>_backup_<stamp>` (suffixed `_1`, `_2`... if it exists).
/// `dest_root` is created when missing. Files that cannot be copied are listed in
/// `failed` and the copy goes on (MaintenancePage `BackupAsync`).
pub fn backup(source: &Path, dest_root: &Path, stamp: &str) -> Result<BackupReport> {
    let meta = fs::metadata(source).map_err(|_| {
        Error::InvalidInput(format!("source path '{}' does not exist", source.display()))
    })?;
    let is_file = meta.is_file();
    fs::create_dir_all(dest_root)?;
    let destination = unique_path(&dest_root.join(backup_name(&source_name(source), stamp)));

    // List everything first so a destination inside the source is never copied into itself.
    let mut dirs: Vec<PathBuf> = Vec::new();
    let mut files: Vec<(PathBuf, PathBuf)> = Vec::new();
    let mut failed: Vec<String> = Vec::new();
    if is_file {
        let name = source.file_name().map(PathBuf::from).unwrap_or_default();
        files.push((source.to_path_buf(), name));
    } else {
        let skip = inner_dir(source, dest_root);
        collect_tree(
            source,
            source,
            skip.as_deref(),
            &mut dirs,
            &mut files,
            &mut failed,
        );
    }

    fs::create_dir_all(&destination)?;
    let mut report = BackupReport {
        source: source.to_path_buf(),
        destination: destination.clone(),
        is_file,
        files: 0,
        bytes: 0,
        failed,
    };
    for rel in dirs {
        if let Err(e) = fs::create_dir_all(destination.join(&rel)) {
            report.failed.push(format!("{}: {e}", rel.display()));
        }
    }
    for (full, rel) in files {
        let target = destination.join(&rel);
        let copied = target
            .parent()
            .map_or(Ok(()), fs::create_dir_all)
            .and_then(|()| fs::copy(&full, &target));
        match copied {
            Ok(bytes) => {
                report.files += 1;
                report.bytes += bytes;
            }
            Err(e) => report.failed.push(format!("{}: {e}", rel.display())),
        }
    }
    Ok(report)
}

/// Last path component, or the drive letter for a root such as `D:\`.
fn source_name(source: &Path) -> String {
    if let Some(name) = source.file_name() {
        return name.to_string_lossy().into_owned();
    }
    let cleaned: String = source
        .to_string_lossy()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect();
    if cleaned.is_empty() {
        "backup".to_string()
    } else {
        cleaned
    }
}

/// Canonical `dest_root` when it lies inside `source` (that folder is left out of the copy).
fn inner_dir(source: &Path, dest_root: &Path) -> Option<PathBuf> {
    let src = fs::canonicalize(source).ok()?;
    let dst = fs::canonicalize(dest_root).ok()?;
    dst.starts_with(&src).then_some(dst)
}

/// What a directory entry is, as far as a backup cares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Dir,
    File,
    /// Symlink or junction: never followed, never copied.
    Link,
    Other,
}

/// Entries of a folder (path and kind), or why it cannot be listed.
type DirReader<'a> = dyn Fn(&Path) -> io::Result<Vec<(PathBuf, Kind)>> + 'a;

fn read_entries(dir: &Path) -> io::Result<Vec<(PathBuf, Kind)>> {
    let mut out = Vec::new();
    for entry in fs::read_dir(dir)?.flatten() {
        let kind = match entry.file_type() {
            Ok(t) if t.is_symlink() => Kind::Link,
            Ok(t) if t.is_dir() => Kind::Dir,
            Ok(t) if t.is_file() => Kind::File,
            _ => Kind::Other,
        };
        out.push((entry.path(), kind));
    }
    Ok(out)
}

/// Lists the tree under `dir`. Folders that cannot be read and links that are not
/// followed are recorded in `failed`, so a backup never looks complete when it is not.
fn collect_tree(
    root: &Path,
    dir: &Path,
    skip: Option<&Path>,
    dirs: &mut Vec<PathBuf>,
    files: &mut Vec<(PathBuf, PathBuf)>,
    failed: &mut Vec<String>,
) {
    collect_tree_with(&read_entries, root, dir, skip, dirs, files, failed);
}

fn collect_tree_with(
    read: &DirReader<'_>,
    root: &Path,
    dir: &Path,
    skip: Option<&Path>,
    dirs: &mut Vec<PathBuf>,
    files: &mut Vec<(PathBuf, PathBuf)>,
    failed: &mut Vec<String>,
) {
    let entries = match read(dir) {
        Ok(e) => e,
        Err(e) => {
            let shown = dir.strip_prefix(root).unwrap_or(dir);
            failed.push(format!("{}: cannot read folder: {e}", shown.display()));
            return;
        }
    };
    for (path, kind) in entries {
        let Ok(rel) = path.strip_prefix(root).map(Path::to_path_buf) else {
            continue;
        };
        match kind {
            Kind::Dir => {
                if skip.is_some_and(|s| fs::canonicalize(&path).is_ok_and(|c| c == s)) {
                    continue;
                }
                dirs.push(rel);
                collect_tree_with(read, root, &path, skip, dirs, files, failed);
            }
            Kind::File => files.push((path, rel)),
            Kind::Link => failed.push(format!("{}: symlink/junction not copied", rel.display())),
            Kind::Other => {}
        }
    }
}

/// PowerShell `-like` match, case-insensitive: `*` any run, `?` one character.
pub fn like_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().flat_map(char::to_lowercase).collect();
    let t: Vec<char> = text.chars().flat_map(char::to_lowercase).collect();
    let (mut pi, mut ti) = (0, 0);
    // Pattern position after the last `*`, and the text position that `*` has reached.
    let mut star: Option<(usize, usize)> = None;
    while ti < t.len() {
        if pi < p.len() && p[pi] != '*' && (p[pi] == '?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            pi += 1;
            star = Some((pi, ti));
        } else if let Some((sp, st)) = star {
            pi = sp;
            ti = st + 1;
            star = Some((sp, st + 1));
        } else {
            return false;
        }
    }
    p[pi..].iter().all(|&c| c == '*')
}

/// Files and folders under `root` whose name is like `*<pattern>*` (Search.ps1),
/// sorted by path. Unreadable folders are skipped; links are not followed.
pub fn search(root: &Path, pattern: &str) -> Result<Vec<SearchHit>> {
    let pattern = pattern.trim();
    if pattern.is_empty() {
        return Err(Error::InvalidInput("search pattern is empty".into()));
    }
    if !root.is_dir() {
        return Err(Error::InvalidInput(format!(
            "folder '{}' does not exist",
            root.display()
        )));
    }
    let wrapped = format!("*{pattern}*");
    let mut hits = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                stack.push(entry.path());
            }
            if !like_match(&wrapped, &entry.file_name().to_string_lossy()) {
                continue;
            }
            let meta = entry.metadata().ok();
            hits.push(SearchHit {
                path: entry.path(),
                is_dir: kind.is_dir(),
                size_bytes: meta.as_ref().filter(|m| m.is_file()).map_or(0, |m| m.len()),
                modified_unix: meta.and_then(|m| m.modified().ok()).map(unix_seconds),
            });
        }
    }
    hits.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(hits)
}

fn unix_seconds(t: std::time::SystemTime) -> i64 {
    match t.duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => i64::try_from(d.as_secs()).unwrap_or(i64::MAX),
        Err(e) => -i64::try_from(e.duration().as_secs()).unwrap_or(i64::MAX),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, len: usize) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, vec![b'x'; len]).unwrap();
    }

    #[test]
    fn clean_frees_exact_bytes_and_keeps_the_folder() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("Temp");
        write(&dir.join("a.tmp"), 100);
        write(&dir.join("sub/b.tmp"), 2000);
        write(&dir.join("sub/deeper/c.log"), 3000);
        fs::create_dir_all(dir.join("empty")).unwrap();
        let ro = dir.join("readonly.tmp");
        write(&ro, 7);
        let mut perm = fs::metadata(&ro).unwrap().permissions();
        perm.set_readonly(true);
        fs::set_permissions(&ro, perm).unwrap();

        assert_eq!(dir_size(&dir), (5107, 4));
        let stats = clean_dir_contents(&dir);
        assert!(stats.existed);
        assert_eq!(stats.freed_bytes, 5107);
        assert_eq!(stats.deleted_files, 4);
        assert_eq!(stats.failed_files, 0);
        assert!(dir.is_dir(), "the target folder itself must stay");
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 0);
        assert_eq!(dir_size(&dir), (0, 0));
    }

    #[test]
    fn clean_missing_folder_is_already_clean() {
        let tmp = tempfile::tempdir().unwrap();
        let stats = clean_dir_contents(&tmp.path().join("nope"));
        assert!(!stats.existed);
        assert_eq!(stats.freed_bytes, 0);
        assert_eq!(stats.failed_files, 0);
        assert_eq!(dir_size(&tmp.path().join("nope")), (0, 0));
    }

    /// Opens `path` so that it cannot be deleted while the handle lives.
    #[cfg(windows)]
    fn lock(path: &Path) -> Option<fs::File> {
        use std::os::windows::fs::OpenOptionsExt;
        // No FILE_SHARE_DELETE: DeleteFile fails with a sharing violation.
        Some(
            fs::OpenOptions::new()
                .read(true)
                .share_mode(0)
                .open(path)
                .unwrap(),
        )
    }

    /// Unix has no mandatory locks: a read-only parent folder blocks the delete instead.
    /// Returns `None` when running as root (permissions are not enforced).
    #[cfg(unix)]
    fn lock(path: &Path) -> Option<fs::File> {
        use std::os::unix::fs::PermissionsExt;
        let parent = path.parent().unwrap();
        fs::set_permissions(parent, fs::Permissions::from_mode(0o555)).unwrap();
        if fs::write(parent.join("probe"), b"").is_ok() {
            return None;
        }
        Some(fs::File::open(path).unwrap())
    }

    #[cfg(unix)]
    fn unlock(path: &Path) {
        use std::os::unix::fs::PermissionsExt;
        let parent = path.parent().unwrap();
        let _ = fs::set_permissions(parent, fs::Permissions::from_mode(0o755));
    }

    #[cfg(windows)]
    fn unlock(_path: &Path) {}

    #[test]
    fn clean_skips_a_locked_file_and_counts_only_what_went_away() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("Temp");
        write(&dir.join("free.tmp"), 1234);
        let locked = dir.join("locked/in use.tmp");
        write(&locked, 500);
        write(&dir.join("z.tmp"), 66);
        let Some(handle) = lock(&locked) else {
            unlock(&locked);
            return; // root on unix: nothing can be locked
        };
        let stats = clean_dir_contents(&dir);
        drop(handle);
        unlock(&locked);
        assert!(stats.existed);
        assert_eq!(stats.freed_bytes, 1300);
        assert_eq!(stats.deleted_files, 2);
        assert_eq!(stats.failed_files, 1);
        assert!(locked.is_file(), "locked file must survive");
        assert!(!dir.join("free.tmp").exists());
    }

    #[cfg(windows)]
    #[test]
    fn unsafe_cleanup_targets_windows() {
        let protected = [
            PathBuf::from(r"C:\Users\andre"),
            PathBuf::from(r"C:\Windows"),
        ];
        let check = |p: &str| unsafe_cleanup_reason(Path::new(p), &protected);
        for ok in [
            r"C:\Users\andre\AppData\Local\Temp",
            r"C:\Windows\Temp",
            r"c:\windows\temp\",
            r"C:\Temp",
            r"D:\cache\x",
        ] {
            assert_eq!(check(ok), None, "{ok}");
        }
        for bad in [
            r"C:\",
            r"D:\",
            "C:",
            r"\\server\share\",
            r"C:\Users",
            r"C:\Users\andre",
            r"c:\USERS\ANDRE\",
            r"C:\Windows",
            r"C:\WINDOWS\",
            r"C:\Windows\Temp\..",
            r"Temp",
            "",
        ] {
            assert!(check(bad).is_some(), "{bad} accepted");
        }
    }

    #[cfg(unix)]
    #[test]
    fn unsafe_cleanup_targets_unix() {
        let protected = [PathBuf::from("/home/andre")];
        let check = |p: &str| unsafe_cleanup_reason(Path::new(p), &protected);
        assert_eq!(check("/tmp/cache"), None);
        assert_eq!(check("/home/andre/.cache"), None);
        for bad in [
            "/",
            "/tmp",
            "/home",
            "/home/andre",
            "/home/andre/",
            "tmp/x",
            "/tmp/x/..",
        ] {
            assert!(check(bad).is_some(), "{bad} accepted");
        }
    }

    #[test]
    fn collect_tree_records_unreadable_folders_and_links() {
        let root = Path::new("root");
        let bad = root.join("locked");
        let reader = |d: &Path| -> io::Result<Vec<(PathBuf, Kind)>> {
            if d == root {
                Ok(vec![
                    (root.join("a.txt"), Kind::File),
                    (bad.clone(), Kind::Dir),
                    (root.join("ln"), Kind::Link),
                ])
            } else {
                Err(io::Error::new(io::ErrorKind::PermissionDenied, "denied"))
            }
        };
        let (mut dirs, mut files, mut failed) = (Vec::new(), Vec::new(), Vec::new());
        collect_tree_with(
            &reader,
            root,
            root,
            None,
            &mut dirs,
            &mut files,
            &mut failed,
        );
        assert_eq!(files.len(), 1);
        assert_eq!(dirs, vec![PathBuf::from("locked")]);
        assert_eq!(failed.len(), 2, "{failed:?}");
        assert!(
            failed
                .iter()
                .any(|f| f.starts_with("locked: cannot read folder"))
        );
        assert!(failed.contains(&"ln: symlink/junction not copied".to_string()));
    }

    #[cfg(unix)]
    #[test]
    fn backup_reports_an_unreadable_subfolder() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("src");
        write(&src.join("ok.txt"), 3);
        write(&src.join("locked/secret.txt"), 3);
        let locked = src.join("locked");
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
        // Root can read anything: then there is nothing to fail.
        let readable = fs::read_dir(&locked).is_ok();
        let report = backup(&src, &tmp.path().join("dest"), "t").unwrap();
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
        if !readable {
            assert_eq!(report.files, 1);
            assert!(
                report
                    .failed
                    .iter()
                    .any(|f| f.starts_with("locked: cannot read folder")),
                "{:?}",
                report.failed
            );
        }
    }

    /// Creates `target` (a folder) and a symlink (Unix) or junction (Windows) `link` to
    /// it. `None` when the platform refuses (the calling test is then skipped).
    fn make_dir_link(target: &Path, link: &Path) -> Option<()> {
        fs::create_dir_all(target).ok()?;
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(target, link).ok()
        }
        #[cfg(windows)]
        {
            let ok = std::process::Command::new("cmd")
                .args(["/c", "mklink", "/J"])
                .arg(link)
                .arg(target)
                .output()
                .ok()?
                .status
                .success();
            ok.then_some(())
        }
    }

    #[test]
    fn backup_lists_links_as_not_copied() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("src");
        write(&src.join("ok.txt"), 3);
        if make_dir_link(&tmp.path().join("elsewhere"), &src.join("jn")).is_none() {
            return;
        }
        let report = backup(&src, &tmp.path().join("dest"), "t").unwrap();
        assert_eq!(report.files, 1);
        assert_eq!(report.failed.len(), 1, "{:?}", report.failed);
        assert!(report.failed[0].contains("symlink/junction not copied"));
        assert!(!report.destination.join("jn").exists());
    }

    #[test]
    fn cleanup_refuses_a_link_to_a_protected_folder() {
        let tmp = tempfile::tempdir().unwrap();
        let protected = tmp.path().join("profile");
        let link = tmp.path().join("looks").join("harmless");
        fs::create_dir_all(link.parent().unwrap()).unwrap();
        if make_dir_link(&protected, &link).is_none() {
            return;
        }
        let reason = unsafe_cleanup_reason(&link, std::slice::from_ref(&protected));
        assert!(
            reason
                .as_deref()
                .is_some_and(|r| r.contains("is or contains")),
            "{reason:?}"
        );
        // An unrelated real folder stays allowed.
        let other = tmp.path().join("other").join("cache");
        fs::create_dir_all(&other).unwrap();
        assert_eq!(unsafe_cleanup_reason(&other, &[protected]), None);
    }

    #[test]
    fn unique_path_never_overwrites() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("show_backup_2026-11-22_10-30");
        assert_eq!(unique_path(&p), p);
        fs::create_dir(&p).unwrap();
        let p1 = tmp.path().join("show_backup_2026-11-22_10-30_1");
        assert_eq!(unique_path(&p), p1);
        fs::write(&p1, b"").unwrap();
        assert_eq!(
            unique_path(&p),
            tmp.path().join("show_backup_2026-11-22_10-30_2")
        );
    }

    #[test]
    fn backup_copies_a_tree_with_spaces_and_accents() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("Proyecto Música");
        write(&src.join("a b.txt"), 10);
        write(&src.join("año 1/canción ñ.wav"), 2048);
        write(&src.join("año 1/vacío/日本.txt"), 3);
        let dest_root = tmp.path().join("Copias de seguridad/nueva");

        let r = backup(&src, &dest_root, "2026-11-22_10-30").unwrap();
        let expected = dest_root.join("Proyecto Música_backup_2026-11-22_10-30");
        assert_eq!(r.destination, expected);
        assert_eq!(r.files, 3);
        assert_eq!(r.bytes, 2061);
        assert!(r.failed.is_empty(), "{:?}", r.failed);
        assert!(!r.is_file);
        assert_eq!(
            fs::read(expected.join("año 1/canción ñ.wav"))
                .unwrap()
                .len(),
            2048
        );
        assert_eq!(fs::read(expected.join("a b.txt")).unwrap(), vec![b'x'; 10]);
        assert!(expected.join("año 1/vacío/日本.txt").is_file());

        // Same minute again: a new folder, the first one untouched.
        let r2 = backup(&src, &dest_root, "2026-11-22_10-30").unwrap();
        assert_eq!(
            r2.destination,
            dest_root.join("Proyecto Música_backup_2026-11-22_10-30_1")
        );
        assert_eq!(r2.files, 3);
    }

    #[test]
    fn backup_of_a_single_file_goes_into_its_folder() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("show file.json");
        write(&src, 42);
        let r = backup(&src, tmp.path(), "2026-01-02_03-04").unwrap();
        assert!(r.is_file);
        assert_eq!(r.files, 1);
        assert_eq!(r.bytes, 42);
        let copied = tmp
            .path()
            .join("show file.json_backup_2026-01-02_03-04")
            .join("show file.json");
        assert_eq!(fs::read(copied).unwrap().len(), 42);
    }

    #[test]
    fn backup_into_its_own_source_does_not_recurse() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("data");
        write(&src.join("one.txt"), 5);
        let r = backup(&src, &src.join("backups"), "2026-01-02_03-04").unwrap();
        assert_eq!(r.files, 1);
        let r2 = backup(&src, &src.join("backups"), "2026-01-02_03-05").unwrap();
        assert_eq!(
            r2.files, 1,
            "earlier backups inside the source are not copied"
        );
    }

    #[test]
    fn backup_missing_source_is_input_error() {
        let tmp = tempfile::tempdir().unwrap();
        let err = backup(&tmp.path().join("nope"), tmp.path(), "x").unwrap_err();
        assert_eq!(err.exit_code(), 2);
        assert!(err.to_string().contains("does not exist"), "{err}");
    }

    #[test]
    fn like_matching() {
        assert!(like_match("*show*", "Final SHOW.mp4"));
        assert!(like_match("*.mp4", "a.MP4"));
        assert!(!like_match("*.mp4", "a.mp4.txt"));
        assert!(like_match("sh?w", "SHOW"));
        assert!(!like_match("sh?w", "shw"));
        assert!(like_match("*", ""));
        assert!(like_match("**a**", "banana"));
        assert!(like_match("canción*", "Canción ñ.wav"));
        assert!(!like_match("abc", "abcd"));
    }

    #[test]
    fn search_finds_files_and_folders_case_insensitively() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        write(&root.join("Show Final.mp4"), 1024);
        write(&root.join("notes.txt"), 1);
        write(&root.join("sub/Other SHOW.wav"), 2);
        fs::create_dir_all(root.join("showfiles")).unwrap();
        let names = |hits: &[SearchHit]| -> Vec<String> {
            hits.iter()
                .map(|h| h.path.file_name().unwrap().to_string_lossy().into_owned())
                .collect()
        };
        let hits = search(root, "show").unwrap();
        assert_eq!(
            names(&hits),
            ["Show Final.mp4", "showfiles", "Other SHOW.wav"]
        );
        assert_eq!(hits[0].size_bytes, 1024);
        assert!(!hits[0].is_dir);
        assert!(hits[1].is_dir);
        assert!(hits[0].modified_unix.is_some());
        assert_eq!(names(&search(root, "*.mp4").unwrap()), ["Show Final.mp4"]);
        assert_eq!(search(root, "sh?w").unwrap().len(), 3);
        assert!(search(root, "zzz").unwrap().is_empty());
        let err = search(&root.join("missing"), "x").unwrap_err();
        assert_eq!(err.exit_code(), 2);
        assert_eq!(search(root, "").unwrap_err().exit_code(), 2);
    }
}
