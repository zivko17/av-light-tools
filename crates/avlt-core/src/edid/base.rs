//! EDID base block (bytes 0..127): `ConvertFrom-EDIDBaseBlock`, `EDIDTools.ps1` lines 177-331.

use serde::Serialize;

use super::timing::{
    DetailedTiming, ESTABLISHED, descriptor_text, manufacturer_id, parse_dtd, round_dp,
};

/// Common PnP manufacturer IDs (`$script:EDIDVendor`). Unknown codes are shown as-is.
const VENDORS: &[(&str, &str)] = &[
    ("GSM", "LG Electronics"),
    ("LGD", "LG Display"),
    ("SAM", "Samsung"),
    ("SEC", "Samsung / Seiko Epson"),
    ("DEL", "Dell"),
    ("ACI", "ASUS"),
    ("AUS", "ASUS"),
    ("BNQ", "BenQ"),
    ("ACR", "Acer"),
    ("HWP", "HP"),
    ("HPN", "HP"),
    ("LEN", "Lenovo"),
    ("VSC", "ViewSonic"),
    ("AOC", "AOC"),
    ("MSI", "MSI"),
    ("GBT", "Gigabyte"),
    ("PHL", "Philips"),
    ("SNY", "Sony"),
    ("NEC", "NEC"),
    ("EIZ", "EIZO"),
    ("SHP", "Sharp"),
    ("TSB", "Toshiba"),
    ("APP", "Apple"),
    ("CMN", "Chimei Innolux"),
    ("BOE", "BOE"),
    ("AUO", "AU Optronics"),
    ("EPS", "Epson"),
    ("VIZ", "Vizio"),
    ("HIT", "Hitachi"),
    ("MEI", "Panasonic"),
    ("PIO", "Pioneer"),
    ("BBY", "Insignia"),
];

/// Vendor name for a PnP ID, if it is in the table.
pub fn vendor_name(id: &str) -> Option<&'static str> {
    VENDORS.iter().find(|(k, _)| *k == id).map(|(_, v)| *v)
}

/// Decoded base block.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BaseBlock {
    pub header_valid: bool,
    /// Three-letter PnP ID, e.g. `DEL`.
    pub manufacturer: String,
    pub vendor_name: Option<String>,
    pub product_code: u16,
    pub serial_number: u32,
    pub week: u8,
    pub year: u16,
    /// `major.minor`, e.g. `1.4`.
    pub version: String,
    pub revision: u8,
    pub is_digital: bool,
    pub input_desc: String,
    /// EDID 1.4 digital only.
    pub bit_depth: Option<String>,
    /// EDID 1.4 digital only.
    pub interface: Option<String>,
    pub width_cm: u8,
    pub height_cm: u8,
    pub diagonal_in: Option<f64>,
    pub gamma: Option<f64>,
    pub features: Vec<String>,
    pub colour_type: String,
    pub established: Vec<String>,
    pub standard: Vec<String>,
    /// Detailed timings of the four descriptors, in order.
    pub timings: Vec<DetailedTiming>,
    /// Preferred (native) timing: the first detailed timing.
    pub native: Option<DetailedTiming>,
    pub monitor_name: Option<String>,
    pub serial_text: Option<String>,
    pub text_fields: Vec<String>,
    pub range: Option<RangeLimits>,
    /// Byte 126: number of extension blocks declared.
    pub ext_count: u8,
}

/// Display range limits descriptor (0xFD).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RangeLimits {
    pub v_min: u32,
    pub v_max: u32,
    /// kHz
    pub h_min: u32,
    /// kHz
    pub h_max: u32,
    pub max_clock_mhz: u32,
}

const HEADER: [u8; 8] = [0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00];

/// Decodes a base block. Every byte pattern decodes; validity is reported in fields.
pub fn decode_base(b: &[u8; 128]) -> BaseBlock {
    let manufacturer = manufacturer_id(b[8], b[9]);
    let product_code = u16::from_le_bytes([b[10], b[11]]);
    let serial_number = u32::from_le_bytes([b[12], b[13], b[14], b[15]]);
    let week = b[16];
    let year = 1990 + u16::from(b[17]);
    let version = format!("{}.{}", b[18], b[19]);
    let rev = b[19];

    // --- video input ---
    let vin = b[20];
    let is_digital = vin & 0x80 != 0;
    let mut input_desc = String::from("Analog");
    let mut bit_depth = None;
    let mut interface = None;
    if is_digital {
        // Bit depth and interface only exist from EDID 1.4 on. In 1.3 the low
        // bits mean "DFP 1.x compatible", so don't invent a depth/interface.
        if rev >= 4 {
            let depth = match (vin >> 4) & 0x07 {
                0 => "undefined bpc",
                1 => "6 bpc",
                2 => "8 bpc",
                3 => "10 bpc",
                4 => "12 bpc",
                5 => "14 bpc",
                6 => "16 bpc",
                _ => "reserved",
            };
            let iface = match vin & 0x0F {
                0 => "undefined interface",
                1 => "DVI",
                2 => "HDMI-a",
                3 => "HDMI-b",
                4 => "MDDI",
                5 => "DisplayPort",
                _ => "reserved interface",
            };
            input_desc = format!("Digital, {iface}, {depth}");
            bit_depth = Some(depth.to_string());
            interface = Some(iface.to_string());
        } else {
            input_desc = "Digital".into();
            if vin & 0x01 != 0 {
                input_desc.push_str(", DFP 1.x compatible");
            }
            input_desc.push_str("   (EDID 1.3: no depth/interface field)");
        }
    }

    // --- physical size / gamma / features ---
    let (h_cm, v_cm) = (b[21], b[22]);
    let diagonal_in = (h_cm > 0 && v_cm > 0).then(|| {
        let (h, v) = (f64::from(h_cm), f64::from(v_cm));
        round_dp((h * h + v * v).sqrt() / 2.54, 1)
    });
    let gamma = (b[23] != 0xFF).then(|| round_dp((f64::from(b[23]) + 100.0) / 100.0, 2));

    let feat = b[24];
    let features: Vec<String> = [
        (0x80, "DPMS standby"),
        (0x40, "DPMS suspend"),
        (0x20, "DPMS active-off"),
        (0x04, "sRGB default"),
        (0x02, "preferred timing is native"),
        (0x01, "continuous frequency"),
    ]
    .iter()
    .filter(|(bit, _)| feat & bit != 0)
    .map(|(_, n)| (*n).to_string())
    .collect();

    let ct = (feat >> 3) & 0x03;
    let colour_type = if is_digital {
        match ct {
            0 => "RGB 4:4:4",
            1 => "RGB 4:4:4 + YCrCb 4:4:4",
            2 => "RGB 4:4:4 + YCrCb 4:2:2",
            _ => "RGB 4:4:4 + YCrCb 4:4:4 + YCrCb 4:2:2",
        }
    } else {
        match ct {
            0 => "Monochrome",
            1 => "RGB colour",
            2 => "Non-RGB colour",
            _ => "Undefined",
        }
    }
    .to_string();

    // --- established timings (bytes 35..37) ---
    let established = ESTABLISHED
        .iter()
        .filter(|(off, bit, _)| b[35 + off] & (1 << bit) != 0)
        .map(|(_, _, t)| (*t).to_string())
        .collect();

    // --- standard timings (bytes 38..53) ---
    let mut standard = Vec::new();
    for i in (38..54).step_by(2) {
        if b[i] == 0x01 && b[i + 1] == 0x01 {
            continue;
        }
        if b[i] == 0x00 || b[i] == 0xFF {
            continue;
        }
        let h = (u32::from(b[i]) + 31) * 8;
        let hz = u32::from(b[i + 1] & 0x3F) + 60;
        // PowerShell's [int] cast rounds half to even.
        let scale = |num: f64, den: f64| (f64::from(h) * num / den).round_ties_even();
        let (v, ar) = match (b[i + 1] >> 6) & 0x03 {
            0 if rev < 3 => (f64::from(h), "1:1"),
            0 => (scale(10.0, 16.0), "16:10"),
            1 => (scale(3.0, 4.0), "4:3"),
            2 => (scale(4.0, 5.0), "5:4"),
            _ => (scale(9.0, 16.0), "16:9"),
        };
        standard.push(format!("{h}x{v}@{hz} ({ar})"));
    }

    // --- the four 18-byte descriptors (bytes 54..125) ---
    let mut timings = Vec::new();
    let mut monitor_name = None;
    let mut serial_text = None;
    let mut text_fields = Vec::new();
    let mut range = None;
    for d in (54..=108).step_by(18) {
        let Some(desc) = b.get(d..d + 18).and_then(|s| <&[u8; 18]>::try_from(s).ok()) else {
            continue;
        };
        if desc[0] == 0 && desc[1] == 0 && desc[2] == 0 {
            match desc[3] {
                0xFC => monitor_name = non_empty(descriptor_text(desc)),
                0xFF => serial_text = non_empty(descriptor_text(desc)),
                0xFE => text_fields.extend(non_empty(descriptor_text(desc))),
                0xFD => {
                    let (mut off_v, mut off_h) = (0, 0);
                    if rev >= 4 {
                        if desc[4] & 0x02 != 0 {
                            off_v = 255;
                        }
                        if desc[4] & 0x08 != 0 {
                            off_h = 255;
                        }
                    }
                    range = Some(RangeLimits {
                        v_min: u32::from(desc[5]),
                        v_max: u32::from(desc[6]) + off_v,
                        h_min: u32::from(desc[7]),
                        h_max: u32::from(desc[8]) + off_h,
                        max_clock_mhz: u32::from(desc[9]) * 10,
                    });
                }
                _ => {}
            }
        } else if let Some(t) = parse_dtd(desc) {
            timings.push(t);
        }
    }

    BaseBlock {
        header_valid: b[0..8] == HEADER,
        vendor_name: vendor_name(&manufacturer).map(str::to_string),
        manufacturer,
        product_code,
        serial_number,
        week,
        year,
        version,
        revision: rev,
        is_digital,
        input_desc,
        bit_depth,
        interface,
        width_cm: h_cm,
        height_cm: v_cm,
        diagonal_in,
        gamma,
        features,
        colour_type,
        established,
        standard,
        native: timings.first().cloned(),
        timings,
        monitor_name,
        serial_text,
        text_fields,
        range,
        ext_count: b[126],
    }
}

/// `Some(s)` unless empty (PowerShell treats an empty name as absent).
fn non_empty(s: String) -> Option<String> {
    (!s.is_empty()).then_some(s)
}

#[cfg(test)]
mod tests {
    use super::super::testdata::{fix_checksum, synthetic_base};
    use super::*;

    fn base() -> BaseBlock {
        decode_base(&synthetic_base())
    }

    #[test]
    fn identity_fields() {
        let b = base();
        assert!(b.header_valid);
        assert_eq!(b.manufacturer, "DEL");
        assert_eq!(b.vendor_name.as_deref(), Some("Dell"));
        assert_eq!(b.product_code, 0xA0B1);
        assert_eq!(b.serial_number, 12_345_678);
        assert_eq!((b.week, b.year), (10, 2020));
        assert_eq!((b.version.as_str(), b.revision), ("1.4", 4));
        assert_eq!(b.monitor_name.as_deref(), Some("DELL TEST"));
        assert_eq!(b.serial_text.as_deref(), Some("ABC123"));
        assert!(b.text_fields.is_empty());
        assert_eq!(b.ext_count, 0);
    }

    #[test]
    fn display_parameters() {
        let b = base();
        assert!(b.is_digital);
        assert_eq!(b.input_desc, "Digital, DisplayPort, 8 bpc");
        assert_eq!(b.bit_depth.as_deref(), Some("8 bpc"));
        assert_eq!(b.interface.as_deref(), Some("DisplayPort"));
        assert_eq!((b.width_cm, b.height_cm), (53, 30));
        assert_eq!(b.diagonal_in, Some(24.0)); // sqrt(53²+30²)/2.54 = 23.98
        assert_eq!(b.gamma, Some(2.2));
        assert_eq!(b.features, ["preferred timing is native"]);
        assert_eq!(b.colour_type, "RGB 4:4:4 + YCrCb 4:4:4");
    }

    #[test]
    fn timings_and_range() {
        let b = base();
        assert_eq!(b.established, ["640x480@60", "800x600@60", "1024x768@60"]);
        assert_eq!(b.standard, ["1920x1080@60 (16:9)"]);
        assert_eq!(b.timings.len(), 1);
        assert_eq!(b.native.as_ref().unwrap().summary, "1920x1080p @ 60 Hz");
        assert_eq!(
            b.range,
            Some(RangeLimits {
                v_min: 56,
                v_max: 76,
                h_min: 30,
                h_max: 83,
                max_clock_mhz: 170
            })
        );
    }

    #[test]
    fn range_offsets_only_in_edid_1_4() {
        let mut raw = synthetic_base();
        raw[76] = 0x0A; // desc 72: byte 4 = +255 V max and +255 H max
        let r = decode_base(&raw).range.unwrap();
        assert_eq!((r.v_max, r.h_max), (76 + 255, 83 + 255));
        raw[19] = 3;
        let r = decode_base(&raw).range.unwrap();
        assert_eq!((r.v_max, r.h_max), (76, 83));
    }

    #[test]
    fn edid_1_3_digital_has_no_depth_or_interface() {
        let mut raw = synthetic_base();
        raw[19] = 3;
        raw[20] = 0x81;
        let b = decode_base(&raw);
        assert_eq!(b.bit_depth, None);
        assert_eq!(b.interface, None);
        assert_eq!(
            b.input_desc,
            "Digital, DFP 1.x compatible   (EDID 1.3: no depth/interface field)"
        );
    }

    #[test]
    fn analog_input_and_colour_type() {
        let mut raw = synthetic_base();
        raw[20] = 0x00;
        raw[24] = 0xE5; // DPMS x3, sRGB, continuous, colour type 0
        let b = decode_base(&raw);
        assert!(!b.is_digital);
        assert_eq!(b.input_desc, "Analog");
        assert_eq!(b.colour_type, "Monochrome");
        assert_eq!(
            b.features,
            [
                "DPMS standby",
                "DPMS suspend",
                "DPMS active-off",
                "sRGB default",
                "continuous frequency"
            ]
        );
    }

    #[test]
    fn unknown_size_gamma_and_vendor() {
        let mut raw = synthetic_base();
        raw[21] = 0;
        raw[23] = 0xFF;
        raw[8] = 0x04; // "AAA"
        raw[9] = 0x21;
        let b = decode_base(&raw);
        assert_eq!(b.diagonal_in, None);
        assert_eq!(b.gamma, None);
        assert_eq!(b.manufacturer, "AAA");
        assert_eq!(b.vendor_name, None);
    }

    #[test]
    fn standard_timing_aspect_ratios() {
        let mut raw = synthetic_base();
        // 1280 = (129+31)*8; ar 0 (16:10) @60, 1 (4:3) @75, 2 (5:4) @60; 1368x769.5 -> 770
        raw[38..46].copy_from_slice(&[129, 0x00, 129, 0x4F, 129, 0x80, 140, 0xC0]);
        let b = decode_base(&raw);
        assert_eq!(
            b.standard,
            [
                "1280x800@60 (16:10)",
                "1280x960@75 (4:3)",
                "1280x1024@60 (5:4)",
                "1368x770@60 (16:9)"
            ]
        );
        raw[19] = 2; // EDID 1.2: aspect 0 means 1:1
        assert_eq!(decode_base(&raw).standard[0], "1280x1280@60 (1:1)");
    }

    #[test]
    fn standard_timing_skips_unused_slots() {
        let mut raw = synthetic_base();
        raw[38..44].copy_from_slice(&[0x00, 0x40, 0xFF, 0x40, 0x01, 0x01]);
        assert!(decode_base(&raw).standard.is_empty());
    }

    #[test]
    fn free_text_and_empty_name() {
        let mut raw = synthetic_base();
        raw[93] = 0xFE; // monitor name descriptor becomes free text
        raw[108..126].copy_from_slice(&[
            0, 0, 0, 0xFC, 0, 0x0A, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20,
            0x20, 0x20,
        ]);
        fix_checksum(&mut raw);
        let b = decode_base(&raw);
        assert_eq!(b.text_fields, ["DELL TEST"]);
        assert_eq!(b.monitor_name, None);
        assert_eq!(b.serial_text, None);
    }
}
