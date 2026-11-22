//! `sys audio` (Audio.ps1): active playback and recording devices, the defaults, and the
//! default output's volume and mute state, through Core Audio (no AudioDeviceCmdlets).

use avlt_core::sys::model::AudioReport;

use crate::Result;

/// Active endpoints of both directions (sorted by name) and the default output volume.
pub fn audio_devices() -> Result<AudioReport> {
    let mut report = imp::audio_devices()?;
    report.playback.sort_by_key(|d| d.name.to_lowercase());
    report.recording.sort_by_key(|d| d.name.to_lowercase());
    Ok(report)
}

#[cfg(windows)]
mod imp {
    use avlt_core::sys::model::{AudioDevice, AudioFlow, AudioReport};
    use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
    use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
    use windows::Win32::Media::Audio::{
        DEVICE_STATE_ACTIVE, EDataFlow, IMMDevice, IMMDeviceEnumerator, MMDeviceEnumerator,
        eCapture, eConsole, eRender,
    };
    use windows::Win32::System::Com::StructuredStorage::{PropVariantClear, PropVariantToBSTR};
    use windows::Win32::System::Com::{
        CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree,
        STGM_READ,
    };

    use crate::{Error, Result};

    fn fail(what: &str) -> impl Fn(windows::core::Error) -> Error + '_ {
        move |e| Error::Other(format!("Core Audio {what}: {}", e.message().trim()))
    }

    /// Endpoint id string (freed with CoTaskMemFree).
    fn device_id(dev: &IMMDevice) -> Option<String> {
        // SAFETY: GetId returns a CoTaskMemAlloc'd string, copied then freed once.
        unsafe {
            let p = dev.GetId().ok()?;
            let id = p.to_string().ok();
            CoTaskMemFree(Some(p.0 as *const _));
            id
        }
    }

    fn friendly_name(dev: &IMMDevice) -> Option<String> {
        // SAFETY: the PROPVARIANT returned by GetValue is owned here and cleared once.
        unsafe {
            let store = dev.OpenPropertyStore(STGM_READ).ok()?;
            let mut pv = store.GetValue(&PKEY_Device_FriendlyName).ok()?;
            let name = PropVariantToBSTR(&pv).ok().map(|b| b.to_string());
            let _ = PropVariantClear(&mut pv);
            name
        }
    }

    fn endpoints(
        enumerator: &IMMDeviceEnumerator,
        flow: AudioFlow,
        data_flow: EDataFlow,
    ) -> Result<Vec<AudioDevice>> {
        // SAFETY: COM calls on a live enumerator; results are owned COM pointers.
        unsafe {
            let default_id = enumerator
                .GetDefaultAudioEndpoint(data_flow, eConsole)
                .ok()
                .and_then(|d| device_id(&d));
            let collection = enumerator
                .EnumAudioEndpoints(data_flow, DEVICE_STATE_ACTIVE)
                .map_err(fail("endpoint list"))?;
            let count = collection.GetCount().map_err(fail("endpoint count"))?;
            let mut out = Vec::new();
            for i in 0..count {
                let Ok(dev) = collection.Item(i) else {
                    continue;
                };
                let id = device_id(&dev).unwrap_or_default();
                out.push(AudioDevice {
                    name: friendly_name(&dev).unwrap_or_else(|| "(unnamed device)".into()),
                    flow,
                    default: default_id.as_deref() == Some(id.as_str()),
                    id,
                });
            }
            Ok(out)
        }
    }

    pub fn audio_devices() -> Result<AudioReport> {
        // SAFETY: COM is initialised for this thread (already-initialised modes are
        // fine); every interface is released when it goes out of scope.
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
                    .map_err(fail("device enumerator"))?;
            let playback = endpoints(&enumerator, AudioFlow::Playback, eRender)?;
            let recording = endpoints(&enumerator, AudioFlow::Recording, eCapture)?;
            let volume: Option<IAudioEndpointVolume> = enumerator
                .GetDefaultAudioEndpoint(eRender, eConsole)
                .ok()
                .and_then(|d| d.Activate(CLSCTX_ALL, None).ok());
            Ok(AudioReport {
                playback,
                recording,
                default_volume_pct: volume
                    .as_ref()
                    .and_then(|v| v.GetMasterVolumeLevelScalar().ok())
                    .map(|s| (f64::from(s) * 100.0).round()),
                default_muted: volume
                    .as_ref()
                    .and_then(|v| v.GetMute().ok())
                    .map(|b| b.as_bool()),
            })
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use avlt_core::sys::model::AudioReport;

    use crate::{Error, Result};

    pub fn audio_devices() -> Result<AudioReport> {
        Err(Error::Unsupported)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn lists_endpoints_consistently() {
        // CI runners have no audio hardware (and maybe no audio service): only check
        // consistency when Core Audio answers.
        let Ok(r) = audio_devices() else { return };
        assert!(r.playback.iter().filter(|d| d.default).count() <= 1);
        assert!(r.recording.iter().filter(|d| d.default).count() <= 1);
        assert!(
            r.default_volume_pct
                .is_none_or(|v| (0.0..=100.0).contains(&v))
        );
        if r.playback.is_empty() {
            assert_eq!(r.default_volume_pct, None);
        }
    }

    #[cfg(not(windows))]
    #[test]
    fn unsupported_elsewhere() {
        assert!(matches!(audio_devices(), Err(crate::Error::Unsupported)));
    }
}
