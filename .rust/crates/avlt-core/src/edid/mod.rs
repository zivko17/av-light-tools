//! EDID decoding (read-only). Port of `EDIDTools.ps1`.
//!
//! [`decode`] never fails: whatever cannot be decoded is listed in [`Edid::problems`].

pub mod base;
pub mod check;
pub mod cta;
pub mod diff;
pub mod report;
pub mod timing;

use serde::{Serialize, Serializer};

pub use base::{BaseBlock, RangeLimits};
pub use check::{Finding, Severity, check};
pub use cta::{AudioDescriptor, CtaBlock, HdmiForumBlock, HdmiVsdb, Svd};
pub use diff::{DiffKind, DiffLine, diff};
pub use report::{Comparison, FieldRow, caps, compare, dump_file_name};
pub use timing::DetailedTiming;

/// Size of one EDID block.
pub const BLOCK: usize = 128;

/// A decoded EDID blob (base block plus extensions).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Edid {
    /// The bytes as read, hex-encoded in JSON.
    #[serde(serialize_with = "hex_bytes")]
    pub raw: Vec<u8>,
    /// Number of complete 128-byte blocks present.
    pub block_count: usize,
    /// Base block; `None` if the blob is shorter than 128 bytes.
    pub base: Option<BaseBlock>,
    /// Decoded CTA-861 extension blocks.
    pub cta: Vec<CtaBlock>,
    /// Every extension block present, decoded or not.
    pub extensions: Vec<Extension>,
    /// Checksum of every complete block.
    pub checksums: Vec<BlockChecksum>,
    /// What could not be decoded or is structurally wrong.
    pub problems: Vec<String>,
}

/// One extension block (block index >= 1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Extension {
    pub index: usize,
    pub tag: u8,
    /// `CTA-861`, `DisplayID`, ... or `Unknown`.
    pub kind: String,
}

/// Checksum result of one 128-byte block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct BlockChecksum {
    pub block: usize,
    pub valid: bool,
}

impl Edid {
    /// The first CTA-861 extension, which is what the tools report on.
    pub fn first_cta(&self) -> Option<&CtaBlock> {
        self.cta.first()
    }
}

/// Name of an extension block tag.
fn extension_kind(tag: u8) -> &'static str {
    match tag {
        0x02 => "CTA-861",
        0x10 => "Video Timing Block",
        0x40 => "Display Information",
        0x50 => "Localised String",
        0x60 => "Microdisplay",
        0x70 => "DisplayID",
        0xF0 => "Block Map",
        _ => "Unknown",
    }
}

/// Decodes an EDID blob. Never fails and never panics.
pub fn decode(bytes: &[u8]) -> Edid {
    let mut edid = Edid {
        raw: bytes.to_vec(),
        block_count: bytes.len() / BLOCK,
        base: None,
        cta: Vec::new(),
        extensions: Vec::new(),
        checksums: Vec::new(),
        problems: Vec::new(),
    };
    let Some((first, _)) = bytes.split_first_chunk::<BLOCK>() else {
        edid.problems.push(format!(
            "EDID blob too short ({} bytes, need at least 128)",
            bytes.len()
        ));
        return edid;
    };
    let trailing = bytes.len() % BLOCK;
    if trailing != 0 {
        edid.problems.push(format!(
            "{trailing} trailing byte(s) after the last complete 128-byte block were ignored"
        ));
    }

    let blocks: Vec<&[u8; BLOCK]> = bytes.as_chunks::<BLOCK>().0.iter().collect();
    edid.checksums = blocks
        .iter()
        .enumerate()
        .map(|(block, b)| BlockChecksum {
            block,
            valid: b.iter().fold(0u8, |s, &x| s.wrapping_add(x)) == 0,
        })
        .collect();

    let base = base::decode_base(first);
    if !base.header_valid {
        edid.problems
            .push("Invalid 00 FF FF FF FF FF FF 00 header".into());
    }
    let present = edid.block_count - 1;
    if usize::from(base.ext_count) != present {
        edid.problems.push(format!(
            "Byte 126 says {} extension block(s) but {present} are present",
            base.ext_count
        ));
    }
    edid.base = Some(base);

    for (index, blk) in blocks.iter().enumerate().skip(1) {
        let tag = blk[0];
        edid.extensions.push(Extension {
            index,
            tag,
            kind: extension_kind(tag).into(),
        });
        if tag == 0x02 {
            let c = cta::decode_cta(index, blk);
            edid.problems.extend(c.problems.iter().cloned());
            edid.cta.push(c);
        }
    }
    edid
}

/// Serializes bytes as space-separated upper-case hex (`00 FF FF ...`).
fn hex_bytes<S: Serializer>(bytes: &[u8], s: S) -> std::result::Result<S::Ok, S::Error> {
    s.serialize_str(&to_hex(bytes))
}

/// Space-separated upper-case hex.
pub fn to_hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Synthetic EDIDs shared by the tests of every edid module.
#[cfg(test)]
pub(crate) mod testdata {
    /// 1920x1080@60, 148.5 MHz, 527x296 mm, digital separate +/+.
    pub const DTD_1080P60: [u8; 18] = [
        0x02, 0x3A, 0x80, 0x18, 0x71, 0x38, 0x2D, 0x40, 0x58, 0x2C, 0x45, 0x00, 0x0F, 0x28, 0x21,
        0x00, 0x00, 0x1E,
    ];

    /// Sets byte 127 so the block sums to 0 mod 256.
    pub fn fix_checksum(block: &mut [u8]) {
        let sum = block[..127].iter().fold(0u8, |s, &x| s.wrapping_add(x));
        block[127] = 0u8.wrapping_sub(sum);
    }

    /// Valid EDID 1.4 base block: DEL, 1920x1080@60 preferred, range limits, name.
    pub fn synthetic_base() -> [u8; 128] {
        let mut b = [0u8; 128];
        b[0..8].copy_from_slice(&[0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00]);
        b[8] = 0x10; // "DEL"
        b[9] = 0xAC;
        b[10..12].copy_from_slice(&0xA0B1u16.to_le_bytes()); // product
        b[12..16].copy_from_slice(&12345678u32.to_le_bytes()); // serial
        b[16] = 10; // week
        b[17] = 30; // 2020
        b[18] = 1; // EDID 1.4
        b[19] = 4;
        b[20] = 0xA5; // digital, 8 bpc, DisplayPort
        b[21] = 53; // cm
        b[22] = 30;
        b[23] = 120; // gamma 2.2
        b[24] = 0x0A; // RGB + YCrCb 4:4:4, preferred timing is native
        b[25..35].copy_from_slice(&[0xEE, 0x91, 0xA3, 0x54, 0x4C, 0x99, 0x26, 0x0F, 0x50, 0x54]);
        b[35] = 0x21; // 640x480@60, 800x600@60
        b[36] = 0x08; // 1024x768@60
        b[37] = 0x00;
        b[38] = 0xD1; // 1920x1080@60 (16:9)
        b[39] = 0xC0;
        for i in (40..54).step_by(2) {
            b[i] = 0x01;
            b[i + 1] = 0x01;
        }
        b[54..72].copy_from_slice(&DTD_1080P60);
        // Range limits: 56-76 Hz, 30-83 kHz, 170 MHz
        b[72..90].copy_from_slice(&[
            0, 0, 0, 0xFD, 0, 56, 76, 30, 83, 17, 0x01, 0x0A, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20,
        ]);
        // Monitor name
        b[90..95].copy_from_slice(&[0, 0, 0, 0xFC, 0]);
        b[95..108].copy_from_slice(b"DELL TEST\n   ");
        // Serial text
        b[108..113].copy_from_slice(&[0, 0, 0, 0xFF, 0]);
        b[113..126].copy_from_slice(b"ABC123\n      ");
        b[126] = 0; // no extensions
        fix_checksum(&mut b);
        b
    }

    /// `synthetic_base` with another manufacturer ID in bytes 8-9.
    pub fn with_mfg(b0: u8, b1: u8) -> [u8; 128] {
        let mut b = synthetic_base();
        b[8] = b0;
        b[9] = b1;
        fix_checksum(&mut b);
        b
    }

    /// CTA-861 rev 3 block: VIC 16 (native) + 97, LPCM 2 ch, speakers, HDMI VSDB, HDMI
    /// Forum, colorimetry, HDR static metadata, a 4:2:0 VDB and one DTD.
    pub fn synthetic_cta() -> [u8; 128] {
        let mut c = [0u8; 128];
        c[0] = 0x02;
        c[1] = 3;
        c[3] = 0xF1; // underscan, basic audio, 4:4:4, 4:2:2, 1 native DTD
        let blocks: &[&[u8]] = &[
            &[0x43, 0x90, 97, 4],                              // video: VIC 16 native, 97, 4
            &[0x23, 0x09, 0x07, 0x07], // audio: LPCM 2 ch 32/44.1/48 16/20/24
            &[0x83, 0x01, 0x00, 0x00], // speakers: FL/FR
            &[0x67, 0x03, 0x0C, 0x00, 0x10, 0x00, 0xB8, 0x3C], // HDMI VSDB 1.0.0.0, 300 MHz
            &[0x67, 0xD8, 0x5D, 0xC4, 0x01, 0x78, 0x80, 0x00], // HF-VSDB v1, 600 MHz, SCDC
            &[0xE3, 0x05, 0xC0, 0x00], // colorimetry: BT.2020 YCC + RGB
            &[0xE6, 0x06, 0x0D, 0x01, 0x80, 0x60, 0x40], // HDR: SDR, PQ, HLG; lum
            &[0xE2, 0x0E, 97],         // 4:2:0 only: VIC 97
            &[0xE2, 0x00, 0xC0],       // VCDB: QY, QS
        ];
        let mut i = 4;
        for blk in blocks {
            c[i..i + blk.len()].copy_from_slice(blk);
            i += blk.len();
        }
        c[2] = i as u8; // DTDs start here
        c[i..i + 18].copy_from_slice(&DTD_1080P60);
        fix_checksum(&mut c);
        c
    }

    /// Base block declaring one extension, followed by [`synthetic_cta`].
    pub fn synthetic_with_cta() -> Vec<u8> {
        let mut b = synthetic_base();
        b[126] = 1;
        fix_checksum(&mut b);
        let mut v = b.to_vec();
        v.extend_from_slice(&synthetic_cta());
        v
    }
}

#[cfg(test)]
mod tests {
    use super::testdata::*;
    use super::*;

    #[test]
    fn synthetic_base_decodes_cleanly() {
        let e = decode(&synthetic_base());
        assert!(e.problems.is_empty(), "{:?}", e.problems);
        assert_eq!(e.block_count, 1);
        let b = e.base.unwrap();
        assert_eq!(b.manufacturer, "DEL"); // bytes 8-9 = 0x10 0xAC
        let native = b.native.as_ref().unwrap();
        assert_eq!((native.h_active, native.v_active), (1920, 1080));
        assert!(
            check(&decode(&synthetic_base()))
                .iter()
                .all(|f| f.severity != Severity::Error)
        );
    }

    #[test]
    fn manufacturer_samsung() {
        // bytes 8-9 = 0x4C 0x2D
        assert_eq!(
            decode(&with_mfg(0x4C, 0x2D)).base.unwrap().manufacturer,
            "SAM"
        );
    }

    #[test]
    fn empty_and_truncated_never_panic() {
        assert!(!decode(&[]).problems.is_empty());
        let short = decode(&synthetic_base()[..100]);
        assert!(!short.problems.is_empty());
        assert!(
            short.problems[0].contains("100 bytes"),
            "{:?}",
            short.problems
        );
        assert!(short.base.is_none());
        for len in 0..300 {
            let mut v = synthetic_with_cta();
            v.resize(len, 0xFF);
            let _ = decode(&v);
        }
    }

    #[test]
    fn broken_header_is_a_problem() {
        let mut bad = synthetic_base();
        bad[0] = 0x12;
        let e = decode(&bad);
        assert!(!e.problems.is_empty());
        assert!(!e.base.unwrap().header_valid);
    }

    #[test]
    fn bad_checksum_is_recorded() {
        let mut sum = synthetic_base();
        sum[127] = sum[127].wrapping_add(1);
        let e = decode(&sum);
        assert_eq!(
            e.checksums,
            [BlockChecksum {
                block: 0,
                valid: false
            }]
        );
    }

    #[test]
    fn declared_extensions_missing_is_a_problem() {
        let mut ext = synthetic_base();
        ext[126] = 2;
        fix_checksum(&mut ext);
        let e = decode(&ext);
        assert!(!e.problems.is_empty());
        assert!(
            e.problems.iter().any(|p| p.contains("says 2 extension")),
            "{:?}",
            e.problems
        );
    }

    #[test]
    fn trailing_partial_block_is_a_problem() {
        let mut v = synthetic_base().to_vec();
        v.extend_from_slice(&[0; 10]);
        let e = decode(&v);
        assert_eq!(e.block_count, 1);
        assert!(
            e.problems.iter().any(|p| p.contains("10 trailing")),
            "{:?}",
            e.problems
        );
    }

    #[test]
    fn non_cta_extension_is_listed_not_decoded() {
        let mut b = synthetic_base();
        b[126] = 1;
        fix_checksum(&mut b);
        let mut v = b.to_vec();
        let mut ext = [0u8; 128];
        ext[0] = 0x70;
        fix_checksum(&mut ext);
        v.extend_from_slice(&ext);
        let e = decode(&v);
        assert!(e.problems.is_empty(), "{:?}", e.problems);
        assert!(e.cta.is_empty());
        assert_eq!(
            e.extensions,
            [Extension {
                index: 1,
                tag: 0x70,
                kind: "DisplayID".into()
            }]
        );
    }

    /// The committed `.bin` fixtures (used by the CLI tests) are exactly the synthetic
    /// EDIDs. Regenerate with `AVLT_WRITE_FIXTURES=1 cargo test -p avlt-core fixtures`.
    #[test]
    fn committed_fixtures_match_synthetic() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/edid");
        for (name, bytes) in [
            ("synthetic_base.bin", synthetic_base().to_vec()),
            ("synthetic_cta.bin", synthetic_with_cta()),
        ] {
            let path = dir.join(name);
            if std::env::var_os("AVLT_WRITE_FIXTURES").is_some() {
                std::fs::write(&path, &bytes).unwrap();
            }
            let on_disk = std::fs::read(&path).unwrap();
            assert_eq!(on_disk, bytes, "{name} is stale");
        }
    }

    #[test]
    fn raw_serializes_as_hex() {
        let e = decode(&synthetic_base());
        let json = serde_json::to_value(&e).unwrap();
        assert!(
            json["raw"]
                .as_str()
                .unwrap()
                .starts_with("00 FF FF FF FF FF FF 00 10 AC")
        );
        assert_eq!(json["base"]["manufacturer"], "DEL");
    }
}
