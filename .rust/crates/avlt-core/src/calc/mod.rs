//! AV / IT calculators. Pure functions: inputs in, serializable results out, nothing printed.
//!
//! Rounding is left to the presenter; results carry full `f64` precision.

pub mod audiobw;
pub mod avoip;
pub mod bitrate;
pub mod brightness;
pub mod cable;
pub mod camera;
pub mod dsp;
pub mod led_cabinets;
pub mod ledwall;
pub mod pixelpitch;
pub mod projector;
pub mod screen;
pub mod speaker;
pub mod subnet;
pub mod viewangle;

use crate::error::{Error, Result};

/// Fails unless `v` is a finite number greater than zero.
pub(crate) fn require_positive(name: &str, v: f64) -> Result<f64> {
    if v.is_finite() && v > 0.0 {
        Ok(v)
    } else {
        Err(Error::InvalidInput(format!(
            "{name} must be greater than 0 (got {v})"
        )))
    }
}

/// Fails unless `v` is a finite number that is zero or greater (zero means "not given").
pub(crate) fn require_non_negative(name: &str, v: f64) -> Result<f64> {
    if v.is_finite() && v >= 0.0 {
        Ok(v)
    } else {
        Err(Error::InvalidInput(format!(
            "{name} must be 0 or greater (got {v})"
        )))
    }
}

/// Parses `w:h` (e.g. `16:9`) into width / height. Both parts must be positive numbers.
pub(crate) fn parse_aspect(aspect: &str) -> Result<f64> {
    let invalid = || Error::InvalidInput(format!("invalid aspect '{aspect}' (use w:h, e.g. 16:9)"));
    let (w, h) = aspect.split_once(':').ok_or_else(invalid)?;
    let w: f64 = w.trim().parse().map_err(|_| invalid())?;
    let h: f64 = h.trim().parse().map_err(|_| invalid())?;
    if w.is_finite() && h.is_finite() && w > 0.0 && h > 0.0 {
        Ok(w / h)
    } else {
        Err(invalid())
    }
}
