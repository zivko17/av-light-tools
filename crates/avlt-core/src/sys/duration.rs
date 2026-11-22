//! `util timer` durations (Timer.ps1): `5m`, `30s`, `1h30m`, `90s`; a bare number is minutes.

use std::time::Duration;

use crate::error::{Error, Result};

/// Parses a timer duration. Units `h`, `m`, `s` (each at most once, in that order);
/// a bare number means minutes (Timer.ps1). Zero, empty and unknown units are errors.
pub fn parse_duration(s: &str) -> Result<Duration> {
    let text = s.trim().to_ascii_lowercase();
    let bad = || {
        Error::InvalidInput(format!(
            "invalid duration '{}'. Examples: 5m, 30s, 1h30m, 90s",
            s.trim()
        ))
    };
    if text.is_empty() {
        return Err(bad());
    }
    if text.bytes().all(|b| b.is_ascii_digit()) {
        let minutes: u64 = text.parse().map_err(|_| bad())?;
        return positive(minutes.checked_mul(60).ok_or_else(bad)?).ok_or_else(bad);
    }
    let mut total: u64 = 0;
    let mut number = String::new();
    // Units must appear in this order, each at most once.
    let mut allowed: &[(char, u64)] = &[('h', 3600), ('m', 60), ('s', 1)];
    for c in text.chars() {
        if c.is_ascii_digit() {
            number.push(c);
            continue;
        }
        let pos = allowed.iter().position(|(u, _)| *u == c).ok_or_else(bad)?;
        if number.is_empty() {
            return Err(bad());
        }
        let n: u64 = number.parse().map_err(|_| bad())?;
        let secs = n.checked_mul(allowed[pos].1).ok_or_else(bad)?;
        total = total.checked_add(secs).ok_or_else(bad)?;
        number.clear();
        allowed = &allowed[pos + 1..];
    }
    if !number.is_empty() {
        return Err(bad());
    }
    positive(total).ok_or_else(bad)
}

fn positive(secs: u64) -> Option<Duration> {
    (secs > 0).then(|| Duration::from_secs(secs))
}

/// Countdown display of Timer.ps1: `HH:MM:SS` from one hour up, else `MM:SS`.
pub fn format_countdown(remaining: Duration) -> String {
    let mut secs = remaining.as_secs();
    if remaining.subsec_nanos() > 0 {
        secs += 1;
    }
    let (h, m, s) = (secs / 3600, secs % 3600 / 60, secs % 60);
    if h > 0 {
        format!("{h:02}:{m:02}:{s:02}")
    } else {
        format!("{m:02}:{s:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brief_examples() -> Result<()> {
        assert_eq!(parse_duration("5m")?, Duration::from_secs(300));
        assert_eq!(parse_duration("1h30m")?, Duration::from_secs(5400));
        assert_eq!(parse_duration("90s")?, Duration::from_secs(90));
        assert!(parse_duration("").is_err());
        assert!(parse_duration("5x").is_err());
        Ok(())
    }

    #[test]
    fn more_forms() -> Result<()> {
        assert_eq!(parse_duration("1h")?, Duration::from_secs(3600));
        assert_eq!(parse_duration("1h2m3s")?, Duration::from_secs(3723));
        assert_eq!(parse_duration("10")?, Duration::from_secs(600));
        assert_eq!(parse_duration(" 2M ")?, Duration::from_secs(120));
        assert_eq!(parse_duration("1s")?, Duration::from_secs(1));
        Ok(())
    }

    #[test]
    fn rejects_bad_input() {
        for bad in [
            "0",
            "0s",
            "0h0m",
            "m",
            "h5",
            "5m5m",
            "30s1m",
            "1.5m",
            "-5m",
            "5 m",
            "s5",
            "5mx",
            "99999999999999999999h",
            "999999999999999999m",
        ] {
            let err = parse_duration(bad).expect_err(bad);
            assert_eq!(err.exit_code(), 2, "{bad}: {err}");
            assert!(err.to_string().contains("5m"), "usage hint missing: {err}");
        }
    }

    #[test]
    fn countdown_display() {
        assert_eq!(format_countdown(Duration::from_secs(0)), "00:00");
        assert_eq!(format_countdown(Duration::from_secs(65)), "01:05");
        assert_eq!(format_countdown(Duration::from_secs(3599)), "59:59");
        assert_eq!(format_countdown(Duration::from_secs(3600)), "01:00:00");
        assert_eq!(format_countdown(Duration::from_secs(5400 + 7)), "01:30:07");
        // Sub-second remainders round up so the display never shows 00:00 early.
        assert_eq!(format_countdown(Duration::from_millis(400)), "00:01");
    }
}
