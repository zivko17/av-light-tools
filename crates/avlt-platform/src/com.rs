//! Process-wide COM setup shared by every COM user (WMI, Core Audio).
//!
//! The `wmi` crate only pins the multithreaded apartment (`CoIncrementMTAUsage`) when
//! COM is not initialised yet. If another thread is in the MTA at that moment (Core
//! Audio's `CoInitializeEx`/`CoUninitialize` pair), the connection piggybacks on that
//! thread's MTA; when it calls `CoUninitialize` the apartment is torn down under the live
//! WMI proxies and the next call crashes (access violation, "overflowed its stack") or
//! hangs. Pinning the MTA once, for the life of the process, before any COM use removes
//! that window on every thread: test threads, the CLI main thread and background workers.

/// Keeps the MTA alive for the rest of the process and sets the default COM security
/// (as the `wmi` crate does on its own first init). Idempotent and thread-safe.
#[cfg(windows)]
pub(crate) fn pin_mta() {
    use std::sync::OnceLock;
    use windows::Win32::System::Com::{
        CoIncrementMTAUsage, CoInitializeSecurity, EOAC_NONE, RPC_C_AUTHN_LEVEL_DEFAULT,
        RPC_C_IMP_LEVEL_IMPERSONATE,
    };

    static PINNED: OnceLock<()> = OnceLock::new();
    PINNED.get_or_init(|| {
        // SAFETY: no pointers are passed. The returned cookie is intentionally dropped
        // without CoDecrementMTAUsage: the MTA must outlive every COM object.
        if unsafe { CoIncrementMTAUsage() }.is_err() {
            return;
        }
        // SAFETY: all optional pointers are None; -1 lets COM choose the services. The
        // same call the `wmi` crate makes. RPC_E_TOO_LATE (security already set, e.g.
        // by a host process) is fine, so the result is ignored.
        let _ = unsafe {
            CoInitializeSecurity(
                None,
                -1,
                None,
                None,
                RPC_C_AUTHN_LEVEL_DEFAULT,
                RPC_C_IMP_LEVEL_IMPERSONATE,
                None,
                EOAC_NONE,
                None,
            )
        };
    });
}
