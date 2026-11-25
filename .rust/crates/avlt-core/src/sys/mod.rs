//! System tools: models and the pure logic behind `avlt sys` / `avlt util`.

pub mod display;
pub mod duration;
pub mod format;
pub mod fsops;
pub mod model;
pub mod process;
pub mod weather;

pub use duration::{format_countdown, parse_duration};
pub use process::{CRITICAL_PROCESSES, cpu_percent, is_critical_process, matches_process};

/// The fixed services of ServicesAudio.ps1: service name and description.
pub const AUDIO_SERVICES: [(&str, &str); 3] = [
    ("Audiosrv", "Windows Audio"),
    ("AudioEndpointBuilder", "Audio Endpoint Builder"),
    ("MMCSS", "Multimedia Class Scheduler"),
];

/// ServicesAudio.ps1 also lists every service named `Dante*` (Audinate), case-insensitive.
pub fn is_dante_service(name: &str) -> bool {
    name.get(..5)
        .is_some_and(|p| p.eq_ignore_ascii_case("dante"))
}

/// The service `sys services-audio --restart` restarts (ServicesAudio.ps1 tip).
pub const AUDIO_RESTART_SERVICE: &str = "Audiosrv";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dante_services() {
        assert!(is_dante_service("DanteVia"));
        assert!(is_dante_service("dante virtual soundcard"));
        assert!(!is_dante_service("Dant"));
        assert!(!is_dante_service("ADante"));
        assert!(!is_dante_service("Dañte"));
    }
}
