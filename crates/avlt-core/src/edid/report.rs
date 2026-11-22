//! What the EDID screens show beyond the decode itself, shared by the CLI and the GUI:
//! capability shorthand, the side-by-side comparison and the dump file name.

use serde::Serialize;

use super::{DiffLine, Edid, diff};

/// Capability shorthand that matters on site (`CTA3`, `HDMI2 600MHz`, `audio`, `HDR PQ/HLG`).
pub fn caps(e: &Edid) -> Vec<String> {
    let mut caps = Vec::new();
    let Some(c) = e.first_cta() else {
        caps.push("no CTA (DVI/VGA-style)".to_string());
        return caps;
    };
    caps.push(format!("CTA{}", c.revision));
    if let Some(r) = c.hdmi_forum.as_ref().and_then(|h| h.max_tmds_char_rate_mhz) {
        caps.push(format!("HDMI2 {r}MHz"));
    } else if let Some(r) = c.hdmi.as_ref().and_then(|h| h.max_tmds_mhz) {
        caps.push(format!("HDMI {r}MHz"));
    } else if c.hdmi.is_some() {
        caps.push("HDMI".into());
    }
    if c.basic_audio {
        caps.push("audio".into());
    }
    let hdr: Vec<&str> = c
        .hdr_eotf
        .iter()
        .filter(|e| e.contains("PQ") || e.contains("HLG"))
        .filter_map(|e| e.split(' ').next())
        .collect();
    if !hdr.is_empty() {
        caps.push(format!("HDR {}", hdr.join("/")));
    }
    caps
}

/// One compared field of a [`Comparison`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FieldRow {
    pub field: String,
    pub a: String,
    pub b: String,
    pub same: bool,
}

/// Two EDIDs side by side (`edid diff`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Comparison {
    pub a: String,
    pub b: String,
    pub a_bytes: usize,
    pub b_bytes: usize,
    pub fields: Vec<FieldRow>,
    pub differences: Vec<DiffLine>,
}

/// Compares `x` (named `a_name`) with `y` (named `b_name`).
pub fn compare(a_name: &str, x: &Edid, b_name: &str, y: &Edid) -> Comparison {
    Comparison {
        fields: diff::fields(x, y)
            .into_iter()
            .map(|(field, va, vb)| FieldRow {
                same: va == vb,
                field,
                a: va,
                b: vb,
            })
            .collect(),
        differences: diff(x, y),
        a_bytes: x.raw.len(),
        b_bytes: y.raw.len(),
        a: a_name.to_string(),
        b: b_name.to_string(),
    }
}

/// `<MFG>_<model>[_out<n>].bin`, with anything but word characters, `-` and `.` as `_`.
/// `display` is the display number when the EDID came from a display.
pub fn dump_file_name(e: &Edid, display: Option<usize>) -> String {
    let (mfg, model) = match &e.base {
        Some(b) => (
            b.manufacturer.clone(),
            b.monitor_name
                .clone()
                .unwrap_or_else(|| format!("{:04X}", b.product_code)),
        ),
        None => ("UNKNOWN".to_string(), "display".to_string()),
    };
    let mut safe: String = format!("{mfg}_{model}")
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, '_' | '-' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect();
    // The display index goes in the name: the same panel can appear on two outputs.
    if let Some(n) = display {
        safe.push_str(&format!("_out{n}"));
    }
    format!("{safe}.bin")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edid::decode;
    use crate::edid::testdata::synthetic_with_cta;

    #[test]
    fn dump_name_without_edid_base() {
        assert_eq!(dump_file_name(&decode(&[]), None), "UNKNOWN_display.bin");
        assert_eq!(
            dump_file_name(&decode(&[]), Some(2)),
            "UNKNOWN_display_out2.bin"
        );
    }

    #[test]
    fn comparing_an_edid_with_itself_finds_no_difference() {
        let e = decode(&synthetic_with_cta());
        let c = compare("x", &e, "y", &e);
        assert!(c.differences.is_empty());
        assert!(c.fields.iter().all(|f| f.same));
        assert_eq!((c.a.as_str(), c.b.as_str()), ("x", "y"));
        assert_eq!(c.a_bytes, 256);
    }

    #[test]
    fn caps_without_cta() {
        assert_eq!(caps(&decode(&[])), ["no CTA (DVI/VGA-style)"]);
    }
}
