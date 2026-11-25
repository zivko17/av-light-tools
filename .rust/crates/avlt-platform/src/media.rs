//! Running ffmpeg / ffprobe for the `media` operations (shared by the CLI and the GUI).

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use avlt_core::media::{self, JobResult, MediaOp, ProbeInfo, failure_reason, non_colliding};

use crate::process::{capture, find_tool, output_cancellable};
use crate::{Error, Result};

/// winget package that provides ffmpeg and ffprobe.
pub const FFMPEG_WINGET: &str = "Gyan.FFmpeg";

/// ffmpeg next to avlt or on PATH ([`Error::MissingTool`] with the winget hint).
pub fn ffmpeg() -> Result<std::path::PathBuf> {
    find_tool("ffmpeg", FFMPEG_WINGET)
}

fn run_ffmpeg(ffmpeg: &Path, args: &[String], cancel: &AtomicBool) -> Result<(i32, String)> {
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    output_cancellable(ffmpeg, &refs, cancel)
}

/// Runs `op` on one `input`, writing into `dir` under a name that never overwrites.
/// `warn` receives non-fatal notes (normalize falling back to a single pass). Setting
/// `cancel` kills ffmpeg; the job then fails with "cancelled".
pub fn run_job(
    ffmpeg: &Path,
    op: &MediaOp,
    input: &Path,
    dir: &Path,
    cancel: &AtomicBool,
    warn: &mut dyn FnMut(&str),
) -> JobResult {
    let target = non_colliding(&dir.join(op.output_name(input)));
    let existed = target.exists();
    let run = match op.single_pass_args(input, &target) {
        Some(args) => run_ffmpeg(ffmpeg, &args, cancel),
        None => {
            // Pass 1: measure. Pass 2: apply the measured values; if the measurement
            // fails (e.g. silence), fall back to a single pass like the original.
            run_ffmpeg(ffmpeg, &media::normalize_pass1_args(input), cancel).and_then(|(_, text)| {
                let args = match media::parse_loudnorm_json(&text) {
                    Ok(m) => media::normalize_pass2_args(input, &target, &m),
                    Err(_) => {
                        warn("measurement failed, single-pass fallback");
                        media::normalize_single_pass_args(input, &target)
                    }
                };
                run_ffmpeg(ffmpeg, &args, cancel)
            })
        }
    };
    let failed = |error: String| JobResult {
        input: input.to_path_buf(),
        output: None,
        ok: false,
        size_bytes: None,
        error: Some(error),
    };
    match run {
        Ok((0, _)) if target.is_file() => JobResult {
            input: input.to_path_buf(),
            size_bytes: std::fs::metadata(&target).ok().map(|m| m.len()),
            output: Some(target),
            ok: true,
            error: None,
        },
        Ok((code, text)) => {
            discard_empty_partial(&target, existed, &text);
            failed(failure_reason(code, &text))
        }
        Err(e) => {
            if cancel.load(Ordering::Relaxed) {
                discard_cancelled_output(&target, existed);
            } else {
                discard_empty_partial(&target, existed, "");
            }
            failed(e.to_string())
        }
    }
}

/// Removes the empty file ffmpeg leaves behind after a failed run. Never touches a file
/// that existed before the run or one ffmpeg refused to overwrite.
pub fn discard_empty_partial(target: &Path, existed: bool, text: &str) {
    if existed || text.contains("already exists") {
        return;
    }
    if std::fs::metadata(target).is_ok_and(|m| m.len() == 0) {
        let _ = std::fs::remove_file(target);
    }
}

/// Removes what a cancelled (killed) ffmpeg left at `target`, whatever its size, unless
/// the file existed before the run.
pub fn discard_cancelled_output(target: &Path, existed: bool) {
    if !existed && target.is_file() {
        let _ = std::fs::remove_file(target);
    }
}

/// Codec, sample rate, channels and duration of `file` (ffprobe).
pub fn probe(file: &Path) -> Result<ProbeInfo> {
    let ffprobe = find_tool("ffprobe", FFMPEG_WINGET)?;
    let file = media::strip_quotes(file);
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
    media::parse_probe(&file.to_string_lossy(), &json)
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn cancelled_output_is_removed_unless_it_existed() {
        let dir = tempfile::tempdir().unwrap();
        let partial = dir.path().join("half done.mp3");
        std::fs::write(&partial, b"truncated data").unwrap();
        discard_cancelled_output(&partial, true);
        assert!(partial.exists(), "a file that existed before stays");
        discard_cancelled_output(&partial, false);
        assert!(!partial.exists(), "our truncated output goes");
        discard_cancelled_output(&partial, false);
    }

    #[test]
    fn a_failing_program_gives_a_failed_job() {
        // Any program that exits non-zero without writing the target stands in for ffmpeg.
        #[cfg(windows)]
        let fake = find_tool("cmd", "none").unwrap();
        #[cfg(not(windows))]
        let fake = find_tool("false", "none").unwrap();
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("in.wav");
        std::fs::write(&input, b"x").unwrap();
        let mut warned = Vec::new();
        let r = run_job(
            &fake,
            &MediaOp::Resample(48_000),
            &input,
            dir.path(),
            &AtomicBool::new(false),
            &mut |w| warned.push(w.to_string()),
        );
        assert!(!r.ok);
        assert!(r.output.is_none());
        assert!(r.error.is_some());
    }
}
