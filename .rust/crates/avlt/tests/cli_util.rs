//! `avlt util ...` integration tests (no network: `weather` is only checked for its
//! argument validation).
#![allow(clippy::unwrap_used)] // helpers outside #[test] functions

use std::path::Path;
use std::process::{Command, Output, Stdio};

use serde_json::Value;

fn avlt(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_avlt"))
        .args(args)
        .stdin(Stdio::null())
        .output()
        .unwrap_or_else(|e| panic!("cannot run avlt: {e}"))
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

fn json(o: &Output) -> Value {
    serde_json::from_slice(&o.stdout)
        .unwrap_or_else(|e| panic!("stdout is not JSON ({e}): {}", stdout(o)))
}

fn s(p: &Path) -> &str {
    p.to_str().unwrap()
}

#[test]
fn timer_counts_down_and_finishes() {
    let o = avlt(&["util", "timer", "1s", "--no-beep"]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let text = stdout(&o);
    assert!(text.contains("Timer started - ends at"), "{text}");
    assert!(text.contains("Timer : DONE!"), "{text}");

    let o = avlt(&[
        "util",
        "timer",
        "2s",
        "--label",
        "Doors",
        "--no-beep",
        "--json",
    ]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let lines: Vec<Value> = stdout(&o)
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert!(lines.len() >= 2, "{lines:?}");
    assert_eq!(lines[0]["label"], "Doors");
    assert_eq!(lines[0]["remaining_s"], 2);
    let last = lines.last().unwrap();
    assert_eq!(last["done"], true);
    assert_eq!(last["remaining_s"], 0);
}

#[test]
fn timer_rejects_bad_durations() {
    for bad in ["", "5x", "0s", "1.5m", "m"] {
        let o = avlt(&["util", "timer", bad]);
        assert_eq!(o.status.code(), Some(2), "{bad:?}: {}", stderr(&o));
        assert!(stderr(&o).contains("5m"), "{}", stderr(&o));
    }
}

#[test]
fn backup_copies_into_dated_folder_without_overwriting() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("Sesión Ñandú");
    std::fs::create_dir_all(src.join("carpeta con espacios")).unwrap();
    std::fs::write(
        src.join("carpeta con espacios/vídeo final.mov"),
        b"0123456789",
    )
    .unwrap();
    std::fs::write(src.join("notas.txt"), b"abc").unwrap();
    let dest = tmp.path().join("Copias");

    let o = avlt(&["util", "backup", s(&src), s(&dest), "--json"]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let v = json(&o);
    assert_eq!(v["files"], 2);
    assert_eq!(v["bytes"], 13);
    let first = std::path::PathBuf::from(v["destination"].as_str().unwrap());
    let name = first.file_name().unwrap().to_string_lossy().into_owned();
    assert!(name.starts_with("Sesión Ñandú_backup_20"), "{name}");
    // _backup_yyyy-MM-dd_HH-mm
    assert_eq!(name.len() - "Sesión Ñandú_backup_".len(), 16, "{name}");
    assert!(first.join("carpeta con espacios/vídeo final.mov").is_file());

    // Quoted paths (pasted from Explorer) work; a second run never reuses the folder.
    let quoted = format!("\"{}\"", s(&src));
    let o = avlt(&["util", "backup", &quoted, s(&dest), "--json"]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let second = json(&o)["destination"].as_str().unwrap().to_string();
    assert_ne!(Path::new(&second), first);
    assert!(first.join("notas.txt").is_file());
}

#[test]
fn backup_errors() {
    let tmp = tempfile::tempdir().unwrap();
    let o = avlt(&["util", "backup", s(&tmp.path().join("nope")), s(tmp.path())]);
    assert_eq!(o.status.code(), Some(2), "{}", stderr(&o));
    assert!(stderr(&o).contains("does not exist"), "{}", stderr(&o));
    let o = avlt(&["util", "backup", s(tmp.path())]);
    assert_eq!(o.status.code(), Some(2), "{}", stderr(&o));
}

#[test]
fn search_lists_matches() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(tmp.path().join("sub")).unwrap();
    std::fs::write(tmp.path().join("Show Final.mp4"), vec![0u8; 2048]).unwrap();
    std::fs::write(tmp.path().join("sub/otro show.wav"), b"x").unwrap();
    std::fs::write(tmp.path().join("readme.txt"), b"x").unwrap();

    let o = avlt(&["util", "search", "SHOW", s(tmp.path()), "--json"]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let hits = json(&o);
    let hits = hits.as_array().unwrap();
    assert_eq!(hits.len(), 2, "{hits:?}");
    assert_eq!(hits[0]["size_bytes"], 2048);

    let o = avlt(&["util", "search", "show", s(tmp.path())]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let text = stdout(&o);
    assert!(text.contains("[1] "), "{text}");
    assert!(text.contains("Size: 2 KB"), "{text}");

    let o = avlt(&["util", "search", "zzz", s(tmp.path())]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert!(stderr(&o).contains("No files found"), "{}", stderr(&o));

    let o = avlt(&["util", "search", "x", s(&tmp.path().join("missing"))]);
    assert_eq!(o.status.code(), Some(2), "{}", stderr(&o));
}

#[test]
fn weather_rejects_unknown_format() {
    let o = avlt(&["util", "weather", "Madrid", "--format", "huge"]);
    assert_eq!(o.status.code(), Some(2), "{}", stderr(&o));
}
