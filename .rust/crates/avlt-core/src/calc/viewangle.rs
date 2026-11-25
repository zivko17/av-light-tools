//! Usable seating width from the off-axis viewing limit (geometry).
//! Ports `Get-ViewingAngle` (AVCalc.ps1 lines 835-851).

use serde::Serialize;

use super::{require_non_negative, require_positive};
use crate::error::{Error, Result};

/// Inputs.
#[derive(Debug, Clone, PartialEq)]
pub struct ViewAngleInput {
    /// Viewer distance (m).
    pub distance: f64,
    /// Maximum off-axis half-angle in degrees (default 45, must be below 90).
    pub max_angle: f64,
    /// Screen width (m) for the edge angle from a centre seat; 0 = skip.
    pub screen_width: f64,
}

impl Default for ViewAngleInput {
    fn default() -> Self {
        ViewAngleInput {
            distance: 0.0,
            max_angle: 45.0,
            screen_width: 0.0,
        }
    }
}

/// Seating width and edge angle.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ViewAngleResult {
    /// Distance from the centre line to the outermost seat.
    pub half_width_m: f64,
    pub usable_width_m: f64,
    /// Angle to the screen edge from a centre seat.
    pub screen_edge_angle_deg: Option<f64>,
}

/// Usable seating width at a distance for an off-axis limit.
pub fn viewangle(input: &ViewAngleInput) -> Result<ViewAngleResult> {
    let distance = require_positive("distance", input.distance)?;
    let angle = require_positive("max-angle", input.max_angle)?;
    if angle >= 90.0 {
        return Err(Error::InvalidInput(format!(
            "max-angle must be below 90 degrees (got {angle})"
        )));
    }
    let screen_width = require_non_negative("screen-width", input.screen_width)?;

    let half = distance * angle.to_radians().tan();
    Ok(ViewAngleResult {
        half_width_m: half,
        usable_width_m: 2.0 * half,
        screen_edge_angle_deg: (screen_width > 0.0)
            .then(|| ((screen_width / 2.0) / distance).atan().to_degrees()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-6, "{a} != {b}");
    }

    fn at(distance: f64) -> ViewAngleInput {
        ViewAngleInput {
            distance,
            ..ViewAngleInput::default()
        }
    }

    #[test]
    fn default_45_degrees() {
        // line 842: half = 10 * tan(45 deg) = 10; usable = 2 * half = 20
        let r = viewangle(&at(10.0)).unwrap();
        close(r.half_width_m, 10.0);
        close(r.usable_width_m, 20.0);
        assert!(r.screen_edge_angle_deg.is_none());
    }

    #[test]
    fn narrower_angle() {
        // half = 10 * tan(30 deg) = 5.773503
        let r = viewangle(&ViewAngleInput {
            max_angle: 30.0,
            ..at(10.0)
        })
        .unwrap();
        close(r.half_width_m, 5.773_502_691_896);
        close(r.usable_width_m, 11.547_005_383_793);
    }

    #[test]
    fn screen_edge_angle() {
        // line 847: atan((8/2)/10) = atan(0.4) = 21.80141 deg
        let r = viewangle(&ViewAngleInput {
            screen_width: 8.0,
            ..at(10.0)
        })
        .unwrap();
        close(r.screen_edge_angle_deg.unwrap(), 21.801_409_486_352);
    }

    #[test]
    fn invalid_input() {
        for bad in [
            at(0.0),
            at(-5.0),
            at(f64::NAN),
            ViewAngleInput {
                max_angle: 0.0,
                ..at(10.0)
            },
            ViewAngleInput {
                max_angle: 90.0,
                ..at(10.0)
            },
            ViewAngleInput {
                max_angle: 120.0,
                ..at(10.0)
            },
            ViewAngleInput {
                max_angle: -10.0,
                ..at(10.0)
            },
            ViewAngleInput {
                screen_width: -1.0,
                ..at(10.0)
            },
        ] {
            assert!(
                matches!(viewangle(&bad), Err(Error::InvalidInput(_))),
                "{bad:?}"
            );
        }
    }
}
