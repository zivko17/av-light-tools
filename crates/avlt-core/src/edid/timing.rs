//! Shared EDID pieces: detailed timing descriptors, PnP IDs, descriptor text and the
//! CTA-861 VIC / established-timing tables (`EDIDTools.ps1` lines 29-49, 58-69, 98-172).

use serde::Serialize;

/// An 18-byte Detailed Timing Descriptor (`ConvertFrom-EDIDDetailedTiming`).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DetailedTiming {
    pub pixel_clock_mhz: f64,
    pub h_active: u32,
    pub h_blank: u32,
    pub h_front_porch: u32,
    pub h_sync_width: u32,
    /// `+`, `-`, or `?` when the sync is not digital separate.
    pub h_polarity: String,
    pub v_active: u32,
    pub v_blank: u32,
    pub v_front_porch: u32,
    pub v_sync_width: u32,
    pub v_polarity: String,
    pub interlaced: bool,
    pub refresh: f64,
    pub width_mm: u32,
    pub height_mm: u32,
    /// e.g. `1920x1080p @ 60 Hz`.
    pub summary: String,
}

/// Decodes an 18-byte descriptor as a timing; `None` if the pixel clock is 0 (not a DTD).
pub fn parse_dtd(d: &[u8; 18]) -> Option<DetailedTiming> {
    let b = |i: usize| u32::from(d[i]);
    let pclk = f64::from((b(1) << 8) | b(0)) * 10_000.0; // Hz
    if pclk <= 0.0 {
        return None;
    }

    let h_act = ((b(4) >> 4) << 8) | b(2);
    let h_bl = ((b(4) & 0x0F) << 8) | b(3);
    let v_act = ((b(7) >> 4) << 8) | b(5);
    let v_bl = ((b(7) & 0x0F) << 8) | b(6);

    let h_sync_off = (((b(11) >> 6) & 0x03) << 8) | b(8);
    let h_sync_w = (((b(11) >> 4) & 0x03) << 8) | b(9);
    let v_sync_off = (((b(11) >> 2) & 0x03) << 4) | ((b(10) >> 4) & 0x0F);
    let v_sync_w = ((b(11) & 0x03) << 4) | (b(10) & 0x0F);

    let h_size = ((b(14) >> 4) << 8) | b(12);
    let v_size = ((b(14) & 0x0F) << 8) | b(13);

    let flags = b(17);
    let interlaced = flags & 0x80 != 0;
    let dig_sep = (flags >> 3) & 0x03 == 0x03;
    let (mut h_pol, mut v_pol) = ("?", "?");
    if dig_sep {
        v_pol = if flags & 0x04 != 0 { "+" } else { "-" };
        h_pol = if flags & 0x02 != 0 { "+" } else { "-" };
    }

    let h_total = h_act + h_bl;
    let v_total = v_act + v_bl;
    let refresh = if h_total > 0 && v_total > 0 {
        pclk / (f64::from(h_total) * f64::from(v_total))
    } else {
        0.0
    };

    Some(DetailedTiming {
        pixel_clock_mhz: round_dp(pclk / 1_000_000.0, 3),
        h_active: h_act,
        h_blank: h_bl,
        h_front_porch: h_sync_off,
        h_sync_width: h_sync_w,
        h_polarity: h_pol.into(),
        v_active: v_act,
        v_blank: v_bl,
        v_front_porch: v_sync_off,
        v_sync_width: v_sync_w,
        v_polarity: v_pol.into(),
        interlaced,
        refresh: round_dp(refresh, 3),
        width_mm: h_size,
        height_mm: v_size,
        summary: format!(
            "{h_act}x{v_act}{} @ {} Hz",
            if interlaced { 'i' } else { 'p' },
            round_dp(refresh, 2)
        ),
    })
}

/// Three-letter PnP manufacturer ID from bytes 8-9 (`ConvertFrom-EDIDManufacturerId`).
pub fn manufacturer_id(b0: u8, b1: u8) -> String {
    let v = (u16::from(b0) << 8) | u16::from(b1);
    [(v >> 10) & 0x1F, (v >> 5) & 0x1F, v & 0x1F]
        .iter()
        .map(|&c| char::from(64 + (c as u8)))
        .collect()
}

/// Text of a monitor name / serial / free-text descriptor: bytes 5..=17, ended by 0x0A.
pub fn descriptor_text(d: &[u8; 18]) -> String {
    d[5..=17]
        .iter()
        .take_while(|&&c| c != 0x0A)
        .map(|&c| char::from(c))
        .collect::<String>()
        .trim()
        .to_string()
}

/// Rounds half to even at `digits` decimals, like PowerShell's `[math]::Round`.
pub fn round_dp(x: f64, digits: i32) -> f64 {
    let m = 10f64.powi(digits);
    (x * m).round_ties_even() / m
}

/// CTA-861 Video Identification Codes; unlisted codes print as `VIC n`.
const VICS: &[(u8, &str)] = &[
    (1, "640x480p60"),
    (2, "720x480p60 4:3"),
    (3, "720x480p60 16:9"),
    (4, "1280x720p60"),
    (5, "1920x1080i60"),
    (6, "720x480i60 4:3"),
    (7, "720x480i60 16:9"),
    (8, "720x240p60 4:3"),
    (9, "720x240p60 16:9"),
    (14, "1440x480p60 4:3"),
    (15, "1440x480p60 16:9"),
    (16, "1920x1080p60"),
    (17, "720x576p50 4:3"),
    (18, "720x576p50 16:9"),
    (19, "1280x720p50"),
    (20, "1920x1080i50"),
    (21, "720x576i50 4:3"),
    (22, "720x576i50 16:9"),
    (29, "1440x576p50 4:3"),
    (30, "1440x576p50 16:9"),
    (31, "1920x1080p50"),
    (32, "1920x1080p24"),
    (33, "1920x1080p25"),
    (34, "1920x1080p30"),
    (35, "2880x480p60 4:3"),
    (36, "2880x480p60 16:9"),
    (37, "2880x576p50 4:3"),
    (38, "2880x576p50 16:9"),
    (39, "1920x1080i50"),
    (40, "1920x1080i100"),
    (41, "1280x720p100"),
    (42, "720x576p100 4:3"),
    (43, "720x576p100 16:9"),
    (44, "720x576i100 4:3"),
    (45, "720x576i100 16:9"),
    (46, "1920x1080i120"),
    (47, "1280x720p120"),
    (48, "720x480p120 4:3"),
    (49, "720x480p120 16:9"),
    (50, "720x480i120 4:3"),
    (51, "720x480i120 16:9"),
    (52, "720x576p200 4:3"),
    (53, "720x576p200 16:9"),
    (54, "720x576i200 4:3"),
    (55, "720x576i200 16:9"),
    (56, "720x480p240 4:3"),
    (57, "720x480p240 16:9"),
    (58, "720x480i240 4:3"),
    (59, "720x480i240 16:9"),
    (60, "1280x720p24"),
    (61, "1280x720p25"),
    (62, "1280x720p30"),
    (63, "1920x1080p120"),
    (64, "1920x1080p100"),
    (93, "3840x2160p24"),
    (94, "3840x2160p25"),
    (95, "3840x2160p30"),
    (96, "3840x2160p50"),
    (97, "3840x2160p60"),
    (98, "4096x2160p24"),
    (99, "4096x2160p25"),
    (100, "4096x2160p30"),
    (101, "4096x2160p50"),
    (102, "4096x2160p60"),
    (103, "3840x2160p24"),
    (104, "3840x2160p25"),
    (105, "3840x2160p30"),
    (106, "3840x2160p50"),
    (107, "3840x2160p60"),
];

/// Name of a VIC from the table, if listed.
pub fn vic_name(vic: u8) -> Option<&'static str> {
    VICS.iter().find(|(v, _)| *v == vic).map(|(_, n)| *n)
}

/// `16 1920x1080p60`, or `VIC n` for unlisted codes (as `edid-diff` prints them).
pub fn vic_label(vic: u8) -> String {
    match vic_name(vic) {
        Some(n) => format!("{vic} {n}"),
        None => format!("VIC {vic}"),
    }
}

/// Established timings bitmaps: (byte offset from 35, bit, timing).
pub const ESTABLISHED: &[(usize, u8, &str)] = &[
    (0, 7, "720x400@70"),
    (0, 6, "720x400@88"),
    (0, 5, "640x480@60"),
    (0, 4, "640x480@67"),
    (0, 3, "640x480@72"),
    (0, 2, "640x480@75"),
    (0, 1, "800x600@56"),
    (0, 0, "800x600@60"),
    (1, 7, "800x600@72"),
    (1, 6, "800x600@75"),
    (1, 5, "832x624@75"),
    (1, 4, "1024x768@87i"),
    (1, 3, "1024x768@60"),
    (1, 2, "1024x768@70"),
    (1, 1, "1024x768@75"),
    (1, 0, "1280x1024@75"),
    (2, 7, "1152x870@75"),
];

#[cfg(test)]
mod tests {
    use super::*;

    use super::super::testdata::DTD_1080P60;

    #[test]
    fn dtd_1080p60_decodes() {
        let t = parse_dtd(&DTD_1080P60).unwrap();
        assert_eq!(t.pixel_clock_mhz, 148.5);
        assert_eq!(
            (t.h_active, t.h_blank, t.h_front_porch, t.h_sync_width),
            (1920, 280, 88, 44)
        );
        assert_eq!(
            (t.v_active, t.v_blank, t.v_front_porch, t.v_sync_width),
            (1080, 45, 4, 5)
        );
        assert_eq!((t.h_polarity.as_str(), t.v_polarity.as_str()), ("+", "+"));
        assert!(!t.interlaced);
        assert_eq!(t.refresh, 60.0);
        assert_eq!((t.width_mm, t.height_mm), (527, 296));
        assert_eq!(t.summary, "1920x1080p @ 60 Hz");
    }

    #[test]
    fn zero_pixel_clock_is_not_a_dtd() {
        let mut d = DTD_1080P60;
        d[0] = 0;
        d[1] = 0;
        assert!(parse_dtd(&d).is_none());
    }

    #[test]
    fn analog_sync_polarity_is_unknown_and_interlace_flag_read() {
        let mut d = DTD_1080P60;
        d[17] = 0x80; // interlaced, analog composite
        let t = parse_dtd(&d).unwrap();
        assert_eq!((t.h_polarity.as_str(), t.v_polarity.as_str()), ("?", "?"));
        assert!(t.interlaced);
        assert!(t.summary.starts_with("1920x1080i"), "{}", t.summary);
    }

    #[test]
    fn zero_totals_give_zero_refresh_without_panic() {
        let mut d = [0u8; 18];
        d[0] = 1;
        let t = parse_dtd(&d).unwrap();
        assert_eq!(t.refresh, 0.0);
        assert_eq!(t.summary, "0x0p @ 0 Hz");
    }

    #[test]
    fn manufacturer_ids() {
        assert_eq!(manufacturer_id(0x10, 0xAC), "DEL");
        assert_eq!(manufacturer_id(0x4C, 0x2D), "SAM");
        assert_eq!(manufacturer_id(0x1E, 0x6D), "GSM");
        assert_eq!(manufacturer_id(0, 0), "@@@");
    }

    #[test]
    fn descriptor_text_stops_at_newline_and_trims() {
        let mut d = [0u8; 18];
        d[3] = 0xFC;
        d[5..18].copy_from_slice(b"DELL U2720Q\n ");
        assert_eq!(descriptor_text(&d), "DELL U2720Q");
        d[5..18].copy_from_slice(b"  ABCDEFGHIJK");
        assert_eq!(descriptor_text(&d), "ABCDEFGHIJK");
    }

    #[test]
    fn rounding_is_half_to_even() {
        assert_eq!(round_dp(2.5, 0), 2.0);
        assert_eq!(round_dp(3.5, 0), 4.0);
        assert_eq!(round_dp(59.9401, 2), 59.94);
    }

    #[test]
    fn vic_lookup() {
        assert_eq!(vic_name(16), Some("1920x1080p60"));
        assert_eq!(vic_name(10), None);
        assert_eq!(vic_label(97), "97 3840x2160p60");
        assert_eq!(vic_label(200), "VIC 200");
    }
}
