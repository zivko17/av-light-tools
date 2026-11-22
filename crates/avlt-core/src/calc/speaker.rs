//! Ceiling speaker coverage layout and amplifier sizing.
//! Ports `Get-SpeakerDesign` (AVCalc.ps1 lines 899-932).

use serde::Serialize;

use super::{require_non_negative, require_positive};
use crate::error::{Error, Result};

/// Inputs. Coverage mode needs `ceiling_height`; amp mode needs `speakers` and `watts_each`.
#[derive(Debug, Clone, PartialEq)]
pub struct SpeakerInput {
    /// Ceiling height (m); 0 = no coverage calculation.
    pub ceiling_height: f64,
    /// Listener ear height (m), 1.2 seated.
    pub ear_height: f64,
    /// Speaker -6 dB coverage angle in degrees (0-180 exclusive).
    pub coverage_angle: f64,
    pub room_width: f64,
    pub room_length: f64,
    /// Speaker count for the amp calculation.
    pub speakers: u32,
    pub watts_each: f64,
    pub headroom_db: f64,
}

impl Default for SpeakerInput {
    fn default() -> Self {
        SpeakerInput {
            ceiling_height: 0.0,
            ear_height: 1.2,
            coverage_angle: 90.0,
            room_width: 0.0,
            room_length: 0.0,
            speakers: 0,
            watts_each: 0.0,
            headroom_db: 6.0,
        }
    }
}

/// Speaker grid for a room.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RoomLayout {
    pub cols: u64,
    pub rows: u64,
    pub total: u64,
}

/// Ceiling coverage.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Coverage {
    /// Coverage diameter at ear height.
    pub diameter_m: f64,
    /// Spacing at the coverage edge.
    pub spacing_edge_m: f64,
    /// Spacing with 0.707 overlap (even coverage).
    pub spacing_overlap_m: f64,
    pub room: Option<RoomLayout>,
}

/// Amplifier sizing.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AmpSizing {
    pub speakers: u32,
    pub watts_each: f64,
    pub total_w: f64,
    pub headroom_db: f64,
    /// Total power with headroom: total x 10^(dB/10).
    pub required_w: f64,
}

/// Coverage layout and/or amp sizing.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SpeakerResult {
    pub coverage: Option<Coverage>,
    pub amp: Option<AmpSizing>,
}

/// Ceiling speaker spacing and room layout, and amplifier power with headroom.
pub fn speaker(input: &SpeakerInput) -> Result<SpeakerResult> {
    let ceiling = require_non_negative("ceiling-height", input.ceiling_height)?;
    let ear = require_non_negative("ear-height", input.ear_height)?;
    let angle = require_positive("coverage-angle", input.coverage_angle)?;
    if angle >= 180.0 {
        return Err(Error::InvalidInput(format!(
            "coverage-angle must be below 180 degrees (got {angle})"
        )));
    }
    let room_w = require_non_negative("room-width", input.room_width)?;
    let room_l = require_non_negative("room-length", input.room_length)?;
    let watts = require_non_negative("watts-each", input.watts_each)?;
    let headroom = require_non_negative("headroom-db", input.headroom_db)?;

    let coverage = if ceiling > 0.0 {
        if ceiling <= ear {
            return Err(Error::InvalidInput(format!(
                "ceiling height ({ceiling} m) must be above the ear height ({ear} m)"
            )));
        }
        let diameter = 2.0 * (ceiling - ear) * (angle.to_radians() / 2.0).tan();
        let spacing = diameter * 0.707;
        let room = (room_w > 0.0 && room_l > 0.0 && spacing > 0.0).then(|| {
            let cols = (room_w / spacing).ceil() as u64;
            let rows = (room_l / spacing).ceil() as u64;
            RoomLayout {
                cols,
                rows,
                total: cols * rows,
            }
        });
        Some(Coverage {
            diameter_m: diameter,
            spacing_edge_m: diameter,
            spacing_overlap_m: spacing,
            room,
        })
    } else {
        None
    };

    let amp = (input.speakers > 0 && watts > 0.0).then(|| {
        let total = f64::from(input.speakers) * watts;
        AmpSizing {
            speakers: input.speakers,
            watts_each: watts,
            total_w: total,
            headroom_db: headroom,
            required_w: total * 10f64.powf(headroom / 10.0),
        }
    });

    if coverage.is_none() && amp.is_none() {
        return Err(Error::InvalidInput(
            "give --ceiling-height (coverage) or --speakers with --watts-each (amp)".into(),
        ));
    }
    Ok(SpeakerResult { coverage, amp })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-6, "{a} != {b}");
    }

    fn ceiling(h: f64) -> SpeakerInput {
        SpeakerInput {
            ceiling_height: h,
            ..SpeakerInput::default()
        }
    }

    #[test]
    fn coverage_at_90_degrees() {
        // lines 913-917: dia = 2 * (3 - 1.2) * tan(45 deg) = 3.6; spacing = 3.6 * 0.707 = 2.5452
        let r = speaker(&ceiling(3.0)).unwrap();
        let c = r.coverage.unwrap();
        close(c.diameter_m, 3.6);
        close(c.spacing_edge_m, 3.6);
        close(c.spacing_overlap_m, 2.5452);
        assert!(c.room.is_none() && r.amp.is_none());
    }

    #[test]
    fn room_layout_rounds_up() {
        // line 919: cols = ceil(10 / 2.5452) = ceil(3.93) = 4; rows = ceil(6 / 2.5452) = ceil(2.36) = 3
        let r = speaker(&SpeakerInput {
            room_width: 10.0,
            room_length: 6.0,
            ..ceiling(3.0)
        })
        .unwrap();
        let room = r.coverage.unwrap().room.unwrap();
        assert_eq!((room.cols, room.rows, room.total), (4, 3, 12));
    }

    #[test]
    fn narrower_coverage_angle() {
        // 60 deg: dia = 2 * 1.8 * tan(30 deg) = 2.0784610
        let c = speaker(&SpeakerInput {
            coverage_angle: 60.0,
            ..ceiling(3.0)
        })
        .unwrap()
        .coverage
        .unwrap();
        close(c.diameter_m, 2.078_460_969_083);
    }

    #[test]
    fn amp_headroom() {
        // line 925: 12 x 10 W = 120 W; +6 dB -> 120 * 10^0.6 = 477.7286 W
        let r = speaker(&SpeakerInput {
            speakers: 12,
            watts_each: 10.0,
            ..SpeakerInput::default()
        })
        .unwrap();
        assert!(r.coverage.is_none());
        let a = r.amp.unwrap();
        close(a.total_w, 120.0);
        close(a.required_w, 477.728_604_664);
        let flat = speaker(&SpeakerInput {
            speakers: 12,
            watts_each: 10.0,
            headroom_db: 0.0,
            ..SpeakerInput::default()
        })
        .unwrap();
        close(flat.amp.unwrap().required_w, 120.0);
    }

    #[test]
    fn both_modes_together() {
        let r = speaker(&SpeakerInput {
            speakers: 4,
            watts_each: 5.0,
            ..ceiling(3.0)
        })
        .unwrap();
        assert!(r.coverage.is_some() && r.amp.is_some());
    }

    #[test]
    fn invalid_input() {
        for bad in [
            // nothing to compute
            SpeakerInput::default(),
            // speakers without watts is not an amp calculation
            SpeakerInput {
                speakers: 12,
                ..SpeakerInput::default()
            },
            // ceiling at or below ear height
            ceiling(1.2),
            ceiling(1.0),
            ceiling(-3.0),
            SpeakerInput {
                coverage_angle: 0.0,
                ..ceiling(3.0)
            },
            SpeakerInput {
                coverage_angle: 180.0,
                ..ceiling(3.0)
            },
            SpeakerInput {
                coverage_angle: -90.0,
                ..ceiling(3.0)
            },
            SpeakerInput {
                ear_height: -1.0,
                ..ceiling(3.0)
            },
            SpeakerInput {
                room_width: -1.0,
                ..ceiling(3.0)
            },
            SpeakerInput {
                headroom_db: -3.0,
                speakers: 2,
                watts_each: 5.0,
                ..SpeakerInput::default()
            },
            SpeakerInput {
                watts_each: -5.0,
                speakers: 2,
                ..SpeakerInput::default()
            },
        ] {
            assert!(
                matches!(speaker(&bad), Err(Error::InvalidInput(_))),
                "{bad:?}"
            );
        }
    }
}
