//! DvLED pixel pitch versus viewing distance, and wall resolution.
//! Ports `Get-PixelPitch` (AVCalc.ps1 lines 789-812).

use serde::Serialize;

use super::require_non_negative;
use crate::error::{Error, Result};

/// Retina rule: one pixel per arcminute, viewing distance = pitch x 3.438.
pub const RETINA_FACTOR: f64 = 3.438;

/// Inputs; give a `pitch` (mm), or a `distance` (m) to get the recommended pitch.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PixelPitchInput {
    /// Pixel pitch (mm); wins over `distance`.
    pub pitch: f64,
    /// Viewing distance (m).
    pub distance: f64,
    /// Wall width (m), with `height_m`, for the resolution.
    pub width_m: f64,
    pub height_m: f64,
}

/// Resolution of a wall at the given pitch.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PitchResolution {
    pub width: u64,
    pub height: u64,
    pub megapixels: f64,
}

/// Pitch / distance results.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PixelPitchResult {
    pub pitch_mm: Option<f64>,
    pub distance_m: Option<f64>,
    /// Minimum optimal viewing distance for `pitch_mm`.
    pub min_distance_m: Option<f64>,
    /// Maximum pitch recommended for `distance_m`.
    pub max_pitch_mm: Option<f64>,
    pub resolution: Option<PitchResolution>,
}

/// Pitch -> viewing distance and resolution, or distance -> recommended pitch.
pub fn pixelpitch(input: &PixelPitchInput) -> Result<PixelPitchResult> {
    let pitch = require_non_negative("pitch", input.pitch)?;
    let distance = require_non_negative("distance", input.distance)?;
    let width_m = require_non_negative("width-m", input.width_m)?;
    let height_m = require_non_negative("height-m", input.height_m)?;

    if pitch <= 0.0 && distance > 0.0 {
        return Ok(PixelPitchResult {
            pitch_mm: None,
            distance_m: Some(distance),
            min_distance_m: None,
            max_pitch_mm: Some(distance / RETINA_FACTOR),
            resolution: None,
        });
    }
    if pitch <= 0.0 {
        return Err(Error::InvalidInput(
            "give a --pitch (mm) or a --distance (m)".into(),
        ));
    }
    let resolution = (width_m > 0.0 && height_m > 0.0).then(|| {
        // PowerShell's [math]::Round rounds half to even.
        let w = (width_m * 1000.0 / pitch).round_ties_even() as u64;
        let h = (height_m * 1000.0 / pitch).round_ties_even() as u64;
        PitchResolution {
            width: w,
            height: h,
            megapixels: (w as f64) * (h as f64) / 1e6,
        }
    });
    Ok(PixelPitchResult {
        pitch_mm: Some(pitch),
        distance_m: None,
        min_distance_m: Some(pitch * RETINA_FACTOR),
        max_pitch_mm: None,
        resolution,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-6, "{a} != {b}");
    }

    #[test]
    fn pitch_to_distance_and_resolution() {
        // lines 805-809: min dist = 2.6 * 3.438 = 8.9388; res = round(5200/2.6) x round(2600/2.6)
        // = 2000 x 1000 = 2.0 MP
        let r = pixelpitch(&PixelPitchInput {
            pitch: 2.6,
            width_m: 5.2,
            height_m: 2.6,
            ..PixelPitchInput::default()
        })
        .unwrap();
        close(r.pitch_mm.unwrap(), 2.6);
        close(r.min_distance_m.unwrap(), 8.9388);
        let res = r.resolution.unwrap();
        assert_eq!((res.width, res.height), (2000, 1000));
        close(res.megapixels, 2.0);
        assert!(r.max_pitch_mm.is_none());
    }

    #[test]
    fn distance_to_recommended_pitch() {
        // line 800: max pitch = distance / 3.438 = 6.876 / 3.438 = 2.0
        let r = pixelpitch(&PixelPitchInput {
            distance: 6.876,
            ..PixelPitchInput::default()
        })
        .unwrap();
        close(r.max_pitch_mm.unwrap(), 2.0);
        close(r.distance_m.unwrap(), 6.876);
        assert!(r.pitch_mm.is_none() && r.resolution.is_none());
    }

    #[test]
    fn pitch_wins_over_distance() {
        let r = pixelpitch(&PixelPitchInput {
            pitch: 3.9,
            distance: 20.0,
            ..PixelPitchInput::default()
        })
        .unwrap();
        close(r.min_distance_m.unwrap(), 3.9 * 3.438);
        assert!(r.max_pitch_mm.is_none());
    }

    #[test]
    fn resolution_needs_both_sides_and_rounds() {
        // only a width: no resolution (line 807); 1 m / 3.9 mm = 256.4 -> 256, 2 m / 3.9 = 512.8 -> 513
        let r = pixelpitch(&PixelPitchInput {
            pitch: 3.9,
            width_m: 4.0,
            ..PixelPitchInput::default()
        })
        .unwrap();
        assert!(r.resolution.is_none());
        let r = pixelpitch(&PixelPitchInput {
            pitch: 3.9,
            width_m: 1.0,
            height_m: 2.0,
            ..PixelPitchInput::default()
        })
        .unwrap();
        let res = r.resolution.unwrap();
        assert_eq!((res.width, res.height), (256, 513));
    }

    #[test]
    fn invalid_input() {
        for bad in [
            PixelPitchInput::default(),
            PixelPitchInput {
                pitch: -2.6,
                ..PixelPitchInput::default()
            },
            PixelPitchInput {
                distance: -1.0,
                ..PixelPitchInput::default()
            },
            PixelPitchInput {
                pitch: 2.6,
                width_m: -1.0,
                ..PixelPitchInput::default()
            },
            PixelPitchInput {
                pitch: f64::NAN,
                ..PixelPitchInput::default()
            },
        ] {
            assert!(
                matches!(pixelpitch(&bad), Err(Error::InvalidInput(_))),
                "{bad:?}"
            );
        }
    }
}
