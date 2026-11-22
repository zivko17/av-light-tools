//! Uncompressed video bitrate with NDI estimates and storage per hour.
//! Ports `Get-VideoBitrate` (AVCalc.ps1 lines 705-732).

use serde::Serialize;

use super::require_positive;
use crate::error::{Error, Result};

/// Inputs; defaults are 25 fps, 8-bit, 3 channels, 1 hour.
#[derive(Debug, Clone, PartialEq)]
pub struct BitrateInput {
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub bit_depth: u32,
    pub channels: u32,
    pub hours: f64,
}

impl Default for BitrateInput {
    fn default() -> Self {
        BitrateInput {
            width: 0,
            height: 0,
            fps: 25.0,
            bit_depth: 8,
            channels: 3,
            hours: 1.0,
        }
    }
}

/// Bitrates in Mbps and storage in GB.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BitrateResult {
    pub pixels: f64,
    pub raw_mbps: f64,
    pub raw_gb_per_hour: f64,
    /// NDI High Bandwidth (SpeedHQ), anchored at 1080p60 = 125 Mbps and scaled by pixels x fps.
    pub ndi_hb_mbps: f64,
    pub ndi_hb_gb_per_hour: f64,
    /// NDI|HX (H.264/5), order of magnitude: one tenth of High Bandwidth.
    pub ndi_hx_mbps: f64,
    pub hours: f64,
    /// Uncompressed storage for `hours`.
    pub raw_gb_total: f64,
}

/// Computes uncompressed and NDI bitrates for a video format.
pub fn bitrate(input: &BitrateInput) -> Result<BitrateResult> {
    if input.width == 0 || input.height == 0 {
        return Err(Error::InvalidInput(
            "width and height must be greater than 0".into(),
        ));
    }
    if input.bit_depth == 0 || input.channels == 0 {
        return Err(Error::InvalidInput(
            "bit depth and channels must be greater than 0".into(),
        ));
    }
    let fps = require_positive("fps", input.fps)?;
    let hours = require_positive("hours", input.hours)?;

    let pixels = f64::from(input.width) * f64::from(input.height);
    let raw_bps = pixels * fps * f64::from(input.bit_depth) * f64::from(input.channels);
    let raw_gb_per_hour = raw_bps * 3600.0 / 8.0 / 1e9;
    // NDI High Bandwidth anchored at 1080p60 = 125 Mbps, scaled by pixels x fps
    let anchor = 1920.0 * 1080.0 * 60.0;
    let ndi_hb = 125.0 * (pixels * fps / anchor);
    Ok(BitrateResult {
        pixels,
        raw_mbps: raw_bps / 1e6,
        raw_gb_per_hour,
        ndi_hb_mbps: ndi_hb,
        ndi_hb_gb_per_hour: ndi_hb * 3600.0 / 8.0 / 1000.0,
        ndi_hx_mbps: ndi_hb / 10.0,
        hours,
        raw_gb_total: raw_gb_per_hour * hours,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-6, "{a} != {b}");
    }

    fn hd(fps: f64) -> BitrateInput {
        BitrateInput {
            width: 1920,
            height: 1080,
            fps,
            ..BitrateInput::default()
        }
    }

    #[test]
    fn hd_25fps_defaults() {
        // lines 715-727: pixels 2_073_600; raw = px*25*8*3 = 1_244_160_000 bps = 1244.16 Mbps;
        // GB/h = raw*3600/8/1e9 = 559.872; NDI HB = 125*(px*25/(1920*1080*60)) = 52.0833;
        // NDI GB/h = HB*3600/8/1000 = 23.4375; HX = HB/10
        let r = bitrate(&hd(25.0)).unwrap();
        close(r.pixels, 2_073_600.0);
        close(r.raw_mbps, 1244.16);
        close(r.raw_gb_per_hour, 559.872);
        close(r.ndi_hb_mbps, 52.083_333_333);
        close(r.ndi_hb_gb_per_hour, 23.4375);
        close(r.ndi_hx_mbps, 5.208_333_333);
        close(r.raw_gb_total, 559.872);
    }

    #[test]
    fn hd_60fps_is_the_ndi_anchor() {
        // anchor: 1080p60 -> exactly 125 Mbps (HB), 12.5 (HX); raw = px*60*24 = 2985.984 Mbps
        let r = bitrate(&hd(60.0)).unwrap();
        close(r.ndi_hb_mbps, 125.0);
        close(r.ndi_hx_mbps, 12.5);
        close(r.ndi_hb_gb_per_hour, 56.25);
        close(r.raw_mbps, 2985.984);
        close(r.raw_gb_per_hour, 1343.6928);
    }

    #[test]
    fn uhd_10bit_and_hours() {
        // line 716: 3840*2160*30*10*3 = 7_464_960_000 bps; line 729: x hours (2 h -> 2x GB/h)
        let r = bitrate(&BitrateInput {
            width: 3840,
            height: 2160,
            fps: 30.0,
            bit_depth: 10,
            hours: 2.0,
            ..BitrateInput::default()
        })
        .unwrap();
        close(r.raw_mbps, 7464.96);
        close(r.raw_gb_total, r.raw_gb_per_hour * 2.0);
        close(r.hours, 2.0);
    }

    #[test]
    fn invalid_input() {
        for bad in [
            BitrateInput {
                width: 0,
                ..hd(25.0)
            },
            BitrateInput {
                height: 0,
                ..hd(25.0)
            },
            hd(0.0),
            hd(-25.0),
            hd(f64::NAN),
            BitrateInput {
                bit_depth: 0,
                ..hd(25.0)
            },
            BitrateInput {
                channels: 0,
                ..hd(25.0)
            },
            BitrateInput {
                hours: 0.0,
                ..hd(25.0)
            },
            BitrateInput {
                hours: -1.0,
                ..hd(25.0)
            },
        ] {
            assert!(
                matches!(bitrate(&bad), Err(Error::InvalidInput(_))),
                "{bad:?}"
            );
        }
    }
}
