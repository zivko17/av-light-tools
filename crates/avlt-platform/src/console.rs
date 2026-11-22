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

#[cfg(windows)]
mod imp {
    use windows::Win32::System::Console::{FreeConsole, GetConsoleProcessList};

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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_test_run_does_not_own_its_console_alone() {
        // cargo (and the terminal) share the test's console, or there is none (CI).
        assert!(!owns_console_alone());
    }
}
