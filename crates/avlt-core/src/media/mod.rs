//! Media presets and ffmpeg argument building (port of `AudioTools.ps1`).
//!
//! Nothing here runs a program or prints: the CLI runs ffmpeg with the lists built here
//! and uses the parsers for what ffmpeg / ffprobe print.

pub mod ffargs;
pub mod paths;
pub mod presets;

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::{Error, Result};

pub use ffargs::{
    channels_args, convert_args, convert_args_with, extract_args, normalize_ext,
    normalize_pass1_args, normalize_pass2_args, normalize_single_pass_args, probe_args,
    resample_args, suffixed_name, trim_args,
};
pub use paths::non_colliding;
pub use presets::{Channels, ExtractFormat, Preset, SAMPLE_RATES};

/// Extensions `Get-AudioInputs` accepts when it walks a folder.
pub const MEDIA_EXTENSIONS: [&str; 14] = [
    "mp3", "flac", "wav", "m4a", "aac", "ogg", "opus", "wma", "aif", "aiff", "mp4", "mkv", "mov",
    "webm",
];

/// Loudness measured by the first `loudnorm` pass (`print_format=json`).
///
/// The values are kept as the text ffmpeg printed (validated as finite numbers), so pass 2
/// receives exactly what pass 1 measured.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LoudnormMeasure {
    pub input_i: String,
    pub input_tp: String,
    pub input_lra: String,
    pub input_thresh: String,
    pub target_offset: String,
}

/// Extracts the `loudnorm` JSON block from ffmpeg's stderr.
///
/// Fails when there is no JSON block or a value is missing or not a finite number
/// (silence measures `-inf`), in which case the caller falls back to a single pass.
pub fn parse_loudnorm_json(stderr: &str) -> Result<LoudnormMeasure> {
    let bad = |why: &str| Error::Other(format!("cannot read loudnorm measurement: {why}"));
    // The block is the `{...}` around "input_i"; ffmpeg's own log lines have no braces
    // of their own, but anchoring on the key keeps this robust to any that appear.
    let key = stderr
        .find("\"input_i\"")
        .ok_or_else(|| bad("no JSON block in ffmpeg output"))?;
    let start = stderr[..key]
        .rfind('{')
        .ok_or_else(|| bad("unterminated JSON block"))?;
    let end = stderr[key..]
        .find('}')
        .map(|i| key + i + 1)
        .ok_or_else(|| bad("unterminated JSON block"))?;
    let value: serde_json::Value =
        serde_json::from_str(&stderr[start..end]).map_err(|e| bad(&e.to_string()))?;
    // Raw text of a value; it must parse as a finite number (silence gives `-inf`).
    let num = |name: &str| -> Result<String> {
        let raw = match value.get(name) {
            Some(serde_json::Value::String(s)) => Some(s.trim().to_string()),
            Some(serde_json::Value::Number(n)) => Some(n.to_string()),
            _ => None,
        };
        raw.filter(|r| r.parse::<f64>().is_ok_and(f64::is_finite))
            .ok_or_else(|| bad(&format!("'{name}' is missing or not a finite number")))
    };
    Ok(LoudnormMeasure {
        input_i: num("input_i")?,
        input_tp: num("input_tp")?,
        input_lra: num("input_lra")?,
        input_thresh: num("input_thresh")?,
        target_offset: num("target_offset")?,
    })
}

/// What `avlt media probe` shows (first audio stream plus the container).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProbeInfo {
    pub file: String,
    pub container: Option<String>,
    pub duration_s: Option<f64>,
    pub size_bytes: Option<u64>,
    pub codec: Option<String>,
    pub sample_rate_hz: Option<u32>,
    pub channels: Option<u32>,
    pub channel_layout: Option<String>,
    pub bit_rate_bps: Option<u64>,
    pub bits_per_sample: Option<u32>,
}

/// Parses `ffprobe -print_format json -show_format -show_streams` output.
pub fn parse_probe(file: &str, json: &str) -> Result<ProbeInfo> {
    let doc: serde_json::Value = serde_json::from_str(json)
        .map_err(|e| Error::Other(format!("cannot read ffprobe output: {e}")))?;
    // ffprobe prints most numbers as strings.
    fn number(v: Option<&serde_json::Value>) -> Option<f64> {
        match v? {
            serde_json::Value::String(s) => s.trim().parse().ok(),
            serde_json::Value::Number(n) => n.as_f64(),
            _ => None,
        }
    }
    fn text(v: Option<&serde_json::Value>) -> Option<String> {
        v?.as_str().filter(|s| !s.is_empty()).map(str::to_string)
    }
    fn whole(v: Option<&serde_json::Value>) -> Option<u64> {
        number(v)
            .filter(|n| *n >= 0.0 && n.is_finite())
            .map(|n| n as u64)
    }
    let audio = doc.get("streams").and_then(|s| s.as_array()).and_then(|s| {
        s.iter()
            .find(|st| st.get("codec_type").and_then(|t| t.as_str()) == Some("audio"))
    });
    let format = doc.get("format");
    let name = Path::new(file.trim_matches('"'))
        .file_name()
        .map_or_else(|| file.to_string(), |n| n.to_string_lossy().into_owned());
    let in_audio = |key: &str| audio.and_then(|a| a.get(key));
    Ok(ProbeInfo {
        file: name,
        container: text(format.and_then(|f| f.get("format_name")))
            .map(|n| n.split(',').next().unwrap_or_default().to_string()),
        duration_s: number(format.and_then(|f| f.get("duration"))),
        size_bytes: whole(format.and_then(|f| f.get("size"))),
        codec: text(in_audio("codec_name")),
        sample_rate_hz: whole(in_audio("sample_rate")).and_then(|n| u32::try_from(n).ok()),
        channels: whole(in_audio("channels")).and_then(|n| u32::try_from(n).ok()),
        channel_layout: text(in_audio("channel_layout")),
        bit_rate_bps: whole(in_audio("bit_rate")),
        bits_per_sample: whole(in_audio("bits_per_sample"))
            .filter(|n| *n > 0)
            .and_then(|n| u32::try_from(n).ok()),
    })
}

/// `path` without surrounding spaces and double quotes (a pasted `"C:\a b\c.wav"`).
pub fn strip_quotes(path: &Path) -> PathBuf {
    let raw = path.to_string_lossy();
    PathBuf::from(raw.trim().trim_matches('"').trim())
}

/// The files `path` stands for: the file itself, or every media file under a folder
/// (recursive, sorted). Surrounding double quotes and spaces are ignored.
pub fn collect_inputs(path: &Path) -> Result<Vec<PathBuf>> {
    let path = strip_quotes(path);
    let path = path.as_path();
    if !path.exists() {
        return Err(Error::InvalidInput(format!(
            "path not found: {}",
            path.display()
        )));
    }
    if !path.is_dir() {
        return Ok(vec![path.to_path_buf()]);
    }
    let mut found = Vec::new();
    walk(path, &mut found)?;
    found.sort();
    if found.is_empty() {
        return Err(Error::InvalidInput(format!(
            "no audio or video files found in {}",
            path.display()
        )));
    }
    Ok(found)
}

fn walk(dir: &Path, found: &mut Vec<PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            walk(&path, found)?;
        } else if path.extension().is_some_and(|e| {
            let e = e.to_string_lossy().to_ascii_lowercase();
            MEDIA_EXTENSIONS.contains(&e.as_str())
        }) {
            found.push(path);
        }
    }
    Ok(())
}

/// One processed file of a media command.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct JobResult {
    pub input: PathBuf,
    pub output: Option<PathBuf>,
    pub ok: bool,
    pub size_bytes: Option<u64>,
    /// Last lines of ffmpeg's output when it failed.
    pub error: Option<String>,
}

/// Result of a media command over one or more files.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MediaReport {
    pub operation: String,
    pub output_dir: PathBuf,
    pub ok: usize,
    pub failed: usize,
    pub files: Vec<JobResult>,
}

impl MediaReport {
    /// Builds the report and its counters from the per-file results.
    pub fn new(operation: &str, output_dir: &Path, files: Vec<JobResult>) -> MediaReport {
        let ok = files.iter().filter(|f| f.ok).count();
        MediaReport {
            operation: operation.to_string(),
            output_dir: output_dir.to_path_buf(),
            ok,
            failed: files.len() - ok,
            files,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOUDNORM_STDERR: &str = r#"ffmpeg version 7.1 Copyright (c) 2000-2024 the FFmpeg developers
Input #0, wav, from 'in file.wav':
  Duration: 00:00:01.00, bitrate: 1411 kb/s
[Parsed_loudnorm_0 @ 000001d4a5c3e6c0]
{
	"input_i" : "-27.61",
	"input_tp" : "-4.47",
	"input_lra" : "18.10",
	"input_thresh" : "-39.20",
	"output_i" : "-16.58",
	"output_tp" : "-1.50",
	"output_lra" : "14.40",
	"output_thresh" : "-27.71",
	"normalization_type" : "dynamic",
	"target_offset" : "0.58"
}
"#;

    #[test]
    fn loudnorm_json_is_extracted_from_stderr() {
        let m = parse_loudnorm_json(LOUDNORM_STDERR).unwrap();
        assert_eq!(
            m,
            LoudnormMeasure {
                input_i: "-27.61".into(),
                input_tp: "-4.47".into(),
                input_lra: "18.10".into(),
                input_thresh: "-39.20".into(),
                target_offset: "0.58".into(),
            }
        );
    }

    #[test]
    fn loudnorm_without_json_or_with_bad_values_is_an_error() {
        assert!(parse_loudnorm_json("").is_err());
        assert!(parse_loudnorm_json("ffmpeg: no such file { not json }").is_err());
        let silent = LOUDNORM_STDERR.replace("\"-27.61\"", "\"-inf\"");
        assert!(parse_loudnorm_json(&silent).is_err());
        let missing = LOUDNORM_STDERR.replace("target_offset", "other");
        assert!(parse_loudnorm_json(&missing).is_err());
    }

    #[test]
    fn loudnorm_accepts_plain_numbers() {
        let json = r#"{"input_i": -20, "input_tp": -1.5, "input_lra": 6, "input_thresh": -30.5, "target_offset": 0}"#;
        assert_eq!(parse_loudnorm_json(json).unwrap().input_tp, "-1.5");
    }

    const PROBE_JSON: &str = r#"{
      "streams": [
        {"codec_type": "video", "codec_name": "mjpeg"},
        {"codec_type": "audio", "codec_name": "mp3", "sample_rate": "44100", "channels": 2,
         "channel_layout": "stereo", "bit_rate": "320000", "bits_per_sample": 0}
      ],
      "format": {"format_name": "mp3,other", "duration": "61.250000", "size": "2450000"}
    }"#;

    #[test]
    fn probe_reads_container_and_first_audio_stream() {
        let p = parse_probe("música/a b.mp3", PROBE_JSON).unwrap();
        assert_eq!(p.file, "a b.mp3");
        assert_eq!(p.container.as_deref(), Some("mp3"));
        assert_eq!(p.duration_s, Some(61.25));
        assert_eq!(p.size_bytes, Some(2_450_000));
        assert_eq!(p.codec.as_deref(), Some("mp3"));
        assert_eq!(p.sample_rate_hz, Some(44_100));
        assert_eq!(p.channels, Some(2));
        assert_eq!(p.channel_layout.as_deref(), Some("stereo"));
        assert_eq!(p.bit_rate_bps, Some(320_000));
        assert_eq!(p.bits_per_sample, None);
    }

    #[test]
    fn probe_without_audio_or_with_garbage() {
        let p = parse_probe(
            "x.png",
            r#"{"streams":[],"format":{"format_name":"png_pipe"}}"#,
        )
        .unwrap();
        assert_eq!(p.codec, None);
        assert_eq!(p.container.as_deref(), Some("png_pipe"));
        assert!(parse_probe("x", "").is_err());
        assert!(parse_probe("x", "not json").is_err());
    }

    #[test]
    fn collect_inputs_single_file_any_extension_and_quotes() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("sin extension é");
        std::fs::write(&f, b"x").unwrap();
        assert_eq!(collect_inputs(&f).unwrap(), vec![f.clone()]);
        let quoted = PathBuf::from(format!("\"{}\"", f.display()));
        assert_eq!(collect_inputs(&quoted).unwrap(), vec![f]);
    }

    #[test]
    fn collect_inputs_walks_folders_for_media_only() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("sub carpeta");
        std::fs::create_dir(&sub).unwrap();
        for name in ["b.MP3", "a.wav", "notes.txt", "cover.jpg"] {
            std::fs::write(dir.path().join(name), b"x").unwrap();
        }
        std::fs::write(sub.join("c.flac"), b"x").unwrap();
        std::fs::write(sub.join("noext"), b"x").unwrap();
        let got = collect_inputs(dir.path()).unwrap();
        assert_eq!(
            got,
            vec![
                dir.path().join("a.wav"),
                dir.path().join("b.MP3"),
                sub.join("c.flac"),
            ]
        );
    }

    #[test]
    fn collect_inputs_missing_path_and_empty_folder_are_invalid_input() {
        let dir = tempfile::tempdir().unwrap();
        let err = collect_inputs(&dir.path().join("nope.wav")).unwrap_err();
        assert_eq!(err.exit_code(), 2, "{err}");
        let err = collect_inputs(dir.path()).unwrap_err();
        assert_eq!(err.exit_code(), 2, "{err}");
    }

    #[test]
    fn report_counts_results() {
        let job = |ok| JobResult {
            input: PathBuf::from("a"),
            output: None,
            ok,
            size_bytes: None,
            error: None,
        };
        let r = MediaReport::new(
            "convert",
            Path::new("out"),
            vec![job(true), job(false), job(true)],
        );
        assert_eq!((r.ok, r.failed), (2, 1));
    }
}
