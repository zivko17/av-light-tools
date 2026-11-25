//! CTA-861 extension block (tag 0x02): `ConvertFrom-EDIDCtaBlock`, `EDIDTools.ps1` lines 336-531.

use serde::Serialize;

use super::timing::{DetailedTiming, parse_dtd, round_dp, vic_name};

/// CTA-861 audio format codes (`$script:EDIDAudioFormat`).
fn audio_format(code: u8) -> Option<&'static str> {
    Some(match code {
        1 => "LPCM",
        2 => "AC-3",
        3 => "MPEG-1",
        4 => "MP3",
        5 => "MPEG-2",
        6 => "AAC LC",
        7 => "DTS",
        8 => "ATRAC",
        9 => "DSD",
        10 => "E-AC-3",
        11 => "DTS-HD",
        12 => "MLP/TrueHD",
        13 => "DST",
        14 => "WMA Pro",
        15 => "Extended",
        _ => return None,
    })
}

const SAMPLE_RATES_KHZ: [&str; 7] = ["32", "44.1", "48", "88.2", "96", "176.4", "192"];

/// A decoded CTA-861 extension block.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CtaBlock {
    /// Block number within the EDID (1 = first extension).
    pub index: usize,
    pub revision: u8,
    pub underscan_it: bool,
    pub basic_audio: bool,
    pub ycbcr444: bool,
    pub ycbcr422: bool,
    pub native_dtds: u8,
    /// Short video descriptors.
    pub video: Vec<Svd>,
    /// VICs supported in YCbCr 4:2:0 only.
    pub video420: Vec<Svd>,
    /// Short audio descriptors.
    pub audio: Vec<AudioDescriptor>,
    pub speakers: Vec<String>,
    pub colorimetry: Vec<String>,
    pub hdr_eotf: Vec<String>,
    /// cd/m².
    pub hdr_max_lum: Option<f64>,
    pub hdr_avg_lum: Option<f64>,
    pub hdr_min_lum: Option<f64>,
    pub hdmi: Option<HdmiVsdb>,
    pub hdmi_forum: Option<HdmiForumBlock>,
    pub quant_rgb: bool,
    pub quant_ycc: bool,
    pub timings: Vec<DetailedTiming>,
    pub unknown_blocks: Vec<String>,
    /// Data blocks that could not be decoded.
    pub problems: Vec<String>,
}

/// Short video descriptor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Svd {
    pub vic: u8,
    pub native: bool,
    /// From the VIC table, if listed.
    pub name: Option<String>,
}

impl Svd {
    fn new(vic: u8, native: bool) -> Svd {
        Svd {
            vic,
            native,
            name: vic_name(vic).map(str::to_string),
        }
    }
}

/// Short audio descriptor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AudioDescriptor {
    pub format_code: u8,
    /// `LPCM`, `AC-3`, ... or `format n`.
    pub format: String,
    pub channels: u8,
    pub rates_khz: Vec<String>,
    /// LPCM only.
    pub bit_depths: Vec<u8>,
    /// Compressed formats only.
    pub max_kbps: Option<u32>,
    /// One-line summary as `edid-info` prints it.
    pub summary: String,
}

/// HDMI 1.x vendor-specific data block (OUI 00-0C-03).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HdmiVsdb {
    pub physical_address: Option<String>,
    pub max_tmds_mhz: Option<u32>,
    pub deep_colour: Vec<String>,
    pub dvi_dual: bool,
    pub supports_ai: bool,
}

/// HDMI Forum vendor-specific data block (OUI C4-5D-D8, HDMI 2.x).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HdmiForumBlock {
    pub version: Option<u8>,
    pub max_tmds_char_rate_mhz: Option<u32>,
    pub scdc: bool,
    pub max_frl_rate: Option<u8>,
}

impl CtaBlock {
    /// Max TMDS rate declared: HDMI Forum first, then HDMI 1.x.
    pub fn max_tmds_mhz(&self) -> Option<u32> {
        self.hdmi_forum
            .as_ref()
            .and_then(|h| h.max_tmds_char_rate_mhz)
            .or_else(|| self.hdmi.as_ref().and_then(|h| h.max_tmds_mhz))
    }
}

/// Decodes a CTA-861 block. `index` is its block number in the EDID. Never panics.
pub fn decode_cta(index: usize, b: &[u8; 128]) -> CtaBlock {
    let d = usize::from(b[2]);
    let f = b[3];
    let mut res = CtaBlock {
        index,
        revision: b[1],
        underscan_it: f & 0x80 != 0,
        basic_audio: f & 0x40 != 0,
        ycbcr444: f & 0x20 != 0,
        ycbcr422: f & 0x10 != 0,
        native_dtds: f & 0x0F,
        video: Vec::new(),
        video420: Vec::new(),
        audio: Vec::new(),
        speakers: Vec::new(),
        colorimetry: Vec::new(),
        hdr_eotf: Vec::new(),
        hdr_max_lum: None,
        hdr_avg_lum: None,
        hdr_min_lum: None,
        hdmi: None,
        hdmi_forum: None,
        quant_rgb: false,
        quant_ycc: false,
        timings: Vec::new(),
        unknown_blocks: Vec::new(),
        problems: Vec::new(),
    };

    // --- data block collection (bytes 4 .. d-1) ---
    if d > 4 {
        let mut i = 4;
        while i < d && i < 127 {
            let tag = (b[i] >> 5) & 0x07;
            let len = usize::from(b[i] & 0x1F);
            if len == 0 {
                i += 1;
                continue;
            }
            let end = i + len;
            if end > 126 {
                res.problems.push(format!(
                    "CTA block {index}: data block at offset {i} ({len} bytes) runs past the end of the block"
                ));
                break;
            }
            let p = &b[i + 1..=end]; // payload, without the header byte
            match tag {
                1 => audio_block(&mut res, p),
                2 => res.video.extend(p.iter().map(|&v| svd(v))),
                3 => vendor_block(&mut res, p),
                4 => res.speakers.extend(bits(p[0], &SPEAKERS)),
                7 => extended_block(&mut res, p),
                _ => res
                    .unknown_blocks
                    .push(format!("Data block tag {tag} ({} bytes)", p.len())),
            }
            i = end + 1;
        }
    }

    // --- detailed timings (from byte d to 126) ---
    if d >= 4 {
        let mut t = d;
        while t + 17 <= 126 {
            if b[t] == 0 && b[t + 1] == 0 {
                break;
            }
            if let Some(dtd) = b
                .get(t..t + 18)
                .and_then(|s| <&[u8; 18]>::try_from(s).ok())
                .and_then(parse_dtd)
            {
                res.timings.push(dtd);
            }
            t += 18;
        }
    }
    res
}

/// SVD: 129..=192 is a native VIC (bit 7 set, VIC - 128); everything else as-is.
fn svd(v: u8) -> Svd {
    if (129..=192).contains(&v) {
        Svd::new(v - 128, true)
    } else {
        Svd::new(v, false)
    }
}

/// Names of the set bits, in bit order 0..7.
fn bits(value: u8, names: &[&'static str; 8]) -> Vec<String> {
    (0..8)
        .filter(|bit| value & (1 << bit) != 0)
        .map(|bit| names[bit].to_string())
        .collect()
}

const SPEAKERS: [&str; 8] = [
    "FL/FR", "LFE", "FC", "RL/RR", "RC", "FLC/FRC", "RLC/RRC", "FLW/FRW",
];

const COLORIMETRY: [&str; 8] = [
    "xvYCC601",
    "xvYCC709",
    "sYCC601",
    "opYCC601",
    "opRGB",
    "BT.2020 cYCC",
    "BT.2020 YCC",
    "BT.2020 RGB",
];

/// Audio Data Block: 3-byte short audio descriptors.
fn audio_block(res: &mut CtaBlock, p: &[u8]) {
    for sad in p.as_chunks::<3>().0 {
        let [b0, b1, b2] = *sad;
        let format_code = (b0 >> 3) & 0x0F;
        let channels = (b0 & 0x07) + 1;
        let format = audio_format(format_code)
            .map(str::to_string)
            .unwrap_or_else(|| format!("format {format_code}"));
        let rates_khz: Vec<String> = SAMPLE_RATES_KHZ
            .iter()
            .enumerate()
            .filter(|(r, _)| b1 & (1 << r) != 0)
            .map(|(_, k)| (*k).to_string())
            .collect();
        let mut bit_depths = Vec::new();
        let mut max_kbps = None;
        let mut extra = String::new();
        if format_code == 1 {
            for (mask, depth) in [(0x01, 16), (0x02, 20), (0x04, 24)] {
                if b2 & mask != 0 {
                    bit_depths.push(depth);
                }
            }
            if !bit_depths.is_empty() {
                let list: Vec<String> = bit_depths.iter().map(u8::to_string).collect();
                extra = format!("  {} bit", list.join("/"));
            }
        } else if b2 > 0 {
            let kbps = u32::from(b2) * 8;
            max_kbps = Some(kbps);
            extra = format!("  max {kbps} kbit/s");
        }
        let summary = format!(
            "{format:<10} {channels} ch   {} kHz{extra}",
            rates_khz.join("/")
        );
        res.audio.push(AudioDescriptor {
            format_code,
            format,
            channels,
            rates_khz,
            bit_depths,
            max_kbps,
            summary,
        });
    }
}

/// Vendor-Specific Data Block: HDMI 1.x, HDMI Forum, or listed as unknown.
fn vendor_block(res: &mut CtaBlock, p: &[u8]) {
    if p.len() < 3 {
        return;
    }
    let oui = format!("{:02X}-{:02X}-{:02X}", p[2], p[1], p[0]);
    let at = |k: usize| p.get(k).copied();
    match oui.as_str() {
        "00-0C-03" => {
            let mut h = HdmiVsdb {
                physical_address: None,
                max_tmds_mhz: None,
                deep_colour: Vec::new(),
                dvi_dual: false,
                supports_ai: false,
            };
            if let (Some(a), Some(b)) = (at(3), at(4)) {
                h.physical_address = Some(format!(
                    "{}.{}.{}.{}",
                    (a >> 4) & 0x0F,
                    a & 0x0F,
                    (b >> 4) & 0x0F,
                    b & 0x0F
                ));
            }
            if let Some(x) = at(5) {
                h.supports_ai = x & 0x80 != 0;
                for (mask, name) in [
                    (0x40, "48-bit"),
                    (0x20, "36-bit"),
                    (0x10, "30-bit"),
                    (0x08, "YCbCr 4:4:4"),
                ] {
                    if x & mask != 0 {
                        h.deep_colour.push(name.into());
                    }
                }
                h.dvi_dual = x & 0x01 != 0;
            }
            h.max_tmds_mhz = at(6).filter(|&v| v > 0).map(|v| u32::from(v) * 5);
            res.hdmi = Some(h);
        }
        "C4-5D-D8" => {
            res.hdmi_forum = Some(HdmiForumBlock {
                version: at(3),
                max_tmds_char_rate_mhz: at(4).filter(|&v| v > 0).map(|v| u32::from(v) * 5),
                scdc: at(5).is_some_and(|v| (v >> 7) & 0x01 != 0),
                max_frl_rate: at(7).map(|v| (v >> 4) & 0x0F),
            });
        }
        _ => res
            .unknown_blocks
            .push(format!("Vendor block OUI {oui} ({} bytes)", p.len())),
    }
}

/// `50 * 2^(v/32)` cd/m², rounded to an integer (CTA-861 desired luminance coding).
fn luminance(v: u8) -> f64 {
    round_dp(50.0 * 2f64.powf(f64::from(v) / 32.0), 0)
}

/// Use Extended Tag data blocks.
fn extended_block(res: &mut CtaBlock, p: &[u8]) {
    let ext = p[0];
    let at = |k: usize| p.get(k).copied();
    match ext {
        0 => {
            // Video Capability Data Block
            if let Some(v) = at(1) {
                res.quant_ycc = (v >> 7) & 0x01 != 0;
                res.quant_rgb = (v >> 6) & 0x01 != 0;
            }
        }
        5 => {
            // Colorimetry Data Block
            if let Some(c) = at(1) {
                res.colorimetry.extend(bits(c, &COLORIMETRY));
            }
        }
        6 => {
            // HDR Static Metadata Data Block
            if let Some(e) = at(1) {
                for (mask, name) in [
                    (0x01, "SDR (traditional gamma)"),
                    (0x02, "HDR (traditional gamma)"),
                    (0x04, "PQ (SMPTE ST 2084)"),
                    (0x08, "HLG"),
                ] {
                    if e & mask != 0 {
                        res.hdr_eotf.push(name.into());
                    }
                }
            }
            // max = 50 * 2^(v/32);  min = max * (v/255)^2 / 100
            res.hdr_max_lum = at(3).filter(|&v| v > 0).map(luminance);
            res.hdr_avg_lum = at(4).filter(|&v| v > 0).map(luminance);
            if let (Some(v), Some(max)) = (at(5), res.hdr_max_lum) {
                res.hdr_min_lum = Some(round_dp(max * (f64::from(v) / 255.0).powi(2) / 100.0, 4));
            }
        }
        14 => {
            // YCbCr 4:2:0 Video Data Block (these VICs are 4:2:0 ONLY)
            res.video420
                .extend(p[1..].iter().map(|&v| Svd::new(v, false)));
        }
        _ => res
            .unknown_blocks
            .push(format!("Extended tag {ext} ({} bytes)", p.len())),
    }
}

#[cfg(test)]
mod tests {
    use super::super::testdata::{fix_checksum, synthetic_cta, synthetic_with_cta};
    use super::super::{check, decode};
    use super::*;

    fn cta() -> CtaBlock {
        decode_cta(1, &synthetic_cta())
    }

    /// CTA block whose data block collection is exactly `blocks`.
    fn cta_with(blocks: &[u8]) -> CtaBlock {
        let mut c = [0u8; 128];
        c[0] = 0x02;
        c[1] = 3;
        c[2] = 4 + blocks.len() as u8;
        c[4..4 + blocks.len()].copy_from_slice(blocks);
        fix_checksum(&mut c);
        decode_cta(1, &c)
    }

    #[test]
    fn header_flags() {
        let c = cta();
        assert_eq!(c.index, 1);
        assert_eq!(c.revision, 3);
        assert!(c.underscan_it && c.basic_audio && c.ycbcr444 && c.ycbcr422);
        assert_eq!(c.native_dtds, 1);
        assert!(c.problems.is_empty(), "{:?}", c.problems);
        assert!(c.unknown_blocks.is_empty(), "{:?}", c.unknown_blocks);
    }

    #[test]
    fn vic_16_lpcm_2ch_and_hdmi_vsdb_decoded() {
        let e = decode(&synthetic_with_cta());
        assert!(e.problems.is_empty(), "{:?}", e.problems);
        let c = e.first_cta().unwrap();
        // VIC 16, flagged native
        assert_eq!(c.video[0], Svd::new(16, true));
        assert_eq!(c.video[0].name.as_deref(), Some("1920x1080p60"));
        // LPCM 2 ch
        let a = &c.audio[0];
        assert_eq!((a.format.as_str(), a.channels), ("LPCM", 2));
        // HDMI vendor block
        let h = c.hdmi.as_ref().unwrap();
        assert_eq!(h.physical_address.as_deref(), Some("1.0.0.0"));
        assert!(
            check(&e)
                .iter()
                .all(|f| f.severity != super::super::Severity::Error)
        );
    }

    #[test]
    fn video_and_420() {
        let c = cta();
        assert_eq!(
            c.video,
            [Svd::new(16, true), Svd::new(97, false), Svd::new(4, false)]
        );
        assert_eq!(c.video420, [Svd::new(97, false)]);
    }

    #[test]
    fn audio_descriptor_fields() {
        let a = &cta().audio[0];
        assert_eq!(a.format_code, 1);
        assert_eq!(a.rates_khz, ["32", "44.1", "48"]);
        assert_eq!(a.bit_depths, [16, 20, 24]);
        assert_eq!(a.max_kbps, None);
        assert_eq!(a.summary, "LPCM       2 ch   32/44.1/48 kHz  16/20/24 bit");
    }

    #[test]
    fn compressed_audio_and_unknown_format() {
        // AC-3 6 ch 48 kHz max 640 kbit/s; format 0 2 ch
        let c = cta_with(&[0x26, 0x15, 0x04, 0x50, 0x01, 0x02, 0x00]);
        assert_eq!(
            c.audio[0].summary,
            "AC-3       6 ch   48 kHz  max 640 kbit/s"
        );
        assert_eq!(c.audio[0].max_kbps, Some(640));
        assert_eq!(c.audio[1].format, "format 0");
        assert_eq!(c.audio[1].summary, "format 0   2 ch   44.1 kHz");
    }

    #[test]
    fn hdmi_vsdb_fields() {
        let h = cta().hdmi.unwrap();
        assert_eq!(h.max_tmds_mhz, Some(300));
        assert_eq!(h.deep_colour, ["36-bit", "30-bit", "YCbCr 4:4:4"]);
        assert!(h.supports_ai);
        assert!(!h.dvi_dual);
    }

    #[test]
    fn hdmi_forum_fields() {
        let c = cta();
        let hf = c.hdmi_forum.as_ref().unwrap();
        assert_eq!(hf.version, Some(1));
        assert_eq!(hf.max_tmds_char_rate_mhz, Some(600));
        assert!(hf.scdc);
        assert_eq!(hf.max_frl_rate, None);
        assert_eq!(c.max_tmds_mhz(), Some(600));
        let frl = cta_with(&[0x68, 0xD8, 0x5D, 0xC4, 0x01, 0x00, 0x00, 0x00, 0x30]);
        let hf = frl.hdmi_forum.unwrap();
        assert_eq!(hf.max_frl_rate, Some(3));
        assert_eq!(hf.max_tmds_char_rate_mhz, None);
        assert!(!hf.scdc);
    }

    #[test]
    fn unknown_vendor_block() {
        let c = cta_with(&[0x63, 0x1A, 0x00, 0x00]);
        assert_eq!(c.unknown_blocks, ["Vendor block OUI 00-00-1A (3 bytes)"]);
        assert_eq!(c.max_tmds_mhz(), None);
    }

    #[test]
    fn speakers_colorimetry_hdr_quant() {
        let c = cta();
        assert_eq!(c.speakers, ["FL/FR"]);
        assert_eq!(c.colorimetry, ["BT.2020 YCC", "BT.2020 RGB"]);
        assert_eq!(
            c.hdr_eotf,
            ["SDR (traditional gamma)", "PQ (SMPTE ST 2084)", "HLG"]
        );
        assert_eq!(c.hdr_max_lum, Some(800.0));
        assert_eq!(c.hdr_avg_lum, Some(400.0));
        assert_eq!(c.hdr_min_lum, Some(0.5039)); // 800 * (64/255)^2 / 100
        assert!(c.quant_rgb && c.quant_ycc);
    }

    #[test]
    fn all_speakers_and_colorimetry_bits() {
        let c = cta_with(&[0x81, 0xFF, 0xE2, 0x05, 0xFF]);
        assert_eq!(
            c.speakers,
            [
                "FL/FR", "LFE", "FC", "RL/RR", "RC", "FLC/FRC", "RLC/RRC", "FLW/FRW"
            ]
        );
        assert_eq!(
            c.colorimetry,
            [
                "xvYCC601",
                "xvYCC709",
                "sYCC601",
                "opYCC601",
                "opRGB",
                "BT.2020 cYCC",
                "BT.2020 YCC",
                "BT.2020 RGB"
            ]
        );
    }

    #[test]
    fn unknown_tags_are_listed() {
        let c = cta_with(&[0xA1, 0x00, 0xE2, 0x0D, 0x00]);
        assert_eq!(
            c.unknown_blocks,
            ["Data block tag 5 (1 bytes)", "Extended tag 13 (2 bytes)"]
        );
    }

    #[test]
    fn detailed_timing_in_cta() {
        let c = cta();
        assert_eq!(c.timings.len(), 1);
        assert_eq!(c.timings[0].summary, "1920x1080p @ 60 Hz");
    }

    #[test]
    fn data_block_overrunning_the_block_is_a_problem() {
        let mut c = [0u8; 128];
        c[0] = 0x02;
        c[1] = 3;
        c[2] = 126;
        c[120] = 0x5F; // tag 2, 31 bytes: runs past byte 126
        let d = decode_cta(1, &c);
        assert_eq!(d.problems.len(), 1, "{:?}", d.problems);
        assert!(d.problems[0].contains("offset 120"), "{:?}", d.problems);
    }

    #[test]
    fn arbitrary_bytes_never_panic() {
        for seed in 0u32..512 {
            let mut c = [0u8; 128];
            let mut x = seed.wrapping_mul(2_654_435_761).wrapping_add(1);
            for byte in c.iter_mut() {
                x ^= x << 13;
                x ^= x >> 17;
                x ^= x << 5;
                *byte = x as u8;
            }
            c[0] = 0x02;
            let _ = decode_cta(1, &c);
        }
    }
}
