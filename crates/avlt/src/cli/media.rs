//! `avlt media ...`: audio toolkit on top of ffmpeg / ffprobe. Port of `AudioTools.ps1`
//! (`Invoke-AudioConvert`, `Invoke-AudioNormalize`, `Invoke-AudioTrim`,
//! `Invoke-ExtractAudio`, `Invoke-AudioResample`, `Invoke-AudioChannels`, `Get-AudioProbe`).
//!
//! Outputs go to `--out`, else the Music known folder (`<Music>\AVLT`, or `_OUTPUT` when
//! portable). An existing file is never overwritten: the name gets `_1`, `_2`, ...
//! The interactive `audiotools` menu is not ported; the GUI covers it.

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use avlt_core::media::{self, Channels, ExtractFormat, MediaOp, MediaReport, Preset, SAMPLE_RATES};
use avlt_core::{Error, Result};
use avlt_platform::media as pmedia;
use avlt_platform::paths::KnownFolder;
use clap::builder::{PossibleValue, PossibleValuesParser};
use clap::{Args, Subcommand};

use super::env;
use super::line;
use super::output::Out;

/// `avlt media` subcommands.
#[derive(Debug, Subcommand)]
pub enum MediaCmd {
    /// Convert audio files to a preset (MP3, AAC, Opus, FLAC, WAV)
    Convert(ConvertArgs),
    /// Normalize loudness to -23 LUFS (two-pass EBU R128)
    Normalize(InputArgs),
    /// Cut a file between two times without re-encoding
    Trim(TrimArgs),
    /// Extract the audio track of video files
    Extract(ExtractArgs),
    /// Change the sample rate
    Resample(ResampleArgs),
    /// Convert to mono or stereo
    Channels(ChannelsArgs),
    /// Show codec, sample rate, channels and duration of a file
    Probe(ProbeArgs),
}

/// A file or folder of files, and where to write.
#[derive(Debug, Args)]
pub struct InputArgs {
    /// Input file, or a folder (every audio/video file under it is processed)
    pub input: PathBuf,
    /// Output folder (default: <Music>\AVLT, or _OUTPUT next to avlt.exe when portable)
    #[arg(short, long)]
    pub out: Option<PathBuf>,
}

/// `media convert` arguments.
#[derive(Debug, Args)]
pub struct ConvertArgs {
    #[command(flatten)]
    pub files: InputArgs,
    /// Output preset
    #[arg(short, long, value_parser = preset_values())]
    pub preset: String,
    /// Also set the sample rate (Hz)
    #[arg(long, value_name = "HZ", value_parser = clap::value_parser!(u32).range(1..))]
    pub sample_rate: Option<u32>,
    /// Also force mono or stereo
    #[arg(long, value_parser = ["mono", "stereo"])]
    pub channels: Option<String>,
}

/// `media trim` arguments.
#[derive(Debug, Args)]
pub struct TrimArgs {
    #[command(flatten)]
    pub files: InputArgs,
    /// Start time (HH:MM:SS or seconds)
    #[arg(long, default_value = "00:00:00")]
    pub start: String,
    /// End time (default: to the end of the file)
    #[arg(long)]
    pub end: Option<String>,
}

/// `media extract` arguments.
#[derive(Debug, Args)]
pub struct ExtractArgs {
    #[command(flatten)]
    pub files: InputArgs,
    /// Output format (`copy` keeps the audio stream untouched, in an .m4a)
    #[arg(long, default_value = "mp3", value_parser = ["mp3", "flac", "wav", "copy"])]
    pub format: String,
}

/// `media resample` arguments.
#[derive(Debug, Args)]
pub struct ResampleArgs {
    #[command(flatten)]
    pub files: InputArgs,
    /// Target sample rate in Hz
    #[arg(long, default_value = "48000", value_parser = ["44100", "48000", "96000", "192000"])]
    pub rate: String,
}

/// `media channels` arguments.
#[derive(Debug, Args)]
pub struct ChannelsArgs {
    #[command(flatten)]
    pub files: InputArgs,
    /// Channel layout
    #[arg(long, value_parser = ["mono", "stereo"])]
    pub mode: String,
}

/// `media probe` arguments.
#[derive(Debug, Args)]
pub struct ProbeArgs {
    /// Media file
    pub file: PathBuf,
}

/// The preset names with their descriptions, for `--help` and validation.
fn preset_values() -> PossibleValuesParser {
    PossibleValuesParser::new(
        Preset::ALL.map(|p| PossibleValue::new(p.name()).help(p.description())),
    )
}

/// Runs an `avlt media` subcommand.
pub fn run(cmd: MediaCmd, out: &Out) -> Result<()> {
    match cmd {
        MediaCmd::Convert(a) => convert(&a, out),
        MediaCmd::Normalize(a) => normalize(&a, out),
        MediaCmd::Trim(a) => trim(&a, out),
        MediaCmd::Extract(a) => extract(&a, out),
        MediaCmd::Resample(a) => resample(&a, out),
        MediaCmd::Channels(a) => channels(&a, out),
        MediaCmd::Probe(a) => probe(&a, out),
    }
}

fn convert(a: &ConvertArgs, out: &Out) -> Result<()> {
    let preset: Preset = a.preset.parse()?;
    let channels = a
        .channels
        .as_deref()
        .map(str::parse::<Channels>)
        .transpose()?;
    let op = MediaOp::Convert {
        preset,
        sample_rate: a.sample_rate,
        channels,
    };
    run_batch(&a.files, out, &op)
}

fn normalize(a: &InputArgs, out: &Out) -> Result<()> {
    run_batch(a, out, &MediaOp::Normalize)
}

fn trim(a: &TrimArgs, out: &Out) -> Result<()> {
    if media::strip_quotes(&a.files.input).is_dir() {
        return Err(Error::InvalidInput(
            "trim takes a single file, not a folder".into(),
        ));
    }
    let op = MediaOp::Trim {
        start: a.start.clone(),
        end: a.end.clone(),
    };
    run_batch(&a.files, out, &op)
}

fn extract(a: &ExtractArgs, out: &Out) -> Result<()> {
    let format: ExtractFormat = a.format.parse()?;
    run_batch(&a.files, out, &MediaOp::Extract(format))
}

fn resample(a: &ResampleArgs, out: &Out) -> Result<()> {
    let rate: u32 = a
        .rate
        .parse()
        .ok()
        .filter(|r| SAMPLE_RATES.contains(r))
        .ok_or_else(|| Error::InvalidInput(format!("unsupported sample rate '{}'", a.rate)))?;
    run_batch(&a.files, out, &MediaOp::Resample(rate))
}

fn channels(a: &ChannelsArgs, out: &Out) -> Result<()> {
    let mode: Channels = a.mode.parse()?;
    run_batch(&a.files, out, &MediaOp::Channels(mode))
}

/// Shared flow: find ffmpeg (exit 3 with the winget hint if missing), expand the input,
/// pick and create the output folder, then run each file and report.
fn run_batch(files: &InputArgs, out: &Out, op: &MediaOp) -> Result<()> {
    let ffmpeg = pmedia::ffmpeg()?;
    let inputs = media::collect_inputs(&files.input)?;
    let dir = match &files.out {
        Some(d) => d.clone(),
        None => env::output_dir(KnownFolder::Music)?,
    };
    std::fs::create_dir_all(&dir)?;

    out.title(&format!("{} -> {}", op.action(inputs.len()), dir.display()));
    line(out, "");

    let total = inputs.len();
    let never = AtomicBool::new(false);
    let mut results = Vec::with_capacity(total);
    for (i, input) in inputs.iter().enumerate() {
        let name = input
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        line(out, &format!("  [{}/{total}] {name}", i + 1));
        let result = pmedia::run_job(&ffmpeg, op, input, &dir, &never, &mut |w| out.warn(w));
        match (&result.output, &result.error) {
            (Some(path), _) => out.ok(&format!(
                "        OK -> {}  ({})",
                path.file_name()
                    .map_or_else(String::new, |n| n.to_string_lossy().into_owned()),
                format_size(result.size_bytes.unwrap_or(0))
            )),
            (None, reason) => out.err(&format!(
                "{name}: FAILED{}",
                reason
                    .as_ref()
                    .map_or_else(String::new, |r| format!(": {r}"))
            )),
        }
        results.push(result);
    }

    let report = MediaReport::new(op.operation(), &dir, results);
    out.emit(&report, |r, out| {
        line(out, "");
        out.title(&format!("Done: {} ok, {} failed", r.ok, r.failed));
        out.field("Output", &r.output_dir.display().to_string());
    });
    if report.failed > 0 {
        return Err(Error::Other(format!(
            "{} of {} files failed",
            report.failed, total
        )));
    }
    Ok(())
}

/// `1.5 MB` (MiB, one decimal) as `Get-Item ... / 1MB` showed it.
fn format_size(bytes: u64) -> String {
    format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
}

/// `hh:mm:ss.ff` like `[timespan]::FromSeconds(...)` (centiseconds are truncated).
fn format_duration(seconds: f64) -> String {
    let cs = (seconds.max(0.0) * 100.0 + 1e-6) as u64;
    format!(
        "{:02}:{:02}:{:02}.{:02}",
        cs / 360_000,
        cs / 6000 % 60,
        cs / 100 % 60,
        cs % 100
    )
}

fn probe(a: &ProbeArgs, out: &Out) -> Result<()> {
    let info = pmedia::probe(&a.file)?;
    out.emit(&info, |p, out| {
        out.title("=== AUDIO PROBE ===");
        out.field("File", &p.file);
        if let Some(c) = &p.container {
            out.field("Container", c);
        }
        if let Some(d) = p.duration_s {
            out.field("Duration", &format_duration(d));
        }
        if let Some(s) = p.size_bytes {
            out.field("Size", &format_size(s));
        }
        match &p.codec {
            None => out.warn("no audio stream found"),
            Some(codec) => {
                line(out, "");
                out.field("Codec", codec);
                if let Some(sr) = p.sample_rate_hz {
                    out.field("Sample rate", &format!("{:.1} kHz", f64::from(sr) / 1000.0));
                }
                if let Some(ch) = p.channels {
                    let layout = p
                        .channel_layout
                        .as_ref()
                        .map_or_else(String::new, |l| format!("  ({l})"));
                    out.field("Channels", &format!("{ch}{layout}"));
                }
                if let Some(br) = p.bit_rate_bps {
                    out.field("Bitrate", &format!("{:.0} kbps", br as f64 / 1000.0));
                }
                if let Some(bits) = p.bits_per_sample {
                    out.field("Bit depth", &format!("{bits}-bit"));
                }
            }
        }
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations_truncate_to_centiseconds() {
        assert_eq!(format_duration(0.0), "00:00:00.00");
        assert_eq!(format_duration(61.259), "00:01:01.25");
        assert_eq!(format_duration(3_725.5), "01:02:05.50");
        assert_eq!(format_duration(-3.0), "00:00:00.00");
    }

    #[test]
    fn sizes_use_mebibytes() {
        assert_eq!(format_size(0), "0.0 MB");
        assert_eq!(format_size(1_572_864), "1.5 MB");
    }
}
