//! Known user folders (Documents, Music, Videos), honouring Windows folder redirection
//! (OneDrive "Known Folder Move"). Elsewhere: `$HOME/<name>`.

use std::path::PathBuf;

/// A per-user folder avlt reads or writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KnownFolder {
    Documents,
    Music,
    Videos,
}

impl KnownFolder {
    /// Folder name under the profile directory.
    #[cfg_attr(windows, allow(dead_code))]
    fn dir_name(self) -> &'static str {
        match self {
            KnownFolder::Documents => "Documents",
            KnownFolder::Music => "Music",
            KnownFolder::Videos => "Videos",
        }
    }
}

/// The resolved path of `f`, or `None` when it cannot be determined.
#[cfg(windows)]
pub fn known_folder(f: KnownFolder) -> Option<PathBuf> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;

    use windows::Win32::System::Com::CoTaskMemFree;
    use windows::Win32::UI::Shell::{
        FOLDERID_Documents, FOLDERID_Music, FOLDERID_Videos, KF_FLAG_DEFAULT, SHGetKnownFolderPath,
    };

    let id = match f {
        KnownFolder::Documents => &FOLDERID_Documents,
        KnownFolder::Music => &FOLDERID_Music,
        KnownFolder::Videos => &FOLDERID_Videos,
    };
    // SAFETY: `id` points to a static GUID; on success the call returns a NUL-terminated
    // wide string allocated by the shell, which is copied out and then freed exactly once
    // with CoTaskMemFree (also on the error path, where the pointer is not returned).
    unsafe {
        let pwstr = SHGetKnownFolderPath(id, KF_FLAG_DEFAULT, None).ok()?;
        let path = OsString::from_wide(pwstr.as_wide());
        CoTaskMemFree(Some(pwstr.0 as *const _));
        if path.is_empty() {
            None
        } else {
            Some(PathBuf::from(path))
        }
    }
}

/// The resolved path of `f`, or `None` when it cannot be determined.
#[cfg(not(windows))]
pub fn known_folder(f: KnownFolder) -> Option<PathBuf> {
    let home = std::env::var_os("HOME").filter(|v| !v.is_empty())?;
    Some(PathBuf::from(home).join(f.dir_name()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn documents_resolves_to_an_existing_absolute_folder() {
        // CI runners and dev machines have a profile; skip silently when it is absent.
        if let Some(p) = known_folder(KnownFolder::Documents) {
            assert!(p.is_absolute(), "{p:?}");
            #[cfg(windows)]
            assert!(p.is_dir(), "{p:?}");
        }
    }

    #[test]
    fn folders_have_distinct_paths() {
        let d = known_folder(KnownFolder::Documents);
        let m = known_folder(KnownFolder::Music);
        let v = known_folder(KnownFolder::Videos);
        if let (Some(d), Some(m), Some(v)) = (d, m, v) {
            assert_ne!(d, m);
            assert_ne!(m, v);
        }
    }
}
