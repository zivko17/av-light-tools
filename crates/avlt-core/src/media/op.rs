//! What one `media` operation does with each input file, shared by the CLI and the GUI:
//! its name, heading, output file name and ffmpeg arguments.

use std::path::{Path, PathBuf};

use super::{
    Channels, ExtractFormat, Preset, channels_args, convert_args_with, extract_args, normalize_ext,
    resample_args, suffixed_name, trim_args,
};

/// A media operation with its options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediaOp {
    Convert {
        preset: Preset,
        sample_rate: Option<u32>,
        channels: Option<Channels>,
    },
    /// Two-pass EBU R128 to -23 LUFS (see `normalize_pass*_args`).
    Normalize,
    Trim {
        start: String,
        end: Option<String>,
    },
    Extract(ExtractFormat),
    Resample(u32),
    Channels(Channels),
}

impl MediaOp {
    /// Operation name in the report (`convert`, `normalize`, ...).
    pub fn operation(&self) -> &'static str {
        match self {
            MediaOp::Convert { .. } => "convert",
            MediaOp::Normalize => "normalize",
            MediaOp::Trim { .. } => "trim",
            MediaOp::Extract(_) => "extract",
            MediaOp::Resample(_) => "resample",
            MediaOp::Channels(_) => "channels",
        }
    }

    /// Heading before the files, e.g. `Converting 3 file(s) to MP3-320`.
    pub fn action(&self, n: usize) -> String {
        match self {
            MediaOp::Convert { preset, .. } => {
                format!("Converting {n} file(s) to {}", preset.label())
            }
            MediaOp::Normalize => format!("Normalizing {n} file(s) to -23 LUFS (two-pass)"),
            MediaOp::Trim { .. } => format!("Trimming {n} file(s)"),
            MediaOp::Extract(_) => format!("Extracting audio from {n} file(s)"),
            MediaOp::Resample(rate) => format!("Resampling {n} file(s) to {rate} Hz"),
            MediaOp::Channels(mode) => format!("Converting {n} file(s) to {}", mode.name()),
        }
    }

    /// Bare output file name for `input` (before collision suffixes).
    pub fn output_name(&self, input: &Path) -> PathBuf {
        match self {
            MediaOp::Convert { preset, .. } => {
                suffixed_name(input, preset.label(), Some(preset.ext()))
            }
            MediaOp::Normalize => suffixed_name(input, "norm", Some(normalize_ext(input))),
            MediaOp::Trim { .. } => suffixed_name(input, "trim", None),
            MediaOp::Extract(format) => suffixed_name(input, "audio", Some(format.ext())),
            MediaOp::Resample(rate) => suffixed_name(input, &rate.to_string(), None),
            MediaOp::Channels(mode) => suffixed_name(input, mode.name(), None),
        }
    }

    /// The single ffmpeg run of every operation but [`MediaOp::Normalize`] (two passes,
    /// built from the first pass's measurement), which returns `None`.
    pub fn single_pass_args(&self, input: &Path, output: &Path) -> Option<Vec<String>> {
        Some(match self {
            MediaOp::Convert {
                preset,
                sample_rate,
                channels,
            } => convert_args_with(input, output, *preset, *sample_rate, *channels),
            MediaOp::Normalize => return None,
            MediaOp::Trim { start, end } => trim_args(input, output, start, end.as_deref()),
            MediaOp::Extract(format) => extract_args(input, output, *format),
            MediaOp::Resample(rate) => resample_args(input, output, *rate),
            MediaOp::Channels(mode) => channels_args(input, output, *mode),
        })
    }
}

/// The last meaningful lines of ffmpeg's output, which carry the actual error.
pub fn failure_reason(code: i32, text: &str) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn names_and_headings() {
        let op = MediaOp::Resample(48_000);
        assert_eq!(op.operation(), "resample");
        assert_eq!(op.action(2), "Resampling 2 file(s) to 48000 Hz");
        assert_eq!(
            op.output_name(Path::new("a b.wav")),
            PathBuf::from("a b_48000.wav")
        );
        assert!(
            MediaOp::Normalize
                .single_pass_args(Path::new("a"), Path::new("b"))
                .is_none()
        );
        assert!(
            op.single_pass_args(Path::new("a"), Path::new("b"))
                .is_some()
        );
    }
}
