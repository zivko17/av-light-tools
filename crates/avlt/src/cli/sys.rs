//! `avlt sys ...`: System tools (SysInfo.ps1, Tele.ps1, Top.ps1, KillP.ps1, USB.ps1,
//! Disks.ps1, Monitors.ps1, MonitorsArrange.ps1, Battery.ps1, Startup.ps1, Audio.ps1,
//! ServicesAudio.ps1, Cleanup.ps1; AVToolbox SystemTools.cs; NetMaint MaintenancePage.cs).

use std::io::{self, BufRead, IsTerminal, Write};
use std::time::Duration;

use avlt_core::sys::display::{parse_resolution, pick_mode, refresh_rates, resolutions};
use avlt_core::sys::format::{
    Level, bar, battery_health_level, charge_level, disk_level, freed_text, gib, group_usb,
    health_level, load_level, mib, percent, process_memory_level, round_to, truncate, uptime_text,
};
use avlt_core::sys::model::{
    AudioDevice, CleanStatus, Display, DisplayMode, KillReport, KillResult, ProcessInfo, Telemetry,
    TopSample, Volume,
};
use avlt_core::sys::process::{is_protected_process, kill_plan, restrict_to_pids};
use avlt_core::sys::{AUDIO_RESTART_SERVICE, AUDIO_SERVICES};
use avlt_core::{Error, Result};
use avlt_platform::sys as psys;
use clap::{Args, Subcommand};

use super::confirm::confirm;
use super::live::{Colour, Ticker, block_on, level_colour, paint};
use super::output::Out;
use super::{json_line, line};

/// `avlt sys` subcommands.
#[derive(Debug, Subcommand)]
pub enum SysCmd {
    /// System summary (identity, OS, CPU, RAM, GPU, storage)
    Info,
    /// Live telemetry (CPU, RAM, GPU, disk, network), every second
    Tele(SamplesArg),
    /// Top processes by instantaneous CPU use, every 2 seconds
    Top(TopArgs),
    /// Kill processes by name (asks first)
    Killp(KillpArgs),
    /// Connected USB devices
    Usb,
    /// Physical disks and local volumes
    Disks,
    /// Connected monitors
    Monitors,
    /// Change a display's resolution and refresh rate
    MonitorsArrange(ArrangeArgs),
    /// Battery status
    Battery,
    /// Startup programs (registry Run keys and Startup folders), read-only
    Startup,
    /// Audio devices and default output volume
    Audio,
    /// Audio-related services (Windows Audio, Dante...)
    ServicesAudio(ServicesAudioArgs),
    /// Delete temp files and browser caches, empty the Recycle Bin (asks first)
    Cleanup(CleanupArgs),
}

/// Sample limit of the live monitors.
#[derive(Debug, Args)]
pub struct SamplesArg {
    /// Stop after N samples (default: run until Ctrl+C)
    #[arg(short = 'n', long, value_parser = clap::value_parser!(u64).range(1..))]
    pub samples: Option<u64>,
}

/// `sys top` arguments.
#[derive(Debug, Args)]
pub struct TopArgs {
    /// Number of processes shown
    #[arg(short, long, default_value_t = 10, value_parser = clap::value_parser!(u16).range(1..))]
    pub count: u16,
    #[command(flatten)]
    pub samples: SamplesArg,
}

/// `sys killp` arguments.
#[derive(Debug, Args)]
pub struct KillpArgs {
    /// Process name, with or without .exe (exact match unless --wildcard)
    pub name: String,
    /// Match every process whose name contains NAME
    #[arg(short, long)]
    pub wildcard: bool,
    /// Only processes with this PID (repeatable); the name must still match
    #[arg(long = "pid", value_name = "PID")]
    pub pids: Vec<u32>,
    /// Also kill critical system processes (svchost, csrss, explorer...). Always asks
    /// interactively; --yes does not answer that question
    #[arg(long)]
    pub include_critical: bool,
    /// Do not ask for confirmation (never covers critical processes)
    #[arg(short = 'y', long)]
    pub yes: bool,
}

/// `sys monitors-arrange` arguments. Missing values are asked for in a terminal.
#[derive(Debug, Args)]
pub struct ArrangeArgs {
    /// Display number (from the list this command prints)
    #[arg(short, long)]
    pub display: Option<u32>,
    /// Resolution, e.g. 1920x1080
    #[arg(short, long)]
    pub resolution: Option<String>,
    /// Refresh rate in Hz (default: the highest the resolution offers)
    #[arg(long)]
    pub hz: Option<u32>,
    /// Only check that the driver accepts the mode (CDS_TEST); change nothing
    #[arg(long)]
    pub test: bool,
    /// Do not ask for confirmation
    #[arg(short = 'y', long)]
    pub yes: bool,
}

/// `sys services-audio` arguments.
#[derive(Debug, Args)]
pub struct ServicesAudioArgs {
    /// Restart Windows Audio (Audiosrv); needs administrator rights
    #[arg(long)]
    pub restart: bool,
    /// Do not ask for confirmation
    #[arg(short = 'y', long)]
    pub yes: bool,
}

/// `sys cleanup` arguments.
#[derive(Debug, Args)]
pub struct CleanupArgs {
    /// Only show how much each target holds; delete nothing
    #[arg(long)]
    pub analyze: bool,
    /// Do not ask for confirmation
    #[arg(short = 'y', long)]
    pub yes: bool,
}

/// Runs an `avlt sys` subcommand.
pub fn run(cmd: SysCmd, out: &Out) -> Result<()> {
    match cmd {
        SysCmd::Info => info(out),
        SysCmd::Tele(a) => tele(a, out),
        SysCmd::Top(a) => top(a, out),
        SysCmd::Killp(a) => killp(a, out),
        SysCmd::Usb => usb(out),
        SysCmd::Disks => disks(out),
        SysCmd::Monitors => monitors(out),
        SysCmd::MonitorsArrange(a) => monitors_arrange(a, out),
        SysCmd::Battery => battery(out),
        SysCmd::Startup => startup(out),
        SysCmd::Audio => audio(out),
        SysCmd::ServicesAudio(a) => services_audio(a, out),
        SysCmd::Cleanup(a) => cleanup(a, out),
    }
}

// ---------------------------------------------------------------- helpers

/// GiB rounded to one decimal, as the scripts print sizes.
fn gb1(bytes: u64) -> f64 {
    round_to(gib(bytes), 1)
}

fn section(out: &Out, s: &str) {
    line(out, &paint(out, Colour::Yellow, &format!("  --- {s} ---")));
}

fn blank(out: &Out) {
    line(out, "");
}

/// In a terminal (colour on), moves the cursor back over the previous `lines` so a live
/// monitor redraws in place (Tele.ps1 / Top.ps1 `SetCursorPosition`).
fn rewind(out: &Out, lines: usize) {
    if out.color && lines > 0 {
        let mut stdout = io::stdout().lock();
        let _ = write!(stdout, "\x1b[{lines}F");
        let _ = stdout.flush();
    }
}

/// A live-monitor line that also clears what a longer previous line left behind.
fn live_line(out: &Out, s: &str) {
    if out.color {
        line(out, &format!("{s}\x1b[K"));
    } else {
        line(out, s);
    }
}

fn volume_row(v: &Volume) -> (String, Level) {
    let used = gb1(v.total_bytes - v.free_bytes);
    (
        format!(
            "{}  {} GB used / {} GB total  ({}%)",
            v.drive,
            used,
            gb1(v.total_bytes),
            v.used_pct.round()
        ),
        disk_level(v.used_pct.round()),
    )
}

/// Reads a 1-based choice from the terminal; `None` for an empty answer.
fn ask_number(question: &str, max: usize) -> Result<Option<usize>> {
    let mut err = io::stderr().lock();
    let _ = write!(err, "  {question}: ");
    let _ = err.flush();
    drop(err);
    let mut answer = String::new();
    io::stdin().lock().read_line(&mut answer)?;
    let answer = answer.trim();
    if answer.is_empty() {
        return Ok(None);
    }
    match answer.parse::<usize>() {
        Ok(n) if (1..=max).contains(&n) => Ok(Some(n)),
        _ => Err(Error::InvalidInput(format!("invalid selection '{answer}'"))),
    }
}

fn interactive(out: &Out) -> bool {
    !out.json && io::stdin().is_terminal()
}

// ---------------------------------------------------------------- info

/// SysInfo.ps1 sections.
fn info(out: &Out) -> Result<()> {
    let info = psys::info()?;
    out.emit(&info, |i, out| {
        out.title("=== SYSTEM INFO ===");
        blank(out);
        section(out, "Identity");
        out.field("Hostname", &i.hostname);
        out.field("Manufacturer", &i.manufacturer);
        out.field("Model", &i.model);
        out.field("Serial Number", &i.serial_number);
        out.field("BIOS Version", &i.bios_version);
        blank(out);
        section(out, "Operating System");
        out.field("OS", &i.os.caption);
        out.field(
            "Version",
            &format!("{}  (build {})", i.os.version, i.os.build),
        );
        out.field("Architecture", &i.os.architecture);
        out.field("Install Date", i.os.install_date.as_deref().unwrap_or("-"));
        out.field("Uptime", &uptime_text(i.os.uptime_s));
        blank(out);
        section(out, "CPU");
        out.field("Name", &i.cpu.name);
        out.field(
            "Cores",
            &format!(
                "{} physical / {} logical",
                i.cpu.cores, i.cpu.logical_processors
            ),
        );
        out.field("Base Speed", &format!("{} MHz", i.cpu.base_mhz));
        blank(out);
        section(out, "RAM");
        out.field("Total", &format!("{} GB", gb1(i.ram.total_bytes)));
        for m in &i.ram.modules {
            let speed = m
                .speed_mhz
                .map_or_else(|| "?".to_string(), |s| s.to_string());
            out.field(
                "Slot",
                &paint(
                    out,
                    Colour::Dim,
                    &format!(
                        "{} GB @ {speed} MHz  ({})",
                        round_to(gib(m.capacity_bytes), 0),
                        m.manufacturer
                    ),
                ),
            );
        }
        blank(out);
        section(out, "GPU");
        for g in &i.gpus {
            out.field("Name", &g.name);
            let vram = g
                .vram_bytes
                .map_or_else(|| "?".to_string(), |v| gb1(v).to_string());
            out.field("VRAM", &paint(out, Colour::Dim, &format!("{vram} GB")));
            out.field(
                "Driver",
                &paint(
                    out,
                    Colour::Dim,
                    &format!(
                        "{}  ({})",
                        g.driver_version,
                        g.driver_date.as_deref().unwrap_or("-")
                    ),
                ),
            );
        }
        blank(out);
        section(out, "Storage Summary");
        for v in &i.volumes {
            let (row, _) = volume_row(v);
            line(out, &paint(out, Colour::Dim, &format!("    {row}")));
        }
        blank(out);
    });
    Ok(())
}

// ---------------------------------------------------------------- tele / top

fn tele_lines(t: &Telemetry, out: &Out) -> Vec<String> {
    let bar_line = |label: &str, pct: f64, level: Level| {
        let pct = round_to(pct, 1);
        format!(
            "{label} {}",
            paint(out, level_colour(level), &format!("{} {pct}%", bar(pct)))
        )
    };
    let gpu = t.gpu_pct.unwrap_or(0.0);
    let disk_used = gb1(t.disk_used_bytes);
    let net = match &t.net_adapter {
        Some(name) => format!(
            "NET  down: {} MB/s  |  up: {} MB/s  ({name})",
            round_to(t.net_down_bytes_per_s / 1_048_576.0, 2),
            round_to(t.net_up_bytes_per_s / 1_048_576.0, 2)
        ),
        None => "NET  (no active adapter)".to_string(),
    };
    vec![
        bar_line("CPU ", t.cpu_pct, load_level(t.cpu_pct)),
        bar_line("RAM ", t.ram_pct, load_level(t.ram_pct)),
        paint(
            out,
            Colour::Dim,
            &format!(
                "     {} GB / {} GB",
                round_to(gib(t.ram_used_bytes), 2),
                round_to(gib(t.ram_total_bytes), 2)
            ),
        ),
        if t.gpu_pct.is_some() {
            bar_line("GPU ", gpu, load_level(gpu))
        } else {
            format!("GPU  {}", paint(out, Colour::Dim, "(counter unavailable)"))
        },
        bar_line("DISK", t.disk_pct, disk_level(t.disk_pct)),
        paint(
            out,
            Colour::Dim,
            &format!("     {disk_used} GB / {} GB used", gb1(t.disk_total_bytes)),
        ),
        paint(out, Colour::Cyan, &net),
        paint(out, Colour::Dim, &format!("Updated: {}", t.time)),
    ]
}

/// Tele.ps1: one sample per second, redrawn in place in a terminal.
fn tele(args: SamplesArg, out: &Out) -> Result<()> {
    let mut sampler = psys::TelemetrySampler::new()?;
    if !out.json {
        out.title("=== SYSTEM TELEMETRY ===");
        line(out, &paint(out, Colour::Dim, "Press Ctrl+C to exit"));
        blank(out);
    }
    block_on(async {
        let mut ticker = Ticker::new(args.samples, false);
        let mut drawn = 0;
        while ticker.next().await {
            let t = sampler.sample()?;
            if out.json {
                json_line(&t);
                continue;
            }
            let lines = tele_lines(&t, out);
            rewind(out, drawn);
            for l in &lines {
                live_line(out, l);
            }
            drawn = if out.color { lines.len() } else { 0 };
            if !out.color {
                blank(out);
            }
        }
        Ok(())
    })?
}

fn top_lines(s: &TopSample, out: &Out) -> Vec<String> {
    let mut lines = vec![
        paint(
            out,
            Colour::Yellow,
            &format!(
                "  {:<30} {:<10} {:<12} {:<8}",
                "PROCESS", "PID", "MEMORY (MB)", "CPU %"
            ),
        ),
        paint(
            out,
            Colour::Dim,
            &format!(
                "  {:<30} {:<10} {:<12} {:<8}",
                "-".repeat(30),
                "-".repeat(10),
                "-".repeat(12),
                "-".repeat(8)
            ),
        ),
    ];
    for p in &s.processes {
        let mem_pct = percent(p.memory_bytes, s.total_memory_bytes);
        let row = format!(
            "  {:<30} {:<10} {:<12} {:<8}",
            truncate(&p.name, 28, ".."),
            p.pid,
            round_to(mib(p.memory_bytes), 1),
            format!("{:.1}", p.cpu_pct)
        );
        lines.push(paint(
            out,
            level_colour(process_memory_level(mem_pct)),
            &row,
        ));
    }
    lines.push(String::new());
    lines.push(paint(
        out,
        Colour::Dim,
        &format!(
            "  Updated: {}   |   Tip: avlt sys killp <name>  to terminate",
            s.time
        ),
    ));
    lines
}

/// Top.ps1 table every 2 s, ordered by CPU use during the interval.
fn top(args: TopArgs, out: &Out) -> Result<()> {
    let mut sampler = psys::TopSampler::new()?;
    if !out.json {
        out.title("=== TOP PROCESSES (CPU/RAM) ===");
        line(
            out,
            &paint(
                out,
                Colour::Dim,
                "Sampling every 2 s - press Ctrl+C to exit",
            ),
        );
        blank(out);
    }
    block_on(async {
        let mut ticker = Ticker::with_period(Duration::from_secs(2), args.samples.samples, false);
        let mut drawn = 0;
        while ticker.next().await {
            let s = sampler.sample(usize::from(args.count))?;
            if out.json {
                json_line(&s);
                continue;
            }
            let lines = top_lines(&s, out);
            rewind(out, drawn);
            for l in &lines {
                live_line(out, l);
            }
            drawn = if out.color { lines.len() } else { 0 };
            if !out.color {
                blank(out);
            }
        }
        Ok(())
    })?
}

// ---------------------------------------------------------------- killp

/// KillP.ps1: list the matches (critical ones flagged), confirm, kill.
fn killp(args: KillpArgs, out: &Out) -> Result<()> {
    let own = std::process::id();
    let matched: Vec<ProcessInfo> =
        restrict_to_pids(psys::find_processes(&args.name, args.wildcard)?, &args.pids)
            .into_iter()
            .filter(|p| p.pid != own)
            .collect();
    let mut report = KillReport {
        pattern: args.name.clone(),
        wildcard: args.wildcard,
        matched,
        results: Vec::new(),
    };
    if report.matched.is_empty() {
        out.warn(&format!("No process found matching '{}'.", args.name));
        if !args.wildcard {
            out.warn("(exact name match; add --wildcard for partial matches)");
        }
        out.emit(&report, |_, _| {});
        return Ok(());
    }
    if !out.json {
        blank(out);
        out.title(&format!("Processes matching '{}':", args.name));
        line(
            out,
            &paint(
                out,
                Colour::Yellow,
                &format!("  {:<30} {:<10} {:<12}", "PROCESS", "PID", "MEMORY (MB)"),
            ),
        );
        for p in &report.matched {
            let row = format!(
                "  {:<30} {:<10} {:<12}{}",
                p.name,
                p.pid,
                round_to(mib(p.memory_bytes), 1),
                if p.critical { "   <- CRITICAL" } else { "" }
            );
            line(
                out,
                &if p.critical {
                    paint(out, Colour::Red, &row)
                } else {
                    row
                },
            );
        }
    }
    let (to_kill, skipped) = kill_plan(&report.matched, args.include_critical, &args.pids);
    let critical: Vec<&ProcessInfo> = to_kill
        .iter()
        .copied()
        .filter(|p| p.critical || is_protected_process(p.pid, &p.name))
        .collect();
    if !critical.is_empty() {
        // Never answered by --yes, never without a terminal.
        out.warn("this matches critical system processes. Killing them can crash Windows.");
        if !io::stdin().is_terminal() {
            return Err(Error::InvalidInput(
                "--include-critical needs an interactive answer in a terminal (--yes does not count)"
                    .into(),
            ));
        }
        confirm(
            false,
            &format!("Really kill {} CRITICAL system processes?", critical.len()),
        )?;
    }
    if !skipped.is_empty() {
        out.warn(&format!(
            "{} critical system processes are skipped (add --include-critical to kill them).",
            skipped.len()
        ));
    }
    let normal = to_kill.len() - critical.len();
    if normal > 0 {
        confirm(args.yes, &format!("Kill {normal} processes?"))?;
    }
    for p in &skipped {
        report.results.push(KillResult {
            name: p.name.clone(),
            pid: p.pid,
            killed: false,
            skipped: true,
            error: None,
        });
    }
    for p in &to_kill {
        let result = psys::kill(p.pid);
        report.results.push(KillResult {
            name: p.name.clone(),
            pid: p.pid,
            killed: result.is_ok(),
            skipped: false,
            error: result.err().map(|e| e.to_string()),
        });
    }
    out.emit(&report, |r, out| {
        for k in &r.results {
            if k.skipped {
                line(
                    out,
                    &paint(
                        out,
                        Colour::Yellow,
                        &format!("  Skipped (critical): {} (PID {})", k.name, k.pid),
                    ),
                );
                continue;
            }
            match &k.error {
                None => out.ok(&format!("  Killed: {} (PID {})", k.name, k.pid)),
                Some(e) => line(
                    out,
                    &paint(
                        out,
                        Colour::Red,
                        &format!("  Failed: {} (PID {}) - {e}", k.name, k.pid),
                    ),
                ),
            }
        }
    });
    let attempted = report.results.iter().filter(|k| !k.skipped).count();
    let failed = report
        .results
        .iter()
        .filter(|k| !k.skipped && !k.killed)
        .count();
    if failed > 0 {
        return Err(Error::Other(format!(
            "{failed} of {attempted} processes could not be killed"
        )));
    }
    let skipped = report.results.len() - attempted;
    if attempted == 0 && skipped > 0 {
        return Err(Error::Other(format!(
            "{skipped} matching processes are protected; nothing was terminated"
        )));
    }
    Ok(())
}

// ---------------------------------------------------------------- devices

/// USB.ps1: devices grouped by class.
fn usb(out: &Out) -> Result<()> {
    let devices = psys::usb_devices()?;
    out.emit(&devices, |d, out| {
        out.title("=== USB DEVICES ===");
        blank(out);
        if d.is_empty() {
            out.warn("No USB devices detected.");
            return;
        }
        for (class, names) in group_usb(d) {
            section(out, &class);
            for n in names {
                line(out, &paint(out, Colour::Green, &format!("    {n}")));
            }
            blank(out);
        }
        line(
            out,
            &paint(
                out,
                Colour::Cyan,
                &format!("  Total: {} USB devices", d.len()),
            ),
        );
    });
    Ok(())
}

/// Disks.ps1: physical disks, then logical volumes.
fn disks(out: &Out) -> Result<()> {
    let report = psys::disks()?;
    out.emit(&report, |r, out| {
        out.title("=== DISKS INFO ===");
        blank(out);
        section(out, "Physical Disks");
        if r.physical.is_empty() {
            out.warn("Could not enumerate physical disks.");
        } else {
            let header = format!(
                "    {:<25} {:<10} {:<10} {:<12} {:<10}",
                "MODEL", "TYPE", "SIZE", "HEALTH", "TEMP"
            );
            line(out, &paint(out, Colour::Dim, &header));
            for d in &r.physical {
                let row = format!(
                    "    {:<25} {:<10} {:<10} {:<12} {:<10}",
                    truncate(&d.model, 23, ".."),
                    d.media_type,
                    format!("{} GB", round_to(gib(d.size_bytes), 0)),
                    d.health,
                    d.temperature_c
                        .map_or_else(|| "-".to_string(), |t| format!("{t} C"))
                );
                let colour = health_level(&d.health).map_or(Colour::Dim, level_colour);
                line(out, &paint(out, colour, &row));
            }
        }
        blank(out);
        section(out, "Logical Volumes");
        let header = format!(
            "    {:<6} {:<15} {:<12} {:<12} {:<8}",
            "DRIVE", "LABEL", "USED", "FREE", "USAGE"
        );
        line(out, &paint(out, Colour::Dim, &header));
        for v in &r.volumes {
            let pct = v.used_pct.round();
            let label = if v.label.is_empty() {
                "(no label)"
            } else {
                &v.label
            };
            let row = format!(
                "    {:<6} {:<15} {:<12} {:<12} {:<8}",
                v.drive,
                truncate(label, 15, ""),
                format!("{} GB", gb1(v.total_bytes - v.free_bytes)),
                format!("{} GB", gb1(v.free_bytes)),
                format!("{pct}%")
            );
            line(out, &paint(out, level_colour(disk_level(pct)), &row));
        }
        blank(out);
    });
    Ok(())
}

/// Monitors.ps1: screens, best listed mode and EDID identity per monitor.
fn monitors(out: &Out) -> Result<()> {
    let report = psys::monitors()?;
    out.emit(&report, |r, out| {
        out.title("=== CONNECTED MONITORS ===");
        for s in &r.screens {
            blank(out);
            let primary = if s.primary { " (PRIMARY)" } else { "" };
            line(
                out,
                &paint(
                    out,
                    Colour::Yellow,
                    &format!("[Monitor {}]{primary}", s.index),
                ),
            );
            out.field("Name", &s.device_name);
            let hz = s
                .refresh_hz
                .map(|h| format!(" @ {h} Hz"))
                .unwrap_or_default();
            out.field(
                "Resolution",
                &paint(
                    out,
                    Colour::Green,
                    &format!("{} x {}{hz}", s.width, s.height),
                ),
            );
            out.field("Position", &format!("X={}  Y={}", s.x, s.y));
            out.field(
                "Work area",
                &format!("{} x {}", s.work_width, s.work_height),
            );
        }
        blank(out);
        out.title("=== REFRESH RATE (highest listed mode) ===");
        for m in r.physical.iter().filter_map(|m| m.best_mode) {
            line(
                out,
                &paint(
                    out,
                    Colour::Green,
                    &format!("  {} x {} @ {}Hz", m.width, m.height, m.refresh_hz),
                ),
            );
        }
        blank(out);
        out.title("=== PHYSICAL INFO ===");
        for m in &r.physical {
            line(
                out,
                &format!(
                    "  Manufacturer: {}  |  Model: {}  |  S/N: {}",
                    m.manufacturer, m.model, m.serial
                ),
            );
        }
        blank(out);
    });
    Ok(())
}

// ---------------------------------------------------------------- monitors-arrange

fn mode_text(m: &DisplayMode) -> String {
    format!("{}x{} @ {} Hz", m.width, m.height, m.refresh_hz)
}

fn print_displays(list: &[Display], out: &Out) {
    out.title("=== MONITOR CONFIGURATOR ===");
    blank(out);
    section(out, "Displays");
    for d in list {
        let current = d
            .current
            .as_ref()
            .map_or_else(|| "-".to_string(), mode_text);
        let primary = if d.primary { "  (PRIMARY)" } else { "" };
        line(
            out,
            &paint(
                out,
                Colour::Green,
                &format!(
                    "    {}) {}  [{}]  {current}{primary}",
                    d.index, d.name, d.device_name
                ),
            ),
        );
    }
    blank(out);
}

/// MonitorsArrange.ps1 steps: display, resolution, refresh rate; then CDS_TEST and apply.
fn monitors_arrange(args: ArrangeArgs, out: &Out) -> Result<()> {
    let list = psys::displays()?;
    if list.is_empty() {
        return Err(Error::Other("no display is attached to the desktop".into()));
    }
    let ask = interactive(out);

    // Step 1: display.
    let index = match args.display {
        Some(i) => i,
        None => {
            if !ask {
                out.emit(&list, |l, out| print_displays(l, out));
                return Ok(());
            }
            print_displays(&list, out);
            match ask_number("Display number (Enter to cancel)", list.len())? {
                Some(n) => list[n - 1].index,
                None => return Err(Error::Other("cancelled".into())),
            }
        }
    };
    let display = list.iter().find(|d| d.index == index).ok_or_else(|| {
        Error::InvalidInput(format!("display {index} does not exist (1-{})", list.len()))
    })?;

    // Step 2: resolution.
    let all_res = resolutions(&display.modes);
    let (width, height) = match &args.resolution {
        Some(r) => parse_resolution(r)?,
        None => {
            if !ask {
                out.emit(display, |d, out| {
                    out.title(&format!("  Display {}: {}", d.index, d.name));
                    for (w, h) in resolutions(&d.modes) {
                        let rates: Vec<String> = refresh_rates(&d.modes, w, h)
                            .iter()
                            .map(u32::to_string)
                            .collect();
                        line(out, &format!("    {w}x{h}  ({} Hz)", rates.join(", ")));
                    }
                });
                return Ok(());
            }
            line(
                out,
                &paint(out, Colour::Yellow, "  --- Select resolution ---"),
            );
            for (i, (w, h)) in all_res.iter().enumerate() {
                line(
                    out,
                    &paint(out, Colour::Green, &format!("    {:>2}) {w}x{h}", i + 1)),
                );
            }
            match ask_number("Resolution number (Enter to cancel)", all_res.len())? {
                Some(n) => all_res[n - 1],
                None => return Err(Error::Other("cancelled".into())),
            }
        }
    };

    // Step 3: refresh rate.
    let hz = match args.hz {
        Some(hz) => Some(hz),
        None if ask && args.resolution.is_none() => {
            let rates = refresh_rates(&display.modes, width, height);
            line(
                out,
                &paint(out, Colour::Yellow, "  --- Select refresh rate ---"),
            );
            for (i, r) in rates.iter().enumerate() {
                line(
                    out,
                    &paint(out, Colour::Green, &format!("    {}) {r} Hz", i + 1)),
                );
            }
            ask_number("Refresh rate number (Enter for the highest)", rates.len())?
                .map(|n| rates[n - 1])
        }
        None => None,
    };
    let mode = pick_mode(&display.modes, width, height, hz)?;

    // Step 4: apply (CDS_TEST first, inside set_mode).
    if !args.test {
        confirm(
            args.yes,
            &format!(
                "Change display {} ({}) to {}?",
                display.index,
                display.name,
                mode_text(&mode)
            ),
        )?;
    }
    let change = psys::set_mode(&display.device_name, mode, args.test)?;
    out.emit(&change, |c, out| {
        if c.test_only {
            out.ok(&format!(
                "  The driver accepts {} on {} (test only, nothing changed).",
                mode_text(&c.mode),
                c.device_name
            ));
        } else if c.restart_required {
            out.warn(&format!(
                "{} set on {}; Windows needs a restart to apply it.",
                mode_text(&c.mode),
                c.device_name
            ));
        } else {
            out.ok(&format!(
                "  {} now runs at {}.",
                c.device_name,
                mode_text(&c.mode)
            ));
        }
    });
    Ok(())
}

// ---------------------------------------------------------------- battery / startup

/// Battery.ps1.
fn battery(out: &Out) -> Result<()> {
    let info = psys::battery()?;
    out.emit(&info, |b, out| {
        let Some(b) = b else {
            out.warn("No battery detected (desktop PC?).");
            return;
        };
        out.title("=== BATTERY STATUS ===");
        blank(out);
        let pct = f64::from(b.charge_pct);
        out.field(
            "Charge",
            &paint(
                out,
                level_colour(charge_level(pct)),
                &format!("{} {}%", bar(pct), b.charge_pct),
            ),
        );
        out.field("Status", &b.status);
        out.field(
            "Time",
            &b.runtime_min.map_or_else(
                || "(calculating or plugged in)".to_string(),
                |m| format!("{}h {}m remaining", m / 60, m % 60),
            ),
        );
        if let (Some(h), Some(d), Some(f)) = (
            b.health_pct,
            b.designed_capacity_mwh,
            b.full_charged_capacity_mwh,
        ) {
            out.field(
                "Health",
                &paint(
                    out,
                    level_colour(battery_health_level(h)),
                    &format!("{h}%  (designed: {d} mWh / current: {f} mWh)"),
                ),
            );
        }
        blank(out);
        line(
            out,
            &paint(
                out,
                Colour::Dim,
                "  Tip: full detailed report -> powercfg /batteryreport",
            ),
        );
    });
    Ok(())
}

/// Startup.ps1: entries grouped by source, commands cut at 100 characters.
fn startup(out: &Out) -> Result<()> {
    let entries = psys::startup_entries()?;
    out.emit(&entries, |list, out| {
        out.title("=== STARTUP PROGRAMS ===");
        blank(out);
        if list.is_empty() {
            out.warn("No startup programs found.");
            return;
        }
        let mut sources: Vec<&str> = Vec::new();
        for e in list {
            if !sources.contains(&e.source.as_str()) {
                sources.push(&e.source);
            }
        }
        for source in sources {
            section(out, source);
            for e in list.iter().filter(|e| e.source == source) {
                line(out, &paint(out, Colour::Green, &format!("    {}", e.name)));
                line(
                    out,
                    &paint(
                        out,
                        Colour::Dim,
                        &format!("      {}", truncate(&e.command, 100, "...")),
                    ),
                );
            }
            blank(out);
        }
        line(
            out,
            &paint(
                out,
                Colour::Cyan,
                &format!("  Total: {} startup entries", list.len()),
            ),
        );
        line(
            out,
            &paint(
                out,
                Colour::Dim,
                "  Tip: use Task Manager > Startup tab to enable/disable items.",
            ),
        );
    });
    Ok(())
}

// ---------------------------------------------------------------- audio

fn audio_list(devices: &[AudioDevice], out: &Out) {
    for d in devices {
        if d.default {
            line(
                out,
                &paint(out, Colour::Green, &format!("[DEFAULT] {}", d.name)),
            );
        } else {
            line(
                out,
                &paint(out, Colour::Dim, &format!("          {}", d.name)),
            );
        }
    }
}

/// Audio.ps1 through Core Audio.
fn audio(out: &Out) -> Result<()> {
    let report = psys::audio_devices()?;
    out.emit(&report, |r, out| {
        out.title("=== AUDIO DEVICES ===");
        blank(out);
        line(
            out,
            &paint(out, Colour::Yellow, "--- PLAYBACK (Output) ---"),
        );
        audio_list(&r.playback, out);
        blank(out);
        line(
            out,
            &paint(out, Colour::Yellow, "--- RECORDING (Input) ---"),
        );
        audio_list(&r.recording, out);
        if let Some(v) = r.default_volume_pct {
            let muted = if r.default_muted == Some(true) {
                " (MUTED)"
            } else {
                ""
            };
            blank(out);
            line(
                out,
                &paint(
                    out,
                    Colour::Cyan,
                    &format!("Default output volume: {v}%{muted}"),
                ),
            );
        }
    });
    Ok(())
}

/// ServicesAudio.ps1, plus the restart its tip suggests.
fn services_audio(args: ServicesAudioArgs, out: &Out) -> Result<()> {
    if args.restart {
        let desc = AUDIO_SERVICES
            .iter()
            .find(|(n, _)| *n == AUDIO_RESTART_SERVICE)
            .map_or("", |(_, d)| d);
        avlt_platform::admin::require_admin()?;
        confirm(
            args.yes,
            &format!("Restart {desc} ({AUDIO_RESTART_SERVICE})? Audio stops for a moment."),
        )?;
        let status = psys::restart_service(AUDIO_RESTART_SERVICE)?;
        out.ok(&format!("  {} restarted: {}", status.name, status.status));
    }
    let list = psys::audio_services()?;
    out.emit(&list, |l, out| {
        out.title("=== AUDIO SERVICES ===");
        blank(out);
        line(
            out,
            &paint(
                out,
                Colour::Yellow,
                &format!("  {:<30} {:<15} {}", "SERVICE", "STATUS", "DESCRIPTION"),
            ),
        );
        for s in l {
            let colour = match s.status.as_str() {
                "Running" => Colour::Green,
                "Stopped" => Colour::Red,
                _ => Colour::Yellow,
            };
            line(
                out,
                &paint(
                    out,
                    colour,
                    &format!("  {:<30} {:<15} {}", s.name, s.status, s.description),
                ),
            );
        }
        if !args.restart {
            blank(out);
            line(
                out,
                &paint(
                    out,
                    Colour::Dim,
                    "  Tip: to restart audio, run as Admin: avlt sys services-audio --restart",
                ),
            );
        }
    });
    Ok(())
}

// ---------------------------------------------------------------- cleanup

/// Cleanup.ps1 / MaintenancePage (without Prefetch).
fn cleanup(args: CleanupArgs, out: &Out) -> Result<()> {
    if args.analyze {
        let analysis = psys::analyze_cleanup()?;
        out.emit(&analysis, |a, out| {
            out.title("=== SYSTEM CLEANUP (analyze) ===");
            blank(out);
            for t in &a.targets {
                line(
                    out,
                    &format!(
                        "  {:<25} {:>10}  ({} files)",
                        t.name,
                        freed_text(t.size_bytes),
                        t.files
                    ),
                );
            }
            line(
                out,
                &format!(
                    "  {:<25} {:>10}  ({} items)",
                    "Recycle Bin",
                    freed_text(a.recycle_bin.bytes),
                    a.recycle_bin.items
                ),
            );
        });
        return Ok(());
    }
    if !out.json {
        out.title("=== SYSTEM CLEANUP ===");
        blank(out);
        line(
            out,
            &paint(
                out,
                Colour::Yellow,
                "  This deletes temp files, browser caches and empties the Recycle Bin.",
            ),
        );
        line(
            out,
            &paint(
                out,
                Colour::Dim,
                "  Files currently in use are skipped automatically.",
            ),
        );
        blank(out);
    }
    confirm(args.yes, "Continue?")?;
    let report = psys::cleanup()?;
    out.emit(&report, |r, out| {
        for i in &r.items {
            let row = match i.status {
                CleanStatus::Freed => paint(
                    out,
                    Colour::Green,
                    &format!(
                        "  {:<25} {:>8} MB freed",
                        i.name,
                        round_to(mib(i.stats.freed_bytes), 1)
                    ),
                ),
                CleanStatus::Locked => paint(
                    out,
                    Colour::Yellow,
                    &format!("  {:<25} in use / locked (skipped)", i.name),
                ),
                CleanStatus::AlreadyClean => {
                    paint(out, Colour::Dim, &format!("  {:<25} already clean", i.name))
                }
                CleanStatus::Unsafe => paint(
                    out,
                    Colour::Red,
                    &format!(
                        "  {:<25} skipped ({})",
                        i.name,
                        i.skipped_reason.as_deref().unwrap_or("unsafe target")
                    ),
                ),
            };
            line(out, &row);
        }
        line(
            out,
            &if r.recycle_bin_emptied {
                paint(
                    out,
                    Colour::Green,
                    &format!(
                        "  {:<25} emptied ({})",
                        "Recycle Bin",
                        freed_text(r.recycle_bin.bytes)
                    ),
                )
            } else {
                paint(
                    out,
                    Colour::Dim,
                    &format!("  {:<25} already empty or skipped", "Recycle Bin"),
                )
            },
        );
        blank(out);
        line(
            out,
            &paint(
                out,
                Colour::Cyan,
                &format!("  TOTAL FREED: {}", freed_text(r.total_freed_bytes)),
            ),
        );
    });
    Ok(())
}
