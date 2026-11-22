//! AV-over-IP bandwidth per codec (estimates).
//! Ports `Get-AVoIPBandwidth` (AVCalc.ps1 lines 736-755).

use serde::Serialize;

use super::require_positive;
use crate::error::{Error, Result};

/// Inputs; defaults are 60 fps, 8-bit.
#[derive(Debug, Clone, PartialEq)]
pub struct AvoipInput {
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub bit_depth: u32,
}

impl Default for AvoipInput {
    fn default() -> Self {
        AvoipInput {
            width: 0,
            height: 0,
            fps: 60.0,
            bit_depth: 8,
        }
    }
}

/// Bandwidth per codec in Mbps.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AvoipResult {
    pub raw_mbps: f64,
    /// Pixels x fps relative to 1080p60.
    pub scale_factor: f64,
    /// SDVoE is a fixed 10GbE class, not scaled.
    pub sdvoe_class_mbps: f64,
    /// JPEG-XS light compression: 800 Mbps at 1080p60.
    pub jpeg_xs_mbps: f64,
    /// NDI High Bandwidth: 125 Mbps at 1080p60.
    pub ndi_hb_mbps: f64,
    /// NDI|HX / H.26x heavy compression: 12 Mbps at 1080p60.
    pub ndi_hx_mbps: f64,
}

/// Estimates AV-over-IP bandwidth for a video format.
pub fn avoip(input: &AvoipInput) -> Result<AvoipResult> {
    if input.width == 0 || input.height == 0 {
        return Err(Error::InvalidInput(
            "width and height must be greater than 0".into(),
        ));
    }
    if input.bit_depth == 0 {
        return Err(Error::InvalidInput(
            "bit depth must be greater than 0".into(),
        ));
    }
    let fps = require_positive("fps", input.fps)?;

    let px = f64::from(input.width) * f64::from(input.height);
    let scale = px * fps / (1920.0 * 1080.0 * 60.0);
    Ok(AvoipResult {
        raw_mbps: px * fps * f64::from(input.bit_depth) * 3.0 / 1e6,
        scale_factor: scale,
        sdvoe_class_mbps: 10_000.0,
        jpeg_xs_mbps: 800.0 * scale,
        ndi_hb_mbps: 125.0 * scale,
        ndi_hx_mbps: 12.0 * scale,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-6, "{a} != {b}");
    }

    fn fmt(width: u32, height: u32, fps: f64) -> AvoipInput {
        AvoipInput {
            width,
            height,
            fps,
            ..AvoipInput::default()
        }
    }

    #[test]
    fn hd60_is_the_anchor() {
        // lines 745-752: sf = px*fps/(1920*1080*60) = 1 -> 800 / 125 / 12; raw = px*60*8*3/1e6
        let r = avoip(&fmt(1920, 1080, 60.0)).unwrap();
        close(r.scale_factor, 1.0);
        close(r.jpeg_xs_mbps, 800.0);
        close(r.ndi_hb_mbps, 125.0);
        close(r.ndi_hx_mbps, 12.0);
        close(r.raw_mbps, 2985.984);
        close(r.sdvoe_class_mbps, 10_000.0);
    }

    #[test]
    fn uhd60_scales_by_four() {
        let r = avoip(&fmt(3840, 2160, 60.0)).unwrap();
        close(r.scale_factor, 4.0);
        close(r.jpeg_xs_mbps, 3200.0);
        close(r.ndi_hb_mbps, 500.0);
        close(r.ndi_hx_mbps, 48.0);
        close(r.raw_mbps, 11_943.936);
    }

    #[test]
    fn hd720_30_and_bit_depth() {
        // 921600 px * 30 / 124_416_000 = 0.2222..; raw = 921600*30*8*3/1e6 = 663.552
        let r = avoip(&fmt(1280, 720, 30.0)).unwrap();
        close(r.jpeg_xs_mbps, 177.777_777_78);
        close(r.ndi_hb_mbps, 27.777_777_78);
        close(r.ndi_hx_mbps, 2.666_666_67);
        close(r.raw_mbps, 663.552);
        // 10-bit raw is 10/8 of 8-bit
        let r10 = avoip(&AvoipInput {
            bit_depth: 10,
            ..fmt(1280, 720, 30.0)
        })
        .unwrap();
        close(r10.raw_mbps, 663.552 * 1.25);
    }

    #[test]
    fn invalid_input() {
        for bad in [
            fmt(0, 1080, 60.0),
            fmt(1920, 0, 60.0),
            fmt(1920, 1080, 0.0),
            fmt(1920, 1080, -60.0),
            fmt(1920, 1080, f64::INFINITY),
            AvoipInput {
                bit_depth: 0,
                ..fmt(1920, 1080, 60.0)
            },
        ] {
            assert!(
                matches!(avoip(&bad), Err(Error::InvalidInput(_))),
                "{bad:?}"
            );
        }
    }
}
