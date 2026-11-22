//! Projector throw distance and required brightness.
//! Ports `Get-Projector` (AVCalc.ps1 lines 759-785).

use serde::Serialize;

use super::{parse_aspect, require_non_negative, require_positive};
use crate::error::{Error, Result};

/// Inputs. Give an image `width` or a `diagonal` (width wins); 0 = not given.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectorInput {
    /// Image width (m).
    pub width: f64,
    /// Image diagonal (m), used with `aspect` when no width is given.
    pub diagonal: f64,
    pub aspect: String,
    /// Throw ratio -> distance (wins over `distance`).
    pub throw_ratio: f64,
    /// Distance (m) -> throw ratio.
    pub distance: f64,
    /// Screen gain.
    pub gain: f64,
    /// Target foot-lamberts (16 dim, 30-50 with ambient light).
    pub target_fl: f64,
}

impl Default for ProjectorInput {
    fn default() -> Self {
        ProjectorInput {
            width: 0.0,
            diagonal: 0.0,
            aspect: "16:9".into(),
            throw_ratio: 0.0,
            distance: 0.0,
            gain: 1.0,
            target_fl: 16.0,
        }
    }
}

/// Image size, throw and brightness.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProjectorResult {
    pub width_m: f64,
    pub height_m: f64,
    pub area_ft2: f64,
    /// Throw distance for the given throw ratio.
    pub throw_distance_m: Option<f64>,
    /// Throw ratio at the given distance (when no ratio was given).
    pub throw_ratio: Option<f64>,
    /// Required ANSI lumens: target fL x area (ft^2) / gain.
    pub lumens: f64,
    pub target_fl: f64,
    pub gain: f64,
}

/// Sizes a projected image and estimates the lumens needed.
pub fn projector(input: &ProjectorInput) -> Result<ProjectorResult> {
    let ar = parse_aspect(&input.aspect)?;
    let mut width = require_non_negative("width", input.width)?;
    let diagonal = require_non_negative("diagonal", input.diagonal)?;
    let throw_ratio = require_non_negative("throw-ratio", input.throw_ratio)?;
    let distance = require_non_negative("distance", input.distance)?;
    let gain = require_positive("gain", input.gain)?;
    let target_fl = require_positive("target-fl", input.target_fl)?;

    if width <= 0.0 && diagonal > 0.0 {
        width = diagonal * ar / (ar * ar + 1.0).sqrt();
    }
    if width <= 0.0 {
        return Err(Error::InvalidInput(
            "give an image --width or --diagonal (e.g. calc projector --width 4 --throw-ratio 1.5)"
                .into(),
        ));
    }
    let height = width / ar;
    let area_ft2 = (width * 3.28084) * (height * 3.28084);
    let (throw_distance_m, ratio) = if throw_ratio > 0.0 {
        (Some(throw_ratio * width), None)
    } else if distance > 0.0 {
        (None, Some(distance / width))
    } else {
        (None, None)
    };
    Ok(ProjectorResult {
        width_m: width,
        height_m: height,
        area_ft2,
        throw_distance_m,
        throw_ratio: ratio,
        lumens: target_fl * area_ft2 / gain,
        target_fl,
        gain,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-6, "{a} != {b}");
    }

    fn base() -> ProjectorInput {
        ProjectorInput {
            width: 4.0,
            aspect: "4:3".into(),
            ..ProjectorInput::default()
        }
    }

    #[test]
    fn throw_distance_and_lumens() {
        // lines 775-782: 4:3 width 4 -> height 3; area = (4*3.28084)*(3*3.28084) = 129.1669 ft^2;
        // lumens = 16 * area / 1 = 2066.6709; throw = ratio * width = 1.5 * 4
        let r = projector(&ProjectorInput {
            throw_ratio: 1.5,
            ..base()
        })
        .unwrap();
        close(r.width_m, 4.0);
        close(r.height_m, 3.0);
        close(r.area_ft2, 129.166_933_267_2);
        close(r.throw_distance_m.unwrap(), 6.0);
        assert!(r.throw_ratio.is_none());
        close(r.lumens, 2_066.670_932_275);
    }

    #[test]
    fn throw_ratio_from_distance() {
        // line 781: ratio = distance / width = 8 / 4
        let r = projector(&ProjectorInput {
            distance: 8.0,
            ..base()
        })
        .unwrap();
        close(r.throw_ratio.unwrap(), 2.0);
        assert!(r.throw_distance_m.is_none());
    }

    #[test]
    fn throw_ratio_wins_over_distance() {
        let r = projector(&ProjectorInput {
            throw_ratio: 1.5,
            distance: 8.0,
            ..base()
        })
        .unwrap();
        assert!(r.throw_distance_m.is_some() && r.throw_ratio.is_none());
    }

    #[test]
    fn width_from_diagonal() {
        // line 773: width = diag * (4/3) / sqrt((4/3)^2 + 1) = 5 * (4/3) / (5/3) = 4
        let r = projector(&ProjectorInput {
            width: 0.0,
            diagonal: 5.0,
            ..base()
        })
        .unwrap();
        close(r.width_m, 4.0);
        close(r.height_m, 3.0);
    }

    #[test]
    fn gain_and_target_fl_scale_lumens() {
        // line 777: lumens = 30 * 129.1669 / 2 = 1937.504
        let r = projector(&ProjectorInput {
            gain: 2.0,
            target_fl: 30.0,
            ..base()
        })
        .unwrap();
        close(r.lumens, 1_937.503_999_008);
    }

    #[test]
    fn invalid_input() {
        for bad in [
            ProjectorInput::default(),
            ProjectorInput {
                aspect: "16:0".into(),
                ..base()
            },
            ProjectorInput {
                width: -1.0,
                ..base()
            },
            ProjectorInput {
                gain: 0.0,
                ..base()
            },
            ProjectorInput {
                gain: -1.0,
                ..base()
            },
            ProjectorInput {
                target_fl: 0.0,
                ..base()
            },
            ProjectorInput {
                throw_ratio: -1.0,
                ..base()
            },
            ProjectorInput {
                distance: f64::NAN,
                ..base()
            },
        ] {
            assert!(
                matches!(projector(&bad), Err(Error::InvalidInput(_))),
                "{bad:?}"
            );
        }
    }
}
