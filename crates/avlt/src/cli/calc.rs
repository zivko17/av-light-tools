//! `avlt calc ...`: AV / IT calculators.
//!
//! Option names are the PowerShell parameter names in kebab-case (`-CabPxW` -> `--cab-px-w`).
//! Core returns full-precision numbers; all rounding for display happens here.

use std::io::{self, Write};

use avlt_core::Error;
use avlt_core::Result;
use avlt_core::calc::{
    audiobw::{self, AudioBwInput},
    avoip::{self, AvoipInput},
    bitrate::{self, BitrateInput},
    brightness::{self, BrightnessContent, BrightnessInput},
    cable::{self, CableInput, CableRating, CableType},
    camera::{self, CameraInput},
    dsp::{self, DspInput},
    led_cabinets::{LedCabinet, LedTier, led_cabinets},
    ledwall::{self, Distribution, LedWallInput, LedWallResult, PowerBasis, led_group_map},
    pixelpitch::{self, PixelPitchInput},
    projector::{self, ProjectorInput},
    screen::{self, Content, ScreenInput, ScreenMode},
    speaker::{self, SpeakerInput},
    subnet,
    viewangle::{self, ViewAngleInput},
};
use clap::{Args, Subcommand, ValueEnum};

use super::output::{Out, exit_if_broken_pipe};

/// `avlt calc` subcommands.
#[derive(Debug, Subcommand)]
pub enum CalcCmd {
    /// IPv4 subnet calculator (CIDR)
    Subnet(SubnetArgs),
    /// Screen size from viewing distance (AVIXA 4/6/8:1), or the reverse
    Screen(ScreenArgs),
    /// LED wall size, resolution, power and NovaStar signal
    Ledwall(LedwallArgs),
    /// Uncompressed video bitrate, NDI estimate and storage
    Bitrate(BitrateArgs),
    /// AV-over-IP bandwidth per codec
    Avoip(AvoipArgs),
    /// Projector throw distance and brightness
    Projector(ProjectorArgs),
    /// DvLED pixel pitch and viewing distance
    Pixelpitch(PixelpitchArgs),
    /// Required display brightness versus ambient light
    Brightness(BrightnessArgs),
    /// Usable seating width from the off-axis viewing limit
    Viewangle(ViewangleArgs),
    /// Camera field of view and DORI distances
    Camera(CameraArgs),
    /// Digital audio data rate and wavelength
    Audiobw(AudiobwArgs),
    /// Ceiling speaker coverage and amplifier sizing
    Speaker(SpeakerArgs),
    /// Speaker wire loss and coax attenuation
    Cable(CableArgs),
    /// dBu conversion, latency budget and matrix size
    Dsp(DspArgs),
}

#[derive(Debug, Args)]
#[command(allow_negative_numbers = true)]
pub struct SubnetArgs {
    /// Address and prefix, e.g. 192.168.10.0/24
    cidr: String,
    /// Print an assignment table for this many devices
    #[arg(long, default_value_t = 0)]
    devices: u32,
    /// Host offset of the first device (1 = first host)
    #[arg(long, default_value_t = 1)]
    start_host: u32,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ContentArg {
    Detailed,
    Basic,
    Passive,
}

#[derive(Debug, Args)]
#[command(allow_negative_numbers = true)]
pub struct ScreenArgs {
    /// Farthest viewer distance (forward mode)
    distance: Option<f64>,
    /// Screen diagonal (reverse mode)
    #[arg(long, default_value_t = 0.0)]
    diagonal: f64,
    #[arg(long, value_enum, default_value_t = ContentArg::Basic)]
    content: ContentArg,
    /// Aspect ratio as w:h
    #[arg(long, default_value = "16:9")]
    aspect: String,
    /// Unit label (m or ft)
    #[arg(long, default_value = "m")]
    unit: String,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum PowerBasisArg {
    Avg,
    Max,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum DistributionArg {
    Vertical,
    Horizontal,
}

#[derive(Debug, Args)]
#[command(allow_negative_numbers = true)]
pub struct LedwallArgs {
    /// Cabinets wide
    cols: Option<u32>,
    /// Cabinets high
    rows: Option<u32>,
    /// Cabinet preset key (see --list)
    #[arg(long)]
    cabinet: Option<String>,
    /// Cabinet resolution, width in px
    #[arg(long, default_value_t = 0)]
    cab_px_w: u32,
    /// Cabinet resolution, height in px
    #[arg(long, default_value_t = 0)]
    cab_px_h: u32,
    /// Cabinet width in mm
    #[arg(long, default_value_t = 0.0)]
    cab_wmm: f64,
    /// Cabinet height in mm
    #[arg(long, default_value_t = 0.0)]
    cab_hmm: f64,
    /// Environment label (indoor / outdoor)
    #[arg(long)]
    env: Option<String>,
    /// Peak watts per cabinet
    #[arg(long, default_value_t = 0.0)]
    max_watt: f64,
    /// Peak power density in W/m^2
    #[arg(long, default_value_t = 0.0)]
    max_wm2: f64,
    /// Average draw as a fraction of peak
    #[arg(long, default_value_t = 0.33)]
    avg_factor: f64,
    /// Weight per cabinet in kg
    #[arg(long, default_value_t = 0.0)]
    weight_kg: f64,
    /// Brightness in nits
    #[arg(long, default_value_t = 0.0)]
    nits: f64,
    /// Receiving cards per cabinet
    #[arg(long, default_value_t = 0)]
    recv_per_cab: u32,
    /// Receiving card, e.g. "A10s Pro"
    #[arg(long)]
    recv_card: Option<String>,
    /// Sender you have, e.g. "VX1000 Pro"
    #[arg(long)]
    sender: Option<String>,
    /// Supply voltage
    #[arg(long, default_value_t = 230.0)]
    voltage: f64,
    /// Breaker rating in amps
    #[arg(long, default_value_t = 16.0)]
    breaker_a: f64,
    /// Maximum cabinets per powerCON daisy-chain (0 = no limit)
    #[arg(long, default_value_t = 10)]
    max_chain: u32,
    #[arg(long, value_enum, default_value_t = PowerBasisArg::Avg)]
    power_basis: PowerBasisArg,
    /// Pixels one data port carries (8-bit / 60 Hz)
    #[arg(long, default_value_t = 650_000.0)]
    px_per_port: f64,
    #[arg(long, value_enum, default_value_t = DistributionArg::Vertical)]
    distribution: DistributionArg,
    /// Redundant signal (main + backup loop)
    #[arg(long)]
    redundant: bool,
    /// Do not draw the signal and power maps
    #[arg(long)]
    no_diagram: bool,
    /// List the cabinet presets and exit
    #[arg(long)]
    list: bool,
}

#[derive(Debug, Args)]
#[command(allow_negative_numbers = true)]
pub struct BitrateArgs {
    /// Width in px
    width: u32,
    /// Height in px
    height: u32,
    #[arg(long, default_value_t = 25.0)]
    fps: f64,
    #[arg(long, default_value_t = 8)]
    bit_depth: u32,
    #[arg(long, default_value_t = 3)]
    channels: u32,
    /// Hours of footage for the storage figure
    #[arg(long, default_value_t = 1.0)]
    hours: f64,
}

#[derive(Debug, Args)]
#[command(allow_negative_numbers = true)]
pub struct AvoipArgs {
    /// Width in px
    width: u32,
    /// Height in px
    height: u32,
    #[arg(long, default_value_t = 60.0)]
    fps: f64,
    #[arg(long, default_value_t = 8)]
    bit_depth: u32,
}

#[derive(Debug, Args)]
#[command(allow_negative_numbers = true)]
pub struct ProjectorArgs {
    /// Image width in metres
    #[arg(long, default_value_t = 0.0)]
    width: f64,
    /// Image diagonal in metres (with --aspect)
    #[arg(long, default_value_t = 0.0)]
    diagonal: f64,
    #[arg(long, default_value = "16:9")]
    aspect: String,
    /// Throw ratio -> distance
    #[arg(long, default_value_t = 0.0)]
    throw_ratio: f64,
    /// Distance in metres -> throw ratio
    #[arg(long, default_value_t = 0.0)]
    distance: f64,
    /// Screen gain
    #[arg(long, default_value_t = 1.0)]
    gain: f64,
    /// Target foot-lamberts (16 dim, 30-50 with ambient light)
    #[arg(long, default_value_t = 16.0)]
    target_fl: f64,
}

#[derive(Debug, Args)]
#[command(allow_negative_numbers = true)]
pub struct PixelpitchArgs {
    /// Pixel pitch in mm
    #[arg(long, default_value_t = 0.0)]
    pitch: f64,
    /// Viewing distance in metres (recommends a pitch)
    #[arg(long, default_value_t = 0.0)]
    distance: f64,
    /// Wall width in metres
    #[arg(long, default_value_t = 0.0)]
    width_m: f64,
    /// Wall height in metres
    #[arg(long, default_value_t = 0.0)]
    height_m: f64,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum BrightnessContentArg {
    Basic,
    Detailed,
}

#[derive(Debug, Args)]
#[command(allow_negative_numbers = true)]
pub struct BrightnessArgs {
    /// Ambient light in lux
    ambient_lux: f64,
    /// Display diffuse reflectance (0-1)
    #[arg(long, default_value_t = 0.15)]
    reflectance: f64,
    #[arg(long, value_enum, default_value_t = BrightnessContentArg::Basic)]
    content: BrightnessContentArg,
}

#[derive(Debug, Args)]
#[command(allow_negative_numbers = true)]
pub struct ViewangleArgs {
    /// Viewer distance in metres
    distance: f64,
    /// Maximum off-axis half-angle in degrees
    #[arg(long, default_value_t = 45.0)]
    max_angle: f64,
    /// Screen width in metres (for the edge angle)
    #[arg(long, default_value_t = 0.0)]
    screen_width: f64,
}

#[derive(Debug, Args)]
#[command(allow_negative_numbers = true)]
pub struct CameraArgs {
    /// Focal length in mm
    focal_length: f64,
    /// Sensor width in mm (36 full-frame, 5.37 for 1/2.8")
    #[arg(long, default_value_t = 36.0)]
    sensor_width: f64,
    /// Horizontal resolution in px
    #[arg(long, default_value_t = 1920)]
    h_resolution: u32,
    /// Distance in metres for the pixel density
    #[arg(long, default_value_t = 0.0)]
    distance: f64,
}

#[derive(Debug, Args)]
#[command(allow_negative_numbers = true)]
pub struct AudiobwArgs {
    #[arg(long, default_value_t = 64)]
    channels: u32,
    #[arg(long, default_value_t = 48_000)]
    sample_rate: u32,
    #[arg(long, default_value_t = 24)]
    bit_depth: u32,
    /// Frequency in Hz for the wavelength
    #[arg(long, default_value_t = 0.0)]
    freq: f64,
}

#[derive(Debug, Args)]
#[command(allow_negative_numbers = true)]
pub struct SpeakerArgs {
    /// Ceiling height in metres
    #[arg(long, default_value_t = 0.0)]
    ceiling_height: f64,
    /// Listener ear height in metres
    #[arg(long, default_value_t = 1.2)]
    ear_height: f64,
    /// Speaker -6 dB coverage angle in degrees
    #[arg(long, default_value_t = 90.0)]
    coverage_angle: f64,
    #[arg(long, default_value_t = 0.0)]
    room_width: f64,
    #[arg(long, default_value_t = 0.0)]
    room_length: f64,
    /// Speaker count (amp mode)
    #[arg(long, default_value_t = 0)]
    speakers: u32,
    #[arg(long, default_value_t = 0.0)]
    watts_each: f64,
    #[arg(long, default_value_t = 6.0)]
    headroom_db: f64,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CableTypeArg {
    Speaker,
    Coax,
}

#[derive(Debug, Args)]
#[command(allow_negative_numbers = true)]
pub struct CableArgs {
    #[arg(long = "type", value_enum, default_value_t = CableTypeArg::Speaker)]
    kind: CableTypeArg,
    /// Wire gauge: 10, 12, 14, 16, 18 or 20
    #[arg(long, default_value_t = 14)]
    awg: u32,
    /// Cable length in metres
    #[arg(long, default_value_t = 0.0)]
    length_m: f64,
    /// Speaker load in ohms
    #[arg(long, default_value_t = 8.0)]
    impedance: f64,
    /// Coax attenuation in dB per 100 m
    #[arg(long, default_value_t = 0.0)]
    atten_per_100m: f64,
    /// Coax attenuation budget in dB (gives the maximum length)
    #[arg(long, default_value_t = 0.0)]
    max_db: f64,
}

#[derive(Debug, Args)]
#[command(allow_negative_numbers = true)]
pub struct DspArgs {
    /// Level to convert, in dBu
    #[arg(long)]
    dbu: Option<f64>,
    /// Latency chain to sum, in ms (comma separated)
    #[arg(long, value_delimiter = ',')]
    latencies_ms: Vec<f64>,
    /// Matrix inputs
    #[arg(long, default_value_t = 0)]
    inputs: u32,
    /// Matrix outputs
    #[arg(long, default_value_t = 0)]
    outputs: u32,
}

/// Runs an `avlt calc` subcommand.
pub fn run(cmd: CalcCmd, out: &Out) -> Result<()> {
    match cmd {
        CalcCmd::Subnet(a) => run_subnet(&a, out),
        CalcCmd::Screen(a) => run_screen(&a, out),
        CalcCmd::Ledwall(a) => run_ledwall(&a, out),
        CalcCmd::Bitrate(a) => run_bitrate(&a, out),
        CalcCmd::Avoip(a) => run_avoip(&a, out),
        CalcCmd::Projector(a) => run_projector(&a, out),
        CalcCmd::Pixelpitch(a) => run_pixelpitch(&a, out),
        CalcCmd::Brightness(a) => run_brightness(&a, out),
        CalcCmd::Viewangle(a) => run_viewangle(&a, out),
        CalcCmd::Camera(a) => run_camera(&a, out),
        CalcCmd::Audiobw(a) => run_audiobw(&a, out),
        CalcCmd::Speaker(a) => run_speaker(&a, out),
        CalcCmd::Cable(a) => run_cable(&a, out),
        CalcCmd::Dsp(a) => run_dsp(&a, out),
    }
}

// ---- presentation helpers ----

/// Formats `v` with `decimals` places and thousands separators (like PowerShell's `{0:N2}`).
fn n(v: f64, decimals: usize) -> String {
    let s = format!("{v:.decimals$}");
    let (int_part, frac) = match s.split_once('.') {
        Some((i, f)) => (i, Some(f)),
        None => (s.as_str(), None),
    };
    let (sign, digits) = match int_part.strip_prefix('-') {
        Some(d) => ("-", d),
        None => ("", int_part),
    };
    let mut grouped = String::new();
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    match frac {
        Some(f) => format!("{sign}{grouped}.{f}"),
        None => format!("{sign}{grouped}"),
    }
}

/// Prints a raw stdout line (text mode only; the caller runs inside `Out::emit`).
fn raw(s: &str) {
    if let Err(e) = writeln!(io::stdout().lock(), "{s}") {
        exit_if_broken_pipe(&e);
    }
}

/// Blank line, then a section title.
fn head(out: &Out, s: &str) {
    raw("");
    out.title(s);
}

fn dim(out: &Out, s: &str) {
    if out.color {
        raw(&format!("\x1b[2m{s}\x1b[0m"));
    } else {
        raw(s);
    }
}

// ---- subnet ----

fn run_subnet(a: &SubnetArgs, out: &Out) -> Result<()> {
    let r = subnet::subnet(&a.cidr, a.devices, a.start_host)?;
    out.emit(&r, |r, out| {
        head(out, "=== SUBNET ===");
        out.field("CIDR:", &r.cidr);
        out.field("Netmask:", &r.netmask);
        out.field("Wildcard:", &r.wildcard);
        out.field("Network:", &r.network);
        out.field("Broadcast:", &r.broadcast);
        if r.prefix >= 31 {
            out.field(
                "Usable:",
                &format!(
                    "{} - {}  ({} addresses)",
                    r.first_usable,
                    r.last_usable,
                    n(r.total_addresses as f64, 0)
                ),
            );
        } else {
            out.field(
                "Usable:",
                &format!("{} - {}", r.first_usable, r.last_usable),
            );
            out.field("Hosts:", &n(r.hosts as f64, 0));
        }
        if r.devices > 0 {
            head(
                out,
                &format!(
                    "--- Assignment ({} devices from host {}) ---",
                    r.devices, r.start_host
                ),
            );
            for (i, addr) in r.assignments.iter().enumerate() {
                raw(&format!("    Device {:<4} {addr}", i + 1));
            }
            if r.assignments_truncated {
                out.warn(&format!(
                    "exceeds usable range at device {}",
                    r.assignments.len() + 1
                ));
            }
        }
    });
    Ok(())
}

// ---- screen ----

fn run_screen(a: &ScreenArgs, out: &Out) -> Result<()> {
    let input = ScreenInput {
        distance: a.distance.unwrap_or(0.0),
        diagonal: a.diagonal,
        content: match a.content {
            ContentArg::Detailed => Content::Detailed,
            ContentArg::Basic => Content::Basic,
            ContentArg::Passive => Content::Passive,
        },
        aspect: a.aspect.clone(),
        unit: a.unit.clone(),
    };
    let r = screen::screen(&input)?;
    out.emit(&r, |r, out| {
        let name = match r.content {
            Content::Detailed => "detailed",
            Content::Basic => "basic",
            Content::Passive => "passive",
        };
        head(
            out,
            &format!(
                "=== SCREEN SIZE (AVIXA {}:1 guideline, {name}) ===",
                r.ratio
            ),
        );
        let u = &r.unit;
        match r.mode {
            ScreenMode::Reverse => {
                out.field("Diagonal:", &format!("{} {u}", r.diagonal));
                out.field("Image height:", &format!("{} {u}", n(r.height, 2)));
                out.field("Image width:", &format!("{} {u}", n(r.width, 2)));
                out.field(
                    "Max view distance:",
                    &format!("{} {u}   (farthest viewer)", n(r.view_distance, 2)),
                );
            }
            ScreenMode::Forward => {
                out.field("Farthest viewer:", &format!("{} {u}", r.view_distance));
                out.field("Min image height:", &format!("{} {u}", n(r.height, 2)));
                out.field("Min image width:", &format!("{} {u}", n(r.width, 2)));
                let inches = r.diagonal_in.unwrap_or(0.0);
                out.field(
                    "Min diagonal:",
                    &format!("{} {u}  (~{} in)", n(r.diagonal, 2), n(inches, 0)),
                );
            }
        }
        dim(
            out,
            "  (Guideline only - verify with full AVIXA DISCAS for critical designs.)",
        );
    });
    Ok(())
}

// ---- ledwall ----

fn run_ledwall(a: &LedwallArgs, out: &Out) -> Result<()> {
    if a.list {
        out.emit(&led_cabinets(), print_cabinet_list);
        return Ok(());
    }
    let (Some(cols), Some(rows)) = (a.cols, a.rows) else {
        return Err(Error::InvalidInput(
            "usage: calc ledwall <cols> <rows> --cabinet <preset> [--redundant] [--distribution horizontal]\n       manual: calc ledwall 10 5 --cab-px-w 168 --cab-px-h 168 --cab-wmm 500 --cab-hmm 500 --max-watt 190\n       list:   calc ledwall --list"
                .into(),
        ));
    };
    let input = LedWallInput {
        cols,
        rows,
        cabinet: a.cabinet.clone(),
        cab_px_w: a.cab_px_w,
        cab_px_h: a.cab_px_h,
        cab_wmm: a.cab_wmm,
        cab_hmm: a.cab_hmm,
        env: a.env.clone(),
        max_watt: a.max_watt,
        max_wm2: a.max_wm2,
        avg_factor: a.avg_factor,
        weight_kg: a.weight_kg,
        nits: a.nits,
        recv_per_cab: a.recv_per_cab,
        recv_card: a.recv_card.clone(),
        sender: a.sender.clone(),
        voltage: a.voltage,
        breaker_a: a.breaker_a,
        max_chain: a.max_chain,
        power_basis: match a.power_basis {
            PowerBasisArg::Avg => PowerBasis::Avg,
            PowerBasisArg::Max => PowerBasis::Max,
        },
        px_per_port: a.px_per_port,
        distribution: match a.distribution {
            DistributionArg::Vertical => Distribution::Vertical,
            DistributionArg::Horizontal => Distribution::Horizontal,
        },
        redundant: a.redundant,
    };
    let r = ledwall::ledwall(&input)?;
    out.emit(&r, |r, out| render_ledwall(r, out, a.no_diagram));
    Ok(())
}

fn print_cabinet_list(cabinets: &&[LedCabinet], out: &Out) {
    head(out, "=== LED CABINET PRESETS ===");
    for (tier, title) in [
        (LedTier::Generic, "Generic (env/pitch/size)"),
        (LedTier::Brand, "Brands (from manufacturer datasheets)"),
    ] {
        out.title(&format!("  --- {title} ---"));
        for c in cabinets.iter().filter(|c| c.tier == tier) {
            let name = match &c.model {
                Some(m) => format!("{} {m}", c.brand),
                None => String::new(),
            };
            raw(&format!(
                "    {:<16} P{:<4} {:>4}x{:<4}px {:<10} {:<8} {:>5} nits {:>4} W/m2 {:>5}kg  {}",
                c.key,
                c.pitch,
                c.px_w,
                c.px_h,
                format!("{}x{}mm", c.w_mm, c.h_mm),
                c.env.as_str(),
                c.nits,
                c.max_w_m2,
                c.kg,
                name
            ));
        }
    }
    raw("");
    dim(
        out,
        "  Use:  calc ledwall <cols> <rows> --cabinet <key> [--redundant] [--recv-card A8s] [--sender \"VX600 Pro\"]",
    );
}

/// What a group map draws.
struct MapSpec<'a> {
    title: &'a str,
    prefix: &'a str,
    cols: u32,
    rows: u32,
    cap: u32,
    groups: u64,
    distribution: Distribution,
    note: &'a str,
}

/// Background colour per group id (dark cyan, green, yellow, magenta, blue, red, blue, gray).
const MAP_BG: [&str; 8] = [
    "\x1b[46m",
    "\x1b[42m",
    "\x1b[43m",
    "\x1b[45m",
    "\x1b[44m",
    "\x1b[41m",
    "\x1b[104m",
    "\x1b[100m",
];

/// Draws the cabinet grid as framed cells (coloured when stdout is a terminal), or a
/// per-group count if the wall is too large to draw.
fn draw_map(out: &Out, m: &MapSpec<'_>) {
    out.title(&format!("  --- {} ---", m.title));
    if m.cols <= 20 && m.rows <= 18 {
        let map = led_group_map(m.cols, m.rows, m.cap, m.distribution);
        let seg = "\u{2500}".repeat(4);
        let cols = m.cols as usize;
        raw(&format!(
            "  \u{250C}{}{seg}\u{2510}",
            format!("{seg}\u{252C}").repeat(cols.saturating_sub(1))
        ));
        for row in &map {
            let mut line = String::from("  \u{2502}");
            for &id in row {
                let cell = format!(" {}{:>2}", m.prefix, id);
                if out.color {
                    let bg = MAP_BG[(usize::from(id).saturating_sub(1)) % MAP_BG.len()];
                    line.push_str(&format!("{bg}\x1b[97m{cell}\x1b[0m"));
                } else {
                    line.push_str(&cell);
                }
                line.push('\u{2502}');
            }
            raw(&line);
        }
        raw(&format!(
            "  \u{2514}{}{seg}\u{2518}",
            format!("{seg}\u{2534}").repeat(cols.saturating_sub(1))
        ));
    } else {
        // too large to draw: group id + cabinet count per group (full chunks, then the rest)
        let total = u64::from(m.cols) * u64::from(m.rows);
        let cap = u64::from(m.cap.max(1));
        for gid in 1..=m.groups {
            let count = cap.min(total - (gid - 1) * cap);
            let chip = format!(" {}{:>2} ", m.prefix, gid);
            if out.color {
                let bg = MAP_BG[((gid - 1) as usize) % MAP_BG.len()];
                raw(&format!("    {bg}\x1b[97m{chip}\x1b[0m  {count} cabinets"));
            } else {
                raw(&format!("    {chip}  {count} cabinets"));
            }
        }
    }
    if !m.note.is_empty() {
        dim(out, &format!("  {}", m.note));
    }
}

fn render_ledwall(r: &LedWallResult, out: &Out, no_diagram: bool) {
    let mut hdr = format!("{} x {} = {} cabinets", r.cols, r.rows, r.total_cabinets);
    match (&r.model, &r.cabinet) {
        (Some(model), _) => hdr = format!("{hdr}  |  {} {model}", r.brand.as_deref().unwrap_or("")),
        (None, Some(key)) => hdr = format!("{hdr}  |  {key}"),
        _ => {}
    }
    head(out, &format!("=== LED WALL ({hdr}) ==="));

    out.title("  --- Resolution ---");
    out.field(
        "Real resolution:",
        &format!(
            "{} x {} px   ({} MP)",
            r.resolution.width,
            r.resolution.height,
            n(r.resolution.megapixels, 2)
        ),
    );
    out.field("Aspect ratio:", &r.resolution.aspect);

    if let Some(p) = &r.physical {
        out.title("  --- Physical ---");
        out.field(
            "Size:",
            &format!(
                "{} x {} m   ({} m^2)",
                n(p.width_m, 2),
                n(p.height_m, 2),
                n(p.area_m2, 2)
            ),
        );
        out.field("Pixel pitch:", &format!("{} mm", n(p.pitch_mm, 3)));
        out.field(
            "Min view dist:",
            &format!("{} m   (retina, ~pitch x 3.4)", n(p.min_view_dist_m, 2)),
        );
        if let Some(nits) = p.nits {
            let env = p
                .env
                .as_deref()
                .map(|e| format!("  ({e})"))
                .unwrap_or_default();
            out.field("Brightness:", &format!("{} nits{env}", n(nits, 0)));
        }
    }

    if let Some(p) = &r.power {
        let basis = match p.basis {
            PowerBasis::Avg => "avg",
            PowerBasis::Max => "max",
        };
        out.title("  --- Power ---");
        if let (Some(peak), Some(avg)) = (p.density_peak_wm2, p.density_avg_wm2) {
            out.field(
                "Density:",
                &format!(
                    "{} W/m^2 peak / {} W/m^2 avg   ({} W/cab peak)",
                    n(peak, 0),
                    n(avg, 0),
                    n(p.max_watt_per_cab, 0)
                ),
            );
        }
        out.field(
            "Peak draw:",
            &format!(
                "{} kW   ({} A @ {} V, all-white)",
                n(p.peak_kw, 2),
                n(p.peak_a, 1),
                n(p.voltage, 0)
            ),
        );
        out.field(
            "Avg draw:",
            &format!(
                "{} kW   (~{:.0}% of peak, real content)",
                n(p.avg_kw, 2),
                p.avg_factor * 100.0
            ),
        );
        out.field(
            "Sizing basis:",
            &format!("{basis} draw = {} kW", n(p.basis_kw, 2)),
        );
        out.field(
            "Circuits:",
            &format!(
                "{} x {} A   (up to {} cabinets/breaker, {} kW usable)",
                p.circuits,
                n(p.breaker_a, 0),
                p.cabinets_per_breaker,
                n(p.usable_kw_per_breaker, 2)
            ),
        );
        let cap_note = if p.powercon_capped {
            " (powerCON cap)"
        } else {
            ""
        };
        out.field(
            "Power cables:",
            &format!(
                "{}   (max {} cabinets/daisy-chain{cap_note})",
                p.cables, p.chain_cap
            ),
        );
        let s = &p.supply;
        if s.fits_one {
            let ph = s
                .a_per_phase
                .map(|a| format!("  ({} A/phase)", n(a, 1)))
                .unwrap_or_default();
            out.field(
                "Supply / distro:",
                &format!("{}   (~{} kW usable){ph}", s.name, n(s.usable_kw, 1)),
            );
        } else {
            out.field(
                "Supply / distro:",
                &format!(
                    "{} x {}   ({basis} load {} kW exceeds one feed)",
                    s.count,
                    s.name,
                    n(p.basis_kw, 1)
                ),
            );
        }
    }

    let sig = &r.signal;
    out.title("  --- Signal (NovaStar) ---");
    out.field(
        "Data ports:",
        &format!(
            "{}   ({} px/port @ 8-bit/60Hz, ~{} cab/port)",
            sig.ports,
            n(sig.px_per_port, 0),
            sig.cabinets_per_port
        ),
    );
    if sig.redundant {
        out.field(
            "Cat cables:",
            &format!(
                "{}   (redundant: {} main + {} backup)",
                sig.cat_cables, sig.ports, sig.ports
            ),
        );
    } else {
        out.field(
            "Cat cables:",
            &format!(
                "{}   (single path; x2 = {} for redundancy)",
                sig.cat_cables, sig.cat_cables_redundant
            ),
        );
    }
    match (&sig.recv_card, &sig.unknown_recv_card) {
        (Some(c), _) => {
            let tag = if c.coex { " [COEX-capable]" } else { "" };
            out.field(
                "Receiving cards:",
                &format!(
                    "{} x {}{tag}   ({}/cabinet)",
                    sig.recv_cards, c.name, sig.recv_per_cab
                ),
            );
            if c.over_cap {
                out.warn(&format!(
                    "cabinet {} px > {} cap ~{} px - needs >1 card/cabinet",
                    n(c.cabinet_px, 0),
                    c.name,
                    n(f64::from(c.px_cap), 0)
                ));
            }
        }
        (None, unknown) => {
            out.field(
                "Receiving cards:",
                &format!("{}   ({}/cabinet)", sig.recv_cards, sig.recv_per_cab),
            );
            if let Some(name) = unknown {
                let known: Vec<&str> = avlt_core::calc::ledwall::NOVA_RECV_CARDS
                    .iter()
                    .map(|c| c.name)
                    .collect();
                dim(
                    out,
                    &format!(
                        "                    (unknown card '{name}'; known: {})",
                        known.join(", ")
                    ),
                );
            }
        }
    }

    let s = &sig.sender;
    let need = if s.redundant {
        format!(" (redundant: 2x = {} ports)", s.need_ports)
    } else {
        String::new()
    };
    if let Some(sel) = &s.selected {
        if sel.ok {
            out.field(
                "Sender:",
                &format!(
                    "{} OK   ({} ports / {} MP; need {} ports / {} MP){need}",
                    sel.name,
                    sel.ports,
                    sel.max_mp,
                    s.need_ports,
                    n(s.need_mp, 2)
                ),
            );
        } else {
            out.field(
                "Sender:",
                &format!(
                    "{} short ({}) - ~{} units{need}",
                    sel.name,
                    sel.shortfall.as_deref().unwrap_or(""),
                    sel.units.unwrap_or(0)
                ),
            );
        }
    } else if let Some(name) = &s.unknown {
        let known: Vec<&str> = avlt_core::calc::ledwall::NOVA_SENDERS
            .iter()
            .map(|c| c.name)
            .collect();
        out.field(
            "Sender:",
            &format!("unknown '{name}'; known: {}", known.join(", ")),
        );
    } else {
        out.field(
            "Sender needs:",
            &format!("{} ports / {} MP{need}", s.need_ports, n(s.need_mp, 2)),
        );
        if let Some(sug) = &s.suggestions {
            match &sug.mctrl {
                Some(k) => raw(&format!(
                    "    MCTRL series:   {}  ({} ports, {} MP)",
                    k.name, k.ports, k.max_mp
                )),
                None => dim(
                    out,
                    "    MCTRL series:   exceeds series (max MCTRL4K: 16 ports / 8.8 MP)",
                ),
            }
            match (&sug.vx, &sug.h) {
                (Some(k), _) => raw(&format!(
                    "    VX series:      {}  ({} ports, {} MP)",
                    k.name, k.ports, k.max_mp
                )),
                (None, Some(h)) => raw(&format!(
                    "    VX -> H series: over VX2000 Pro (13 MP) - use {} ({} MP)",
                    h.name, h.max_mp
                )),
                (None, None) => {
                    raw("    VX -> H series: over a single H15 - multiple H-series units")
                }
            }
            if let Some(k) = &sug.coex {
                let card = sig
                    .recv_card
                    .as_ref()
                    .map(|c| c.name.as_str())
                    .unwrap_or("");
                raw(&format!(
                    "    COEX series:    {}  ({} ports, {} MP)  [{card} is COEX-capable]",
                    k.name, k.ports, k.max_mp
                ));
            } else if !sig.recv_card.as_ref().is_some_and(|c| c.coex) {
                dim(
                    out,
                    "    COEX series:    possible if receiving cards are A10s Pro / Armor (--recv-card 'A10s Pro')",
                );
            }
        }
    }
    if let (Some(sel), Some(card)) = (&s.selected, &s.coex_incompatible_card) {
        out.warn(&format!(
            "{} (COEX) needs A10s Pro / Armor cards; '{card}' is not compatible",
            sel.name
        ));
    } else if s.coex_needs_armor {
        dim(
            out,
            "                    (COEX/MX requires A10s Pro / Armor receiving cards)",
        );
    }

    if let Some(w) = &r.weight {
        out.title("  --- Weight ---");
        out.field("Total:", &format!("{} kg", n(w.total_kg, 0)));
        out.field(
            "Per hanging col:",
            &format!(
                "{} kg   ({} cabinets high)",
                n(w.per_column_kg, 0),
                w.cabinets_high
            ),
        );
    }

    if !no_diagram {
        raw("");
        let dist = match r.distribution {
            Distribution::Vertical => "vertical",
            Distribution::Horizontal => "horizontal",
        };
        let mut note = format!("cell colour = data port (P#); {dist} serpentine wiring");
        if sig.redundant {
            note.push_str("; x2 cables for backup loop");
        }
        draw_map(
            out,
            &MapSpec {
                title: &format!("Signal map (NovaStar ports, {dist})"),
                prefix: "P",
                cols: r.cols,
                rows: r.rows,
                cap: sig.cabinets_per_port,
                groups: sig.ports,
                distribution: r.distribution,
                note: &note,
            },
        );
        if let Some(p) = &r.power {
            raw("");
            draw_map(
                out,
                &MapSpec {
                    title: &format!("Power map (daisy-chains, {dist})"),
                    prefix: "C",
                    cols: r.cols,
                    rows: r.rows,
                    cap: u32::try_from(p.chain_cap).unwrap_or(u32::MAX),
                    groups: p.cables,
                    distribution: r.distribution,
                    note: &format!(
                        "cell colour = power cable / daisy-chain (C#), max {} cabinets",
                        p.chain_cap
                    ),
                },
            );
        }
    }
    dim(
        out,
        "  (Brand presets = datasheet values; generic = typical medians. Verify your exact panel/revision.)",
    );
}

// ---- bitrate / avoip ----

fn run_bitrate(a: &BitrateArgs, out: &Out) -> Result<()> {
    let r = bitrate::bitrate(&BitrateInput {
        width: a.width,
        height: a.height,
        fps: a.fps,
        bit_depth: a.bit_depth,
        channels: a.channels,
        hours: a.hours,
    })?;
    out.emit(&r, |r, out| {
        head(
            out,
            &format!(
                "=== VIDEO BITRATE ({} x {} @ {}fps, {}-bit, {}ch) ===",
                a.width, a.height, a.fps, a.bit_depth, a.channels
            ),
        );
        out.field(
            "Uncompressed:",
            &format!(
                "{} Mbps   ({} GB/hour)",
                n(r.raw_mbps, 0),
                n(r.raw_gb_per_hour, 1)
            ),
        );
        out.field(
            "NDI High BW:",
            &format!(
                "~{} Mbps   (~{} GB/hour)",
                n(r.ndi_hb_mbps, 0),
                n(r.ndi_hb_gb_per_hour, 1)
            ),
        );
        out.field(
            "NDI|HX:",
            &format!("~{} Mbps   (H.264/5)", n(r.ndi_hx_mbps, 0)),
        );
        if (r.hours - 1.0).abs() > f64::EPSILON {
            out.field(
                &format!("Uncompressed x {}h:", r.hours),
                &format!("{} GB", n(r.raw_gb_total, 1)),
            );
        }
        dim(
            out,
            "  (NDI figures are estimates; real bitrate varies with content.)",
        );
    });
    Ok(())
}

fn run_avoip(a: &AvoipArgs, out: &Out) -> Result<()> {
    let r = avoip::avoip(&AvoipInput {
        width: a.width,
        height: a.height,
        fps: a.fps,
        bit_depth: a.bit_depth,
    })?;
    out.emit(&r, |r, out| {
        head(
            out,
            &format!(
                "=== AVoIP BANDWIDTH ({}x{}@{}) ===",
                a.width, a.height, a.fps
            ),
        );
        out.field("Uncompressed:", &format!("{} Mbps", n(r.raw_mbps, 0)));
        out.field(
            "SDVoE (10GbE):",
            &format!(
                "visually lossless, ~{} Mbps class",
                n(r.sdvoe_class_mbps, 0)
            ),
        );
        out.field(
            "JPEG-XS:",
            &format!("~{} Mbps  (light compression)", n(r.jpeg_xs_mbps, 0)),
        );
        out.field("NDI High BW:", &format!("~{} Mbps", n(r.ndi_hb_mbps, 0)));
        out.field(
            "NDI|HX / H.26x:",
            &format!("~{} Mbps  (heavy compression)", n(r.ndi_hx_mbps, 0)),
        );
        dim(
            out,
            "  (Estimates; SDVoE is a fixed 10G class, others scale with pixels x fps.)",
        );
    });
    Ok(())
}

// ---- projector / pixel pitch / brightness ----

fn run_projector(a: &ProjectorArgs, out: &Out) -> Result<()> {
    let r = projector::projector(&ProjectorInput {
        width: a.width,
        diagonal: a.diagonal,
        aspect: a.aspect.clone(),
        throw_ratio: a.throw_ratio,
        distance: a.distance,
        gain: a.gain,
        target_fl: a.target_fl,
    })?;
    out.emit(&r, |r, out| {
        head(out, "=== PROJECTOR ===");
        out.field(
            "Image:",
            &format!(
                "{} x {} m  ({} ft^2)",
                n(r.width_m, 2),
                n(r.height_m, 2),
                n(r.area_ft2, 1)
            ),
        );
        if let Some(d) = r.throw_distance_m {
            out.field(
                "Throw distance:",
                &format!("{} m   (ratio {})", n(d, 2), a.throw_ratio),
            );
        } else if let Some(t) = r.throw_ratio {
            out.field(
                "Throw ratio:",
                &format!("{}   (at {} m)", n(t, 2), a.distance),
            );
        }
        out.field(
            "Brightness:",
            &format!(
                "~{} ANSI lumens   (target {} fL, gain {})",
                n(r.lumens, 0),
                r.target_fl,
                r.gain
            ),
        );
        dim(
            out,
            "  (Brightness is a guideline; raise target fL for high ambient light.)",
        );
    });
    Ok(())
}

fn run_pixelpitch(a: &PixelpitchArgs, out: &Out) -> Result<()> {
    let r = pixelpitch::pixelpitch(&PixelPitchInput {
        pitch: a.pitch,
        distance: a.distance,
        width_m: a.width_m,
        height_m: a.height_m,
    })?;
    out.emit(&r, |r, out| {
        head(out, "=== DvLED PIXEL PITCH ===");
        if let (Some(d), Some(p)) = (r.distance_m, r.max_pitch_mm) {
            out.field("Viewing distance:", &format!("{d} m"));
            out.field(
                "Recommended max pitch:",
                &format!("{} mm   (retina, ~pitch x 3.44 m)", n(p, 2)),
            );
        }
        if let (Some(p), Some(d)) = (r.pitch_mm, r.min_distance_m) {
            out.field("Pixel pitch:", &format!("{p} mm"));
            out.field(
                "Optimal min dist:",
                &format!("{} m   (retina, 1px/arcmin)", n(d, 2)),
            );
        }
        if let Some(res) = &r.resolution {
            out.field(
                "Resolution:",
                &format!(
                    "{} x {} px  ({} MP)",
                    res.width,
                    res.height,
                    n(res.megapixels, 1)
                ),
            );
        }
    });
    Ok(())
}

fn run_brightness(a: &BrightnessArgs, out: &Out) -> Result<()> {
    let r = brightness::brightness(&BrightnessInput {
        ambient_lux: a.ambient_lux,
        reflectance: a.reflectance,
        content: match a.content {
            BrightnessContentArg::Basic => BrightnessContent::Basic,
            BrightnessContentArg::Detailed => BrightnessContent::Detailed,
        },
    })?;
    out.emit(&r, |r, out| {
        let name = match r.content {
            BrightnessContent::Basic => "basic",
            BrightnessContent::Detailed => "detailed",
        };
        head(
            out,
            &format!(
                "=== DISPLAY BRIGHTNESS (guideline, {name} {}:1) ===",
                r.contrast_ratio
            ),
        );
        out.field("Ambient:", &format!("{} lux", a.ambient_lux));
        out.field(
            "Reflected light:",
            &format!("{} nits", n(r.reflected_nits, 1)),
        );
        out.field(
            "Required display:",
            &format!(
                "~{} nits   (for {}:1 contrast)",
                n(r.required_nits, 0),
                r.contrast_ratio
            ),
        );
        dim(
            out,
            "  (Estimate from ambient reflection; verify with AVIXA ambient-light method.)",
        );
    });
    Ok(())
}

// ---- viewing angle / camera ----

fn run_viewangle(a: &ViewangleArgs, out: &Out) -> Result<()> {
    let r = viewangle::viewangle(&ViewAngleInput {
        distance: a.distance,
        max_angle: a.max_angle,
        screen_width: a.screen_width,
    })?;
    out.emit(&r, |r, out| {
        head(
            out,
            &format!(
                "=== VIEWING ANGLE (max +/-{} deg off-axis) ===",
                a.max_angle
            ),
        );
        out.field("Viewer distance:", &format!("{} m", a.distance));
        out.field(
            "Usable width:",
            &format!(
                "{} m   (+/-{} m from centre)",
                n(r.usable_width_m, 2),
                n(r.half_width_m, 2)
            ),
        );
        if let Some(deg) = r.screen_edge_angle_deg {
            out.field(
                "Screen-edge angle:",
                &format!("{} deg   (from a centre seat)", n(deg, 1)),
            );
        }
        dim(
            out,
            "  (AVIXA viewing-cone guideline; check the display's own spec angle.)",
        );
    });
    Ok(())
}

fn run_camera(a: &CameraArgs, out: &Out) -> Result<()> {
    let r = camera::camera(&CameraInput {
        focal_length: a.focal_length,
        sensor_width: a.sensor_width,
        h_resolution: a.h_resolution,
        distance: a.distance,
    })?;
    out.emit(&r, |r, out| {
        head(
            out,
            &format!(
                "=== CAMERA / DORI ({}mm, {}mm sensor, {}px) ===",
                a.focal_length, a.sensor_width, a.h_resolution
            ),
        );
        out.field("Horizontal FOV:", &format!("{} deg", n(r.hfov_deg, 1)));
        if let (Some(w), Some(ppm)) = (r.fov_width_m, r.px_per_m) {
            out.field(
                &format!("FOV width @ {}m:", a.distance),
                &format!("{} m  ->  {} px/m", n(w, 2), n(ppm, 0)),
            );
        }
        out.title("  --- DORI max distance (px/m threshold) ---");
        for d in &r.dori {
            raw(&format!(
                "    {:<10} ({:>5} px/m): up to {} m",
                d.name,
                d.px_per_m,
                n(d.max_distance_m, 1)
            ));
        }
    });
    Ok(())
}

// ---- audio ----

fn run_audiobw(a: &AudiobwArgs, out: &Out) -> Result<()> {
    let r = audiobw::audiobw(&AudioBwInput {
        channels: a.channels,
        sample_rate: a.sample_rate,
        bit_depth: a.bit_depth,
        freq: a.freq,
    })?;
    out.emit(&r, |r, out| {
        head(out, "=== AUDIO BANDWIDTH ===");
        raw(&format!(
            "  {} ch x {} Hz x {}-bit",
            a.channels, a.sample_rate, a.bit_depth
        ));
        out.field("Payload data rate:", &format!("{} Mbps", n(r.mbps, 1)));
        dim(
            out,
            "  (Dante/AES67 add packet + protocol overhead on top.)",
        );
        if let Some(w) = r.wavelength_m {
            out.field(
                &format!("Wavelength @ {} Hz:", a.freq),
                &format!("{} m   (343 m/s)", n(w, 3)),
            );
        }
    });
    Ok(())
}

fn run_speaker(a: &SpeakerArgs, out: &Out) -> Result<()> {
    let r = speaker::speaker(&SpeakerInput {
        ceiling_height: a.ceiling_height,
        ear_height: a.ear_height,
        coverage_angle: a.coverage_angle,
        room_width: a.room_width,
        room_length: a.room_length,
        speakers: a.speakers,
        watts_each: a.watts_each,
        headroom_db: a.headroom_db,
    })?;
    out.emit(&r, |r, out| {
        head(out, "=== SPEAKER DESIGN ===");
        if let Some(c) = &r.coverage {
            out.field(
                "Coverage diameter @ ear:",
                &format!("{} m", n(c.diameter_m, 2)),
            );
            out.field("Spacing (edge):", &format!("{} m", n(c.spacing_edge_m, 2)));
            out.field(
                "Spacing (0.707):",
                &format!("{} m   (even coverage)", n(c.spacing_overlap_m, 2)),
            );
            if let Some(room) = &c.room {
                out.field(
                    "Room:",
                    &format!(
                        "{}x{} m -> {} x {} = {} speakers",
                        a.room_width, a.room_length, room.cols, room.rows, room.total
                    ),
                );
            }
        }
        if let Some(amp) = &r.amp {
            out.field(
                "Amp:",
                &format!(
                    "{} x {} W = {} W, +{} dB headroom -> {} W",
                    amp.speakers,
                    amp.watts_each,
                    amp.total_w,
                    amp.headroom_db,
                    n(amp.required_w, 0)
                ),
            );
        }
    });
    Ok(())
}

fn run_cable(a: &CableArgs, out: &Out) -> Result<()> {
    let r = cable::cable(&CableInput {
        kind: match a.kind {
            CableTypeArg::Speaker => CableType::Speaker,
            CableTypeArg::Coax => CableType::Coax,
        },
        awg: a.awg,
        length_m: a.length_m,
        impedance: a.impedance,
        atten_per_100m: a.atten_per_100m,
        max_db: a.max_db,
    })?;
    out.emit(&r, |r, out| {
        head(out, "=== CABLE ===");
        match r.kind {
            CableType::Speaker => {
                out.field(
                    "Run:",
                    &format!("AWG {}, {} m, {} ohm load", a.awg, a.length_m, a.impedance),
                );
                out.field(
                    "Cable resistance:",
                    &format!("{} ohm (round trip)", n(r.resistance_ohm.unwrap_or(0.0), 3)),
                );
                let rating = match r.rating {
                    Some(CableRating::Good) => "good",
                    Some(CableRating::Marginal) => "marginal",
                    Some(CableRating::Poor) => "poor - go thicker",
                    None => "",
                };
                out.field(
                    "Power loss:",
                    &format!(
                        "{} dB  ({}%)  {rating}",
                        n(r.loss_db.unwrap_or(0.0), 2),
                        n(r.loss_percent.unwrap_or(0.0), 1)
                    ),
                );
                dim(out, "  (Aim for <= 0.5 dB; go thicker for long runs.)");
            }
            CableType::Coax => {
                if let Some(db) = r.attenuation_db {
                    out.field(
                        &format!("Attenuation over {} m:", a.length_m),
                        &format!("{} dB", n(db, 1)),
                    );
                }
                if let Some(len) = r.max_length_m {
                    out.field(
                        &format!("Max length for {} dB:", a.max_db),
                        &format!("{} m", n(len, 0)),
                    );
                }
                dim(
                    out,
                    "  (SDI receivers tolerate ~20 dB; use your cable's dB/100m at the SDI clock.)",
                );
            }
        }
    });
    Ok(())
}

fn run_dsp(a: &DspArgs, out: &Out) -> Result<()> {
    let r = dsp::dsp(&DspInput {
        dbu: a.dbu,
        latencies_ms: a.latencies_ms.clone(),
        inputs: a.inputs,
        outputs: a.outputs,
    })?;
    out.emit(&r, |r, out| {
        head(out, "=== DSP SYSTEM ===");
        if let Some(l) = &r.level {
            out.field(
                "Level:",
                &format!(
                    "{} dBu = {} Vrms = {} dBV",
                    l.dbu,
                    n(l.vrms, 4),
                    n(l.dbv, 2)
                ),
            );
        }
        if let Some(l) = &r.latency {
            let chain: Vec<String> = l.chain_ms.iter().map(f64::to_string).collect();
            out.field(
                "Latency chain:",
                &format!("{} = {} ms total", chain.join(" + "), n(l.total_ms, 2)),
            );
        }
        if let Some(m) = &r.matrix {
            out.field(
                "Matrix:",
                &format!(
                    "{} in x {} out = {} crosspoints",
                    m.inputs,
                    m.outputs,
                    n(m.crosspoints as f64, 0)
                ),
            );
        }
        dim(out, "  (Simplified helper; not a full DSP design tool.)");
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::n;

    #[test]
    fn thousands_separators_and_rounding() {
        assert_eq!(n(1244.16, 0), "1,244");
        assert_eq!(n(559.872, 1), "559.9");
        assert_eq!(n(0.5, 2), "0.50");
        assert_eq!(n(1_234_567.891, 2), "1,234,567.89");
        assert_eq!(n(-1234.5, 1), "-1,234.5");
        assert_eq!(n(999.0, 0), "999");
        assert_eq!(n(4_294_967_294.0, 0), "4,294,967,294");
    }
}
