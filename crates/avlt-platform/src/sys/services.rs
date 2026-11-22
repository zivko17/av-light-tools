//! `sys services-audio` (ServicesAudio.ps1): state of the Windows audio services and of
//! any Dante service, and an optional restart of Windows Audio (administrator).

use std::time::Duration;

use avlt_core::sys::format::service_state_name;
use avlt_core::sys::model::ServiceStatus;
use avlt_core::sys::{AUDIO_SERVICES, is_dante_service};

use crate::Result;

/// How long a restart waits for the service to stop, and again to start.
pub const SERVICE_TIMEOUT: Duration = Duration::from_secs(20);

/// A service as the Service Control Manager lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceEntry {
    pub name: String,
    pub display_name: String,
    /// `SERVICE_STATUS.dwCurrentState`.
    pub state: u32,
}

/// The fixed audio services that exist (ServicesAudio.ps1 order), then every `Dante*`.
pub fn audio_services() -> Result<Vec<ServiceStatus>> {
    Ok(select_audio(&imp::all_services()?))
}

/// Stops and starts service `name` (Restart-Service -Force), waiting up to
/// [`SERVICE_TIMEOUT`] for each step. Needs administrator rights.
pub fn restart_service(name: &str) -> Result<ServiceStatus> {
    crate::admin::require_admin()?;
    let state = imp::restart(name, SERVICE_TIMEOUT)?;
    Ok(ServiceStatus {
        name: name.to_string(),
        status: service_state_name(state).to_string(),
        description: String::new(),
    })
}

fn select_audio(all: &[ServiceEntry]) -> Vec<ServiceStatus> {
    let status = |e: &ServiceEntry, description: &str| ServiceStatus {
        name: e.name.clone(),
        status: service_state_name(e.state).to_string(),
        description: description.to_string(),
    };
    let mut out: Vec<ServiceStatus> = AUDIO_SERVICES
        .iter()
        .filter_map(|(name, desc)| {
            all.iter()
                .find(|e| e.name.eq_ignore_ascii_case(name))
                .map(|e| status(e, desc))
        })
        .collect();
    let mut dante: Vec<&ServiceEntry> = all.iter().filter(|e| is_dante_service(&e.name)).collect();
    dante.sort_by_key(|e| e.name.to_lowercase());
    out.extend(dante.into_iter().map(|e| status(e, &e.display_name)));
    out
}

#[cfg(windows)]
mod imp {
    use std::time::{Duration, Instant};

    use windows::Win32::System::Services::{
        CloseServiceHandle, ControlService, ENUM_SERVICE_STATUS_PROCESSW, EnumServicesStatusExW,
        OpenSCManagerW, OpenServiceW, QueryServiceStatus, SC_ENUM_PROCESS_INFO, SC_HANDLE,
        SC_MANAGER_CONNECT, SC_MANAGER_ENUMERATE_SERVICE, SERVICE_CONTROL_STOP,
        SERVICE_QUERY_STATUS, SERVICE_RUNNING, SERVICE_START, SERVICE_STATE_ALL, SERVICE_STATUS,
        SERVICE_STOP, SERVICE_STOPPED, SERVICE_WIN32, StartServiceW,
    };
    use windows::core::HSTRING;

    use super::ServiceEntry;
    use crate::{Error, Result};

    /// Closes the SCM / service handle on drop.
    struct Handle(SC_HANDLE);

    impl Drop for Handle {
        fn drop(&mut self) {
            // SAFETY: the handle came from OpenSCManagerW / OpenServiceW and is closed once.
            unsafe {
                let _ = CloseServiceHandle(self.0);
            }
        }
    }

    fn fail(what: String) -> impl Fn(windows::core::Error) -> Error {
        move |e| Error::Other(format!("{what}: {}", e.message().trim()))
    }

    fn manager(access: u32) -> Result<Handle> {
        // SAFETY: plain call; the handle is owned by `Handle`.
        unsafe { OpenSCManagerW(None, None, access) }
            .map(Handle)
            .map_err(fail("cannot open the service manager".into()))
    }

    pub fn all_services() -> Result<Vec<ServiceEntry>> {
        let scm = manager(SC_MANAGER_ENUMERATE_SERVICE)?;
        let mut out = Vec::new();
        let mut resume = 0u32;
        // 8-byte aligned storage for the ENUM_SERVICE_STATUS_PROCESSW array.
        let mut storage: Vec<u64> = vec![0; 64 * 1024 / 8];
        loop {
            let (mut needed, mut returned) = (0u32, 0u32);
            // SAFETY: the byte view covers exactly `storage`, which outlives its use.
            let buf = unsafe {
                std::slice::from_raw_parts_mut(storage.as_mut_ptr().cast::<u8>(), storage.len() * 8)
            };
            let result = unsafe {
                EnumServicesStatusExW(
                    scm.0,
                    SC_ENUM_PROCESS_INFO,
                    SERVICE_WIN32,
                    SERVICE_STATE_ALL,
                    Some(buf),
                    &mut needed,
                    &mut returned,
                    Some(&mut resume),
                    None,
                )
            };
            // SAFETY: on success or ERROR_MORE_DATA the first `returned` entries are
            // valid; their strings point into `storage`, copied out before reuse.
            let items = unsafe {
                std::slice::from_raw_parts(
                    storage.as_ptr().cast::<ENUM_SERVICE_STATUS_PROCESSW>(),
                    returned as usize,
                )
            };
            for s in items {
                // SAFETY: NUL-terminated strings written by the SCM into `storage`.
                let (name, display) = unsafe {
                    (
                        s.lpServiceName.to_string().unwrap_or_default(),
                        s.lpDisplayName.to_string().unwrap_or_default(),
                    )
                };
                out.push(ServiceEntry {
                    name,
                    display_name: display,
                    state: s.ServiceStatusProcess.dwCurrentState.0,
                });
            }
            match result {
                Ok(()) => return Ok(out),
                // ERROR_MORE_DATA: call again from `resume`, with room for `needed`.
                Err(e) if e.code().0 as u32 == 0x8007_00EA => {
                    let want = (needed as usize).div_ceil(8);
                    if want > storage.len() {
                        storage.resize(want, 0);
                    }
                }
                Err(e) => return Err(fail("cannot list services".into())(e)),
            }
        }
    }

    fn query(service: &Handle, name: &str) -> Result<u32> {
        let mut status = SERVICE_STATUS::default();
        // SAFETY: `status` is a valid out-parameter.
        unsafe { QueryServiceStatus(service.0, &mut status) }
            .map_err(fail(format!("cannot query {name}")))?;
        Ok(status.dwCurrentState.0)
    }

    fn wait_for(service: &Handle, name: &str, state: u32, timeout: Duration) -> Result<u32> {
        let start = Instant::now();
        loop {
            let now = query(service, name)?;
            if now == state {
                return Ok(now);
            }
            if start.elapsed() > timeout {
                return Err(Error::Other(format!(
                    "{name} did not reach the expected state within {} s",
                    timeout.as_secs()
                )));
            }
            std::thread::sleep(Duration::from_millis(250));
        }
    }

    pub fn restart(name: &str, timeout: Duration) -> Result<u32> {
        let scm = manager(SC_MANAGER_CONNECT)?;
        // SAFETY: plain call; the handle is owned by `Handle`.
        let service = unsafe {
            OpenServiceW(
                scm.0,
                &HSTRING::from(name),
                SERVICE_STOP | SERVICE_START | SERVICE_QUERY_STATUS,
            )
        }
        .map(Handle)
        .map_err(fail(format!("cannot open service {name}")))?;
        if query(&service, name)? != SERVICE_STOPPED.0 {
            let mut status = SERVICE_STATUS::default();
            // SAFETY: `status` is a valid out-parameter.
            unsafe { ControlService(service.0, SERVICE_CONTROL_STOP, &mut status) }
                .map_err(fail(format!("cannot stop {name}")))?;
            wait_for(&service, name, SERVICE_STOPPED.0, timeout)?;
        }
        // SAFETY: no service arguments.
        unsafe { StartServiceW(service.0, None) }.map_err(fail(format!("cannot start {name}")))?;
        wait_for(&service, name, SERVICE_RUNNING.0, timeout)
    }
}

#[cfg(not(windows))]
mod imp {
    use std::time::Duration;

    use super::ServiceEntry;
    use crate::{Error, Result};

    pub fn all_services() -> Result<Vec<ServiceEntry>> {
        Err(Error::Unsupported)
    }

    pub fn restart(_name: &str, _timeout: Duration) -> Result<u32> {
        Err(Error::Unsupported)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(name: &str, display: &str, state: u32) -> ServiceEntry {
        ServiceEntry {
            name: name.into(),
            display_name: display.into(),
            state,
        }
    }

    #[test]
    fn selection_follows_services_audio_ps1() {
        let all = [
            e("DanteVia", "Dante Via", 1),
            e("mmcss", "Multimedia Class Scheduler", 4),
            e("Spooler", "Print Spooler", 4),
            e("Audiosrv", "Windows Audio", 4),
            e("DanteDomainManager", "Dante Domain Manager", 2),
        ];
        let sel = select_audio(&all);
        let rows: Vec<(&str, &str, &str)> = sel
            .iter()
            .map(|s| (s.name.as_str(), s.status.as_str(), s.description.as_str()))
            .collect();
        assert_eq!(
            rows,
            [
                ("Audiosrv", "Running", "Windows Audio"),
                ("mmcss", "Running", "Multimedia Class Scheduler"),
                ("DanteDomainManager", "StartPending", "Dante Domain Manager"),
                ("DanteVia", "Stopped", "Dante Via"),
            ]
        );
    }

    #[cfg(windows)]
    #[test]
    fn lists_the_audio_services() {
        let list = audio_services().unwrap();
        // Windows Audio exists on every desktop Windows, CI runners included.
        assert!(list.iter().any(|s| s.name.eq_ignore_ascii_case("Audiosrv")));
    }

    #[cfg(windows)]
    #[test]
    fn restart_requires_admin() {
        // Never restarts anything when not elevated; elevated runs skip the check.
        if !crate::admin::is_admin() {
            let err = restart_service("Audiosrv").unwrap_err();
            assert!(matches!(err, crate::Error::NeedsAdmin), "{err:?}");
        }
    }

    #[cfg(not(windows))]
    #[test]
    fn unsupported_elsewhere() {
        assert!(matches!(audio_services(), Err(crate::Error::Unsupported)));
    }
}
