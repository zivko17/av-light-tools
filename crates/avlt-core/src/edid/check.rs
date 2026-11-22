//! EDID validation: `Test-EDIDIntegrity`, `EDIDTools.ps1` lines 1025-1100.

use serde::Serialize;

use super::Edid;

/// How serious a finding is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Severity {
    Ok,
    Warn,
    Error,
}

/// One result of [`check`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Finding {
    pub severity: Severity,
    pub message: String,
}

/// Validates an EDID against the current year. Errors come first, then warnings;
/// a clean EDID gives a single `Ok` finding.
pub fn check(edid: &Edid) -> Vec<Finding> {
    check_at(edid, current_year())
}

/// [`check`] with an explicit "current year" for the manufacture-year plausibility test.
fn check_at(edid: &Edid, current_year: i64) -> Vec<Finding> {
    // Structural problems found while decoding (header, extension count, truncation,
    // overrunning data blocks) are errors.
    let mut issues: Vec<String> = edid.problems.clone();
    let mut warns: Vec<String> = Vec::new();

    for chk in edid.checksums.iter().filter(|c| !c.valid) {
        issues.push(format!("Block {} checksum is wrong", chk.block));
    }

    if let Some(b) = &edid.base {
        if b.version != "1.3" && b.version != "1.4" {
            warns.push(format!("Unusual EDID version {}", b.version));
        }
        let year = i64::from(b.year);
        if year < 1990 || year > current_year + 1 {
            warns.push(format!("Implausible manufacture year {}", b.year));
        }
        if b.timings.is_empty() {
            issues.push(
                "No detailed timing in the base block (no native resolution declared)".into(),
            );
        }
        if b.monitor_name.is_none() {
            warns.push("No monitor name descriptor (0xFC)".into());
        }
        if b.width_cm == 0 {
            warns.push("Physical size not reported (scaling/DPI may be wrong)".into());
        }

        // Preferred timing vs declared limits - the classic "why no 4K60" case
        if let Some(pref) = &b.native {
            if let Some(r) = &b.range {
                if r.max_clock_mhz > 0 && pref.pixel_clock_mhz > f64::from(r.max_clock_mhz) {
                    issues.push(format!(
                        "Preferred timing needs {} MHz but range limits cap at {} MHz",
                        pref.pixel_clock_mhz, r.max_clock_mhz
                    ));
                }
                if pref.refresh < f64::from(r.v_min) || pref.refresh > f64::from(r.v_max) {
                    warns.push(format!(
                        "Preferred refresh {} Hz is outside the declared {}-{} Hz range",
                        pref.refresh, r.v_min, r.v_max
                    ));
                }
            }
            if let Some(max_rate) = edid.first_cta().and_then(|c| c.max_tmds_mhz()) {
                if pref.pixel_clock_mhz > f64::from(max_rate) {
                    warns.push(format!(
                        "Preferred timing is {} MHz but the HDMI block declares max {max_rate} MHz - \
                         needs 4:2:0, or the sink expects DisplayPort/FRL",
                        pref.pixel_clock_mhz
                    ));
                }
                // 600 MHz is the HDMI 2.0 TMDS ceiling; anything above it in a
                // TMDS field is a lying dongle, a splitter or a corrupted block.
                if max_rate > 600 {
                    warns.push(format!(
                        "HDMI block declares {max_rate} MHz max TMDS, above the 600 MHz spec limit - suspect EDID"
                    ));
                }
            }
        }
    }

    if issues.is_empty() && warns.is_empty() {
        return vec![Finding {
            severity: Severity::Ok,
            message: "No problems found.".into(),
        }];
    }
    let finding = |severity| move |message| Finding { severity, message };
    issues
        .into_iter()
        .map(finding(Severity::Error))
        .chain(warns.into_iter().map(finding(Severity::Warn)))
        .collect()
}

/// The current calendar year (UTC), from the system clock.
fn current_year() -> i64 {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    year_from_unix_days(i64::try_from(secs / 86_400).unwrap_or(0))
}

/// Gregorian year of a day count since 1970-01-01 (H. Hinnant's `civil_from_days`).
fn year_from_unix_days(days: i64) -> i64 {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    yoe + era * 400 + i64::from(month <= 2)
}

#[cfg(test)]
mod tests {
    use super::super::decode;
    use super::super::testdata::*;
    use super::*;

    fn messages(f: &[Finding], sev: Severity) -> Vec<String> {
        f.iter()
            .filter(|x| x.severity == sev)
            .map(|x| x.message.clone())
            .collect()
    }

    #[test]
    fn clean_edid_is_ok() {
        let f = check(&decode(&synthetic_base()));
        assert_eq!(
            f,
            [Finding {
                severity: Severity::Ok,
                message: "No problems found.".into()
            }]
        );
        let f = check(&decode(&synthetic_with_cta()));
        assert_eq!(f.len(), 1, "{f:?}");
        assert_eq!(f[0].severity, Severity::Ok);
    }

    #[test]
    fn bad_checksum_is_an_error() {
        let mut sum = synthetic_base();
        sum[127] = sum[127].wrapping_add(1);
        let f = check(&decode(&sum));
        assert!(
            f.iter()
                .any(|f| f.severity == Severity::Error && f.message.contains("checksum"))
        );
        assert_eq!(messages(&f, Severity::Error), ["Block 0 checksum is wrong"]);
    }

    #[test]
    fn structural_problems_are_errors() {
        let mut bad = synthetic_base();
        bad[0] = 0x12;
        fix_checksum(&mut bad);
        assert_eq!(
            messages(&check(&decode(&bad)), Severity::Error),
            ["Invalid 00 FF FF FF FF FF FF 00 header"]
        );

        let mut ext = synthetic_base();
        ext[126] = 2;
        fix_checksum(&mut ext);
        assert_eq!(
            messages(&check(&decode(&ext)), Severity::Error),
            ["Byte 126 says 2 extension block(s) but 0 are present"]
        );

        for short in [&[][..], &synthetic_base()[..100]] {
            let f = check(&decode(short));
            assert!(!f.is_empty());
            assert!(f.iter().all(|f| f.severity == Severity::Error), "{f:?}");
        }
    }

    #[test]
    fn missing_dtd_name_and_size() {
        let mut raw = synthetic_base();
        raw[54..72].fill(0); // DTD -> dummy descriptor
        raw[93] = 0x10; // name -> dummy
        raw[21] = 0;
        fix_checksum(&mut raw);
        let f = check(&decode(&raw));
        assert_eq!(
            messages(&f, Severity::Error),
            ["No detailed timing in the base block (no native resolution declared)"]
        );
        assert_eq!(
            messages(&f, Severity::Warn),
            [
                "No monitor name descriptor (0xFC)",
                "Physical size not reported (scaling/DPI may be wrong)"
            ]
        );
    }

    #[test]
    fn version_and_year() {
        let mut raw = synthetic_base();
        raw[19] = 2;
        raw[17] = 200; // 2190
        fix_checksum(&mut raw);
        let f = check_at(&decode(&raw), 2026);
        assert_eq!(
            messages(&f, Severity::Warn),
            [
                "Unusual EDID version 1.2",
                "Implausible manufacture year 2190"
            ]
        );
        raw[17] = 37; // 2027 = current + 1 is fine
        fix_checksum(&mut raw);
        assert!(
            messages(&check_at(&decode(&raw), 2026), Severity::Warn)
                .iter()
                .all(|m| !m.contains("year"))
        );
    }

    #[test]
    fn preferred_timing_vs_range_limits() {
        let mut raw = synthetic_base();
        raw[81] = 10; // max clock 100 MHz < 148.5
        raw[77] = 61; // V range 61-76 Hz excludes 60
        fix_checksum(&mut raw);
        let f = check(&decode(&raw));
        assert_eq!(
            messages(&f, Severity::Error),
            ["Preferred timing needs 148.5 MHz but range limits cap at 100 MHz"]
        );
        assert_eq!(
            messages(&f, Severity::Warn),
            ["Preferred refresh 60 Hz is outside the declared 61-76 Hz range"]
        );
    }

    #[test]
    fn hdmi_tmds_limits() {
        // Max TMDS 100 MHz (HDMI 1.x only) is below the 148.5 MHz preferred timing.
        let mut v = synthetic_with_cta();
        let c = &mut v[128..];
        // Turn the HF-VSDB OUI into an unknown one so the HDMI 1.x value is used.
        let hf = c.windows(3).position(|w| w == [0xD8, 0x5D, 0xC4]).unwrap();
        c[hf] = 0x00;
        let vs = c.windows(3).position(|w| w == [0x03, 0x0C, 0x00]).unwrap();
        c[vs + 6] = 20; // 100 MHz
        fix_checksum(c);
        let f = check(&decode(&v));
        assert_eq!(
            messages(&f, Severity::Warn),
            [
                "Preferred timing is 148.5 MHz but the HDMI block declares max 100 MHz - \
                 needs 4:2:0, or the sink expects DisplayPort/FRL"
            ]
        );

        // HF-VSDB claiming 1000 MHz TMDS is above the 600 MHz HDMI 2.0 ceiling.
        let mut v = synthetic_with_cta();
        let c = &mut v[128..];
        let hf = c.windows(3).position(|w| w == [0xD8, 0x5D, 0xC4]).unwrap();
        c[hf + 4] = 200;
        fix_checksum(c);
        let f = check(&decode(&v));
        assert_eq!(
            messages(&f, Severity::Warn),
            ["HDMI block declares 1000 MHz max TMDS, above the 600 MHz spec limit - suspect EDID"]
        );
    }

    #[test]
    fn cta_overrun_is_an_error() {
        let mut v = synthetic_with_cta();
        let c = &mut v[128..];
        c[2] = 126;
        c[49..127].fill(0);
        c[120] = 0x5F;
        fix_checksum(c);
        let f = check(&decode(&v));
        assert!(
            messages(&f, Severity::Error)
                .iter()
                .any(|m| m.contains("runs past the end")),
            "{f:?}"
        );
    }

    #[test]
    fn civil_year_from_days() {
        assert_eq!(year_from_unix_days(0), 1970);
        assert_eq!(year_from_unix_days(365), 1971);
        assert_eq!(year_from_unix_days(20_818), 2026); // 2026-12-31
        assert_eq!(year_from_unix_days(20_819), 2027); // 2027-01-01
        assert_eq!(year_from_unix_days(20_454), 2026); // 2026-01-01
        assert_eq!(year_from_unix_days(20_453), 2025); // 2025-12-31
    }
}
