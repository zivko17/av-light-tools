//! `avlt media ...`: audio toolkit on top of ffmpeg / ffprobe. Port of `AudioTools.ps1`
//! (`Invoke-AudioConvert`, `Invoke-AudioNormalize`, `Invoke-AudioTrim`,
//! `Invoke-ExtractAudio`, `Invoke-AudioResample`, `Invoke-AudioChannels`, `Get-AudioProbe`).
//!
//! Outputs go to `--out`, else the Music known folder (`<Music>\AVLT`, or `_OUTPUT` when
//! portable). An existing file is never overwritten: the name gets `_1`, `_2`, ...
//! The interactive `audiotools` menu is not ported; the GUI covers it.

use std::path::{Path, PathBuf};

use avlt_core::media::{
    self, Channels, ExtractFormat, JobResult, MediaReport, Preset, SAMPLE_RATES, non_colliding,
};
use avlt_core::{Error, Result};
use avlt_platform::paths::KnownFolder;
use avlt_platform::process::{capture, find_tool, output};
use clap::builder::{PossibleValue, PossibleValuesParser};
use clap::{Args, Subcommand};

use super::env;
use super::line;
use super::output::Out;

/// winget package that provides ffmpeg and ffprobe.
const FFMPEG_WINGET: &str = "Gyan.FFmpeg";

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

/// Outcome of the ffmpeg run(s) for one file: exit code and the program's text.
type Run = (i32, String);

/// Runs ffmpeg for `(ffmpeg, input, output, out)`.
type RunFn<'a> = dyn Fn(&Path, &Path, &Path, &Out) -> Result<Run> + 'a;

/// What one `media` operation does with each input file.
struct Batch<'a> {
    /// Operation name for the report and heading.
    operation: &'a str,
    /// Heading shown before the files (`Converting 3 file(s) to MP3-320`).
    action: String,
    /// Bare output file name for an input.
    name_for: Box<dyn Fn(&Path) -> PathBuf + 'a>,
    /// Runs ffmpeg for `(ffmpeg, input, output)`.
    run: Box<RunFn<'a>>,
}

fn convert(a: &ConvertArgs, out: &Out) -> Result<()> {
    let preset: Preset = a.preset.parse()?;
    let channels = a
        .channels
        .as_deref()
        .map(str::parse::<Channels>)
        .transpose()?;
    let rate = a.sample_rate;
    run_batch(&a.files, out, |n| Batch {
        operation: "convert",
        action: format!("Converting {n} file(s) to {}", preset.label()),
        name_for: Box::new(move |i| media::suffixed_name(i, preset.label(), Some(preset.ext()))),
        run: Box::new(move |ff, i, o, _| {
            let args = media::convert_args_with(i, o, preset, rate, channels);
            run_ffmpeg(ff, &args)
        }),
    })
}

fn normalize(a: &InputArgs, out: &Out) -> Result<()> {
    run_batch(a, out, |n| Batch {
        operation: "normalize",
        action: format!("Normalizing {n} file(s) to -23 LUFS (two-pass)"),
        name_for: Box::new(|i| media::suffixed_name(i, "norm", Some(media::normalize_ext(i)))),
        run: Box::new(|ff, i, o, out| {
            // Pass 1: measure. Pass 2: apply the measured values; if the measurement
            // fails (e.g. silence), fall back to a single pass like the original.
            let (_, text) = run_ffmpeg(ff, &media::normalize_pass1_args(i))?;
            let args = match media::parse_loudnorm_json(&text) {
                Ok(m) => media::normalize_pass2_args(i, o, &m),
                Err(_) => {
                    out.warn("measurement failed, single-pass fallback");
                    media::normalize_single_pass_args(i, o)
                }
            };
            run_ffmpeg(ff, &args)
        }),
    })
}

fn trim(a: &TrimArgs, out: &Out) -> Result<()> {
    if media::strip_quotes(&a.files.input).is_dir() {
        return Err(Error::InvalidInput(
            "trim takes a single file, not a folder".into(),
        ));
    }
    let (start, end) = (a.start.clone(), a.end.clone());
    run_batch(&a.files, out, |n| Batch {
        operation: "trim",
        action: format!("Trimming {n} file(s)"),
        name_for: Box::new(|i| media::suffixed_name(i, "trim", None)),
        run: Box::new(move |ff, i, o, _| {
            run_ffmpeg(ff, &media::trim_args(i, o, &start, end.as_deref()))
        }),
    })
}

fn extract(a: &ExtractArgs, out: &Out) -> Result<()> {
    let format: ExtractFormat = a.format.parse()?;
    run_batch(&a.files, out, |n| Batch {
        operation: "extract",
        action: format!("Extracting audio from {n} file(s)"),
        name_for: Box::new(move |i| media::suffixed_name(i, "audio", Some(format.ext()))),
        run: Box::new(move |ff, i, o, _| run_ffmpeg(ff, &media::extract_args(i, o, format))),
    })
}

fn resample(a: &ResampleArgs, out: &Out) -> Result<()> {
    let rate: u32 = a
        .rate
        .parse()
        .ok()
        .filter(|r| SAMPLE_RATES.contains(r))
        .ok_or_else(|| Error::InvalidInput(format!("unsupported sample rate '{}'", a.rate)))?;
    run_batch(&a.files, out, |n| Batch {
        operation: "resample",
        action: format!("Resampling {n} file(s) to {rate} Hz"),
        name_for: Box::new(move |i| media::suffixed_name(i, &rate.to_string(), None)),
        run: Box::new(move |ff, i, o, _| run_ffmpeg(ff, &media::resample_args(i, o, rate))),
    })
}

fn channels(a: &ChannelsArgs, out: &Out) -> Result<()> {
    let mode: Channels = a.mode.parse()?;
    run_batch(&a.files, out, |n| Batch {
        operation: "channels",
        action: format!("Converting {n} file(s) to {}", mode.name()),
        name_for: Box::new(move |i| media::suffixed_name(i, mode.name(), None)),
        run: Box::new(move |ff, i, o, _| run_ffmpeg(ff, &media::channels_args(i, o, mode))),
    })
}

fn run_ffmpeg(ffmpeg: &Path, args: &[String]) -> Result<Run> {
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    output(ffmpeg, &refs)
}

/// Shared flow: find ffmpeg (exit 3 with the winget hint if missing), expand the input,
/// pick and create the output folder, then run each file and report.
fn run_batch<'a>(
    files: &InputArgs,
    out: &Out,
    make: impl FnOnce(usize) -> Batch<'a>,
) -> Result<()> {
    let ffmpeg = find_tool("ffmpeg", FFMPEG_WINGET)?;
    let inputs = media::collect_inputs(&files.input)?;
    let dir = match &files.out {
        Some(d) => d.clone(),
        None => env::output_dir(KnownFolder::Music)?,
    };
    std::fs::create_dir_all(&dir)?;

    let batch = make(inputs.len());
    out.title(&format!("{} -> {}", batch.action, dir.display()));
    line(out, "");

    let total = inputs.len();
    let mut results = Vec::with_capacity(total);
    for (i, input) in inputs.iter().enumerate() {
        let name = input
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        line(out, &format!("  [{}/{total}] {name}", i + 1));
        let target = non_colliding(&dir.join((batch.name_for)(input)));
        let existed = target.exists();
        let result = match (batch.run)(&ffmpeg, input, &target, out) {
            Ok((0, _)) if target.is_file() => JobResult {
                input: input.clone(),
                size_bytes: std::fs::metadata(&target).ok().map(|m| m.len()),
                output: Some(target.clone()),
                ok: true,
                error: None,
            },
            Ok((code, text)) => {
                discard_empty_partial(&target, existed, &text);
                JobResult {
                    input: input.clone(),
                    output: None,
                    ok: false,
                    size_bytes: None,
                    error: Some(failure_reason(code, &text)),
                }
            }
            Err(e) => JobResult {
                input: input.clone(),
                output: None,
                ok: false,
                size_bytes: None,
                error: Some(e.to_string()),
            },
        };
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

    let report = MediaReport::new(batch.operation, &dir, results);
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

/// Removes the empty file ffmpeg leaves behind after a failed run. Never touches a file
/// that existed before the run or one ffmpeg refused to overwrite.
fn discard_empty_partial(target: &Path, existed: bool, text: &str) {
    if existed || text.contains("already exists") {
        return;
    }
    if std::fs::metadata(target).is_ok_and(|m| m.len() == 0) {
        let _ = std::fs::remove_file(target);
    }
}

/// The last meaningful lines of ffmpeg's output, which carry the actual error.
fn failure_reason(code: i32, text: &str) -> String {
    let tail: Vec<&str> = text
        .split(['\r', '\n'])
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    let tail = &tail[tail.len().saturating_sub(2)..];
    if tail.is_empty() {
        format!("ffmpeg exited with code {code}")
    } else {
        format!("ffmpeg exited with code {code}: {}", tail.join(" | "))
    }
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
    let ffprobe = find_tool("ffprobe", FFMPEG_WINGET)?;
    let file = media::strip_quotes(&a.file);
    if !file.exists() {
        return Err(Error::InvalidInput(format!(
            "file not found: {}",
            file.display()
        )));
    }
    let args = media::probe_args(&file);
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let json = capture(&ffprobe, &refs)?;
    if json.trim().is_empty() {
        return Err(Error::Other("ffprobe returned no data".into()));
    }
    let info = media::parse_probe(&file.to_string_lossy(), &json)?;
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

    #[test]
    fn failure_reason_keeps_the_last_lines() {
        let text = "banner\r\nInput #0\nbad.wav: Invalid data found when processing input\n";
        assert_eq!(
            failure_reason(1, text),
            "ffmpeg exited with code 1: Input #0 | bad.wav: Invalid data found when processing input"
        );
        assert_eq!(failure_reason(5, "  \n"), "ffmpeg exited with code 5");
    }

    #[test]
    fn partial_output_is_removed_only_when_empty_and_ours() {
        let dir = tempfile::tempdir().unwrap();
        let empty = dir.path().join("empty é.mp3");
        std::fs::write(&empty, b"").unwrap();
        discard_empty_partial(&empty, true, "");
        assert!(empty.exists(), "pre-existing file must stay");
        discard_empty_partial(&empty, false, "File 'x' already exists. Exiting.");
        assert!(empty.exists(), "refused overwrite must stay");
        discard_empty_partial(&empty, false, "boom");
        assert!(!empty.exists());
        let full = dir.path().join("full.mp3");
        std::fs::write(&full, b"data").unwrap();
        discard_empty_partial(&full, false, "boom");
        assert!(full.exists());
    }
}
