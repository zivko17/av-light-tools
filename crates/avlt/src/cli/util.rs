//! `avlt util ...`: Utilities (Backup.ps1, Search.ps1, Weather.ps1, Timer.ps1;
//! NetMaint MaintenancePage.cs backup).

use std::io::{self, BufRead, IsTerminal, Write};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use avlt_core::sys::format::{countdown_level, freed_text, round_to};
use avlt_core::sys::fsops;
use avlt_core::sys::weather::{WeatherFormat, weather_url};
use avlt_core::sys::{format_countdown, parse_duration};
use avlt_core::{Error, Result};
use avlt_platform::sys as psys;
use clap::{Args, Subcommand, ValueEnum};
use serde::Serialize;

use super::live::{Colour, block_on, level_colour, paint};
use super::output::Out;
use super::{env, json_line, line};

/// `avlt util` subcommands.
#[derive(Debug, Subcommand)]
pub enum UtilCmd {
    /// Copy a file or folder to <dest>/<name>_backup_<yyyy-MM-dd_HH-mm>
    Backup(BackupArgs),
    /// Find files and folders by name
    Search(SearchArgs),
    /// Weather report from wttr.in
    Weather(WeatherArgs),
    /// Countdown timer (5m, 30s, 1h30m, 90s; a bare number is minutes)
    Timer(TimerArgs),
}

/// `util backup` arguments.
#[derive(Debug, Args)]
pub struct BackupArgs {
    /// File or folder to back up
    pub source: String,
    /// Folder that receives the backup (created if missing)
    pub dest: String,
}

/// `util search` arguments.
#[derive(Debug, Args)]
pub struct SearchArgs {
    /// Part of the name; * and ? work as wildcards
    pub pattern: String,
    /// Folder to search (default: the current folder)
    pub path: Option<PathBuf>,
}

/// wttr.in output style.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum FormatArg {
    /// Three-day forecast
    Full,
    /// Data-rich report
    Short,
    /// One line
    Mini,
}

/// `util weather` arguments.
#[derive(Debug, Args)]
pub struct WeatherArgs {
    /// City (default: avlt.toml `weather_city`, else Madrid)
    pub city: Option<String>,
    /// Report style
    #[arg(short, long, value_enum, default_value_t = FormatArg::Short)]
    pub format: FormatArg,
}

/// `util timer` arguments.
#[derive(Debug, Args)]
pub struct TimerArgs {
    /// Duration: 5m, 30s, 1h30m, 90s (a bare number is minutes)
    pub duration: String,
    /// Name shown next to the countdown
    #[arg(short, long, default_value = "Timer")]
    pub label: String,
    /// Do not beep at the end
    #[arg(long)]
    pub no_beep: bool,
}

/// Runs an `avlt util` subcommand.
pub fn run(cmd: UtilCmd, out: &Out) -> Result<()> {
    match cmd {
        UtilCmd::Backup(a) => backup(a, out),
        UtilCmd::Search(a) => search(a, out),
        UtilCmd::Weather(a) => weather(a, out),
        UtilCmd::Timer(a) => timer(a, out),
    }
}

/// Backup.ps1 strips surrounding quotes and blanks from pasted paths.
fn clean_path(s: &str) -> PathBuf {
    PathBuf::from(s.trim().trim_matches('"').trim())
}

// ---------------------------------------------------------------- backup

/// Backup.ps1 / MaintenancePage: copy into a new dated folder, never overwriting.
fn backup(args: BackupArgs, out: &Out) -> Result<()> {
    let source = clean_path(&args.source);
    let dest = clean_path(&args.dest);
    if dest.as_os_str().is_empty() {
        return Err(Error::InvalidInput("choose a destination folder".into()));
    }
    if !dest.exists() && !out.json {
        line(
            out,
            &paint(
                out,
                Colour::Yellow,
                &format!("  Creating destination folder: {}", dest.display()),
            ),
        );
    }
    let stamp = chrono::Local::now().format("%Y-%m-%d_%H-%M").to_string();
    let started = Instant::now();
    let report = fsops::backup(&source, &dest, &stamp)?;
    let elapsed = started.elapsed().as_secs();
    out.emit(&report, |r, out| {
        line(out, "");
        out.title("  Backing up:");
        out.field("Source", &r.source.display().to_string());
        out.field("Destination", &r.destination.display().to_string());
        out.field(
            "Copied",
            &paint(
                out,
                Colour::Dim,
                &format!("{} files, {}", r.files, freed_text(r.bytes)),
            ),
        );
        line(out, "");
        out.ok(&format!(
            "  Done in {}h {}m {}s",
            elapsed / 3600,
            elapsed % 3600 / 60,
            elapsed % 60
        ));
        line(
            out,
            &paint(
                out,
                Colour::Cyan,
                &format!("  Backup saved to: {}", r.destination.display()),
            ),
        );
    });
    if !report.failed.is_empty() {
        for f in report.failed.iter().take(10) {
            out.warn(&format!("not copied: {f}"));
        }
        return Err(Error::Other(format!(
            "{} files could not be copied; the backup is incomplete",
            report.failed.len()
        )));
    }
    Ok(())
}

// ---------------------------------------------------------------- search

fn local_time(unix: i64) -> String {
    chrono::DateTime::from_timestamp(unix, 0).map_or_else(
        || "-".to_string(),
        |t| {
            t.with_timezone(&chrono::Local)
                .format("%Y-%m-%d %H:%M")
                .to_string()
        },
    )
}

/// Search.ps1: numbered matches; in a terminal, the chosen one opens.
fn search(args: SearchArgs, out: &Out) -> Result<()> {
    let root = match args.path {
        Some(p) => p,
        None => std::env::current_dir()?,
    };
    if !out.json {
        out.title(&format!(
            "Searching files named like '{}' under '{}'...",
            args.pattern,
            root.display()
        ));
    }
    let hits = fsops::search(&root, &args.pattern)?;
    out.emit(&hits, |h, out| {
        for (i, hit) in h.iter().enumerate() {
            line(
                out,
                &format!(
                    "{} {}",
                    paint(out, Colour::Yellow, &format!("[{}]", i + 1)),
                    paint(out, Colour::Green, &hit.path.display().to_string())
                ),
            );
            let size = if hit.is_dir {
                "folder".to_string()
            } else {
                format!("{} KB", round_to(hit.size_bytes as f64 / 1024.0, 2))
            };
            let modified = hit
                .modified_unix
                .map_or_else(|| "-".to_string(), local_time);
            line(
                out,
                &paint(
                    out,
                    Colour::Dim,
                    &format!("     Size: {size}  |  Modified: {modified}"),
                ),
            );
        }
    });
    if hits.is_empty() {
        out.warn("No files found.");
        return Ok(());
    }
    if out.json || !io::stdin().is_terminal() {
        return Ok(());
    }
    let mut err = io::stderr().lock();
    let _ = write!(
        err,
        "\nType the number of the file to open (or Enter to exit): "
    );
    let _ = err.flush();
    drop(err);
    let mut answer = String::new();
    io::stdin().lock().read_line(&mut answer)?;
    let answer = answer.trim();
    if answer.is_empty() {
        return Ok(());
    }
    match answer.parse::<usize>() {
        Ok(n) if (1..=hits.len()).contains(&n) => psys::open_path(&hits[n - 1].path),
        _ => Err(Error::InvalidInput("number out of range".into())),
    }
}

// ---------------------------------------------------------------- weather

/// `util weather --json` result.
#[derive(Debug, Serialize)]
struct WeatherReport {
    city: String,
    url: String,
    report: String,
}

/// Weather.ps1: wttr.in in English, metric.
fn weather(args: WeatherArgs, out: &Out) -> Result<()> {
    let city = match args.city {
        Some(c) if !c.trim().is_empty() => c.trim().to_string(),
        _ => env::load_config()?.weather_city().to_string(),
    };
    let format = match args.format {
        FormatArg::Full => WeatherFormat::Full,
        FormatArg::Short => WeatherFormat::Short,
        FormatArg::Mini => WeatherFormat::Mini,
    };
    // Terminal colours only when stdout shows them (and never inside JSON).
    let url = weather_url(&city, format, out.color && !out.json);
    let report = psys::fetch_weather(&url)?;
    let result = WeatherReport { city, url, report };
    out.emit(&result, |r, out| {
        line(out, "");
        line(out, &r.report);
        line(out, "");
    });
    Ok(())
}

// ---------------------------------------------------------------- timer

/// One `util timer --json` line.
#[derive(Debug, Serialize)]
struct TimerTick<'a> {
    label: &'a str,
    remaining_s: u64,
    done: bool,
}

/// Timer.ps1: live countdown, then three beeps.
fn timer(args: TimerArgs, out: &Out) -> Result<()> {
    let total = parse_duration(&args.duration)?;
    let label = args.label.as_str();
    let ends = chrono::Local::now()
        + chrono::Duration::from_std(total).unwrap_or(chrono::Duration::zero());
    if !out.json {
        line(out, "");
        out.title(&format!(
            "  {label} started - ends at {}",
            ends.format("%H:%M:%S")
        ));
        line(out, &paint(out, Colour::Dim, "  Press Ctrl+C to cancel"));
        line(out, "");
    }
    let tty = out.color;
    let end = Instant::now() + total;
    let finished = block_on(async {
        // If Ctrl+C cannot be hooked the timer just runs until killed.
        let ctrl_c = async {
            if tokio::signal::ctrl_c().await.is_err() {
                std::future::pending::<()>().await;
            }
        };
        tokio::pin!(ctrl_c);
        loop {
            let now = Instant::now();
            if now >= end {
                return true;
            }
            let remaining = end - now;
            if out.json {
                json_line(&TimerTick {
                    label,
                    remaining_s: remaining.as_secs_f64().ceil() as u64,
                    done: false,
                });
            } else if tty {
                let colour = level_colour(countdown_level(remaining.as_secs_f64()));
                let text = paint(
                    out,
                    colour,
                    &format!("  {label} : {}    ", format_countdown(remaining)),
                );
                let mut stdout = io::stdout().lock();
                let _ = write!(stdout, "\r{text}");
                let _ = stdout.flush();
            }
            // Wake when the displayed second changes (or at the end).
            let step = match remaining.subsec_nanos() {
                0 => Duration::from_secs(1),
                n => Duration::from_nanos(u64::from(n)),
            };
            tokio::select! {
                biased;
                () = &mut ctrl_c => return false,
                () = tokio::time::sleep(step) => {}
            }
        }
    })?;
    if !finished {
        if tty && !out.json {
            line(out, "");
        }
        return Err(Error::Other("cancelled".into()));
    }
    if out.json {
        json_line(&TimerTick {
            label,
            remaining_s: 0,
            done: true,
        });
    } else {
        let prefix = if tty { "\r" } else { "" };
        line(
            out,
            &format!(
                "{prefix}{}",
                paint(out, Colour::Green, &format!("  {label} : DONE!        "))
            ),
        );
        line(out, "");
    }
    if !args.no_beep {
        for _ in 0..3 {
            // No speaker or audio device (CI, RDP): the timer still succeeds.
            let _ = psys::beep(1000, 300);
            std::thread::sleep(Duration::from_millis(200));
        }
    }
    Ok(())
}
