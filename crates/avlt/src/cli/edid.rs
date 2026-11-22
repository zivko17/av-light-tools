//! `avlt edid ...`: EDID tools (read-only). Port of the `edid-*` commands of
//! `EDIDTools.ps1` (`Get-EDIDList`, `Show-EDIDInfo`, `Test-EDIDIntegrity`,
//! `Compare-EDIDSource`, `Export-EDIDBinary`, `Resolve-EDIDSource`).
//!
//! Read-only by design: nothing here writes an EDID override or touches the display
//! driver. To force an EDID use CRU (ToastyX) or a hardware EDID emulator.

use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use avlt_core::edid::{self, DiffKind, DiffLine, Edid, Finding, Severity, timing::vic_label};
use avlt_core::media::non_colliding;
use avlt_core::{Error, Result};
use avlt_platform::edid_source::{self, DisplayDevice, EdidSource};
use avlt_platform::paths::KnownFolder;
use clap::{Args, Subcommand};
use serde::Serialize;

use super::env;
use super::output::{Out, exit_if_broken_pipe};

/// `avlt edid` subcommands.
#[derive(Debug, Subcommand)]
pub enum EdidCmd {
    /// List connected monitors and their EDID
    List(ListArgs),
    /// Decode an EDID
    Info(SourceArgs),
    /// Validate an EDID (header, checksums, extensions, timing vs declared limits)
    Check(CheckArgs),
    /// Compare two EDIDs (e.g. direct vs through a switcher or extender)
    Diff(DiffArgs),
    /// Save the EDID of connected monitors to .bin files (never overwrites)
    Dump(DumpArgs),
}

/// `edid list` arguments.
#[derive(Debug, Args)]
pub struct ListArgs {
    /// Also list displays that are no longer attached but still in the registry
    #[arg(long)]
    pub remembered: bool,
}

/// A single EDID source.
#[derive(Debug, Args)]
pub struct SourceArgs {
    /// Display number (from `avlt edid list`) or path to a .bin EDID file
    #[arg(default_value = "1")]
    pub source: String,
    /// Display numbers also count displays remembered in the registry
    #[arg(long)]
    pub remembered: bool,
}

/// `edid check` arguments.
#[derive(Debug, Args)]
pub struct CheckArgs {
    /// Display number or .bin path; omitted: every connected display
    pub source: Option<String>,
    /// Display numbers also count displays remembered in the registry
    #[arg(long)]
    pub remembered: bool,
}

/// `edid diff` arguments.
#[derive(Debug, Args)]
pub struct DiffArgs {
    /// Display number or .bin path
    pub a: String,
    /// Display number or .bin path
    pub b: String,
    /// List every differing byte (default: the first 20)
    #[arg(long)]
    pub bytes: bool,
    /// Display numbers also count displays remembered in the registry
    #[arg(long)]
    pub remembered: bool,
}

/// `edid dump` arguments.
#[derive(Debug, Args)]
pub struct DumpArgs {
    /// Display number or .bin path
    #[arg(default_value = "1", conflicts_with = "all")]
    pub source: String,
    /// Output folder (default: <output dir>/EDID)
    #[arg(conflicts_with = "out")]
    pub dir: Option<PathBuf>,
    /// Output folder; also works with --all (same as the positional folder)
    #[arg(long, short = 'o', value_name = "DIR")]
    pub out: Option<PathBuf>,
    /// Dump every display that has an EDID
    #[arg(long)]
    pub all: bool,
    /// Include displays remembered in the registry (numbers, and --all)
    #[arg(long)]
    pub remembered: bool,
}

/// Runs an `avlt edid` subcommand.
pub fn run(cmd: EdidCmd, out: &Out) -> Result<()> {
    match cmd {
        EdidCmd::List(a) => list(&a, out),
        EdidCmd::Info(a) => info(&a, out),
        EdidCmd::Check(a) => check(&a, out),
        EdidCmd::Diff(a) => diff(&a, out),
        EdidCmd::Dump(a) => dump(&a, out),
    }
}

// ------------------------------------------------------------
//  Sources
// ------------------------------------------------------------

/// A resolved EDID: where it came from and its decode.
struct Resolved {
    name: String,
    /// Display number when the source is a display.
    display: Option<usize>,
    edid: Edid,
}

/// `"1"` (display number) or a path to a .bin file (`Resolve-EDIDSource`).
fn resolve(source: &str, remembered: bool) -> Result<Resolved> {
    let s = source.trim().trim_matches('"');
    if let Ok(n) = s.parse::<usize>() {
        let devs = edid_source::list(remembered)?;
        let dev = devs.into_iter().find(|d| d.index == n).ok_or_else(|| {
            Error::InvalidInput(format!(
                "No display with index {n}. Run 'avlt edid list' to see them."
            ))
        })?;
        if dev.edid.is_empty() {
            return Err(Error::Other(format!(
                "Display {n} reported no EDID (source: {}).",
                source_name(dev.source)
            )));
        }
        return Ok(Resolved {
            name: format!("display {n}"),
            display: Some(n),
            edid: edid::decode(&dev.edid),
        });
    }
    let path = Path::new(s);
    if !path.is_file() {
        return Err(Error::InvalidInput(format!("File not found: {s}")));
    }
    let bytes = std::fs::read(path)?;
    Ok(Resolved {
        name: path
            .file_name()
            .map_or_else(|| s.to_string(), |n| n.to_string_lossy().into_owned()),
        display: None,
        edid: edid::decode(&bytes),
    })
}

/// Every display with an EDID, as resolved sources.
fn all_displays(remembered: bool) -> Result<Vec<Resolved>> {
    Ok(edid_source::list(remembered)?
        .into_iter()
        .filter(|d| !d.edid.is_empty())
        .map(|d| Resolved {
            name: format!("display {}", d.index),
            display: Some(d.index),
            edid: edid::decode(&d.edid),
        })
        .collect())
}

fn source_name(s: EdidSource) -> &'static str {
    match s {
        EdidSource::Registry => "registry",
        EdidSource::Wmi => "wmi",
        EdidSource::Unavailable => "unavailable",
    }
}

/// A plain stdout line (text mode only; a closed pipe ends quietly).
fn line(s: &str) {
    if let Err(e) = writeln!(io::stdout().lock(), "{s}") {
        exit_if_broken_pipe(&e);
    }
}

fn yes_no(v: bool) -> &'static str {
    if v { "yes" } else { "no" }
}

/// Cuts `s` to `max` characters, ending in `.` when cut (as the original tables do).
fn clip(s: &str, max: usize) -> String {
    if s.chars().count() > max {
        let mut t: String = s.chars().take(max - 1).collect();
        t.push('.');
        t
    } else {
        s.to_string()
    }
}

// ------------------------------------------------------------
//  edid list
// ------------------------------------------------------------

/// One row of `edid list`.
#[derive(Debug, Serialize)]
struct ListEntry {
    #[serde(flatten)]
    device: DisplayDevice,
    manufacturer: Option<String>,
    model: Option<String>,
    year: Option<u16>,
    version: Option<String>,
    preferred: Option<String>,
    caps: Vec<String>,
}

/// Capability shorthand that matters on site.
fn caps(e: &Edid) -> Vec<String> {
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

fn list(a: &ListArgs, out: &Out) -> Result<()> {
    let entries: Vec<ListEntry> = edid_source::list(a.remembered)?
        .into_iter()
        .map(|device| {
            let e = (!device.edid.is_empty()).then(|| edid::decode(&device.edid));
            let base = e.as_ref().and_then(|e| e.base.as_ref());
            ListEntry {
                manufacturer: base.map(|b| b.manufacturer.clone()),
                model: base.map(|b| {
                    b.monitor_name
                        .clone()
                        .unwrap_or_else(|| format!("0x{:04X}", b.product_code))
                }),
                year: base.map(|b| b.year),
                version: base.map(|b| b.version.clone()),
                preferred: base
                    .and_then(|b| b.native.as_ref())
                    .map(|t| t.summary.clone()),
                caps: e
                    .as_ref()
                    .filter(|e| e.base.is_some())
                    .map(caps)
                    .unwrap_or_default(),
                device,
            }
        })
        .collect();

    out.emit(&entries, |entries, out| {
        out.title("=== EDID - DISPLAYS ===");
        if entries.is_empty() {
            out.warn("No displays found (WmiMonitorID returned nothing).");
            out.warn("Some virtual / remote sessions do not expose an EDID.");
            return;
        }
        line(&format!(
            "   {:<2} {:<4} {:<22} {:<6} {:<5} {:<20} {}",
            "#", "MFG", "MODEL", "YEAR", "EDID", "PREFERRED TIMING", "CAPS"
        ));
        line(&format!("   {}", "-".repeat(92)));
        for d in entries {
            let Some(mfg) = &d.manufacturer else {
                line(&format!(
                    "   {:<2} EDID unavailable ({})",
                    d.device.index,
                    source_name(d.device.source)
                ));
                line(&format!("      {}", d.device.instance_id));
                continue;
            };
            line(&format!(
                "   {:<2} {:<4} {:<22} {:<6} {:<5} {:<20} {}",
                d.device.index,
                mfg,
                clip(d.model.as_deref().unwrap_or(""), 22),
                d.year.map(|y| y.to_string()).unwrap_or_default(),
                d.version.as_deref().unwrap_or(""),
                d.preferred.as_deref().unwrap_or(""),
                d.caps.join(", ")
            ));
            let mut tail = d.device.instance_id.clone();
            if !d.device.active {
                tail.push_str("   [not connected]");
            }
            line(&format!("      {tail}"));
        }
        line("");
        line("  avlt edid info <n>   full decode        avlt edid dump <n>   save .bin");
        line("  avlt edid check <n>  validate           avlt edid diff a b   compare");
    });
    Ok(())
}

// ------------------------------------------------------------
//  edid info
// ------------------------------------------------------------

/// Result of `edid info`.
#[derive(Debug, Serialize)]
struct InfoOut {
    source: String,
    edid: Edid,
}

fn info(a: &SourceArgs, out: &Out) -> Result<()> {
    let r = resolve(&a.source, a.remembered)?;
    let value = InfoOut {
        source: r.name,
        edid: r.edid,
    };
    out.emit(&value, |v, out| print_info(&v.source, &v.edid, out));
    // Text mode lists problems under Integrity; with --json they also go to stderr.
    if out.json {
        for p in &value.edid.problems {
            out.warn(p);
        }
    }
    Ok(())
}

fn print_info(name: &str, e: &Edid, out: &Out) {
    out.title(&format!("=== EDID - {} ===", name.to_uppercase()));
    if let Some(b) = &e.base {
        out.title("--- Identity ---");
        let mut mfg = b.manufacturer.clone();
        if let Some(v) = &b.vendor_name {
            mfg.push_str(&format!("  ({v})"));
        }
        out.field("Manufacturer:", &mfg);
        if let Some(n) = &b.monitor_name {
            out.field("Model name:", n);
        }
        out.field(
            "Product code:",
            &format!("0x{:04X}  ({})", b.product_code, b.product_code),
        );
        out.field("Serial number:", &b.serial_number.to_string());
        if let Some(s) = &b.serial_text {
            out.field("Serial (text):", s);
        }
        for t in &b.text_fields {
            out.field("Text:", t);
        }
        let date = if (1..=54).contains(&b.week) {
            format!("week {} / {}", b.week, b.year)
        } else {
            b.year.to_string()
        };
        out.field("Manufactured:", &date);
        out.field("EDID version:", &b.version);
        out.field(
            "Blocks:",
            &format!(
                "{}  (base + {} extension)",
                e.block_count,
                e.block_count.saturating_sub(1)
            ),
        );
        line("");

        out.title("--- Display parameters ---");
        out.field("Input:", &b.input_desc);
        if b.width_cm > 0 {
            let diag = b.diagonal_in.map(|d| d.to_string()).unwrap_or_default();
            out.field(
                "Screen size:",
                &format!("{} x {} cm   ({diag}\")", b.width_cm, b.height_cm),
            );
        } else {
            out.field("Screen size:", "not reported");
        }
        if let Some(g) = b.gamma {
            out.field("Gamma:", &g.to_string());
        }
        out.field("Colour formats:", &b.colour_type);
        if !b.features.is_empty() {
            out.field("Features:", &b.features.join(", "));
        }
        line("");

        if let Some(t) = &b.native {
            out.title("--- Preferred timing (base block) ---");
            out.ok(&format!(
                "    {}      pixel clock {} MHz",
                t.summary, t.pixel_clock_mhz
            ));
            line(&format!(
                "    H: {:>5} active {:>4} blank   front {:>4}  sync {:>4}  ({})",
                t.h_active, t.h_blank, t.h_front_porch, t.h_sync_width, t.h_polarity
            ));
            line(&format!(
                "    V: {:>5} active {:>4} blank   front {:>4}  sync {:>4}  ({})",
                t.v_active, t.v_blank, t.v_front_porch, t.v_sync_width, t.v_polarity
            ));
            if t.width_mm > 0 {
                line(&format!(
                    "    Image size: {} x {} mm",
                    t.width_mm, t.height_mm
                ));
            }
            line("");
            if b.timings.len() > 1 {
                out.title("--- Other detailed timings (base block) ---");
                for t in &b.timings[1..] {
                    line(&format!("    {:<28} {} MHz", t.summary, t.pixel_clock_mhz));
                }
                line("");
            }
        }

        if let Some(r) = &b.range {
            out.title("--- Range limits ---");
            line(&format!(
                "    V: {}-{} Hz    H: {}-{} kHz    Max pixel clock: {} MHz",
                r.v_min, r.v_max, r.h_min, r.h_max, r.max_clock_mhz
            ));
            line("");
        }

        if !b.established.is_empty() || !b.standard.is_empty() {
            out.title("--- Established / standard timings ---");
            if !b.established.is_empty() {
                line(&format!("    {}", b.established.join(", ")));
            }
            if !b.standard.is_empty() {
                line(&format!("    {}", b.standard.join(", ")));
            }
            line("");
        }
    }

    for ext in &e.extensions {
        let Some(c) = e.cta.iter().find(|c| c.index == ext.index) else {
            out.title(&format!(
                "--- Extension {}: {} (tag 0x{:02X}) ---",
                ext.index, ext.kind, ext.tag
            ));
            line("    Not decoded by this tool.");
            line("");
            continue;
        };
        out.title(&format!(
            "--- CTA-861 extension (block {}, rev {}) ---",
            c.index, c.revision
        ));
        out.field("Basic audio:", yes_no(c.basic_audio));
        out.field(
            "YCbCr:",
            &format!(
                "4:4:4 {}   4:2:2 {}",
                yes_no(c.ycbcr444),
                yes_no(c.ycbcr422)
            ),
        );
        out.field("Underscan IT:", yes_no(c.underscan_it));
        if c.quant_rgb || c.quant_ycc {
            out.field(
                "Quant. range:",
                &format!(
                    "RGB selectable: {}   YCC selectable: {}",
                    yes_no(c.quant_rgb),
                    yes_no(c.quant_ycc)
                ),
            );
        }
        if !c.video.is_empty() {
            line("    Video (SVDs, * = native):");
            let cells: Vec<String> = c
                .video
                .iter()
                .map(|v| {
                    let name = v.name.clone().unwrap_or_else(|| format!("VIC {}", v.vic));
                    format!("{}{:<3} {name}", if v.native { '*' } else { ' ' }, v.vic)
                })
                .collect();
            for chunk in cells.chunks(3) {
                let row: String = chunk.iter().map(|c| format!("{c:<26}")).collect();
                line(&format!("      {}", row.trim_end()));
            }
        }
        if !c.video420.is_empty() {
            line("    YCbCr 4:2:0 only:");
            let l: Vec<String> = c.video420.iter().map(|v| vic_label(v.vic)).collect();
            line(&format!("      {}", l.join(",  ")));
        }
        if !c.audio.is_empty() {
            line("    Audio (SADs):");
            for a in &c.audio {
                line(&format!("      {}", a.summary));
            }
        }
        if !c.speakers.is_empty() {
            out.field("Speakers:", &c.speakers.join(", "));
        }
        if let Some(h) = &c.hdmi {
            line("    HDMI vendor block (00-0C-03):");
            if let Some(p) = &h.physical_address {
                line(&format!("      Source physical address: {p}"));
            }
            if let Some(m) = h.max_tmds_mhz {
                line(&format!("      Max TMDS clock:          {m} MHz"));
            }
            if !h.deep_colour.is_empty() {
                line(&format!(
                    "      Deep colour:             {}",
                    h.deep_colour.join(", ")
                ));
            }
        }
        if let Some(hf) = &c.hdmi_forum {
            line("    HDMI Forum block (C4-5D-D8, HDMI 2.x):");
            if let Some(m) = hf.max_tmds_char_rate_mhz {
                out.ok(&format!("      Max TMDS character rate: {m} MHz"));
            }
            line(&format!(
                "      SCDC:                    {}",
                yes_no(hf.scdc)
            ));
            if let Some(f) = hf.max_frl_rate.filter(|&f| f > 0) {
                line(&format!("      Max FRL rate:            {f}"));
            }
        }
        if !c.hdr_eotf.is_empty() {
            line("    HDR static metadata:");
            out.ok(&format!("      EOTF: {}", c.hdr_eotf.join(", ")));
            if let Some(max) = c.hdr_max_lum {
                let mut lum = format!("      Luminance: max {max} cd/m2");
                if let Some(avg) = c.hdr_avg_lum {
                    lum.push_str(&format!("   frame-avg {avg}"));
                }
                if let Some(min) = c.hdr_min_lum.filter(|&m| m != 0.0) {
                    lum.push_str(&format!("   min {min}"));
                }
                line(&lum);
            }
        }
        if !c.colorimetry.is_empty() {
            out.field("Colorimetry:", &c.colorimetry.join(", "));
        }
        if !c.timings.is_empty() {
            line("    Detailed timings in this block:");
            for t in &c.timings {
                line(&format!(
                    "      {:<28} {} MHz",
                    t.summary, t.pixel_clock_mhz
                ));
            }
        }
        for u in &c.unknown_blocks {
            line(&format!("    (not decoded) {u}"));
        }
        line("");
    }

    out.title("--- Integrity ---");
    if let Some(b) = &e.base {
        out.field("Header:", if b.header_valid { "OK" } else { "INVALID" });
    }
    for chk in &e.checksums {
        let label = if chk.block == 0 {
            "Base checksum:".to_string()
        } else {
            format!("Ext {} checksum:", chk.block)
        };
        out.field(&label, if chk.valid { "OK" } else { "BAD" });
    }
    for p in &e.problems {
        out.field("Problem:", p);
    }
}

// ------------------------------------------------------------
//  edid check
// ------------------------------------------------------------

/// Result of `edid check` for one source.
#[derive(Debug, Serialize)]
struct CheckOut {
    source: String,
    findings: Vec<Finding>,
}

fn check(a: &CheckArgs, out: &Out) -> Result<()> {
    let targets = match &a.source {
        Some(s) => vec![resolve(s, a.remembered)?],
        None => all_displays(a.remembered)?,
    };
    let results: Vec<CheckOut> = targets
        .into_iter()
        .map(|t| CheckOut {
            findings: edid::check(&t.edid),
            source: t.name,
        })
        .collect();

    out.emit(&results, |results, out| {
        out.title("=== EDID CHECK ===");
        if results.is_empty() {
            out.warn("No displays with an EDID found.");
        }
        for r in results {
            line("");
            out.title(&format!("--- {} ---", r.source));
            for f in &r.findings {
                match f.severity {
                    Severity::Ok => out.ok(&format!("    {}", f.message)),
                    Severity::Error => line(&format!("    [ERROR] {}", f.message)),
                    Severity::Warn => line(&format!("    [WARN ] {}", f.message)),
                }
            }
        }
    });

    let errors = results
        .iter()
        .flat_map(|r| &r.findings)
        .filter(|f| f.severity == Severity::Error)
        .count();
    if errors > 0 {
        return Err(Error::Other(format!("EDID check found {errors} error(s)")));
    }
    Ok(())
}

// ------------------------------------------------------------
//  edid diff
// ------------------------------------------------------------

/// One compared field of `edid diff`.
#[derive(Debug, Serialize)]
struct FieldRow {
    field: String,
    a: String,
    b: String,
    same: bool,
}

/// Result of `edid diff`.
#[derive(Debug, Serialize)]
struct DiffOut {
    a: String,
    b: String,
    a_bytes: usize,
    b_bytes: usize,
    fields: Vec<FieldRow>,
    differences: Vec<DiffLine>,
}

fn diff(a: &DiffArgs, out: &Out) -> Result<()> {
    let x = resolve(&a.a, a.remembered).map_err(|e| side_error("A", e))?;
    let y = resolve(&a.b, a.remembered).map_err(|e| side_error("B", e))?;
    let value = DiffOut {
        fields: edid::diff::fields(&x.edid, &y.edid)
            .into_iter()
            .map(|(field, va, vb)| FieldRow {
                same: va == vb,
                field,
                a: va,
                b: vb,
            })
            .collect(),
        differences: edid::diff(&x.edid, &y.edid),
        a_bytes: x.edid.raw.len(),
        b_bytes: y.edid.raw.len(),
        a: x.name,
        b: y.name,
    };
    let all_bytes = a.bytes;
    out.emit(&value, |v, out| print_diff(v, all_bytes, out));
    Ok(())
}

/// Prefixes which side failed (`error: (A) File not found: ...`).
fn side_error(side: &str, e: Error) -> Error {
    match e {
        Error::InvalidInput(m) => Error::InvalidInput(format!("({side}) {m}")),
        Error::Other(m) => Error::Other(format!("({side}) {m}")),
        other => other,
    }
}

fn print_diff(v: &DiffOut, all_bytes: bool, out: &Out) {
    out.title("=== EDID DIFF ===");
    line(&format!("  A: {}   ({} bytes)", v.a, v.a_bytes));
    line(&format!("  B: {}   ({} bytes)", v.b, v.b_bytes));
    line("");
    line(&format!("  {:<18} {:<32} B", "FIELD", "A"));
    line(&format!("  {}", "-".repeat(88)));
    for r in &v.fields {
        line(&format!(
            "{} {:<18} {:<32} {}",
            if r.same { ' ' } else { '!' },
            r.field,
            clip(&r.a, 31),
            clip(&r.b, 31)
        ));
    }
    line("");

    let of = |k: DiffKind| v.differences.iter().filter(move |d| d.kind == k);
    let only_a: Vec<&str> = of(DiffKind::Vic)
        .filter(|d| d.a == "present")
        .map(|d| d.field.as_str())
        .collect();
    let only_b: Vec<&str> = of(DiffKind::Vic)
        .filter(|d| d.b == "present")
        .map(|d| d.field.as_str())
        .collect();
    if !only_a.is_empty() || !only_b.is_empty() {
        out.title("--- Video modes (VIC) present on one side only ---");
        if !only_a.is_empty() {
            line(&format!("    only in A:  {}", only_a.join(",  ")));
        }
        if !only_b.is_empty() {
            line(&format!("    only in B:  {}", only_b.join(",  ")));
        }
        line("");
    }

    let bytes: Vec<&DiffLine> = of(DiffKind::Byte).collect();
    out.title(&format!(
        "--- Raw bytes: {} of {} differ ---",
        bytes.len(),
        v.a_bytes.max(v.b_bytes)
    ));
    if bytes.is_empty() {
        out.ok("    Identical.");
    } else {
        let shown = if all_bytes { bytes.len() } else { 20 };
        for d in bytes.iter().take(shown) {
            line(&format!("    {}:  {}  ->  {}", d.field, d.a, d.b));
        }
        if bytes.len() > shown {
            line(&format!(
                "    ... {} more. Use --bytes to list them all.",
                bytes.len() - shown
            ));
        }
    }
    line("");
    let decoded = v.differences.len() - bytes.len();
    if v.differences.is_empty() {
        out.ok("  The two EDIDs are byte-identical.");
    } else {
        line(&format!("  {decoded} decoded field(s) differ."));
    }
}

// ------------------------------------------------------------
//  edid dump
// ------------------------------------------------------------

/// One file written (or failed) by `edid dump`.
#[derive(Debug, Serialize)]
struct DumpOut {
    source: String,
    path: Option<PathBuf>,
    bytes: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

/// `<MFG>_<model>[_out<n>].bin`, with anything but word characters, `-` and `.` as `_`.
fn dump_file_name(r: &Resolved) -> String {
    let (mfg, model) = match &r.edid.base {
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
    if let Some(n) = r.display {
        safe.push_str(&format!("_out{n}"));
    }
    format!("{safe}.bin")
}

/// Writes `bytes` to `path` or the first free `_1`, `_2`, ... name; never overwrites.
fn write_new(path: &Path, bytes: &[u8]) -> Result<PathBuf> {
    loop {
        let target = non_colliding(path);
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)
        {
            Ok(mut f) => {
                f.write_all(bytes)?;
                return Ok(target);
            }
            // Created by someone else between the check and the open: try the next name.
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e.into()),
        }
    }
}

/// Writes every target into `dir`. A failed write is reported and does not stop the rest;
/// returns the per-file results and the number of failures.
fn write_all(targets: &[Resolved], dir: &Path, out: &Out) -> (Vec<DumpOut>, usize) {
    let mut written = Vec::new();
    let mut failed = 0usize;
    for t in targets {
        let (path, error) = match write_new(&dir.join(dump_file_name(t)), &t.edid.raw) {
            Ok(p) => (Some(p), None),
            Err(e) => {
                failed += 1;
                out.warn(&format!("{}: {e}", t.name));
                (None, Some(e.to_string()))
            }
        };
        written.push(DumpOut {
            source: t.name.clone(),
            path,
            bytes: t.edid.raw.len(),
            error,
        });
    }
    (written, failed)
}

fn dump(a: &DumpArgs, out: &Out) -> Result<()> {
    let targets = if a.all {
        all_displays(a.remembered)?
    } else {
        vec![resolve(&a.source, a.remembered)?]
    };
    let dir = match a.out.as_ref().or(a.dir.as_ref()) {
        Some(d) => d.clone(),
        None => env::output_dir(KnownFolder::Documents)?.join("EDID"),
    };
    std::fs::create_dir_all(&dir)?;

    let (written, failed) = write_all(&targets, &dir, out);

    out.emit(&written, |written, out| {
        out.title("=== EDID DUMP ===");
        if written.is_empty() {
            out.warn("No displays with an EDID found.");
        }
        for w in written {
            if let Some(path) = &w.path {
                out.ok(&format!(
                    "  {:<14} -> {}  ({} bytes)",
                    w.source,
                    path.display(),
                    w.bytes
                ));
            }
        }
        line("");
        line("  Load these in CRU / AW EDID Editor, or push them to a matrix or EDID emulator.");
    });
    if failed > 0 {
        return Err(Error::Other(format!(
            "{failed} of {} EDID dumps failed",
            targets.len()
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolved(bytes: &[u8], display: Option<usize>) -> Resolved {
        Resolved {
            name: "x".into(),
            display,
            edid: edid::decode(bytes),
        }
    }

    #[test]
    fn dump_name_without_edid_base() {
        assert_eq!(dump_file_name(&resolved(&[], None)), "UNKNOWN_display.bin");
        assert_eq!(
            dump_file_name(&resolved(&[], Some(2))),
            "UNKNOWN_display_out2.bin"
        );
    }

    #[test]
    fn dump_all_continues_after_a_failed_write() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("no_such_subdir");
        let targets = [resolved(&[], Some(1)), resolved(&[], Some(2))];
        let out = Out::new(true);

        // Every write fails (folder missing): each entry carries an error, none abort the loop.
        let (res, failed) = write_all(&targets, &missing, &out);
        assert_eq!(failed, 2);
        assert_eq!(res.len(), 2);
        assert!(res.iter().all(|r| r.error.is_some() && r.path.is_none()));

        let (res, failed) = write_all(&targets, dir.path(), &out);
        assert_eq!(failed, 0);
        assert!(res.iter().all(|r| r.error.is_none() && r.path.is_some()));
    }

    #[test]
    fn clip_marks_cut_text() {
        assert_eq!(clip("abcdef", 4), "abc.");
        assert_eq!(clip("abc", 4), "abc");
    }
}
