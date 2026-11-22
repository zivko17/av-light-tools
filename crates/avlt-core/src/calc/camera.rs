//! Camera field of view and DORI distances (IEC 62676-4).
//! Ports `Get-CameraDistance` (AVCalc.ps1 lines 855-876).

use serde::Serialize;

use super::{require_non_negative, require_positive};
use crate::error::{Error, Result};

/// DORI levels: name and pixels-per-metre threshold.
pub const DORI_LEVELS: [(&str, f64); 4] = [
    ("Detect", 25.0),
    ("Observe", 62.5),
    ("Recognize", 125.0),
    ("Identify", 250.0),
];

/// Inputs.
#[derive(Debug, Clone, PartialEq)]
pub struct CameraInput {
    /// Focal length (mm).
    pub focal_length: f64,
    /// Sensor width (mm): 36 full-frame, about 5.37 for 1/2.8".
    pub sensor_width: f64,
    /// Horizontal resolution (px).
    pub h_resolution: u32,
    /// Distance (m) for the pixels-per-metre figure; 0 = skip.
    pub distance: f64,
}

impl Default for CameraInput {
    fn default() -> Self {
        CameraInput {
            focal_length: 0.0,
            sensor_width: 36.0,
            h_resolution: 1920,
            distance: 0.0,
        }
    }
}

/// Maximum distance for one DORI level.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DoriLevel {
    pub name: &'static str,
    pub px_per_m: f64,
    pub max_distance_m: f64,
}

/// Field of view and DORI table.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CameraResult {
    pub hfov_deg: f64,
    /// Scene width at `distance`.
    pub fov_width_m: Option<f64>,
    /// Pixel density at `distance`.
    pub px_per_m: Option<f64>,
    pub dori: Vec<DoriLevel>,
}

/// Horizontal field of view and DORI distances for a lens and sensor.
pub fn camera(input: &CameraInput) -> Result<CameraResult> {
    let focal = require_positive("focal length", input.focal_length)?;
    let sensor = require_positive("sensor-width", input.sensor_width)?;
    if input.h_resolution == 0 {
        return Err(Error::InvalidInput(
            "h-resolution must be greater than 0".into(),
        ));
    }
    let distance = require_non_negative("distance", input.distance)?;
    let res = f64::from(input.h_resolution);

    let hfov = 2.0 * (sensor / (2.0 * focal)).atan().to_degrees();
    let fov_width = (distance > 0.0).then(|| distance * sensor / focal);
    Ok(CameraResult {
        hfov_deg: hfov,
        fov_width_m: fov_width,
        px_per_m: fov_width.map(|w| res / w),
        dori: DORI_LEVELS
            .iter()
            .map(|&(name, ppm)| DoriLevel {
                name,
                px_per_m: ppm,
                max_distance_m: res * focal / (ppm * sensor),
            })
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-6, "{a} != {b}");
    }

    fn lens(f: f64) -> CameraInput {
        CameraInput {
            focal_length: f,
            ..CameraInput::default()
        }
    }

    #[test]
    fn full_frame_50mm() {
        // line 864: hfov = 2 * atan(36 / (2*50)) in degrees = 39.59775
        let r = camera(&lens(50.0)).unwrap();
        close(r.hfov_deg, 39.597_752_709_049);
        assert!(r.fov_width_m.is_none() && r.px_per_m.is_none());
    }

    #[test]
    fn dori_distances() {
        // line 873: res * f / (ppm * sensor) = 1920*50/(25*36) = 106.667; /62.5 -> 42.667;
        // /125 -> 21.333; /250 -> 10.667
        let r = camera(&lens(50.0)).unwrap();
        let names: Vec<&str> = r.dori.iter().map(|d| d.name).collect();
        assert_eq!(names, ["Detect", "Observe", "Recognize", "Identify"]);
        let dist: Vec<f64> = r.dori.iter().map(|d| d.max_distance_m).collect();
        close(dist[0], 106.666_666_667);
        close(dist[1], 42.666_666_667);
        close(dist[2], 21.333_333_333);
        close(dist[3], 10.666_666_667);
        close(r.dori[1].px_per_m, 62.5);
    }

    #[test]
    fn pixel_density_at_distance() {
        // lines 868-869: fov width = 10 * 36 / 50 = 7.2 m; px/m = 1920 / 7.2 = 266.667
        let r = camera(&CameraInput {
            distance: 10.0,
            ..lens(50.0)
        })
        .unwrap();
        close(r.fov_width_m.unwrap(), 7.2);
        close(r.px_per_m.unwrap(), 266.666_666_667);
    }

    #[test]
    fn small_sensor_cctv() {
        // 1/2.8" sensor 5.37 mm, 2.8 mm lens: hfov = 2*atan(5.37/5.6) = 87.5978 deg;
        // detect distance = 1920 * 2.8 / (25 * 5.37) = 40.0447 m
        let r = camera(&CameraInput {
            sensor_width: 5.37,
            ..lens(2.8)
        })
        .unwrap();
        close(r.hfov_deg, 87.597_794_194_264);
        close(r.dori[0].max_distance_m, 40.044_692_737_43);
    }

    #[test]
    fn invalid_input() {
        for bad in [
            lens(0.0),
            lens(-35.0),
            CameraInput {
                sensor_width: 0.0,
                ..lens(50.0)
            },
            CameraInput {
                h_resolution: 0,
                ..lens(50.0)
            },
            CameraInput {
                distance: -1.0,
                ..lens(50.0)
            },
            lens(f64::NAN),
        ] {
            assert!(
                matches!(camera(&bad), Err(Error::InvalidInput(_))),
                "{bad:?}"
            );
        }
    }
}
