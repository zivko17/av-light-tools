//! Elevation: whether this process runs as administrator, and relaunching it elevated.

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

/// Starts this executable again with `args` through the UAC prompt (`ShellExecuteW`
/// with the `runas` verb). The current process keeps running; the caller closes it.
pub fn relaunch_elevated(args: &[&str]) -> Result<()> {
    let exe = std::env::current_exe()?;
    imp::run_as(&exe, &command_line(args))
}

/// Joins arguments into one Windows command line that `CommandLineToArgvW` / the MSVC
/// runtime split back into the same arguments: arguments with spaces, tabs or quotes
/// (or empty ones) are quoted, `"` becomes `\"`, and backslashes are doubled only where
/// they precede a quote (including the closing one).
pub fn command_line(args: &[&str]) -> String {
    args.iter()
        .map(|a| quote_arg(a))
        .collect::<Vec<_>>()
        .join(" ")
}

fn quote_arg(a: &str) -> String {
    if !a.is_empty() && !a.contains([' ', '\t', '"']) {
        return a.to_string();
    }
    let mut out = String::from('"');
    let mut backslashes = 0usize;
    for c in a.chars() {
        match c {
            '\\' => backslashes += 1,
            '"' => {
                out.push_str(&"\\".repeat(backslashes * 2 + 1));
                out.push('"');
                backslashes = 0;
            }
            _ => {
                out.push_str(&"\\".repeat(backslashes));
                out.push(c);
                backslashes = 0;
            }
        }
    }
    out.push_str(&"\\".repeat(backslashes * 2));
    out.push('"');
    out
}

#[cfg(windows)]
mod imp {
    use std::path::Path;

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

    pub fn run_as(exe: &Path, params: &str) -> crate::Result<()> {
        use std::os::windows::ffi::OsStrExt;

        use windows::Win32::UI::Shell::ShellExecuteW;
        use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
        use windows::core::{PCWSTR, w};

        let file: Vec<u16> = exe.as_os_str().encode_wide().chain(Some(0)).collect();
        let params: Vec<u16> = params.encode_utf16().chain(Some(0)).collect();
        // SAFETY: `file` and `params` are NUL-terminated UTF-16 strings that outlive the
        // call; the verb is a literal and no window handle or directory is passed.
        let result = unsafe {
            ShellExecuteW(
                None,
                w!("runas"),
                PCWSTR(file.as_ptr()),
                PCWSTR(params.as_ptr()),
                PCWSTR::null(),
                SW_SHOWNORMAL,
            )
        };
        // ShellExecute reports success with a value greater than 32; the user declining
        // the UAC prompt comes back as an error.
        if result.0 as usize > 32 {
            Ok(())
        } else {
            Err(crate::Error::Other(format!(
                "elevation was refused or failed (ShellExecute error {})",
                result.0 as usize
            )))
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use std::path::Path;

    pub fn is_admin() -> bool {
        false
    }

    pub fn run_as(_exe: &Path, _params: &str) -> crate::Result<()> {
        Err(crate::Error::Unsupported)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_line_quotes_only_when_needed() {
        assert_eq!(command_line(&["gui"]), "gui");
        assert_eq!(command_line(&["a b", "c"]), "\"a b\" c");
        assert_eq!(command_line(&["say \"hi\""]), "\"say \\\"hi\\\"\"");
        assert_eq!(command_line(&[""]), "\"\"");
        // Backslashes only double before a quote (MSVC rules).
        assert_eq!(command_line(&[r"C:\My Dir\"]), r#""C:\My Dir\\""#);
        assert_eq!(command_line(&[r"a\b c"]), r#""a\b c""#);
        assert_eq!(command_line(&[r#"x\"y z"#]), r#""x\\\"y z""#);
    }

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
