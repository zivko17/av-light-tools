//! The console window of a console-subsystem executable: telling a double-click from a
//! terminal launch, and detaching from the console before opening the GUI.

/// `true` when this process is the only one attached to its console, i.e. Windows
/// created the console for it (double-click in Explorer) rather than a terminal.
/// Always `false` outside Windows or without a console.
pub fn owns_console_alone() -> bool {
    imp::console_process_count() == 1
}

/// Detaches from the console (`FreeConsole`); the console window Windows opened for a
/// double-click closes with it. Does nothing outside Windows.
pub fn detach_console() {
    imp::detach();
}

/// Text of the startup-failure dialog: the error plus the way out for machines where the
/// GUI cannot open (no OpenGL, RDP session, basic display driver).
pub fn startup_error_text(error: &str) -> String {
    format!(
        "avlt could not open its window.

{error}

Use avlt from a terminal (avlt --help)."
    )
}

/// Shows a modal error message box. Does nothing outside Windows. Never call from tests.
pub fn show_error_dialog(title: &str, text: &str) {
    imp::message_box(title, text);
}

#[cfg(windows)]
mod imp {
    use windows::Win32::System::Console::{FreeConsole, GetConsoleProcessList};
    use windows::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW};
    use windows::core::HSTRING;

    pub fn message_box(title: &str, text: &str) {
        let (title, text) = (HSTRING::from(title), HSTRING::from(text));
        // SAFETY: both HSTRINGs are valid NUL-terminated UTF-16 that outlive the call; a
        // null owner window is allowed.
        unsafe {
            MessageBoxW(None, &text, &title, MB_OK | MB_ICONERROR);
        }
    }

    pub fn console_process_count() -> u32 {
        let mut pids = [0u32; 4];
        // SAFETY: the buffer is a valid, writable slice; the API writes at most its length.
        unsafe { GetConsoleProcessList(&mut pids) }
    }

    pub fn detach() {
        // SAFETY: FreeConsole takes no arguments; failing (no console) is harmless.
        let _ = unsafe { FreeConsole() };
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn console_process_count() -> u32 {
        0
    }

    pub fn detach() {}

    pub fn message_box(_title: &str, _text: &str) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_test_run_does_not_own_its_console_alone() {
        // cargo (and the terminal) share the test's console, or there is none (CI).
        assert!(!owns_console_alone());
    }

    #[test]
    fn startup_error_text_has_the_error_and_the_terminal_hint() {
        let text = startup_error_text("cannot open the window: no OpenGL 2");
        assert!(text.contains("no OpenGL 2"));
        assert!(text.contains("Use avlt from a terminal (avlt --help)"));
    }
}
