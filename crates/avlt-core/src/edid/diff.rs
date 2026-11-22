//! EDID comparison: `Compare-EDIDSource`, `EDIDTools.ps1` lines 1107-1234.

use serde::Serialize;

use super::timing::vic_label;
use super::{CtaBlock, Edid};

/// What a [`DiffLine`] compares.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum DiffKind {
    /// A decoded field ("the things that actually break a show").
    Field,
    /// A video mode (VIC) present on one side only.
    Vic,
    /// A raw byte.
    Byte,
}

/// One difference between two EDIDs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DiffLine {
    pub kind: DiffKind,
    pub field: String,
    pub a: String,
    pub b: String,
}

/// The decoded fields compared by `edid diff`, as `(label, a, b)` text, equal or not.
pub fn fields(a: &Edid, b: &Edid) -> Vec<(String, String, String)> {
    let (fa, fb) = (field_values(a), field_values(b));
    fa.into_iter()
        .zip(fb)
        .map(|((label, va), (_, vb))| (label.to_string(), va, vb))
        .collect()
}

/// Every difference between `a` and `b`: decoded fields, VICs on one side only,
/// then raw bytes. Empty when the two are identical.
pub fn diff(a: &Edid, b: &Edid) -> Vec<DiffLine> {
    let mut out: Vec<DiffLine> = fields(a, b)
        .into_iter()
        .filter(|(_, va, vb)| va != vb)
        .map(|(field, a, b)| DiffLine {
            kind: DiffKind::Field,
            field,
            a,
            b,
        })
        .collect();

    // Video modes present on one side only - the reason a resolution "disappears"
    let (vic_a, vic_b) = (vics(a), vics(b));
    let one_side = |from: &[u8], other: &[u8], a_side: bool| {
        from.iter()
            .filter(|v| !other.contains(v))
            .map(|&v| {
                let (pa, pb) = if a_side {
                    ("present", "absent")
                } else {
                    ("absent", "present")
                };
                DiffLine {
                    kind: DiffKind::Vic,
                    field: vic_label(v),
                    a: pa.into(),
                    b: pb.into(),
                }
            })
            .collect::<Vec<_>>()
    };
    out.extend(one_side(&vic_a, &vic_b, true));
    out.extend(one_side(&vic_b, &vic_a, false));

    // Byte-level diff
    let len = a.raw.len().max(b.raw.len());
    let hex = |v: Option<&u8>| v.map_or_else(|| "--".to_string(), |x| format!("0x{x:02X}"));
    for i in 0..len {
        let (va, vb) = (a.raw.get(i), b.raw.get(i));
        if va != vb {
            out.push(DiffLine {
                kind: DiffKind::Byte,
                field: format!("byte {i} (blk {} off {})", i / 128, i % 128),
                a: hex(va),
                b: hex(vb),
            });
        }
    }
    out
}

/// VICs of the first CTA block's video data block.
fn vics(e: &Edid) -> Vec<u8> {
    e.first_cta()
        .map(|c| c.video.iter().map(|s| s.vic).collect())
        .unwrap_or_default()
}

fn yes_no(v: bool) -> String {
    if v { "yes" } else { "no" }.into()
}

/// The compared fields of one EDID, in display order.
fn field_values(e: &Edid) -> Vec<(&'static str, String)> {
    let dash = || "-".to_string();
    let base = e.base.as_ref();
    let from_base = |f: &dyn Fn(&super::BaseBlock) -> String| base.map_or_else(dash, f);
    let cta: Option<&CtaBlock> = e.first_cta();
    let from_cta = |f: &dyn Fn(&CtaBlock) -> String| cta.map_or_else(dash, f);
    let list_or_none = |v: Option<&Vec<String>>| match v {
        Some(v) if !v.is_empty() => v.join(", "),
        _ => "none".to_string(),
    };

    vec![
        ("Manufacturer", from_base(&|b| b.manufacturer.clone())),
        (
            "Model",
            from_base(&|b| b.monitor_name.clone().unwrap_or_default()),
        ),
        (
            "Product code",
            from_base(&|b| format!("0x{:04X}", b.product_code)),
        ),
        ("Serial", from_base(&|b| b.serial_number.to_string())),
        ("EDID version", from_base(&|b| b.version.clone())),
        (
            "Extension blocks",
            from_base(&|_| (e.block_count.saturating_sub(1)).to_string()),
        ),
        ("Input", from_base(&|b| b.input_desc.clone())),
        ("Colour formats", from_base(&|b| b.colour_type.clone())),
        (
            "Preferred timing",
            base.and_then(|b| b.native.as_ref())
                .map_or_else(dash, |t| t.summary.clone()),
        ),
        (
            "Preferred clock",
            base.and_then(|b| b.native.as_ref())
                .map_or_else(dash, |t| format!("{} MHz", t.pixel_clock_mhz)),
        ),
        (
            "Range limits",
            base.and_then(|b| b.range.as_ref()).map_or_else(dash, |r| {
                format!("{}-{} Hz, max {} MHz", r.v_min, r.v_max, r.max_clock_mhz)
            }),
        ),
        (
            "CTA-861",
            cta.map_or_else(|| "absent".into(), |c| format!("rev {}", c.revision)),
        ),
        ("Basic audio", from_cta(&|c| yes_no(c.basic_audio))),
        ("YCbCr 4:4:4", from_cta(&|c| yes_no(c.ycbcr444))),
        ("YCbCr 4:2:2", from_cta(&|c| yes_no(c.ycbcr422))),
        (
            "Max TMDS",
            match (
                cta.and_then(|c| c.hdmi_forum.as_ref())
                    .and_then(|h| h.max_tmds_char_rate_mhz),
                cta.and_then(|c| c.hdmi.as_ref())
                    .and_then(|h| h.max_tmds_mhz),
            ) {
                (Some(hf), _) => format!("{hf} MHz (HF)"),
                (None, Some(h)) => format!("{h} MHz"),
                (None, None) => dash(),
            },
        ),
        ("HDR EOTF", list_or_none(cta.map(|c| &c.hdr_eotf))),
        ("Colorimetry", list_or_none(cta.map(|c| &c.colorimetry))),
    ]
}

#[cfg(test)]
mod tests {
    use super::super::decode;
    use super::super::testdata::*;
    use super::*;

    fn kind(lines: &[DiffLine], k: DiffKind) -> Vec<&DiffLine> {
        lines.iter().filter(|l| l.kind == k).collect()
    }

    fn line(kind: DiffKind, field: &str, a: &str, b: &str) -> DiffLine {
        DiffLine {
            kind,
            field: field.into(),
            a: a.into(),
            b: b.into(),
        }
    }

    #[test]
    fn identical_is_empty() {
        let e = decode(&synthetic_base());
        assert!(diff(&e, &e).is_empty());
        let c = decode(&synthetic_with_cta());
        assert!(diff(&c, &c).is_empty());
    }

    #[test]
    fn fields_lists_every_row() {
        let e = decode(&synthetic_base());
        let rows = fields(&e, &e);
        let labels: Vec<&str> = rows.iter().map(|r| r.0.as_str()).collect();
        assert_eq!(
            labels,
            [
                "Manufacturer",
                "Model",
                "Product code",
                "Serial",
                "EDID version",
                "Extension blocks",
                "Input",
                "Colour formats",
                "Preferred timing",
                "Preferred clock",
                "Range limits",
                "CTA-861",
                "Basic audio",
                "YCbCr 4:4:4",
                "YCbCr 4:2:2",
                "Max TMDS",
                "HDR EOTF",
                "Colorimetry"
            ]
        );
        let get = |l: &str| rows.iter().find(|r| r.0 == l).unwrap().1.clone();
        assert_eq!(get("Product code"), "0xA0B1");
        assert_eq!(get("Preferred clock"), "148.5 MHz");
        assert_eq!(get("Range limits"), "56-76 Hz, max 170 MHz");
        assert_eq!(get("CTA-861"), "absent");
        assert_eq!(get("Basic audio"), "-");
        assert_eq!(get("HDR EOTF"), "none");
    }

    #[test]
    fn manufacturer_change() {
        let a = decode(&synthetic_base());
        let b = decode(&with_mfg(0x4C, 0x2D));
        let d = diff(&a, &b);
        assert_eq!(
            kind(&d, DiffKind::Field),
            [&line(DiffKind::Field, "Manufacturer", "DEL", "SAM")]
        );
        let bytes = kind(&d, DiffKind::Byte);
        assert_eq!(bytes.len(), 3, "{bytes:?}"); // bytes 8, 9 and the checksum
        assert_eq!(
            *bytes[0],
            line(DiffKind::Byte, "byte 8 (blk 0 off 8)", "0x10", "0x4C")
        );
    }

    #[test]
    fn cta_vs_no_cta() {
        let a = decode(&synthetic_base());
        let b = decode(&synthetic_with_cta());
        let d = diff(&a, &b);
        let f = kind(&d, DiffKind::Field);
        let row = |l: &str| {
            f.iter()
                .find(|x| x.field == l)
                .map(|x| (x.a.clone(), x.b.clone()))
        };
        assert_eq!(row("Extension blocks"), Some(("0".into(), "1".into())));
        assert_eq!(row("CTA-861"), Some(("absent".into(), "rev 3".into())));
        assert_eq!(row("Basic audio"), Some(("-".into(), "yes".into())));
        assert_eq!(row("Max TMDS"), Some(("-".into(), "600 MHz (HF)".into())));
        assert_eq!(
            row("HDR EOTF"),
            Some((
                "none".into(),
                "SDR (traditional gamma), PQ (SMPTE ST 2084), HLG".into()
            ))
        );
        assert_eq!(row("Manufacturer"), None);
        assert_eq!(
            kind(&d, DiffKind::Vic),
            [
                &line(DiffKind::Vic, "16 1920x1080p60", "absent", "present"),
                &line(DiffKind::Vic, "97 3840x2160p60", "absent", "present"),
                &line(DiffKind::Vic, "4 1280x720p60", "absent", "present"),
            ]
        );
        // 128 extra bytes on B plus bytes 126/127 of the base block
        let bytes = kind(&d, DiffKind::Byte);
        assert_eq!(bytes.len(), 130);
        assert_eq!(bytes.last().unwrap().a, "--");
        assert_eq!(bytes.last().unwrap().field, "byte 255 (blk 1 off 127)");
    }

    #[test]
    fn hdmi_1x_tmds_label() {
        let mut v = synthetic_with_cta();
        let c = &mut v[128..];
        let hf = c.windows(3).position(|w| w == [0xD8, 0x5D, 0xC4]).unwrap();
        c[hf] = 0;
        fix_checksum(c);
        let e = decode(&v);
        let rows = fields(&e, &e);
        assert_eq!(
            rows.iter().find(|r| r.0 == "Max TMDS").unwrap().1,
            "300 MHz"
        );
    }

    #[test]
    fn undecodable_sides_do_not_panic() {
        let a = decode(&[]);
        let b = decode(&synthetic_base());
        let d = diff(&a, &b);
        assert!(!d.is_empty());
        assert!(diff(&a, &a).is_empty());
        let rows = fields(&a, &a);
        assert_eq!(rows.iter().find(|r| r.0 == "Manufacturer").unwrap().1, "-");
    }
}
