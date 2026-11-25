//! Required display luminance versus ambient light (guideline).
//! Ports `Get-DisplayBrightness` (AVCalc.ps1 lines 816-831).

use serde::Serialize;

use super::{require_non_negative, require_positive};
use crate::error::{Error, Result};

/// Content type; sets the AVIXA contrast-ratio target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BrightnessContent {
    /// 7:1.
    #[default]
    Basic,
    /// 15:1.
    Detailed,
}

/// Inputs.
#[derive(Debug, Clone, PartialEq)]
pub struct BrightnessInput {
    /// Ambient light at the screen (lux).
    pub ambient_lux: f64,
    /// Display diffuse reflectance (0-1).
    pub reflectance: f64,
    pub content: BrightnessContent,
}

impl Default for BrightnessInput {
    fn default() -> Self {
        BrightnessInput {
            ambient_lux: 0.0,
            reflectance: 0.15,
            content: BrightnessContent::Basic,
        }
    }
}

/// Luminance needed.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BrightnessResult {
    pub content: BrightnessContent,
    /// Contrast-ratio target (7 or 15).
    pub contrast_ratio: u32,
    /// Ambient light reflected by the screen (cd/m^2, Lambertian).
    pub reflected_nits: f64,
    /// Display luminance for the contrast target.
    pub required_nits: f64,
}

/// Required display brightness for an ambient light level.
pub fn brightness(input: &BrightnessInput) -> Result<BrightnessResult> {
    let lux = require_non_negative("ambient lux", input.ambient_lux)?;
    let reflectance = require_positive("reflectance", input.reflectance)?;
    if reflectance > 1.0 {
        return Err(Error::InvalidInput(format!(
            "reflectance must be between 0 and 1 (got {reflectance})"
        )));
    }
    // AVIXA contrast-ratio targets
    let contrast_ratio = match input.content {
        BrightnessContent::Basic => 7,
        BrightnessContent::Detailed => 15,
    };
    let reflected = lux * reflectance / std::f64::consts::PI; // cd/m^2 (Lambertian)
    Ok(BrightnessResult {
        content: input.content,
        contrast_ratio,
        reflected_nits: reflected,
        required_nits: reflected * f64::from(contrast_ratio - 1),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-6, "{a} != {b}");
    }

    fn lux(v: f64) -> BrightnessInput {
        BrightnessInput {
            ambient_lux: v,
            ..BrightnessInput::default()
        }
    }

    #[test]
    fn basic_content() {
        // lines 823-828: reflected = 500 * 0.15 / pi = 23.87324; required = reflected * (7 - 1)
        let r = brightness(&lux(500.0)).unwrap();
        assert_eq!(r.contrast_ratio, 7);
        close(r.reflected_nits, 23.873_241_463_784);
        close(r.required_nits, 143.239_448_782_706);
    }

    #[test]
    fn detailed_content() {
        // required = reflected * (15 - 1) = 334.22538
        let r = brightness(&BrightnessInput {
            content: BrightnessContent::Detailed,
            ..lux(500.0)
        })
        .unwrap();
        assert_eq!(r.contrast_ratio, 15);
        close(r.required_nits, 334.225_380_492_993);
    }

    #[test]
    fn reflectance_scales_linearly() {
        // lux = pi, reflectance 0.5 -> reflected = 0.5 nits, required basic = 3.0
        let r = brightness(&BrightnessInput {
            ambient_lux: std::f64::consts::PI,
            reflectance: 0.5,
            ..BrightnessInput::default()
        })
        .unwrap();
        close(r.reflected_nits, 0.5);
        close(r.required_nits, 3.0);
    }

    #[test]
    fn zero_lux_needs_zero_nits() {
        let r = brightness(&lux(0.0)).unwrap();
        close(r.reflected_nits, 0.0);
        close(r.required_nits, 0.0);
    }

    #[test]
    fn invalid_input() {
        for bad in [
            lux(-100.0),
            lux(f64::NAN),
            BrightnessInput {
                reflectance: 0.0,
                ..lux(500.0)
            },
            BrightnessInput {
                reflectance: -0.1,
                ..lux(500.0)
            },
            BrightnessInput {
                reflectance: 1.5,
                ..lux(500.0)
            },
        ] {
            assert!(
                matches!(brightness(&bad), Err(Error::InvalidInput(_))),
                "{bad:?}"
            );
        }
    }
}
