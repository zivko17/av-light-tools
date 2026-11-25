//! Digital audio data rate and wavelength.
//! Ports `Get-AudioBandwidth` (AVCalc.ps1 lines 880-895).

use serde::Serialize;

use super::require_non_negative;
use crate::error::{Error, Result};

/// Speed of sound used for the wavelength (m/s).
pub const SPEED_OF_SOUND: f64 = 343.0;

/// Inputs; defaults are 64 channels, 48 kHz, 24-bit.
#[derive(Debug, Clone, PartialEq)]
pub struct AudioBwInput {
    pub channels: u32,
    pub sample_rate: u32,
    pub bit_depth: u32,
    /// Frequency (Hz) for the wavelength; 0 = skip.
    pub freq: f64,
}

impl Default for AudioBwInput {
    fn default() -> Self {
        AudioBwInput {
            channels: 64,
            sample_rate: 48_000,
            bit_depth: 24,
            freq: 0.0,
        }
    }
}

/// Payload data rate (Dante/AES67 add packet and protocol overhead on top).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AudioBwResult {
    pub bits_per_second: f64,
    pub mbps: f64,
    /// Wavelength at `freq` (343 m/s).
    pub wavelength_m: Option<f64>,
}

/// Audio payload data rate for a channel count and format.
pub fn audiobw(input: &AudioBwInput) -> Result<AudioBwResult> {
    if input.channels == 0 || input.sample_rate == 0 || input.bit_depth == 0 {
        return Err(Error::InvalidInput(
            "channels, sample rate and bit depth must be greater than 0".into(),
        ));
    }
    let freq = require_non_negative("freq", input.freq)?;
    let bps = f64::from(input.channels) * f64::from(input.sample_rate) * f64::from(input.bit_depth);
    Ok(AudioBwResult {
        bits_per_second: bps,
        mbps: bps / 1e6,
        wavelength_m: (freq > 0.0).then(|| SPEED_OF_SOUND / freq),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-9, "{a} != {b}");
    }

    #[test]
    fn defaults_64ch_48k_24bit() {
        // line 888: 64 * 48000 * 24 = 73_728_000 bps = 73.728 Mbps
        let r = audiobw(&AudioBwInput::default()).unwrap();
        close(r.bits_per_second, 73_728_000.0);
        close(r.mbps, 73.728);
        assert!(r.wavelength_m.is_none());
    }

    #[test]
    fn stereo_cd() {
        // 2 * 44100 * 16 = 1_411_200 bps
        let r = audiobw(&AudioBwInput {
            channels: 2,
            sample_rate: 44_100,
            bit_depth: 16,
            freq: 0.0,
        })
        .unwrap();
        close(r.bits_per_second, 1_411_200.0);
        close(r.mbps, 1.4112);
    }

    #[test]
    fn wavelength() {
        // line 893: 343 / 1000 Hz = 0.343 m; 343 / 343 Hz = 1 m
        let r = audiobw(&AudioBwInput {
            freq: 1000.0,
            ..AudioBwInput::default()
        })
        .unwrap();
        close(r.wavelength_m.unwrap(), 0.343);
        let r = audiobw(&AudioBwInput {
            freq: 343.0,
            ..AudioBwInput::default()
        })
        .unwrap();
        close(r.wavelength_m.unwrap(), 1.0);
    }

    #[test]
    fn invalid_input() {
        for bad in [
            AudioBwInput {
                channels: 0,
                ..AudioBwInput::default()
            },
            AudioBwInput {
                sample_rate: 0,
                ..AudioBwInput::default()
            },
            AudioBwInput {
                bit_depth: 0,
                ..AudioBwInput::default()
            },
            AudioBwInput {
                freq: -20.0,
                ..AudioBwInput::default()
            },
            AudioBwInput {
                freq: f64::NAN,
                ..AudioBwInput::default()
            },
        ] {
            assert!(
                matches!(audiobw(&bad), Err(Error::InvalidInput(_))),
                "{bad:?}"
            );
        }
    }
}
