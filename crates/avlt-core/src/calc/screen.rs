//! Screen size versus viewing distance (AVIXA 4/6/8:1 guideline).
//! Ports `Get-ScreenSize` (AVCalc.ps1 lines 76-113).

use serde::Serialize;

use super::{parse_aspect, require_non_negative};
use crate::error::{Error, Result};

/// Content type; decides the viewing-distance ratio (distance = height x ratio).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Content {
    /// 4:1 - detailed content (spreadsheets, CAD).
    Detailed,
    /// 6:1 - basic decision-making content.
    Basic,
    /// 8:1 - passive viewing.
    Passive,
}

impl Content {
    /// Farthest-viewer distance as a multiple of the image height.
    pub fn ratio(self) -> u32 {
        match self {
            Content::Detailed => 4,
            Content::Basic => 6,
            Content::Passive => 8,
        }
    }
}

/// Which direction the calculation ran.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ScreenMode {
    /// Farthest viewer distance given; minimum screen size computed.
    Forward,
    /// Diagonal given; maximum viewing distance computed.
    Reverse,
}

/// Inputs; `distance` and `diagonal` are alternatives (diagonal wins), 0 = not given.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ScreenInput {
    /// Farthest viewer distance.
    pub distance: f64,
    /// Screen diagonal (reverse mode).
    pub diagonal: f64,
    pub content: Content,
    /// `w:h`, default `16:9`.
    pub aspect: String,
    /// Unit label (`m` or `ft`); `m` converts the diagonal to inches with 39.37, anything else with 12.
    pub unit: String,
}

impl Default for ScreenInput {
    fn default() -> Self {
        ScreenInput {
            distance: 0.0,
            diagonal: 0.0,
            content: Content::Basic,
            aspect: "16:9".into(),
            unit: "m".into(),
        }
    }
}

/// Result in the input's unit.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ScreenResult {
    pub mode: ScreenMode,
    pub content: Content,
    /// 4, 6 or 8.
    pub ratio: u32,
    pub unit: String,
    /// Aspect as width / height.
    pub aspect_ratio: f64,
    /// Farthest viewer distance (given in forward mode, maximum in reverse mode).
    pub view_distance: f64,
    pub height: f64,
    pub width: f64,
    pub diagonal: f64,
    /// Diagonal in inches (forward mode only, as in the original).
    pub diagonal_in: Option<f64>,
}

/// Sizes a screen from the farthest viewer distance, or the reverse from a diagonal.
pub fn screen(input: &ScreenInput) -> Result<ScreenResult> {
    let ratio = input.content.ratio();
    let aspect_ratio = parse_aspect(&input.aspect)?;
    let distance = require_non_negative("distance", input.distance)?;
    let diagonal = require_non_negative("diagonal", input.diagonal)?;
    // diagonal = height * sqrt((w/h)^2 + 1)
    let diag_factor = (aspect_ratio * aspect_ratio + 1.0).sqrt();
    let unit = input.unit.clone();

    if diagonal > 0.0 {
        let height = diagonal / diag_factor;
        Ok(ScreenResult {
            mode: ScreenMode::Reverse,
            content: input.content,
            ratio,
            unit,
            aspect_ratio,
            view_distance: height * f64::from(ratio),
            height,
            width: height * aspect_ratio,
            diagonal,
            diagonal_in: None,
        })
    } else if distance > 0.0 {
        let height = distance / f64::from(ratio);
        let diag = height * diag_factor;
        let inches = if input.unit.eq_ignore_ascii_case("m") {
            diag * 39.37
        } else {
            diag * 12.0
        };
        Ok(ScreenResult {
            mode: ScreenMode::Forward,
            content: input.content,
            ratio,
            unit,
            aspect_ratio,
            view_distance: distance,
            height,
            width: height * aspect_ratio,
            diagonal: diag,
            diagonal_in: Some(inches),
        })
    } else {
        Err(Error::InvalidInput(
            "give a farthest viewer distance or a --diagonal (e.g. calc screen 8 --content basic)"
                .into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOL: f64 = 1e-6;

    fn d() -> ScreenInput {
        ScreenInput::default()
    }

    fn close(a: f64, b: f64) {
        assert!((a - b).abs() < TOL, "{a} != {b}");
    }

    #[test]
    fn unit_m_is_case_insensitive() {
        let base = ScreenInput {
            distance: 6.0,
            ..d()
        };
        let lower = screen(&ScreenInput {
            unit: "m".into(),
            ..base.clone()
        })
        .unwrap();
        let upper = screen(&ScreenInput {
            unit: "M".into(),
            ..base
        })
        .unwrap();
        close(upper.diagonal_in.unwrap(), lower.diagonal_in.unwrap());
    }

    // 4:3 gives an exact diagonal factor sqrt((4/3)^2 + 1) = 5/3 (AVCalc.ps1 line 89).
    #[test]
    fn forward_detailed_4_3() {
        // line 98-101: height = distance/ratio = 6/4; width = height*4/3; diag = height*5/3
        let r = screen(&ScreenInput {
            distance: 6.0,
            content: Content::Detailed,
            aspect: "4:3".into(),
            ..d()
        })
        .unwrap();
        assert_eq!(r.mode, ScreenMode::Forward);
        assert_eq!(r.ratio, 4);
        close(r.height, 1.5);
        close(r.width, 2.0);
        close(r.diagonal, 2.5);
        close(r.view_distance, 6.0);
        close(r.diagonal_in.unwrap(), 2.5 * 39.37);
    }

    #[test]
    fn forward_in_feet_uses_12_inches() {
        // line 101: unit != 'm' -> inch = diag * 12
        let r = screen(&ScreenInput {
            distance: 6.0,
            content: Content::Detailed,
            aspect: "4:3".into(),
            unit: "ft".into(),
            ..d()
        })
        .unwrap();
        close(r.diagonal_in.unwrap(), 30.0);
    }

    #[test]
    fn reverse_from_diagonal() {
        // lines 93-96: height = diagonal / (5/3) = 1.5; max distance = height * 6 (basic)
        let r = screen(&ScreenInput {
            diagonal: 2.5,
            aspect: "4:3".into(),
            ..d()
        })
        .unwrap();
        assert_eq!(r.mode, ScreenMode::Reverse);
        assert_eq!(r.ratio, 6);
        close(r.height, 1.5);
        close(r.width, 2.0);
        close(r.view_distance, 9.0);
        assert!(r.diagonal_in.is_none());
    }

    #[test]
    fn passive_ratio_is_8_and_default_aspect_is_16_9() {
        // distance 8 -> height 1; width 16/9 = 1.7777..
        let r = screen(&ScreenInput {
            distance: 8.0,
            content: Content::Passive,
            ..d()
        })
        .unwrap();
        assert_eq!(r.ratio, 8);
        close(r.height, 1.0);
        close(r.width, 16.0 / 9.0);
    }

    #[test]
    fn diagonal_wins_over_distance() {
        let r = screen(&ScreenInput {
            distance: 100.0,
            diagonal: 2.5,
            aspect: "4:3".into(),
            ..d()
        })
        .unwrap();
        assert_eq!(r.mode, ScreenMode::Reverse);
    }

    #[test]
    fn invalid_input() {
        let ok = ScreenInput {
            distance: 10.0,
            ..d()
        };
        for aspect in ["16:0", "0:9", "16", "a:b", "16:9:1", "-16:9", ""] {
            let r = screen(&ScreenInput {
                aspect: aspect.into(),
                ..ok.clone()
            });
            assert!(
                matches!(r, Err(Error::InvalidInput(_))),
                "aspect {aspect:?}"
            );
        }
        for bad in [
            ScreenInput {
                distance: -1.0,
                ..d()
            },
            ScreenInput {
                diagonal: -2.0,
                distance: 5.0,
                ..d()
            },
            ScreenInput {
                distance: f64::NAN,
                ..d()
            },
            // neither distance nor diagonal: usage error
            d(),
        ] {
            assert!(
                matches!(screen(&bad), Err(Error::InvalidInput(_))),
                "{bad:?}"
            );
        }
    }
}
