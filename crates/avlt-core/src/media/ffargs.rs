//! FFmpeg / ffprobe argument lists, ported from `AudioTools.ps1`.
//!
//! Every function returns the arguments as a list: paths are passed verbatim (spaces,
//! accents and quotes need no escaping because no shell is involved). Output files are
//! opened with `-n`, so ffmpeg itself refuses to overwrite an existing file (the
//! original used `-y`).

use std::path::{Path, PathBuf};

use super::LoudnormMeasure;
use super::presets::{Channels, ExtractFormat, Preset};

/// Loudness target of `normalize` (EBU R128, -23 LUFS).
const LOUDNORM_TARGET: &str = "loudnorm=I=-23:LRA=7:TP=-2";

fn path_arg(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

/// `-hide_banner -n -i <input>`: the start of every conversion.
fn base(input: &Path) -> Vec<String> {
    let mut a = strings(&["-hide_banner", "-n", "-i"]);
    a.push(path_arg(input));
    a
}

/// Lower-case extension of `p` without the dot.
fn ext_of(p: &Path) -> String {
    p.extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default()
}

/// Codec used to re-encode into the input's own container (`resample`, `channels`).
fn same_format_codec(input: &Path) -> &'static [&'static str] {
    match ext_of(input).as_str() {
        "flac" => &["-c:a", "flac"],
        "wav" => &["-c:a", "pcm_s16le"],
        "mp3" => &["-c:a", "libmp3lame", "-b:a", "320k"],
        _ => &["-c:a", "aac", "-b:a", "256k"],
    }
}

/// Convert `input` to `output` with `preset`.
pub fn convert_args(input: &Path, output: &Path, preset: Preset) -> Vec<String> {
    convert_args_with(input, output, preset, None, None)
}

/// [`convert_args`] with an optional sample rate (`-ar`) and channel layout (`-ac`).
pub fn convert_args_with(
    input: &Path,
    output: &Path,
    preset: Preset,
    sample_rate: Option<u32>,
    channels: Option<Channels>,
) -> Vec<String> {
    let mut a = base(input);
    a.extend(strings(preset.codec_args()));
    if let Some(rate) = sample_rate {
        a.extend(strings(&["-ar", &rate.to_string()]));
    }
    if let Some(ch) = channels {
        a.extend(strings(&["-ac", &ch.count().to_string()]));
    }
    a.push(path_arg(output));
    a
}

/// Pass 1 of `normalize`: measure loudness (`loudnorm` prints JSON stats to stderr).
pub fn normalize_pass1_args(input: &Path) -> Vec<String> {
    let mut a = strings(&["-hide_banner", "-i"]);
    a.push(path_arg(input));
    a.extend(strings(&[
        "-af",
        "loudnorm=I=-23:LRA=7:TP=-2:print_format=json",
        "-f",
        "null",
        "-",
    ]));
    a
}

/// Codec and output extension of `normalize`: the output container must match the codec.
fn normalize_codec(input: &Path) -> (&'static str, &'static str) {
    match ext_of(input).as_str() {
        "flac" => ("flac", ".flac"),
        "wav" => ("pcm_s16le", ".wav"),
        _ => ("libmp3lame", ".mp3"),
    }
}

/// Extension (with the dot) of the file `normalize` writes for `input`.
pub fn normalize_ext(input: &Path) -> &'static str {
    normalize_codec(input).1
}

fn normalize_with_filter(input: &Path, output: &Path, filter: &str) -> Vec<String> {
    let mut a = base(input);
    a.extend(strings(&["-af", filter, "-c:a", normalize_codec(input).0]));
    a.push(path_arg(output));
    a
}

/// Pass 2 of `normalize`: apply the measured values (true EBU R128, linear).
pub fn normalize_pass2_args(
    input: &Path,
    output: &Path,
    measured: &LoudnormMeasure,
) -> Vec<String> {
    let filter = format!(
        "{LOUDNORM_TARGET}:measured_I={}:measured_TP={}:measured_LRA={}:measured_thresh={}:offset={}:linear=true",
        measured.input_i,
        measured.input_tp,
        measured.input_lra,
        measured.input_thresh,
        measured.target_offset,
    );
    normalize_with_filter(input, output, &filter)
}

/// Single-pass `normalize`, used when the measurement failed.
pub fn normalize_single_pass_args(input: &Path, output: &Path) -> Vec<String> {
    normalize_with_filter(input, output, LOUDNORM_TARGET)
}

/// Cut `input` from `start` (and up to `end`) without re-encoding.
pub fn trim_args(input: &Path, output: &Path, start: &str, end: Option<&str>) -> Vec<String> {
    let mut a = base(input);
    a.extend(strings(&["-ss", start]));
    if let Some(end) = end {
        a.extend(strings(&["-to", end]));
    }
    a.extend(strings(&["-c", "copy"]));
    a.push(path_arg(output));
    a
}

/// Extract the audio track of `input`.
pub fn extract_args(input: &Path, output: &Path, format: ExtractFormat) -> Vec<String> {
    let mut a = base(input);
    a.push("-vn".to_string());
    a.extend(strings(match format {
        ExtractFormat::Mp3 => &["-c:a", "libmp3lame", "-b:a", "320k"][..],
        ExtractFormat::Flac => &["-c:a", "flac"],
        ExtractFormat::Wav => &["-c:a", "pcm_s16le"],
        ExtractFormat::Copy => &["-c:a", "copy"],
    }));
    a.push(path_arg(output));
    a
}

/// Change the sample rate, keeping the input's format.
pub fn resample_args(input: &Path, output: &Path, sample_rate: u32) -> Vec<String> {
    let mut a = base(input);
    a.extend(strings(&["-ar", &sample_rate.to_string()]));
    a.extend(strings(same_format_codec(input)));
    a.push(path_arg(output));
    a
}

/// Change the channel layout, keeping the input's format.
pub fn channels_args(input: &Path, output: &Path, channels: Channels) -> Vec<String> {
    let mut a = base(input);
    a.extend(strings(&["-ac", &channels.count().to_string()]));
    a.extend(strings(same_format_codec(input)));
    a.push(path_arg(output));
    a
}

/// ffprobe arguments for `parse_probe`. `--` ends option parsing before the path.
pub fn probe_args(input: &Path) -> Vec<String> {
    let mut a = strings(&[
        "-v",
        "quiet",
        "-print_format",
        "json",
        "-show_format",
        "-show_streams",
        "--",
    ]);
    a.push(path_arg(input));
    a
}

/// `<stem>_<suffix><ext>` as a bare file name; `ext` (with the dot) defaults to the
/// input's own extension.
pub fn suffixed_name(input: &Path, suffix: &str, ext: Option<&str>) -> PathBuf {
    let stem = input
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let ext = match ext {
        Some(e) => e.to_string(),
        None => input
            .extension()
            .map(|e| format!(".{}", e.to_string_lossy()))
            .unwrap_or_default(),
    };
    PathBuf::from(format!("{stem}_{suffix}{ext}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    fn measure() -> LoudnormMeasure {
        LoudnormMeasure {
            input_i: -27.61,
            input_tp: -4.47,
            input_lra: 18.1,
            input_thresh: -38.2,
            target_offset: 0.7,
        }
    }

    #[test]
    fn convert_passes_paths_verbatim() {
        let a = convert_args(
            Path::new("in file.wav"),
            Path::new("out é.mp3"),
            Preset::Mp3,
        );
        assert!(a.contains(&"in file.wav".to_string()) && a.contains(&"out é.mp3".to_string()));
        assert!(
            a.iter().all(|x| !x.contains('"') && !x.contains('\\')),
            "{a:?}"
        );
    }

    #[test]
    fn convert_args_per_preset_match_audiotools_ps1() {
        let i = Path::new("a.wav");
        let o = Path::new("o");
        assert_eq!(
            convert_args(i, o, Preset::Mp3),
            s(&[
                "-hide_banner",
                "-n",
                "-i",
                "a.wav",
                "-c:a",
                "libmp3lame",
                "-b:a",
                "320k",
                "o"
            ])
        );
        assert_eq!(
            convert_args(i, o, Preset::Flac),
            s(&[
                "-hide_banner",
                "-n",
                "-i",
                "a.wav",
                "-c:a",
                "flac",
                "-compression_level",
                "5",
                "o"
            ])
        );
        assert_eq!(
            convert_args(i, o, Preset::Wav24),
            s(&[
                "-hide_banner",
                "-n",
                "-i",
                "a.wav",
                "-c:a",
                "pcm_s24le",
                "o"
            ])
        );
    }

    #[test]
    fn convert_with_rate_and_channels_goes_before_output() {
        let a = convert_args_with(
            Path::new("a.wav"),
            Path::new("o.mp3"),
            Preset::Mp3Low,
            Some(48_000),
            Some(Channels::Mono),
        );
        assert_eq!(
            a,
            s(&[
                "-hide_banner",
                "-n",
                "-i",
                "a.wav",
                "-c:a",
                "libmp3lame",
                "-b:a",
                "192k",
                "-ar",
                "48000",
                "-ac",
                "1",
                "o.mp3"
            ])
        );
    }

    #[test]
    fn normalize_pass1_measures_to_null() {
        assert_eq!(
            normalize_pass1_args(Path::new("a b.wav")),
            s(&[
                "-hide_banner",
                "-i",
                "a b.wav",
                "-af",
                "loudnorm=I=-23:LRA=7:TP=-2:print_format=json",
                "-f",
                "null",
                "-"
            ])
        );
    }

    #[test]
    fn normalize_pass2_uses_measured_values_and_target() {
        let a = normalize_pass2_args(Path::new("a.mp3"), Path::new("a_norm.mp3"), &measure());
        assert!(a.iter().any(|x| x.contains("I=-23")), "{a:?}");
        assert_eq!(
            a,
            s(&[
                "-hide_banner",
                "-n",
                "-i",
                "a.mp3",
                "-af",
                "loudnorm=I=-23:LRA=7:TP=-2:measured_I=-27.61:measured_TP=-4.47:measured_LRA=18.1:measured_thresh=-38.2:offset=0.7:linear=true",
                "-c:a",
                "libmp3lame",
                "a_norm.mp3"
            ])
        );
    }

    #[test]
    fn normalize_codec_follows_input_extension() {
        let m = measure();
        let codec = |name: &str| {
            let a = normalize_pass2_args(Path::new(name), Path::new("o"), &m);
            a[a.iter().position(|x| x == "-c:a").unwrap() + 1].clone()
        };
        assert_eq!(codec("a.FLAC"), "flac");
        assert_eq!(codec("a.wav"), "pcm_s16le");
        assert_eq!(codec("a.m4a"), "libmp3lame");
        assert_eq!(codec("movie.mp4"), "libmp3lame");
        assert_eq!(normalize_ext(Path::new("a.FLAC")), ".flac");
        assert_eq!(normalize_ext(Path::new("a.wav")), ".wav");
        assert_eq!(normalize_ext(Path::new("a.opus")), ".mp3");
    }

    #[test]
    fn normalize_single_pass_fallback() {
        let a = normalize_single_pass_args(Path::new("a.wav"), Path::new("o.wav"));
        assert_eq!(
            a,
            s(&[
                "-hide_banner",
                "-n",
                "-i",
                "a.wav",
                "-af",
                "loudnorm=I=-23:LRA=7:TP=-2",
                "-c:a",
                "pcm_s16le",
                "o.wav"
            ])
        );
    }

    #[test]
    fn trim_with_and_without_end() {
        assert_eq!(
            trim_args(Path::new("a.mp3"), Path::new("o.mp3"), "00:00:05", None),
            s(&[
                "-hide_banner",
                "-n",
                "-i",
                "a.mp3",
                "-ss",
                "00:00:05",
                "-c",
                "copy",
                "o.mp3"
            ])
        );
        assert_eq!(
            trim_args(
                Path::new("a.mp3"),
                Path::new("o.mp3"),
                "00:00:05",
                Some("00:01:00")
            ),
            s(&[
                "-hide_banner",
                "-n",
                "-i",
                "a.mp3",
                "-ss",
                "00:00:05",
                "-to",
                "00:01:00",
                "-c",
                "copy",
                "o.mp3"
            ])
        );
    }

    #[test]
    fn extract_args_per_format() {
        let i = Path::new("v.mp4");
        let o = Path::new("o");
        let head = ["-hide_banner", "-n", "-i", "v.mp4", "-vn"];
        let with = |tail: &[&str]| s(&[&head[..], tail, &["o"]].concat());
        assert_eq!(
            extract_args(i, o, ExtractFormat::Mp3),
            with(&["-c:a", "libmp3lame", "-b:a", "320k"])
        );
        assert_eq!(
            extract_args(i, o, ExtractFormat::Flac),
            with(&["-c:a", "flac"])
        );
        assert_eq!(
            extract_args(i, o, ExtractFormat::Wav),
            with(&["-c:a", "pcm_s16le"])
        );
        assert_eq!(
            extract_args(i, o, ExtractFormat::Copy),
            with(&["-c:a", "copy"])
        );
    }

    #[test]
    fn resample_and_channels_pick_codec_by_extension() {
        assert_eq!(
            resample_args(Path::new("a.WAV"), Path::new("o.wav"), 44_100),
            s(&[
                "-hide_banner",
                "-n",
                "-i",
                "a.WAV",
                "-ar",
                "44100",
                "-c:a",
                "pcm_s16le",
                "o.wav"
            ])
        );
        let a = resample_args(Path::new("a.flac"), Path::new("o.flac"), 96_000);
        assert_eq!(&a[4..], &s(&["-ar", "96000", "-c:a", "flac", "o.flac"])[..]);
        let a = resample_args(Path::new("a.mp3"), Path::new("o.mp3"), 48_000);
        assert_eq!(
            &a[4..],
            &s(&[
                "-ar",
                "48000",
                "-c:a",
                "libmp3lame",
                "-b:a",
                "320k",
                "o.mp3"
            ])[..]
        );
        assert_eq!(
            channels_args(Path::new("a.m4a"), Path::new("o.m4a"), Channels::Stereo),
            s(&[
                "-hide_banner",
                "-n",
                "-i",
                "a.m4a",
                "-ac",
                "2",
                "-c:a",
                "aac",
                "-b:a",
                "256k",
                "o.m4a"
            ])
        );
    }

    #[test]
    fn probe_args_end_option_parsing_before_the_path() {
        assert_eq!(
            probe_args(Path::new("-odd name.wav")),
            s(&[
                "-v",
                "quiet",
                "-print_format",
                "json",
                "-show_format",
                "-show_streams",
                "--",
                "-odd name.wav"
            ])
        );
    }

    #[test]
    fn suffixed_names_keep_stem_and_extension() {
        assert_eq!(
            suffixed_name(Path::new("C:/x/Señal 1.WAV"), "trim", None),
            PathBuf::from("Señal 1_trim.WAV")
        );
        assert_eq!(
            suffixed_name(Path::new("a.wav"), "MP3-320", Some(".mp3")),
            PathBuf::from("a_MP3-320.mp3")
        );
        assert_eq!(
            suffixed_name(Path::new("noext"), "mono", None),
            PathBuf::from("noext_mono")
        );
    }
}
