//! Elevation check: whether this process runs as administrator.

use crate::{Error, Result};

/// `true` if the process token is elevated ("Run as administrator").
/// Always `false` outside Windows.
pub fn is_admin() -> bool {
    imp::is_admin()
}

/// `Ok` when elevated; [`Error::NeedsAdmin`] otherwise ([`Error::Unsupported`] outside Windows).
pub fn require_admin() -> Result<()> {
    if !cfg!(windows) {
        return Err(Error::Unsupported);
    }
    if is_admin() {
        Ok(())
    } else {
        Err(Error::NeedsAdmin)
    }
}

#[cfg(windows)]
mod imp {
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::Security::{
        GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation,
    };
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    pub fn is_admin() -> bool {
        let mut token = HANDLE::default();
        // SAFETY: the pseudo-handle of the current process is always valid; `token` is
        // an out-parameter owned by this frame and closed below.
        if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) }.is_err() {
            return false;
        }
        let mut elevation = TOKEN_ELEVATION::default();
        let mut len = 0u32;
        // SAFETY: the buffer is a TOKEN_ELEVATION of the size passed in.
        let ok = unsafe {
            GetTokenInformation(
                token,
                TokenElevation,
                Some((&raw mut elevation).cast()),
                size_of::<TOKEN_ELEVATION>() as u32,
                &mut len,
            )
        }
        .is_ok();
        // SAFETY: `token` was opened above and is not used afterwards.
        let _ = unsafe { CloseHandle(token) };
        ok && elevation.TokenIsElevated != 0
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn is_admin() -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn require_admin_agrees_with_is_admin() {
        let admin = is_admin();
        let result = require_admin();
        if cfg!(windows) {
            match result {
                Ok(()) => assert!(admin),
                Err(Error::NeedsAdmin) => assert!(!admin),
                Err(e) => panic!("unexpected {e:?}"),
            }
        } else {
            assert!(!admin);
            assert!(matches!(result, Err(Error::Unsupported)), "{result:?}");
        }
    }
}
