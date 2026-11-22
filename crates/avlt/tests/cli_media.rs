//! `avlt media`: missing-tool behaviour, argument errors and real conversions on small
//! generated files (skipped when ffmpeg is not installed).

use std::path::Path;
use std::process::{Command, Output};

fn avlt() -> Command {
    Command::new(env!("CARGO_BIN_EXE_avlt"))
}

/// Runs avlt with `PATH` pointing at an empty folder, so ffmpeg / ffprobe cannot be found.
fn avlt_without_tools(args: &[&str]) -> Output {
    let empty = tempfile::tempdir().unwrap();
    avlt()
        .args(args)
        .env("PATH", empty.path())
        .output()
        .unwrap()
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

#[test]
fn convert_without_ffmpeg_exits_3_with_winget_hint() {
    // find_tool also looks next to the exe: make sure no ffmpeg sits in target/.
    let exe_dir = Path::new(env!("CARGO_BIN_EXE_avlt")).parent().unwrap();
    assert!(
        !exe_dir.join("ffmpeg.exe").exists() && !exe_dir.join("ffmpeg").exists(),
        "ffmpeg next to the test binary would hide the missing-tool path"
    );
    let o = avlt_without_tools(&["media", "convert", "x.wav", "--preset", "mp3"]);
    assert_eq!(o.status.code(), Some(3), "{}", stderr(&o));
    assert!(
        stderr(&o).contains("winget install Gyan.FFmpeg"),
        "{}",
        stderr(&o)
    );
    assert!(stdout(&o).is_empty(), "{}", stdout(&o));
}

#[test]
fn every_ffmpeg_command_reports_the_missing_tool() {
    for args in [
        &["media", "normalize", "x.wav"][..],
        &["media", "extract", "x.mp4"],
        &["media", "resample", "x.wav"],
        &["media", "channels", "x.wav", "--mode", "mono"],
        &["media", "trim", "x.wav"],
        &["media", "probe", "x.wav"],
    ] {
        let o = avlt_without_tools(args);
        assert_eq!(o.status.code(), Some(3), "{args:?}: {}", stderr(&o));
        assert!(
            stderr(&o).contains("winget install Gyan.FFmpeg"),
            "{args:?}: {}",
            stderr(&o)
        );
    }
}

#[test]
fn bad_arguments_are_usage_errors() {
    for args in [
        &["media", "convert", "x.wav", "--preset", "ogg"][..],
        &["media", "convert", "x.wav"],
        &["media", "resample", "x.wav", "--rate", "12345"],
        &["media", "channels", "x.wav", "--mode", "surround"],
        &["media", "extract", "x.mp4", "--format", "aac"],
    ] {
        let o = avlt_without_tools(args);
        assert_eq!(o.status.code(), Some(2), "{args:?}: {}", stderr(&o));
    }
    // A folder cannot be trimmed; this is checked before looking for ffmpeg.
    let dir = tempfile::tempdir().unwrap();
    let o = avlt_without_tools(&["media", "trim", &dir.path().to_string_lossy()]);
    assert_eq!(o.status.code(), Some(2), "{}", stderr(&o));
}

#[test]
fn convert_help_lists_the_presets() {
    let o = avlt()
        .args(["media", "convert", "--help"])
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(0));
    for name in ["mp3", "mp3-low", "aac", "opus", "flac", "wav16", "wav24"] {
        assert!(stdout(&o).contains(name), "{}", stdout(&o));
    }
}

fn ffmpeg_installed() -> bool {
    ["ffmpeg", "ffprobe"].iter().all(|t| {
        Command::new(t)
            .arg("-version")
            .output()
            .is_ok_and(|o| o.status.success())
    })
}

/// A one-second 440 Hz tone at `path`.
fn make_tone(path: &Path) {
    let status = Command::new("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error", "-n"])
        .args(["-f", "lavfi", "-i", "sine=frequency=440:duration=1"])
        .arg(path)
        .status()
        .unwrap();
    assert!(status.success());
}

fn names_in(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

#[test]
fn convert_handles_odd_paths_and_never_overwrites() {
    if !ffmpeg_installed() {
        eprintln!("skipped: ffmpeg not installed");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("carpeta é ñ");
    std::fs::create_dir(&src).unwrap();
    let input = src.join("señal de prueba.wav");
    make_tone(&input);
    let out_dir = dir.path().join("salida con espacios");

    let run = || {
        avlt()
            .args(["media", "convert"])
            .arg(&input)
            .args(["--preset", "mp3", "--json", "-o"])
            .arg(&out_dir)
            .output()
            .unwrap()
    };
    let first = run();
    assert_eq!(first.status.code(), Some(0), "{}", stderr(&first));
    assert_eq!(names_in(&out_dir), ["señal de prueba_MP3-320.mp3"]);
    let first_bytes = std::fs::read(out_dir.join("señal de prueba_MP3-320.mp3")).unwrap();
    assert!(!first_bytes.is_empty());

    let json: serde_json::Value = serde_json::from_str(&stdout(&first)).unwrap();
    assert_eq!(json["ok"], 1);
    assert_eq!(json["failed"], 0);

    let second = run();
    assert_eq!(second.status.code(), Some(0), "{}", stderr(&second));
    assert_eq!(
        names_in(&out_dir),
        [
            "señal de prueba_MP3-320.mp3",
            "señal de prueba_MP3-320_1.mp3"
        ]
    );
    assert_eq!(
        std::fs::read(out_dir.join("señal de prueba_MP3-320.mp3")).unwrap(),
        first_bytes,
        "the first output must not be touched"
    );
}

#[test]
fn folder_input_and_failures_are_reported_per_file() {
    if !ffmpeg_installed() {
        eprintln!("skipped: ffmpeg not installed");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("in");
    std::fs::create_dir(&src).unwrap();
    make_tone(&src.join("good.wav"));
    std::fs::write(src.join("bad.wav"), b"this is not audio").unwrap();
    let out_dir = dir.path().join("out");
    let o = avlt()
        .args(["media", "channels", "--mode", "mono", "--json", "-o"])
        .arg(&out_dir)
        .arg(&src)
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(1), "{}", stderr(&o));
    let json: serde_json::Value = serde_json::from_str(&stdout(&o)).unwrap();
    assert_eq!(
        (json["ok"].as_u64(), json["failed"].as_u64()),
        (Some(1), Some(1))
    );
    assert_eq!(
        names_in(&out_dir),
        ["good_mono.wav"],
        "no leftover for the failed file"
    );
    assert!(stderr(&o).contains("bad.wav"), "{}", stderr(&o));
}

#[test]
fn missing_input_is_a_usage_error() {
    if !ffmpeg_installed() {
        eprintln!("skipped: ffmpeg not installed");
        return;
    }
    let o = avlt()
        .args(["media", "normalize", "no such file.wav"])
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(2), "{}", stderr(&o));
    assert!(stderr(&o).contains("path not found"), "{}", stderr(&o));
}

#[test]
fn normalize_trim_extract_resample_and_probe_work() {
    if !ffmpeg_installed() {
        eprintln!("skipped: ffmpeg not installed");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("tono ü.wav");
    make_tone(&input);
    let out_dir = dir.path().join("out");

    let run = |args: &[&str]| {
        let o = avlt()
            .arg("media")
            .args(args)
            .arg(&input)
            .arg("-o")
            .arg(&out_dir)
            .output()
            .unwrap();
        assert_eq!(o.status.code(), Some(0), "{args:?}: {}", stderr(&o));
    };
    run(&["normalize"]);
    run(&["trim", "--start", "00:00:00", "--end", "00:00:00.5"]);
    run(&["extract", "--format", "flac"]);
    run(&["resample", "--rate", "44100"]);
    assert_eq!(
        names_in(&out_dir),
        [
            "tono ü_44100.wav",
            "tono ü_audio.flac",
            "tono ü_norm.wav",
            "tono ü_trim.wav"
        ]
    );

    let o = avlt()
        .args(["media", "probe", "--json"])
        .arg(out_dir.join("tono ü_44100.wav"))
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let json: serde_json::Value = serde_json::from_str(&stdout(&o)).unwrap();
    assert_eq!(json["sample_rate_hz"], 44_100);
    assert_eq!(json["codec"], "pcm_s16le");
    assert_eq!(json["file"], "tono ü_44100.wav");

    let o = avlt()
        .args(["media", "probe"])
        .arg(&input)
        .output()
        .unwrap();
    assert!(stdout(&o).contains("=== AUDIO PROBE ==="), "{}", stdout(&o));
}
