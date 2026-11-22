//! Rental LED wall: resolution, physical size, power and NovaStar signal sizing.
//! Ports `Get-LEDWall` (AVCalc.ps1 lines 326-579), the NovaStar tables (226-255),
//! `Find-NovaSender` (257-263) and `Get-LEDGroupMap` (268-291). The wizard
//! (`Invoke-LEDWizard`, lines 607-702) and the diagram drawing are not part of core.

use serde::Serialize;

use super::led_cabinets::find_led_cabinet;
use super::{require_non_negative, require_positive};
use crate::error::{Error, Result};

/// Largest wall (cabinets per side) accepted.
pub const MAX_WALL_SIDE: u32 = 10_000;

/// Receiving card. `coex` marks Armor-series cards (A10s Pro and newer) required by
/// the COEX/MX platform; legacy A5s/A8s are not COEX-compatible.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct NovaRecvCard {
    pub name: &'static str,
    /// Pixel capacity per card.
    pub px_cap: u32,
    pub coex: bool,
}

/// NovaStar sender / processor with Gigabit-port count and maximum loading.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct NovaSender {
    pub name: &'static str,
    /// `MCTRL`, `VX`, `COEX` or `H`.
    pub series: &'static str,
    pub ports: u32,
    /// Maximum loading in megapixels.
    pub max_mp: f64,
}

/// Receiving cards (AVCalc.ps1 lines 228-234).
pub const NOVA_RECV_CARDS: [NovaRecvCard; 5] = [
    NovaRecvCard {
        name: "A5s Plus",
        px_cap: 512_000,
        coex: false,
    },
    NovaRecvCard {
        name: "A8s",
        px_cap: 1_024_000,
        coex: false,
    },
    NovaRecvCard {
        name: "A8s Plus",
        px_cap: 1_300_000,
        coex: false,
    },
    NovaRecvCard {
        name: "A10s Pro",
        px_cap: 1_300_000,
        coex: true,
    },
    NovaRecvCard {
        name: "CA50E",
        px_cap: 2_600_000,
        coex: true,
    },
];

/// Senders, smallest first within each series (AVCalc.ps1 lines 239-255).
pub const NOVA_SENDERS: [NovaSender; 15] = [
    NovaSender {
        name: "MCTRL300",
        series: "MCTRL",
        ports: 2,
        max_mp: 1.3,
    },
    NovaSender {
        name: "MCTRL660",
        series: "MCTRL",
        ports: 4,
        max_mp: 2.6,
    },
    NovaSender {
        name: "MCTRL660 Pro",
        series: "MCTRL",
        ports: 6,
        max_mp: 3.9,
    },
    NovaSender {
        name: "MCTRL R5",
        series: "MCTRL",
        ports: 8,
        max_mp: 4.1,
    },
    NovaSender {
        name: "MCTRL4K",
        series: "MCTRL",
        ports: 16,
        max_mp: 8.8,
    },
    NovaSender {
        name: "VX400 Pro",
        series: "VX",
        ports: 4,
        max_mp: 2.6,
    },
    NovaSender {
        name: "VX600 Pro",
        series: "VX",
        ports: 6,
        max_mp: 3.9,
    },
    NovaSender {
        name: "VX1000 Pro",
        series: "VX",
        ports: 10,
        max_mp: 6.5,
    },
    NovaSender {
        name: "VX2000 Pro",
        series: "VX",
        ports: 20,
        max_mp: 13.0,
    },
    NovaSender {
        name: "MX40 Pro",
        series: "COEX",
        ports: 20,
        max_mp: 9.0,
    },
    NovaSender {
        name: "MX2000 Pro",
        series: "COEX",
        ports: 80,
        max_mp: 35.4,
    },
    NovaSender {
        name: "H2",
        series: "H",
        ports: 40,
        max_mp: 26.0,
    },
    NovaSender {
        name: "H5",
        series: "H",
        ports: 60,
        max_mp: 39.0,
    },
    NovaSender {
        name: "H9",
        series: "H",
        ports: 200,
        max_mp: 65.0,
    },
    NovaSender {
        name: "H15",
        series: "H",
        ports: 320,
        max_mp: 208.0,
    },
];

/// Smallest sender in `series` with enough ports and loading for the wall.
pub fn find_nova_sender(series: &str, need_ports: u32, total_px: f64) -> Option<NovaSender> {
    NOVA_SENDERS
        .iter()
        .find(|s| s.series == series && s.ports >= need_ports && s.max_mp * 1e6 >= total_px)
        .copied()
}

/// What the power sizing is based on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PowerBasis {
    /// About a third of peak (real content).
    #[default]
    Avg,
    /// Peak (all-white worst case).
    Max,
}

/// Wiring order used by the signal and power maps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Distribution {
    /// Serpentine column by column.
    #[default]
    Vertical,
    /// Serpentine row by row.
    Horizontal,
}

/// Inputs. Zero (or `None`) means "not given" and is filled from the preset when
/// `cabinet` names one.
#[derive(Debug, Clone, PartialEq)]
pub struct LedWallInput {
    /// Cabinets wide.
    pub cols: u32,
    /// Cabinets high.
    pub rows: u32,
    /// Preset key (see `led_cabinets`).
    pub cabinet: Option<String>,
    pub cab_px_w: u32,
    pub cab_px_h: u32,
    pub cab_wmm: f64,
    pub cab_hmm: f64,
    pub env: Option<String>,
    /// Peak watts per cabinet (wins over `max_wm2` and the preset).
    pub max_watt: f64,
    /// Peak W/m^2 (wins over the preset).
    pub max_wm2: f64,
    /// Average draw as a fraction of peak.
    pub avg_factor: f64,
    pub weight_kg: f64,
    pub nits: f64,
    pub recv_per_cab: u32,
    pub recv_card: Option<String>,
    pub sender: Option<String>,
    pub voltage: f64,
    pub breaker_a: f64,
    /// Physical powerCON daisy-chain limit (0 = no limit).
    pub max_chain: u32,
    pub power_basis: PowerBasis,
    /// Pixels one Gigabit data port carries (8-bit / 60 Hz).
    pub px_per_port: f64,
    pub distribution: Distribution,
    pub redundant: bool,
}

impl Default for LedWallInput {
    fn default() -> Self {
        LedWallInput {
            cols: 0,
            rows: 0,
            cabinet: None,
            cab_px_w: 0,
            cab_px_h: 0,
            cab_wmm: 0.0,
            cab_hmm: 0.0,
            env: None,
            max_watt: 0.0,
            max_wm2: 0.0,
            avg_factor: 0.33,
            weight_kg: 0.0,
            nits: 0.0,
            recv_per_cab: 0,
            recv_card: None,
            sender: None,
            voltage: 230.0,
            breaker_a: 16.0,
            max_chain: 10,
            power_basis: PowerBasis::Avg,
            px_per_port: 650_000.0,
            distribution: Distribution::Vertical,
            redundant: false,
        }
    }
}

/// Wall resolution.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Resolution {
    pub width: u64,
    pub height: u64,
    pub megapixels: f64,
    /// Reduced aspect ratio, e.g. `2:1`.
    pub aspect: String,
}

/// Physical size (only when the cabinet size in mm is known).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Physical {
    pub width_m: f64,
    pub height_m: f64,
    pub area_m2: f64,
    pub pitch_mm: f64,
    /// Retina viewing distance, pitch x 3.438.
    pub min_view_dist_m: f64,
    pub nits: Option<f64>,
    pub env: Option<String>,
}

/// Incoming supply / distro board.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Supply {
    /// e.g. `16 A 1-phase`.
    pub name: String,
    /// 80 % derated capacity of one feed.
    pub usable_kw: f64,
    pub phases: u32,
    /// Load per phase (three-phase feeds that cover the load).
    pub a_per_phase: Option<f64>,
    /// Number of feeds needed (1 unless the load exceeds the largest feed).
    pub count: u32,
    /// One feed covers the load.
    pub fits_one: bool,
}

/// Power sizing (only when per-cabinet watts are known).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Power {
    pub max_watt_per_cab: f64,
    pub density_peak_wm2: Option<f64>,
    pub density_avg_wm2: Option<f64>,
    pub peak_kw: f64,
    pub peak_a: f64,
    pub avg_factor: f64,
    pub avg_kw: f64,
    pub basis: PowerBasis,
    pub basis_kw: f64,
    pub voltage: f64,
    pub breaker_a: f64,
    /// Breakers needed.
    pub circuits: u64,
    /// Cabinets one breaker can power (80 % derated).
    pub cabinets_per_breaker: u64,
    pub usable_kw_per_breaker: f64,
    /// Power cables (daisy-chains) needed.
    pub cables: u64,
    /// Cabinets per daisy-chain.
    pub chain_cap: u64,
    /// `chain_cap` is limited by `max_chain` rather than by the breaker.
    pub powercon_capped: bool,
    pub supply: Supply,
}

/// Receiving card check.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RecvCardCheck {
    pub name: String,
    pub coex: bool,
    pub px_cap: u32,
    pub cabinet_px: f64,
    /// A cabinet has more pixels than one card can drive.
    pub over_cap: bool,
}

/// Result of checking a user-chosen sender.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SenderCheck {
    pub name: String,
    pub series: String,
    pub ports: u32,
    pub max_mp: f64,
    pub ok: bool,
    /// Units needed when short.
    pub units: Option<u64>,
    /// Why it is short, e.g. `need 6 ports, has 4`.
    pub shortfall: Option<String>,
}

/// Smallest sender per series that fits the wall.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SenderSuggestions {
    pub mctrl: Option<NovaSender>,
    pub vx: Option<NovaSender>,
    /// Only searched when no VX sender fits.
    pub h: Option<NovaSender>,
    /// Only searched when the receiving card is COEX-capable.
    pub coex: Option<NovaSender>,
}

/// Sender sizing.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SenderReport {
    /// Ports the sender must provide (doubled with redundancy).
    pub need_ports: u32,
    pub need_mp: f64,
    pub redundant: bool,
    /// The sender the user named, if it is known.
    pub selected: Option<SenderCheck>,
    /// A sender name that is not in the table.
    pub unknown: Option<String>,
    /// Suggestions (when no sender was named).
    pub suggestions: Option<SenderSuggestions>,
    /// A COEX sender with a receiving card that is not COEX-compatible.
    pub coex_incompatible_card: Option<String>,
    /// A COEX sender that needs A10s Pro / Armor receiving cards.
    pub coex_needs_armor: bool,
}

/// Signal (NovaStar) sizing.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Signal {
    pub ports: u64,
    pub px_per_port: f64,
    pub cabinets_per_port: u32,
    /// Cat cables for the chosen path (doubled when redundant).
    pub cat_cables: u64,
    pub cat_cables_redundant: u64,
    pub redundant: bool,
    pub recv_cards: u64,
    pub recv_per_cab: u32,
    pub recv_card: Option<RecvCardCheck>,
    /// A receiving card name that is not in the table.
    pub unknown_recv_card: Option<String>,
    pub sender: SenderReport,
}

/// Weight.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Weight {
    pub total_kg: f64,
    pub per_column_kg: f64,
    pub cabinets_high: u32,
}

/// LED wall result.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LedWallResult {
    pub cols: u32,
    pub rows: u32,
    pub total_cabinets: u64,
    /// Preset key, if one was used.
    pub cabinet: Option<String>,
    pub brand: Option<String>,
    pub model: Option<String>,
    pub cabinet_px_w: u32,
    pub cabinet_px_h: u32,
    pub resolution: Resolution,
    pub physical: Option<Physical>,
    pub power: Option<Power>,
    pub signal: Signal,
    pub weight: Option<Weight>,
    pub distribution: Distribution,
}

/// Assigns cabinets to groups (circuits / ports) in serpentine order, chunked by
/// `chunk_cap`. Returns `rows` x `cols` group ids (1-based). `Vertical` snakes
/// column by column, `Horizontal` row by row.
pub fn led_group_map(
    cols: u32,
    rows: u32,
    chunk_cap: u32,
    distribution: Distribution,
) -> Vec<Vec<u16>> {
    let cap = u64::from(chunk_cap.max(1));
    let (cols, rows) = (cols as usize, rows as usize);
    let mut map = vec![vec![0u16; cols]; rows];
    let mut idx: u64 = 0;
    let mut assign = |map: &mut Vec<Vec<u16>>, r: usize, c: usize| {
        map[r][c] = u16::try_from(idx / cap + 1).unwrap_or(u16::MAX);
        idx += 1;
    };
    match distribution {
        Distribution::Horizontal => {
            for r in 0..rows {
                if r % 2 == 0 {
                    (0..cols).for_each(|c| assign(&mut map, r, c));
                } else {
                    (0..cols).rev().for_each(|c| assign(&mut map, r, c));
                }
            }
        }
        Distribution::Vertical => {
            for c in 0..cols {
                if c % 2 == 0 {
                    (0..rows).for_each(|r| assign(&mut map, r, c));
                } else {
                    (0..rows).rev().for_each(|r| assign(&mut map, r, c));
                }
            }
        }
    }
    map
}

/// One of the standard CEE feeds: name, capacity in kW (before the 80 % derating), phases.
fn distros(voltage: f64) -> [(&'static str, f64, u32); 4] {
    [
        ("16 A 1-phase", voltage * 16.0 / 1000.0, 1),
        ("32 A 1-phase", voltage * 32.0 / 1000.0, 1),
        ("63 A 3-phase", voltage * 63.0 * 3.0 / 1000.0, 3),
        ("125 A 3-phase", voltage * 125.0 * 3.0 / 1000.0, 3),
    ]
}

fn gcd(a: u64, b: u64) -> u64 {
    if b == 0 { a } else { gcd(b, a % b) }
}

fn find_card(name: &str) -> Option<&'static NovaRecvCard> {
    NOVA_RECV_CARDS
        .iter()
        .find(|c| c.name.eq_ignore_ascii_case(name.trim()))
}

fn find_sender(name: &str) -> Option<&'static NovaSender> {
    NOVA_SENDERS
        .iter()
        .find(|s| s.name.eq_ignore_ascii_case(name.trim()))
}

fn non_empty(s: &Option<String>) -> Option<&str> {
    s.as_deref().map(str::trim).filter(|s| !s.is_empty())
}

/// Per-cabinet values after the preset has filled the gaps.
struct Resolved {
    px_w: u32,
    px_h: u32,
    w_mm: f64,
    h_mm: f64,
    kg: f64,
    nits: f64,
    recv_per_cab: u32,
    env: Option<String>,
    max_watt: f64,
}

fn power_section(input: &LedWallInput, total_cab: u64, max_watt: f64, cab_area: f64) -> Power {
    let total = total_cab as f64;
    let max_kw = total * max_watt / 1000.0;
    let avg_kw = max_kw * input.avg_factor;
    let basis_w = match input.power_basis {
        PowerBasis::Avg => max_watt * input.avg_factor,
        PowerBasis::Max => max_watt,
    };
    let basis_kw = total * basis_w / 1000.0;
    let max_a = total * max_watt / input.voltage;
    // 80 % derated breaker
    let usable_w = input.breaker_a * 0.8 * input.voltage;
    let elec_chain = ((usable_w / basis_w).floor() as u64).max(1);
    let mut chain_cap = elec_chain;
    let mut capped = false;
    if input.max_chain > 0 && u64::from(input.max_chain) < chain_cap {
        chain_cap = u64::from(input.max_chain);
        capped = true;
    }
    let circuits = total_cab.div_ceil(elec_chain);
    let cables = total_cab.div_ceil(chain_cap);

    let feeds = distros(input.voltage);
    let supply = match feeds.iter().find(|f| f.1 * 0.8 >= basis_kw) {
        Some(&(name, kw, phases)) => Supply {
            name: name.into(),
            usable_kw: kw * 0.8,
            phases,
            a_per_phase: (phases == 3).then(|| basis_kw * 1000.0 / (3.0 * input.voltage)),
            count: 1,
            fits_one: true,
        },
        None => {
            let (name, kw, phases) = feeds[feeds.len() - 1];
            Supply {
                name: name.into(),
                usable_kw: kw * 0.8,
                phases,
                a_per_phase: None,
                count: (basis_kw / (kw * 0.8)).ceil() as u32,
                fits_one: false,
            }
        }
    };

    let density = (cab_area > 0.0).then(|| max_watt / cab_area);
    Power {
        max_watt_per_cab: max_watt,
        density_peak_wm2: density,
        density_avg_wm2: density.map(|d| d * input.avg_factor),
        peak_kw: max_kw,
        peak_a: max_a,
        avg_factor: input.avg_factor,
        avg_kw,
        basis: input.power_basis,
        basis_kw,
        voltage: input.voltage,
        breaker_a: input.breaker_a,
        circuits,
        cabinets_per_breaker: elec_chain,
        usable_kw_per_breaker: usable_w / 1000.0,
        cables,
        chain_cap,
        powercon_capped: capped,
        supply,
    }
}

fn sender_report(
    input: &LedWallInput,
    ports: u64,
    total_px: f64,
    coex_card: bool,
    recv_card_given: bool,
) -> SenderReport {
    // With redundancy one unit must carry both loops, so it needs twice the ports.
    let need_ports_wide = if input.redundant { 2 * ports } else { ports };
    let need_ports = u32::try_from(need_ports_wide).unwrap_or(u32::MAX);
    let mut report = SenderReport {
        need_ports,
        need_mp: total_px / 1e6,
        redundant: input.redundant,
        selected: None,
        unknown: None,
        suggestions: None,
        coex_incompatible_card: None,
        coex_needs_armor: false,
    };

    match non_empty(&input.sender) {
        Some(name) => match find_sender(name) {
            Some(snd) => {
                let ports_ok = snd.ports >= need_ports;
                let mp_ok = snd.max_mp * 1e6 >= total_px;
                let ok = ports_ok && mp_ok;
                let (units, shortfall) = if ok {
                    (None, None)
                } else {
                    let by_ports = need_ports_wide.div_ceil(u64::from(snd.ports));
                    let by_mp = (total_px / (snd.max_mp * 1e6)).ceil() as u64;
                    let why = if mp_ok {
                        format!("need {need_ports} ports, has {}", snd.ports)
                    } else {
                        format!("over {} MP", snd.max_mp)
                    };
                    (Some(by_ports.max(by_mp)), Some(why))
                };
                report.selected = Some(SenderCheck {
                    name: snd.name.into(),
                    series: snd.series.into(),
                    ports: snd.ports,
                    max_mp: snd.max_mp,
                    ok,
                    units,
                    shortfall,
                });
                if snd.series == "COEX" {
                    if recv_card_given && !coex_card {
                        report.coex_incompatible_card =
                            non_empty(&input.recv_card).map(String::from);
                    } else {
                        report.coex_needs_armor = true;
                    }
                }
            }
            None => report.unknown = Some(name.to_string()),
        },
        None => {
            let mctrl = find_nova_sender("MCTRL", need_ports, total_px);
            let vx = find_nova_sender("VX", need_ports, total_px);
            let h = if vx.is_none() {
                find_nova_sender("H", need_ports, total_px)
            } else {
                None
            };
            let coex = if coex_card {
                find_nova_sender("COEX", need_ports, total_px)
            } else {
                None
            };
            report.suggestions = Some(SenderSuggestions { mctrl, vx, h, coex });
        }
    }
    report
}

/// Sizes an LED wall: resolution, physical size, power and NovaStar signal chain.
pub fn ledwall(input: &LedWallInput) -> Result<LedWallResult> {
    if input.cols < 1 || input.rows < 1 {
        return Err(Error::InvalidInput(
            "cols and rows must be at least 1 (e.g. calc ledwall 10 5 --cabinet in-p2.6)".into(),
        ));
    }
    if input.cols > MAX_WALL_SIDE || input.rows > MAX_WALL_SIDE {
        return Err(Error::InvalidInput(format!(
            "cols and rows must be at most {MAX_WALL_SIDE}"
        )));
    }
    for (name, v) in [
        ("cab-wmm", input.cab_wmm),
        ("cab-hmm", input.cab_hmm),
        ("max-watt", input.max_watt),
        ("max-wm2", input.max_wm2),
        ("weight-kg", input.weight_kg),
        ("nits", input.nits),
    ] {
        require_non_negative(name, v)?;
    }
    for (name, v) in [
        ("avg-factor", input.avg_factor),
        ("voltage", input.voltage),
        ("breaker-a", input.breaker_a),
        ("px-per-port", input.px_per_port),
    ] {
        require_positive(name, v)?;
    }

    let mut r = Resolved {
        px_w: input.cab_px_w,
        px_h: input.cab_px_h,
        w_mm: input.cab_wmm,
        h_mm: input.cab_hmm,
        kg: input.weight_kg,
        nits: input.nits,
        recv_per_cab: input.recv_per_cab,
        env: non_empty(&input.env).map(String::from),
        max_watt: input.max_watt,
    };
    let (mut cabinet_key, mut brand, mut model) = (None, None, None);
    let mut preset_wm2 = 0.0;
    if let Some(key) = non_empty(&input.cabinet) {
        let c = find_led_cabinet(key).ok_or_else(|| {
            Error::InvalidInput(format!(
                "unknown cabinet '{key}'. Run 'avlt calc ledwall --list' to see presets"
            ))
        })?;
        if r.px_w == 0 {
            r.px_w = c.px_w;
        }
        if r.px_h == 0 {
            r.px_h = c.px_h;
        }
        if r.w_mm <= 0.0 {
            r.w_mm = c.w_mm;
        }
        if r.h_mm <= 0.0 {
            r.h_mm = c.h_mm;
        }
        if r.kg <= 0.0 {
            r.kg = c.kg;
        }
        if r.nits <= 0.0 {
            r.nits = f64::from(c.nits);
        }
        if r.recv_per_cab == 0 {
            r.recv_per_cab = c.recv_per_cab;
        }
        if r.env.is_none() {
            r.env = Some(c.env.as_str().into());
        }
        preset_wm2 = f64::from(c.max_w_m2);
        cabinet_key = Some(c.key.clone());
        brand = Some(c.brand.clone());
        model = c.model.clone();
    }
    if r.recv_per_cab == 0 {
        r.recv_per_cab = 1;
    }
    if r.px_w == 0 || r.px_h == 0 {
        return Err(Error::InvalidInput(
            "need per-cabinet resolution: use --cabinet <preset> or --cab-px-w/--cab-px-h".into(),
        ));
    }

    let total_cab = u64::from(input.cols) * u64::from(input.rows);
    let res_w = u64::from(input.cols) * u64::from(r.px_w);
    let res_h = u64::from(input.rows) * u64::from(r.px_h);
    let total_px = res_w as f64 * res_h as f64;
    let cab_px = f64::from(r.px_w) * f64::from(r.px_h);
    let cab_area = r.w_mm * r.h_mm / 1e6; // m^2 (0 if mm unknown)

    // Effective per-cabinet peak watts: max_watt, then max_wm2 x area, then preset density x area.
    if r.max_watt <= 0.0 {
        if input.max_wm2 > 0.0 && cab_area > 0.0 {
            r.max_watt = input.max_wm2 * cab_area;
        } else if preset_wm2 > 0.0 && cab_area > 0.0 {
            r.max_watt = preset_wm2 * cab_area;
        }
    }

    let g = gcd(res_w, res_h);
    let resolution = Resolution {
        width: res_w,
        height: res_h,
        megapixels: total_px / 1e6,
        aspect: format!("{}:{}", res_w / g, res_h / g),
    };

    let physical = (r.w_mm > 0.0 && r.h_mm > 0.0).then(|| {
        let phys_w = f64::from(input.cols) * r.w_mm;
        let phys_h = f64::from(input.rows) * r.h_mm;
        let pitch = r.w_mm / f64::from(r.px_w);
        Physical {
            width_m: phys_w / 1000.0,
            height_m: phys_h / 1000.0,
            area_m2: phys_w * phys_h / 1e6,
            pitch_mm: pitch,
            min_view_dist_m: pitch * 3.438,
            nits: (r.nits > 0.0).then_some(r.nits),
            env: r.env.clone(),
        }
    });

    let power = (r.max_watt > 0.0).then(|| power_section(input, total_cab, r.max_watt, cab_area));

    // signal (NovaStar)
    let cab_per_port = ((input.px_per_port / cab_px).floor() as u32).max(1);
    let ports = total_cab.div_ceil(u64::from(cab_per_port));
    let recv_cards = total_cab * u64::from(r.recv_per_cab);
    let (mut recv_card, mut unknown_recv_card, mut coex_card) = (None, None, false);
    let card_name = non_empty(&input.recv_card);
    if let Some(name) = card_name {
        match find_card(name) {
            Some(c) => {
                coex_card = c.coex;
                recv_card = Some(RecvCardCheck {
                    name: c.name.into(),
                    coex: c.coex,
                    px_cap: c.px_cap,
                    cabinet_px: cab_px,
                    over_cap: cab_px > f64::from(c.px_cap),
                });
            }
            None => unknown_recv_card = Some(name.to_string()),
        }
    }
    let signal = Signal {
        ports,
        px_per_port: input.px_per_port,
        cabinets_per_port: cab_per_port,
        cat_cables: if input.redundant { 2 * ports } else { ports },
        cat_cables_redundant: 2 * ports,
        redundant: input.redundant,
        recv_cards,
        recv_per_cab: r.recv_per_cab,
        recv_card,
        unknown_recv_card,
        sender: sender_report(input, ports, total_px, coex_card, card_name.is_some()),
    };

    let weight = (r.kg > 0.0).then(|| Weight {
        total_kg: total_cab as f64 * r.kg,
        per_column_kg: f64::from(input.rows) * r.kg,
        cabinets_high: input.rows,
    });

    Ok(LedWallResult {
        cols: input.cols,
        rows: input.rows,
        total_cabinets: total_cab,
        cabinet: cabinet_key,
        brand,
        model,
        cabinet_px_w: r.px_w,
        cabinet_px_h: r.px_h,
        resolution,
        physical,
        power,
        signal,
        weight,
        distribution: input.distribution,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOL: f64 = 1e-6;

    fn close(a: f64, b: f64) {
        assert!((a - b).abs() < TOL, "{a} != {b}");
    }

    /// 10 x 5 wall of the generic 192x192, 500x500 mm, 550 W/m^2 preset.
    fn wall() -> LedWallInput {
        LedWallInput {
            cols: 10,
            rows: 5,
            cabinet: Some("in-p2.6".into()),
            ..LedWallInput::default()
        }
    }

    fn manual() -> LedWallInput {
        LedWallInput {
            cols: 2,
            rows: 2,
            cab_px_w: 100,
            cab_px_h: 100,
            ..LedWallInput::default()
        }
    }

    #[test]
    fn defaults_match_the_original() {
        let d = LedWallInput::default();
        close(d.avg_factor, 0.33);
        close(d.voltage, 230.0);
        close(d.breaker_a, 16.0);
        assert_eq!(d.max_chain, 10);
        close(d.px_per_port, 650_000.0);
        assert_eq!(d.power_basis, PowerBasis::Avg);
        assert_eq!(d.distribution, Distribution::Vertical);
    }

    #[test]
    fn resolution_and_physical() {
        // lines 391-395, 419-424: 10 x 192 = 1920, 5 x 192 = 960, 1.8432 MP, gcd 960 -> 2:1
        // size 10 x 500 mm = 5 m, 2.5 m, 12.5 m^2; pitch 500/192; min dist pitch x 3.438
        let r = ledwall(&wall()).unwrap();
        assert_eq!(r.total_cabinets, 50);
        assert_eq!((r.resolution.width, r.resolution.height), (1920, 960));
        close(r.resolution.megapixels, 1.8432);
        assert_eq!(r.resolution.aspect, "2:1");
        assert_eq!(r.cabinet.as_deref(), Some("in-p2.6"));
        assert_eq!(r.brand.as_deref(), Some("Generic"));
        let p = r.physical.unwrap();
        close(p.width_m, 5.0);
        close(p.height_m, 2.5);
        close(p.area_m2, 12.5);
        close(p.pitch_mm, 500.0 / 192.0);
        close(p.min_view_dist_m, 500.0 / 192.0 * 3.438);
        assert_eq!(p.nits, Some(1200.0));
        assert_eq!(p.env.as_deref(), Some("indoor"));
    }

    #[test]
    fn power_average_basis() {
        // lines 399-451: max_watt = 550 * 0.25 = 137.5; peak 50*137.5/1000 = 6.875 kW;
        // basis_w = 137.5*0.33 = 45.375; usable_w = 16*0.8*230 = 2944; floor(2944/45.375) = 64;
        // chain min(64, 10) = 10 (powerCON cap); circuits ceil(50/64) = 1; cables ceil(50/10) = 5
        let p = ledwall(&wall()).unwrap().power.unwrap();
        close(p.max_watt_per_cab, 137.5);
        close(p.density_peak_wm2.unwrap(), 550.0);
        close(p.density_avg_wm2.unwrap(), 550.0 * 0.33);
        close(p.peak_kw, 6.875);
        close(p.peak_a, 50.0 * 137.5 / 230.0);
        close(p.avg_kw, 6.875 * 0.33);
        close(p.basis_kw, 2.26875);
        assert_eq!(p.cabinets_per_breaker, 64);
        assert_eq!((p.circuits, p.cables, p.chain_cap), (1, 5, 10));
        assert!(p.powercon_capped);
        close(p.usable_kw_per_breaker, 2.944);
        // line 460: 16 A feed 3.68 kW * 0.8 = 2.944 >= 2.26875
        assert_eq!(p.supply.name, "16 A 1-phase");
        close(p.supply.usable_kw, 2.944);
        assert!(p.supply.fits_one);
        assert_eq!(p.supply.a_per_phase, None);
    }

    #[test]
    fn power_max_basis_needs_three_phase() {
        // basis_w = 137.5; floor(2944/137.5) = 21; circuits ceil(50/21) = 3; basis_kw = 6.875
        // 16 A: 2.944 kW, 32 A: 5.888 kW (both < 6.875); 63 A 3-phase: 230*63*3/1000*0.8 = 34.776
        // per phase: 6875 / (3 * 230) = 9.9638 A (line 470)
        let r = ledwall(&LedWallInput {
            power_basis: PowerBasis::Max,
            ..wall()
        })
        .unwrap();
        let p = r.power.unwrap();
        close(p.basis_kw, 6.875);
        assert_eq!((p.cabinets_per_breaker, p.circuits, p.cables), (21, 3, 5));
        assert_eq!(p.supply.name, "63 A 3-phase");
        close(p.supply.usable_kw, 230.0 * 63.0 * 3.0 / 1000.0 * 0.8);
        close(p.supply.a_per_phase.unwrap(), 6875.0 / (3.0 * 230.0));
    }

    #[test]
    fn power_without_chain_cap_uses_breaker_limit() {
        // max_chain 0 = no powerCON cap -> chain = 64 cabinets, cables = ceil(50/64) = 1
        let p = ledwall(&LedWallInput {
            max_chain: 0,
            ..wall()
        })
        .unwrap()
        .power
        .unwrap();
        assert_eq!((p.chain_cap, p.cables, p.circuits), (64, 1, 1));
        assert!(!p.powercon_capped);
    }

    #[test]
    fn supply_larger_than_one_feed() {
        // 100 x 50 = 5000 cabinets: basis = 5000 * 45.375 / 1000 = 226.875 kW;
        // largest feed 125 A 3-phase = 86.25 kW * 0.8 = 69 -> ceil(226.875/69) = 4 (lines 473-475)
        let p = ledwall(&LedWallInput {
            cols: 100,
            rows: 50,
            ..wall()
        })
        .unwrap()
        .power
        .unwrap();
        assert_eq!(p.supply.name, "125 A 3-phase");
        assert_eq!(p.supply.count, 4);
        assert!(!p.supply.fits_one);
    }

    #[test]
    fn watt_precedence() {
        // lines 399-403: max_watt > max_wm2 x area > preset density x area
        let by_density = ledwall(&LedWallInput {
            max_wm2: 1000.0,
            ..wall()
        })
        .unwrap();
        close(by_density.power.unwrap().max_watt_per_cab, 250.0);
        let by_watt = ledwall(&LedWallInput {
            max_watt: 100.0,
            max_wm2: 1000.0,
            ..wall()
        })
        .unwrap();
        close(by_watt.power.unwrap().max_watt_per_cab, 100.0);
    }

    #[test]
    fn manual_cabinet_without_size_has_no_physical_or_power() {
        let r = ledwall(&manual()).unwrap();
        assert!(r.physical.is_none());
        assert!(r.power.is_none());
        assert!(r.weight.is_none());
        assert_eq!((r.resolution.width, r.resolution.height), (200, 200));
        assert_eq!(r.resolution.aspect, "1:1");
    }

    #[test]
    fn manual_watts_without_size_has_no_density() {
        let p = ledwall(&LedWallInput {
            max_watt: 200.0,
            ..manual()
        })
        .unwrap()
        .power
        .unwrap();
        close(p.max_watt_per_cab, 200.0);
        assert!(p.density_peak_wm2.is_none());
        close(p.peak_kw, 0.8);
    }

    #[test]
    fn signal_ports_and_cables() {
        // lines 480-489: cab_px = 36864; floor(650000/36864) = 17; ports = ceil(50/17) = 3
        let s = ledwall(&wall()).unwrap().signal;
        assert_eq!((s.cabinets_per_port, s.ports), (17, 3));
        assert_eq!((s.cat_cables, s.cat_cables_redundant), (3, 6));
        assert_eq!((s.recv_cards, s.recv_per_cab), (50, 1));
        let s = ledwall(&LedWallInput {
            redundant: true,
            ..wall()
        })
        .unwrap()
        .signal;
        assert_eq!(s.cat_cables, 6);
        assert_eq!(s.sender.need_ports, 6);
    }

    #[test]
    fn cabinet_bigger_than_a_port_still_gets_one_cabinet_per_port() {
        // line 481: floor(650000/1_000_000) = 0 -> 1
        let s = ledwall(&LedWallInput {
            cab_px_w: 1000,
            cab_px_h: 1000,
            ..manual()
        })
        .unwrap()
        .signal;
        assert_eq!((s.cabinets_per_port, s.ports), (1, 4));
    }

    #[test]
    fn sender_suggestions_per_series() {
        // needs 3 ports / 1.8432 MP: MCTRL300 has 2 ports; MCTRL660 (4 / 2.6) fits;
        // VX400 Pro (4 / 2.6) fits so H is not searched; no COEX card -> no COEX lookup
        let s = ledwall(&wall()).unwrap().signal.sender;
        close(s.need_mp, 1.8432);
        let sug = s.suggestions.unwrap();
        assert_eq!(sug.mctrl.unwrap().name, "MCTRL660");
        assert_eq!(sug.vx.unwrap().name, "VX400 Pro");
        assert!(sug.h.is_none() && sug.coex.is_none());
        assert!(s.selected.is_none());
    }

    #[test]
    fn sender_suggestions_for_a_huge_wall_fall_back_to_h_series() {
        // 5000 cabinets: 295 ports / 184.32 MP -> MCTRL and VX exceeded, H9 has 200 ports, H15 fits
        let sug = ledwall(&LedWallInput {
            cols: 100,
            rows: 50,
            ..wall()
        })
        .unwrap()
        .signal
        .sender
        .suggestions
        .unwrap();
        assert!(sug.mctrl.is_none());
        assert!(sug.vx.is_none());
        assert_eq!(sug.h.unwrap().name, "H15");
    }

    #[test]
    fn named_sender_ok_or_short() {
        // MCTRL4K: 16 ports / 8.8 MP covers 3 ports / 1.84 MP
        let ok = ledwall(&LedWallInput {
            sender: Some("mctrl4k".into()),
            ..wall()
        })
        .unwrap()
        .signal
        .sender;
        let sel = ok.selected.unwrap();
        assert!(sel.ok);
        assert!(sel.units.is_none() && sel.shortfall.is_none());
        assert!(ok.suggestions.is_none());

        // redundant needs 6 ports; MCTRL660 has 4 -> units = max(ceil(6/4), ceil(1.8432/2.6)) = 2
        let short = ledwall(&LedWallInput {
            sender: Some("MCTRL660".into()),
            redundant: true,
            ..wall()
        })
        .unwrap()
        .signal
        .sender;
        let sel = short.selected.unwrap();
        assert!(!sel.ok);
        assert_eq!(sel.units, Some(2));
        assert_eq!(sel.shortfall.as_deref(), Some("need 6 ports, has 4"));
    }

    #[test]
    fn named_sender_over_its_megapixel_limit() {
        // 100x50 on MCTRL300: 184.32 MP vs 1.3 MP -> over 1.3 MP; units = max(ceil(295/2) = 148, ceil(184.32/1.3) = 142)
        let sel = ledwall(&LedWallInput {
            cols: 100,
            rows: 50,
            sender: Some("MCTRL300".into()),
            ..wall()
        })
        .unwrap()
        .signal
        .sender
        .selected
        .unwrap();
        assert!(!sel.ok);
        assert_eq!(sel.shortfall.as_deref(), Some("over 1.3 MP"));
        assert_eq!(sel.units, Some(inner_units(184_320_000.0, 1.3e6, 295, 2)));
    }

    fn inner_units(total_px: f64, cap_px: f64, need_ports: u64, ports: u64) -> u64 {
        let by_mp = (total_px / cap_px).ceil() as u64;
        let by_ports = need_ports.div_ceil(ports);
        by_mp.max(by_ports)
    }

    #[test]
    fn unknown_sender_is_reported() {
        let s = ledwall(&LedWallInput {
            sender: Some("Foo".into()),
            ..wall()
        })
        .unwrap()
        .signal
        .sender;
        assert_eq!(s.unknown.as_deref(), Some("Foo"));
        assert!(s.selected.is_none());
    }

    #[test]
    fn coex_sender_with_legacy_card_is_flagged() {
        let s = ledwall(&LedWallInput {
            sender: Some("MX40 Pro".into()),
            recv_card: Some("A8s".into()),
            ..wall()
        })
        .unwrap()
        .signal;
        assert_eq!(s.sender.coex_incompatible_card.as_deref(), Some("A8s"));
        assert!(!s.sender.coex_needs_armor);
        let card = s.recv_card.unwrap();
        assert!(!card.coex);
        assert_eq!(card.px_cap, 1_024_000);
    }

    #[test]
    fn coex_sender_with_armor_card_only_notes_requirement() {
        let s = ledwall(&LedWallInput {
            sender: Some("MX40 Pro".into()),
            recv_card: Some("a10s pro".into()),
            ..wall()
        })
        .unwrap()
        .signal;
        assert!(s.sender.coex_incompatible_card.is_none());
        assert!(s.sender.coex_needs_armor);
        assert!(s.recv_card.unwrap().coex);
    }

    #[test]
    fn coex_card_adds_coex_suggestion() {
        // lines 550-552: with a COEX-capable card the COEX series is searched: MX40 Pro (20 / 9 MP)
        let sug = ledwall(&LedWallInput {
            recv_card: Some("A10s Pro".into()),
            ..wall()
        })
        .unwrap()
        .signal
        .sender
        .suggestions
        .unwrap();
        assert_eq!(sug.coex.unwrap().name, "MX40 Pro");
    }

    #[test]
    fn receiving_card_over_capacity_and_unknown_card() {
        // line 501: 1000 x 1100 px = 1.1 MP > A8s cap 1.024 MP
        let s = ledwall(&LedWallInput {
            cab_px_w: 1000,
            cab_px_h: 1100,
            recv_card: Some("A8s".into()),
            ..manual()
        })
        .unwrap()
        .signal;
        assert!(s.recv_card.unwrap().over_cap);
        let s = ledwall(&LedWallInput {
            recv_card: Some("zz".into()),
            ..manual()
        })
        .unwrap()
        .signal;
        assert!(s.recv_card.is_none());
        assert_eq!(s.unknown_recv_card.as_deref(), Some("zz"));
    }

    #[test]
    fn weight_totals() {
        // lines 559-562: 7.5 kg x 50 = 375; per hanging column 5 x 7.5 = 37.5
        let w = ledwall(&wall()).unwrap().weight.unwrap();
        close(w.total_kg, 375.0);
        close(w.per_column_kg, 37.5);
        assert_eq!(w.cabinets_high, 5);
    }

    #[test]
    fn explicit_values_override_the_preset() {
        let r = ledwall(&LedWallInput {
            cab_px_w: 100,
            cab_px_h: 50,
            weight_kg: 10.0,
            recv_per_cab: 3,
            env: Some("outdoor".into()),
            ..wall()
        })
        .unwrap();
        assert_eq!((r.cabinet_px_w, r.cabinet_px_h), (100, 50));
        close(r.weight.unwrap().total_kg, 500.0);
        assert_eq!(r.signal.recv_per_cab, 3);
        assert_eq!(r.physical.unwrap().env.as_deref(), Some("outdoor"));
    }

    #[test]
    fn group_map_vertical_serpentine() {
        // 3 cols x 2 rows, 3 per group: column 0 down (1,1), column 1 up (r1 = 1, r0 = 2), column 2 down (2,2)
        assert_eq!(
            led_group_map(3, 2, 3, Distribution::Vertical),
            vec![vec![1, 2, 2], vec![1, 1, 2]]
        );
    }

    #[test]
    fn group_map_horizontal_serpentine() {
        // row 0 left to right: ids 1,1,2; row 1 right to left: idx 3 -> 2, 4 -> 3, 5 -> 3
        assert_eq!(
            led_group_map(3, 2, 2, Distribution::Horizontal),
            vec![vec![1, 1, 2], vec![3, 3, 2]]
        );
    }

    #[test]
    fn group_map_zero_capacity_counts_as_one() {
        assert_eq!(
            led_group_map(2, 1, 0, Distribution::Horizontal),
            vec![vec![1, 2]]
        );
    }

    #[test]
    fn invalid_input() {
        let bad =
            |i: LedWallInput| assert!(matches!(ledwall(&i), Err(Error::InvalidInput(_))), "{i:?}");
        bad(LedWallInput { cols: 0, ..wall() });
        bad(LedWallInput { rows: 0, ..wall() });
        bad(LedWallInput {
            cols: MAX_WALL_SIDE + 1,
            ..wall()
        });
        bad(LedWallInput {
            cabinet: Some("nope".into()),
            ..wall()
        });
        // no preset and no per-cabinet resolution
        bad(LedWallInput {
            cabinet: None,
            ..wall()
        });
        bad(LedWallInput {
            voltage: 0.0,
            ..wall()
        });
        bad(LedWallInput {
            voltage: -230.0,
            ..wall()
        });
        bad(LedWallInput {
            breaker_a: 0.0,
            ..wall()
        });
        bad(LedWallInput {
            avg_factor: 0.0,
            ..wall()
        });
        bad(LedWallInput {
            px_per_port: 0.0,
            ..wall()
        });
        bad(LedWallInput {
            max_watt: -1.0,
            ..wall()
        });
        bad(LedWallInput {
            cab_wmm: f64::NAN,
            ..wall()
        });
        bad(LedWallInput {
            weight_kg: -5.0,
            ..wall()
        });
    }
}
