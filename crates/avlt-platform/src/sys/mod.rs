//! System access (info, telemetry, processes, devices, services).

pub mod audio;
pub mod battery;
pub mod cleanup;
pub mod disks;
pub mod info;
pub mod monitors;
pub mod processes;
pub mod services;
pub mod startup;
pub mod telemetry;
pub mod usb;
pub mod weather;

use std::path::Path;

pub use audio::audio_devices;
pub use battery::battery;
pub use cleanup::{CleanupTarget, analyze as analyze_cleanup, cleanup, cleanup_targets};
pub use disks::{disks, fixed_volumes};
pub use info::info;
pub use monitors::{displays, monitors, set_mode};
pub use processes::{TopSampler, find_processes, kill};
pub use services::{audio_services, restart_service};
pub use startup::startup_entries;
pub use telemetry::TelemetrySampler;
pub use usb::usb_devices;
pub use weather::fetch_weather;

use crate::Result;

/// Plays a tone on the PC speaker / default output (Timer.ps1 `[console]::beep`).
pub fn beep(frequency_hz: u32, duration_ms: u32) -> Result<()> {
    imp::beep(frequency_hz, duration_ms)
}

/// Opens `path` with its default program, as a double-click would (Search.ps1
/// `Start-Process`).
pub fn open_path(path: &Path) -> Result<()> {
    imp::open_path(path)
}

#[cfg(windows)]
mod imp {
    use std::path::Path;

    use crate::{Error, Result};

    pub fn beep(frequency_hz: u32, duration_ms: u32) -> Result<()> {
        // SAFETY: plain Win32 call without pointers.
        unsafe { windows::Win32::System::Diagnostics::Debug::Beep(frequency_hz, duration_ms) }
            .map_err(|e| Error::Other(format!("beep: {}", e.message().trim())))
    }

    pub fn open_path(path: &Path) -> Result<()> {
        // explorer.exe hands the file to its registered program (or opens the folder).
        let explorer = crate::process::find_tool("explorer", "-")?;
        std::process::Command::new(explorer).arg(path).spawn()?;
        Ok(())
    }
}

#[cfg(not(windows))]
mod imp {
    use std::path::Path;

    use crate::{Error, Result};

    pub fn beep(_frequency_hz: u32, _duration_ms: u32) -> Result<()> {
        Err(Error::Unsupported)
    }

    pub fn open_path(_path: &Path) -> Result<()> {
        Err(Error::Unsupported)
    }
}
