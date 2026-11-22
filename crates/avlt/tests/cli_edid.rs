//! `avlt edid` on .bin files: info, check, diff and dump (no display hardware needed).
// Test-only helpers outside #[test] fns are not covered by clippy.toml's test allowance.
#![allow(clippy::unwrap_used)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn avlt(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_avlt"))
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("cannot run avlt: {e}"))
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

/// Committed synthetic fixtures (kept equal to `synthetic_base()` by an avlt-core test).
fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../avlt-core/tests/fixtures/edid")
        .join(name)
}

fn s(p: &Path) -> &str {
    p.to_str().unwrap()
}

fn json(o: &Output) -> serde_json::Value {
    serde_json::from_slice(&o.stdout)
        .unwrap_or_else(|e| panic!("bad JSON ({e}): {}\n{}", stdout(o), stderr(o)))
}

/// Copy of the base fixture with a wrong checksum, in `dir`.
fn corrupt_copy(dir: &Path) -> PathBuf {
    let mut bytes = std::fs::read(fixture("synthetic_base.bin")).unwrap();
    bytes[127] = bytes[127].wrapping_add(1);
    let p = dir.join("corrupt.bin");
    std::fs::write(&p, bytes).unwrap();
    p
}

#[test]
fn edid_subcommands_have_help() {
    for leaf in ["list", "info", "check", "diff", "dump"] {
        let o = avlt(&["edid", leaf, "--help"]);
        assert_eq!(o.status.code(), Some(0), "edid {leaf}: {}", stderr(&o));
    }
}

#[test]
fn info_json_has_manufacturer() {
    let f = fixture("synthetic_base.bin");
    let o = avlt(&["edid", "info", s(&f), "--json"]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let compact = stdout(&o).replace(char::is_whitespace, "");
    assert!(
        compact.contains(r#""manufacturer":"DEL""#),
        "{}",
        stdout(&o)
    );
    let v = json(&o);
    assert_eq!(v["source"], "synthetic_base.bin");
    assert_eq!(v["edid"]["base"]["native"]["h_active"], 1920);
    assert!(v["edid"]["problems"].as_array().unwrap().is_empty());
}

#[test]
fn info_text_shows_base_and_cta() {
    let o = avlt(&["edid", "info", s(&fixture("synthetic_cta.bin"))]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let out = stdout(&o);
    for needle in [
        "DEL  (Dell)",
        "DELL TEST",
        "1920x1080p @ 60 Hz",
        "CTA-861 extension (block 1, rev 3)",
        "*16  1920x1080p60",
        "LPCM       2 ch",
        "HDMI vendor block (00-0C-03)",
        "Source physical address: 1.0.0.0",
        "PQ (SMPTE ST 2084)",
        "Ext 1 checksum:",
    ] {
        assert!(out.contains(needle), "missing {needle:?}:\n{out}");
    }
    assert!(!out.contains('\x1b'), "piped output must not be coloured");
}

#[test]
fn info_on_truncated_file_reports_problem_without_failing() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("short.bin");
    std::fs::write(
        &p,
        &std::fs::read(fixture("synthetic_base.bin")).unwrap()[..100],
    )
    .unwrap();
    let o = avlt(&["edid", "info", s(&p), "--json"]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let v = json(&o);
    assert!(v["edid"]["base"].is_null());
    let problems = v["edid"]["problems"].as_array().unwrap();
    assert!(
        problems[0].as_str().unwrap().contains("100 bytes"),
        "{problems:?}"
    );
    assert!(stderr(&o).contains("100 bytes"), "{}", stderr(&o));
}

#[test]
fn missing_file_is_usage_error() {
    let o = avlt(&["edid", "info", "no such file.bin"]);
    assert_eq!(o.status.code(), Some(2), "{}", stderr(&o));
    assert!(stderr(&o).contains("File not found"), "{}", stderr(&o));
}

#[test]
fn check_clean_file_passes() {
    let o = avlt(&["edid", "check", s(&fixture("synthetic_cta.bin"))]);
    assert_eq!(o.status.code(), Some(0), "{}{}", stdout(&o), stderr(&o));
    assert!(stdout(&o).contains("No problems found."), "{}", stdout(&o));
}

#[test]
fn check_corrupt_file_fails() {
    let dir = tempfile::tempdir().unwrap();
    let p = corrupt_copy(dir.path());
    let o = avlt(&["edid", "check", s(&p)]);
    assert_ne!(o.status.code(), Some(0), "{}", stdout(&o));
    assert!(
        stdout(&o).contains("[ERROR] Block 0 checksum is wrong"),
        "{}",
        stdout(&o)
    );
    assert!(stderr(&o).contains("1 error"), "{}", stderr(&o));

    let o = avlt(&["edid", "check", s(&p), "--json"]);
    assert_ne!(o.status.code(), Some(0));
    let v = json(&o);
    assert_eq!(v[0]["findings"][0]["severity"], "Error");
}

#[test]
fn diff_identical_and_different() {
    let base = fixture("synthetic_base.bin");
    let o = avlt(&["edid", "diff", s(&base), s(&base), "--json"]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert!(json(&o)["differences"].as_array().unwrap().is_empty());

    let o = avlt(&["edid", "diff", s(&base), s(&base)]);
    assert!(stdout(&o).contains("byte-identical"), "{}", stdout(&o));

    let o = avlt(&["edid", "diff", s(&base), s(&fixture("synthetic_cta.bin"))]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let out = stdout(&o);
    assert!(out.contains("! CTA-861"), "{out}");
    assert!(out.contains("only in B:  16 1920x1080p60"), "{out}");
    assert!(out.contains("Raw bytes: 130 of 256 differ"), "{out}");
    assert!(out.contains("more. Use --bytes"), "{out}");
}

#[test]
fn diff_reports_failing_side() {
    let o = avlt(&[
        "edid",
        "diff",
        s(&fixture("synthetic_base.bin")),
        "nope.bin",
    ]);
    assert_eq!(o.status.code(), Some(2));
    assert!(stderr(&o).contains("(B) File not found"), "{}", stderr(&o));
}

#[test]
fn dump_to_path_with_spaces_and_accent_never_overwrites() {
    let tmp = tempfile::tempdir().unwrap();
    let out_dir = tmp.path().join("salida con espacios é");
    let src_dir = tmp.path().join("orígenes y más");
    std::fs::create_dir(&src_dir).unwrap();
    let src = src_dir.join("pantalla ñ.bin");
    std::fs::copy(fixture("synthetic_base.bin"), &src).unwrap();
    let bytes = std::fs::read(&src).unwrap();

    let o = avlt(&["edid", "dump", s(&src), s(&out_dir), "--json"]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let first = out_dir.join("DEL_DELL_TEST.bin");
    assert_eq!(std::fs::read(&first).unwrap(), bytes);
    assert_eq!(json(&o)[0]["path"], s(&first));

    let o = avlt(&["edid", "dump", s(&src), s(&out_dir)]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let second = out_dir.join("DEL_DELL_TEST_1.bin");
    assert_eq!(std::fs::read(&second).unwrap(), bytes);
    assert!(stdout(&o).contains("DEL_DELL_TEST_1.bin"), "{}", stdout(&o));
    assert_eq!(std::fs::read(&first).unwrap(), bytes);
    assert_eq!(std::fs::read_dir(&out_dir).unwrap().count(), 2);
}

#[cfg(not(windows))]
#[test]
fn display_sources_are_unsupported_off_windows() {
    let o = avlt(&["edid", "list"]);
    assert_eq!(o.status.code(), Some(5), "{}", stderr(&o));
    let o = avlt(&["edid", "info", "1"]);
    assert_eq!(o.status.code(), Some(5), "{}", stderr(&o));
}
