//! LED cabinet database (generic standards plus brand datasheet models).
//! Ports `$Script:LEDGeneric`, `$Script:LEDBrandDefs`, `$Script:LEDBrandDefs2`,
//! `New-LEDEntry` and `New-LEDEntry2` (AVCalc.ps1 lines 122-213).
//!
//! Geometry (size + pitch -> px) is exact; electrical, weight and brightness values are
//! datasheet or typical rental values - always confirm on the manufacturer's datasheet.
//! The original's empty "your cabinets" block (`$Script:LEDUser`) is not ported.

use std::sync::OnceLock;

use serde::Serialize;

/// Where a cabinet is meant to be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LedEnv {
    Indoor,
    Outdoor,
}

impl LedEnv {
    /// `"indoor"` or `"outdoor"`.
    pub fn as_str(self) -> &'static str {
        match self {
            LedEnv::Indoor => "indoor",
            LedEnv::Outdoor => "outdoor",
        }
    }
}

/// Which table an entry comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LedTier {
    /// Typical medians by environment, pitch and cabinet size.
    Generic,
    /// Manufacturer datasheet models.
    Brand,
}

/// One LED cabinet preset.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LedCabinet {
    /// Lookup key (lowercase), e.g. `in-p2.6` or `absen-nt-in-1.9`.
    pub key: String,
    pub tier: LedTier,
    pub brand: String,
    /// Model name (brand entries only).
    pub model: Option<String>,
    /// Pixel pitch label in mm, e.g. `"2.6"`.
    pub pitch: String,
    /// Cabinet resolution.
    pub px_w: u32,
    pub px_h: u32,
    /// Cabinet size in mm.
    pub w_mm: f64,
    pub h_mm: f64,
    pub env: LedEnv,
    pub nits: u32,
    /// Peak power density in W/m^2 (all-white).
    pub max_w_m2: u32,
    pub kg: f64,
    /// Receiving cards per cabinet.
    pub recv_per_cab: u32,
}

/// Generic row: key, pitch, px_w, px_h, w_mm, h_mm, env, nits, W/m^2, kg, recv/cab.
type GenericRow = (
    &'static str,
    &'static str,
    u32,
    u32,
    f64,
    f64,
    LedEnv,
    u32,
    u32,
    f64,
    u32,
);

const IN: LedEnv = LedEnv::Indoor;
const OUT: LedEnv = LedEnv::Outdoor;

// Generic peaks are datasheet-representative medians: indoor ~550, outdoor ~700 W/m^2.
const GENERIC: [GenericRow; 12] = [
    (
        "in-p1.9", "1.9", 256, 256, 500.0, 500.0, IN, 1000, 550, 8.0, 1,
    ),
    (
        "in-p2.6", "2.6", 192, 192, 500.0, 500.0, IN, 1200, 550, 7.5, 1,
    ),
    (
        "in-p2.9", "2.9", 168, 168, 500.0, 500.0, IN, 1500, 550, 7.5, 1,
    ),
    (
        "in-p3.9", "3.9", 128, 128, 500.0, 500.0, IN, 1500, 550, 7.0, 1,
    ),
    (
        "in-p4.8", "4.8", 104, 104, 500.0, 500.0, IN, 1500, 550, 7.0, 1,
    ),
    (
        "out-p3.9", "3.9", 128, 128, 500.0, 500.0, OUT, 5000, 700, 9.0, 1,
    ),
    (
        "out-p4.8", "4.8", 104, 104, 500.0, 500.0, OUT, 5500, 700, 9.0, 1,
    ),
    (
        "in-p2.6-510",
        "2.6",
        192,
        384,
        500.0,
        1000.0,
        IN,
        1200,
        550,
        15.0,
        2,
    ),
    (
        "in-p2.9-510",
        "2.9",
        168,
        336,
        500.0,
        1000.0,
        IN,
        1500,
        550,
        15.0,
        2,
    ),
    (
        "in-p3.9-510",
        "3.9",
        128,
        256,
        500.0,
        1000.0,
        IN,
        1500,
        550,
        14.0,
        2,
    ),
    (
        "out-p3.9-510",
        "3.9",
        128,
        256,
        500.0,
        1000.0,
        OUT,
        5000,
        700,
        18.0,
        2,
    ),
    (
        "out-p4.8-510",
        "4.8",
        104,
        208,
        500.0,
        1000.0,
        OUT,
        5500,
        700,
        18.0,
        2,
    ),
];

/// Brand row (500 mm wide cabinets, 500 or 1000 mm tall): brand, key prefix, series slug,
/// model, px wide, pitch, env, tall (500x1000), peak W/m^2, nits, kg.
type BrandRow = (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    u32,
    &'static str,
    LedEnv,
    bool,
    u32,
    u32,
    f64,
);

// Absen (usabsen.com), Traulux (Charmex) and Unilumin datasheets.
const BRANDS: [BrandRow; 19] = [
    (
        "Absen",
        "absen",
        "nt",
        "NT1.9 Pro V2",
        256,
        "1.9",
        IN,
        false,
        645,
        1000,
        7.1,
    ),
    (
        "Absen",
        "absen",
        "nt",
        "NT2.6 Pro V2",
        192,
        "2.6",
        IN,
        false,
        600,
        1000,
        6.8,
    ),
    (
        "Absen",
        "absen",
        "pl",
        "PL2.5 Pro V2",
        200,
        "2.5",
        IN,
        false,
        680,
        1500,
        7.8,
    ),
    (
        "Absen",
        "absen",
        "pl",
        "PL2.9 Plus V2",
        168,
        "2.9",
        IN,
        false,
        560,
        1000,
        7.8,
    ),
    (
        "Absen",
        "absen",
        "pl",
        "PL3.9 Plus V2",
        128,
        "3.9",
        IN,
        false,
        560,
        1000,
        7.8,
    ),
    (
        "Absen",
        "absen",
        "pl",
        "PL2.9 Pro V2",
        168,
        "2.9",
        OUT,
        false,
        610,
        3000,
        8.0,
    ),
    (
        "Absen",
        "absen",
        "pl",
        "PL3.9 Pro V2",
        128,
        "3.9",
        OUT,
        false,
        800,
        4500,
        8.0,
    ),
    (
        "Absen",
        "absen",
        "pl",
        "PL3.9 Pro V2 500x1000",
        128,
        "3.9",
        OUT,
        true,
        800,
        4500,
        13.0,
    ),
    (
        "Absen",
        "absen",
        "pl",
        "PL4.8 Pro V2",
        104,
        "4.8",
        OUT,
        false,
        800,
        4500,
        8.0,
    ),
    (
        "Absen",
        "absen",
        "pr",
        "PR3.9 (IP65)",
        128,
        "3.9",
        OUT,
        false,
        600,
        6000,
        9.5,
    ),
    (
        "Traulux",
        "traulux",
        "lx",
        "T-Rent-LX 1.9",
        256,
        "1.9",
        IN,
        false,
        580,
        1000,
        7.5,
    ),
    (
        "Traulux",
        "traulux",
        "l",
        "T-Rent-L 2.6",
        192,
        "2.6",
        IN,
        false,
        490,
        800,
        7.9,
    ),
    (
        "Traulux",
        "traulux",
        "lx",
        "T-Rent-LX 2.9",
        168,
        "2.9",
        OUT,
        false,
        650,
        3800,
        7.3,
    ),
    (
        "Traulux",
        "traulux",
        "lx",
        "T-Rent-LX 3.9",
        128,
        "3.9",
        OUT,
        false,
        630,
        4000,
        7.5,
    ),
    (
        "Traulux",
        "traulux",
        "blk",
        "T-Rent 3.9 Black 500x1000",
        128,
        "3.9",
        OUT,
        true,
        700,
        3500,
        12.2,
    ),
    (
        "Unilumin",
        "unilumin",
        "upad",
        "UpadIV 2.6",
        192,
        "2.6",
        IN,
        false,
        480,
        1000,
        6.3,
    ),
    (
        "Unilumin",
        "unilumin",
        "urm",
        "URMIII 3.9",
        128,
        "3.9",
        IN,
        false,
        496,
        800,
        8.0,
    ),
    (
        "Unilumin",
        "unilumin",
        "ugm",
        "UGMII 2.6",
        192,
        "2.6",
        IN,
        false,
        480,
        1000,
        6.0,
    ),
    (
        "Unilumin",
        "unilumin",
        "urm",
        "URMIII 03 3.9 500x1000",
        128,
        "3.9",
        OUT,
        true,
        670,
        3000,
        13.9,
    ),
];

/// Varied-geometry brand row (datasheet power in W per cabinet): brand, key prefix, series
/// slug, model, px_w, px_h, w_mm, h_mm, pitch, env, W per cabinet, nits, kg, recv/cab.
type BrandRow2 = (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    u32,
    u32,
    f64,
    f64,
    &'static str,
    LedEnv,
    f64,
    u32,
    f64,
    u32,
);

// ROE Visual and INFiLED datasheets. W/cab is converted to W/m^2 with the real cabinet area.
const BRANDS2: [BrandRow2; 20] = [
    (
        "ROE Visual",
        "roe",
        "bp",
        "Black Pearl BP2 V2",
        176,
        176,
        500.0,
        500.0,
        "2.8",
        IN,
        160.0,
        1500,
        9.35,
        1,
    ),
    (
        "ROE Visual",
        "roe",
        "dm",
        "Diamond DM2.6",
        192,
        192,
        500.0,
        500.0,
        "2.6",
        IN,
        180.0,
        1500,
        5.76,
        1,
    ),
    (
        "ROE Visual",
        "roe",
        "dm",
        "Diamond DM3.9",
        128,
        128,
        500.0,
        500.0,
        "3.9",
        IN,
        150.0,
        1500,
        5.76,
        1,
    ),
    (
        "ROE Visual",
        "roe",
        "rb",
        "Ruby RB1.9B V2",
        256,
        256,
        500.0,
        500.0,
        "1.9",
        IN,
        160.0,
        1500,
        9.2,
        1,
    ),
    (
        "ROE Visual",
        "roe",
        "rb",
        "Ruby RB2.6",
        192,
        192,
        500.0,
        500.0,
        "2.6",
        IN,
        160.0,
        1500,
        8.5,
        1,
    ),
    (
        "ROE Visual",
        "roe",
        "cb",
        "Carbon CB3 MKII",
        160,
        320,
        600.0,
        1200.0,
        "3.75",
        OUT,
        600.0,
        4500,
        14.0,
        2,
    ),
    (
        "ROE Visual",
        "roe",
        "cb",
        "Carbon CB5 MKII",
        104,
        208,
        600.0,
        1200.0,
        "5.77",
        OUT,
        480.0,
        6000,
        12.2,
        2,
    ),
    (
        "ROE Visual",
        "roe",
        "cb",
        "Carbon CB8 MKII",
        72,
        144,
        600.0,
        1200.0,
        "8.33",
        OUT,
        530.0,
        6000,
        11.0,
        2,
    ),
    (
        "ROE Visual",
        "roe",
        "bm",
        "Black Marble BM4",
        128,
        128,
        609.6,
        609.6,
        "4.8",
        OUT,
        290.0,
        2000,
        17.5,
        1,
    ),
    (
        "ROE Visual",
        "roe",
        "vn",
        "Vanish V5S MKII",
        176,
        176,
        1000.0,
        1000.0,
        "5.6",
        IN,
        420.0,
        2000,
        10.9,
        1,
    ),
    (
        "ROE Visual",
        "roe",
        "vn",
        "Vanish V8S MKII",
        112,
        112,
        1000.0,
        1000.0,
        "8.9",
        IN,
        460.0,
        3500,
        9.6,
        1,
    ),
    (
        "INFiLED",
        "infiled",
        "ar",
        "AR IL-RSS-IRAR2.97",
        168,
        336,
        500.0,
        1000.0,
        "2.9",
        IN,
        270.0,
        1200,
        14.0,
        2,
    ),
    (
        "INFiLED",
        "infiled",
        "ar",
        "AR IL-RSS-IRAR3.91",
        128,
        256,
        500.0,
        1000.0,
        "3.9",
        IN,
        270.0,
        1200,
        12.5,
        2,
    ),
    (
        "INFiLED",
        "infiled",
        "ar",
        "AR IL-RSS-IRAR4.81",
        104,
        208,
        500.0,
        1000.0,
        "4.8",
        IN,
        270.0,
        1200,
        12.3,
        2,
    ),
    (
        "INFiLED",
        "infiled",
        "ar",
        "AR IL-RSS-ORAR3.91",
        128,
        256,
        500.0,
        1000.0,
        "3.9",
        OUT,
        270.0,
        5000,
        12.5,
        2,
    ),
    (
        "INFiLED",
        "infiled",
        "ar",
        "AR IL-RSS-ORAR4.63",
        108,
        216,
        500.0,
        1000.0,
        "4.6",
        OUT,
        270.0,
        5000,
        12.7,
        2,
    ),
    (
        "INFiLED",
        "infiled",
        "db",
        "DB IL-RSS-IRDB1.9",
        256,
        256,
        500.0,
        500.0,
        "1.9",
        IN,
        150.0,
        800,
        8.8,
        1,
    ),
    (
        "INFiLED",
        "infiled",
        "db",
        "DB IL-RSS-IRDB2.6",
        192,
        192,
        500.0,
        500.0,
        "2.6",
        IN,
        150.0,
        1200,
        8.8,
        1,
    ),
    (
        "INFiLED",
        "infiled",
        "db",
        "DB IL-RSS-IRDB3.4",
        144,
        144,
        500.0,
        500.0,
        "3.4",
        IN,
        150.0,
        1500,
        8.8,
        1,
    ),
    (
        "INFiLED",
        "infiled",
        "x",
        "X-Series 2.9 Flex",
        168,
        168,
        500.0,
        500.0,
        "2.9",
        IN,
        180.0,
        1200,
        8.9,
        1,
    ),
];

fn env_code(env: LedEnv) -> &'static str {
    match env {
        LedEnv::Indoor => "in",
        LedEnv::Outdoor => "out",
    }
}

fn build() -> Vec<LedCabinet> {
    let mut all = Vec::with_capacity(GENERIC.len() + BRANDS.len() + BRANDS2.len());

    for &(key, pitch, px_w, px_h, w_mm, h_mm, env, nits, max_w_m2, kg, recv_per_cab) in &GENERIC {
        all.push(LedCabinet {
            key: key.into(),
            tier: LedTier::Generic,
            brand: "Generic".into(),
            model: None,
            pitch: pitch.into(),
            px_w,
            px_h,
            w_mm,
            h_mm,
            env,
            nits,
            max_w_m2,
            kg,
            recv_per_cab,
        });
    }

    // New-LEDEntry: 500x500 (1 receiving card) or 500x1000 "tall" (2 cards, double height in px).
    for &(brand, prefix, series, model, px, pitch, env, tall, max_w_m2, nits, kg) in &BRANDS {
        let (h_mm, px_h, recv_per_cab) = if tall {
            (1000.0, px * 2, 2)
        } else {
            (500.0, px, 1)
        };
        let suffix = if tall { "-510" } else { "" };
        all.push(LedCabinet {
            key: format!("{prefix}-{series}-{}-{pitch}{suffix}", env_code(env)),
            tier: LedTier::Brand,
            brand: brand.into(),
            model: Some(model.into()),
            pitch: pitch.into(),
            px_w: px,
            px_h,
            w_mm: 500.0,
            h_mm,
            env,
            nits,
            max_w_m2,
            kg,
            recv_per_cab,
        });
    }

    // New-LEDEntry2: datasheet W per cabinet -> W/m^2 using the real cabinet area.
    // PowerShell's [math]::Round rounds half to even.
    for &(
        brand,
        prefix,
        series,
        model,
        px_w,
        px_h,
        w_mm,
        h_mm,
        pitch,
        env,
        w_cab,
        nits,
        kg,
        recv,
    ) in &BRANDS2
    {
        let area = w_mm * h_mm / 1e6;
        all.push(LedCabinet {
            key: format!("{prefix}-{series}-{}-{pitch}", env_code(env)),
            tier: LedTier::Brand,
            brand: brand.into(),
            model: Some(model.into()),
            pitch: pitch.into(),
            px_w,
            px_h,
            w_mm,
            h_mm,
            env,
            nits,
            max_w_m2: (w_cab / area).round_ties_even() as u32,
            kg,
            recv_per_cab: recv,
        });
    }
    all
}

/// Every cabinet preset: generic first, then brands (the order of the original `-List`).
pub fn led_cabinets() -> &'static [LedCabinet] {
    static ALL: OnceLock<Vec<LedCabinet>> = OnceLock::new();
    ALL.get_or_init(build).as_slice()
}

/// Looks a preset up by key, ignoring case.
pub fn find_led_cabinet(key: &str) -> Option<&'static LedCabinet> {
    let key = key.trim().to_lowercase();
    led_cabinets().iter().find(|c| c.key == key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_sizes() {
        let all = led_cabinets();
        assert_eq!(all.len(), 12 + 19 + 20);
        assert_eq!(
            all.iter().filter(|c| c.tier == LedTier::Generic).count(),
            12
        );
    }

    #[test]
    fn keys_are_unique_lowercase() {
        let mut keys: Vec<&str> = led_cabinets().iter().map(|c| c.key.as_str()).collect();
        assert!(keys.iter().all(|k| *k == k.to_lowercase()));
        keys.sort_unstable();
        let n = keys.len();
        keys.dedup();
        assert_eq!(keys.len(), n, "duplicate cabinet key");
    }

    #[test]
    fn generic_entry_copied_verbatim() {
        // line 128: 'in-p2.6' 192x192 500x500 indoor 1200 nits 550 W/m2 7.5 kg 1 card
        let c = find_led_cabinet("in-p2.6").unwrap();
        assert_eq!((c.px_w, c.px_h, c.w_mm, c.h_mm), (192, 192, 500.0, 500.0));
        assert_eq!(
            (c.env, c.nits, c.max_w_m2, c.recv_per_cab),
            (LedEnv::Indoor, 1200, 550, 1)
        );
        assert!((c.kg - 7.5).abs() < 1e-6);
        assert_eq!(c.brand, "Generic");
        assert!(c.model.is_none());
        // line 138: 'out-p4.8-510'
        let c = find_led_cabinet("out-p4.8-510").unwrap();
        assert_eq!(
            (c.px_w, c.px_h, c.h_mm, c.recv_per_cab),
            (104, 208, 1000.0, 2)
        );
        assert_eq!((c.env, c.nits, c.max_w_m2), (LedEnv::Outdoor, 5500, 700));
    }

    #[test]
    fn brand_entry_via_new_ledentry() {
        // line 151: Absen NT1.9 Pro V2 256 px, indoor, W=645, 1000 nits, 7.1 kg -> key built on line 177
        let c = find_led_cabinet("absen-nt-in-1.9").unwrap();
        assert_eq!(c.model.as_deref(), Some("NT1.9 Pro V2"));
        assert_eq!((c.px_w, c.px_h, c.w_mm, c.h_mm), (256, 256, 500.0, 500.0));
        assert_eq!((c.max_w_m2, c.nits, c.recv_per_cab), (645, 1000, 1));
        // line 158: tall cabinet -> "-510", height 1000 mm, px height doubled, 2 receiving cards
        let c = find_led_cabinet("absen-pl-out-3.9-510").unwrap();
        assert_eq!(
            (c.px_w, c.px_h, c.w_mm, c.h_mm, c.recv_per_cab),
            (128, 256, 500.0, 1000.0, 2)
        );
        assert!((c.kg - 13.0).abs() < 1e-6);
    }

    #[test]
    fn brand_entry2_converts_watts_per_cabinet() {
        // New-LEDEntry2 (line 181-185): W/m2 = round(Wcab / (Wmm*Hmm/1e6))
        // BP2: 160 / 0.25 = 640
        assert_eq!(find_led_cabinet("roe-bp-in-2.8").unwrap().max_w_m2, 640);
        // CB3: 600 / 0.72 = 833.33 -> 833
        assert_eq!(find_led_cabinet("roe-cb-out-3.75").unwrap().max_w_m2, 833);
        // BM4: 290 / (0.6096 * 0.6096) = 780.4 -> 780
        assert_eq!(find_led_cabinet("roe-bm-out-4.8").unwrap().max_w_m2, 780);
        // INFiLED AR (500x1000): 270 / 0.5 = 540
        assert_eq!(find_led_cabinet("infiled-ar-in-2.9").unwrap().max_w_m2, 540);
    }

    #[test]
    fn lookup_ignores_case_and_rejects_unknown() {
        assert!(find_led_cabinet("ABSEN-NT-IN-1.9").is_some());
        assert!(find_led_cabinet("nope").is_none());
    }
}
