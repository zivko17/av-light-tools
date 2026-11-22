//! Display mode lists for `sys monitors` / `sys monitors-arrange` (Monitors.ps1,
//! MonitorsArrange.ps1).

use crate::error::{Error, Result};
use crate::sys::model::DisplayMode;

/// One entry of `WmiMonitorListedSupportedSourceModes.MonitorSourceModes`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceMode {
    pub width: u32,
    pub height: u32,
    pub refresh_numerator: u32,
    pub refresh_denominator: u32,
}

/// Monitors.ps1 "REFRESH RATE": the best listed mode, i.e. the largest resolution at its
/// highest refresh rate (rounded to whole hertz). Monitors.ps1 compared raw refresh
/// numerators, which come in different scales per mode, and showed e.g. 800x600; and
/// legacy modes such as 1280x1024 @ 75 Hz must not beat the native resolution.
pub fn best_source_mode(modes: &[SourceMode]) -> Option<DisplayMode> {
    modes
        .iter()
        .filter(|m| m.width > 0 && m.refresh_denominator > 0)
        .map(|m| DisplayMode {
            width: m.width,
            height: m.height,
            refresh_hz: (f64::from(m.refresh_numerator) / f64::from(m.refresh_denominator)).round()
                as u32,
        })
        .max_by_key(|m| (u64::from(m.width) * u64::from(m.height), m.refresh_hz))
}

/// Distinct resolutions, widest first (MonitorsArrange.ps1 step 2), then tallest first.
pub fn resolutions(modes: &[DisplayMode]) -> Vec<(u32, u32)> {
    let mut res: Vec<(u32, u32)> = modes
        .iter()
        .filter(|m| m.width > 0 && m.height > 0)
        .map(|m| (m.width, m.height))
        .collect();
    res.sort_unstable_by(|a, b| b.cmp(a));
    res.dedup();
    res
}

/// Distinct refresh rates of `width`x`height`, highest first (MonitorsArrange.ps1 step 3).
pub fn refresh_rates(modes: &[DisplayMode], width: u32, height: u32) -> Vec<u32> {
    let mut rates: Vec<u32> = modes
        .iter()
        .filter(|m| m.width == width && m.height == height && m.refresh_hz > 0)
        .map(|m| m.refresh_hz)
        .collect();
    rates.sort_unstable_by(|a, b| b.cmp(a));
    rates.dedup();
    rates
}

/// Parses `1920x1080` (also `X` or `×` as separator).
pub fn parse_resolution(s: &str) -> Result<(u32, u32)> {
    let bad = || {
        Error::InvalidInput(format!(
            "invalid resolution '{}': use WIDTHxHEIGHT, e.g. 1920x1080",
            s.trim()
        ))
    };
    let lower = s.trim().to_lowercase().replace('×', "x");
    let (w, h) = lower.split_once('x').ok_or_else(bad)?;
    let w: u32 = w.trim().parse().map_err(|_| bad())?;
    let h: u32 = h.trim().parse().map_err(|_| bad())?;
    if w == 0 || h == 0 {
        return Err(bad());
    }
    Ok((w, h))
}

/// The mode of `modes` with that resolution and refresh rate (highest rate if `hz` is
/// `None`). Errors list what the display offers.
pub fn pick_mode(
    modes: &[DisplayMode],
    width: u32,
    height: u32,
    hz: Option<u32>,
) -> Result<DisplayMode> {
    let rates = refresh_rates(modes, width, height);
    if rates.is_empty() {
        let offered: Vec<String> = resolutions(modes)
            .iter()
            .map(|(w, h)| format!("{w}x{h}"))
            .collect();
        return Err(Error::InvalidInput(format!(
            "{width}x{height} is not offered by this display. Available: {}",
            offered.join(", ")
        )));
    }
    let refresh_hz = match hz {
        None => rates[0],
        Some(r) if rates.contains(&r) => r,
        Some(r) => {
            let offered: Vec<String> = rates.iter().map(u32::to_string).collect();
            return Err(Error::InvalidInput(format!(
                "{r} Hz is not offered at {width}x{height}. Available: {} Hz",
                offered.join(", ")
            )));
        }
    };
    Ok(DisplayMode {
        width,
        height,
        refresh_hz,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(width: u32, height: u32, refresh_hz: u32) -> DisplayMode {
        DisplayMode {
            width,
            height,
            refresh_hz,
        }
    }

    fn sm(width: u32, height: u32, num: u32, den: u32) -> SourceMode {
        SourceMode {
            width,
            height,
            refresh_numerator: num,
            refresh_denominator: den,
        }
    }

    #[test]
    fn best_source_mode_is_the_native_mode() {
        // Numerators come in different scales: 60317/1000 Hz must not beat 60/1 Hz at a
        // larger resolution (both round to 60 Hz; the larger mode wins the tie).
        let modes = [sm(800, 600, 60_317, 1000), sm(1920, 1080, 60, 1)];
        assert_eq!(best_source_mode(&modes), Some(m(1920, 1080, 60)));
        // A faster legacy mode does not beat the native resolution.
        let modes = [sm(1920, 1080, 60, 1), sm(1280, 1024, 75_000, 1000)];
        assert_eq!(best_source_mode(&modes), Some(m(1920, 1080, 60)));
        let modes = [sm(1920, 1080, 60, 1), sm(1920, 1080, 143_998, 1000)];
        assert_eq!(best_source_mode(&modes), Some(m(1920, 1080, 144)));
    }

    #[test]
    fn best_source_mode_picks_highest_rate() {
        let modes = [
            sm(1920, 1080, 60_000, 1000),
            sm(2560, 1440, 143_998, 1000),
            sm(0, 0, 999_999, 1),
        ];
        assert_eq!(best_source_mode(&modes), Some(m(2560, 1440, 144)));
        assert_eq!(best_source_mode(&[sm(1920, 1080, 60, 0)]), None);
        assert_eq!(best_source_mode(&[]), None);
    }

    #[test]
    fn resolutions_and_rates_sorted_like_the_script() {
        let modes = [
            m(1920, 1080, 60),
            m(1280, 720, 60),
            m(1920, 1080, 144),
            m(1920, 1200, 60),
            m(3840, 2160, 30),
            m(1920, 1080, 60),
        ];
        assert_eq!(
            resolutions(&modes),
            [(3840, 2160), (1920, 1200), (1920, 1080), (1280, 720)]
        );
        assert_eq!(refresh_rates(&modes, 1920, 1080), [144, 60]);
        assert!(refresh_rates(&modes, 800, 600).is_empty());
    }

    #[test]
    fn resolution_parsing() {
        assert_eq!(parse_resolution("1920x1080").unwrap(), (1920, 1080));
        assert_eq!(parse_resolution(" 1280X720 ").unwrap(), (1280, 720));
        assert_eq!(parse_resolution("3840×2160").unwrap(), (3840, 2160));
        for bad in ["", "1920", "1920x", "x1080", "0x1080", "axb", "-1x5"] {
            assert_eq!(parse_resolution(bad).unwrap_err().exit_code(), 2, "{bad}");
        }
    }

    #[test]
    fn picking_a_mode() {
        let modes = [m(1920, 1080, 60), m(1920, 1080, 144), m(1280, 720, 60)];
        assert_eq!(
            pick_mode(&modes, 1920, 1080, None).unwrap(),
            m(1920, 1080, 144)
        );
        assert_eq!(
            pick_mode(&modes, 1920, 1080, Some(60)).unwrap(),
            m(1920, 1080, 60)
        );
        let err = pick_mode(&modes, 1920, 1080, Some(75)).unwrap_err();
        assert!(err.to_string().contains("144, 60"), "{err}");
        let err = pick_mode(&modes, 800, 600, None).unwrap_err();
        assert!(err.to_string().contains("1920x1080, 1280x720"), "{err}");
        assert_eq!(err.exit_code(), 2);
    }
}
