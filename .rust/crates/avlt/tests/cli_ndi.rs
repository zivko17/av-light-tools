//! `avlt ndi`: launch aliases and install detection (against a fake install), the config
//! reader (against a temporary `%APPDATA%`) and the read-only diagnostics.
// Test-only helpers outside #[test] fns are not covered by clippy.toml's test allowance.
#![allow(clippy::unwrap_used)]

use std::path::Path;
use std::process::{Command, Output};

fn avlt(args: &[&str]) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_avlt"));
    c.args(args);
    c
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

#[test]
fn launch_help_lists_every_alias() {
    let o = avlt(&["ndi", "launch", "--help"]).output().unwrap();
    assert_eq!(o.status.code(), Some(0));
    for alias in [
        "monitor", "router", "test", "bridge", "manager", "screen", "launcher",
    ] {
        assert!(stdout(&o).contains(alias), "{}", stdout(&o));
    }
}

#[test]
fn launch_rejects_unknown_apps_and_missing_app() {
    for args in [&["ndi", "launch", "help"][..], &["ndi", "launch"]] {
        let o = avlt(args).output().unwrap();
        assert_eq!(o.status.code(), Some(2), "{args:?}: {}", stderr(&o));
    }
}

#[test]
fn launch_without_an_install_is_a_missing_tool() {
    let empty = tempfile::tempdir().unwrap();
    // Pre-flight: `launch` may only run once AVLT_NDI_ROOT is proven to be honoured,
    // otherwise it would start the real NDI app of the machine running the tests.
    if cfg!(windows) {
        let pre = avlt(&["ndi", "check", "--json"])
            .env("AVLT_NDI_ROOT", empty.path())
            .output()
            .unwrap();
        let json: serde_json::Value = serde_json::from_str(&stdout(&pre)).unwrap();
        let base = json["install"]["base"].as_str().unwrap();
        assert!(
            base.starts_with(empty.path().to_str().unwrap()),
            "AVLT_NDI_ROOT not honoured ({base}); refusing to run `ndi launch`"
        );
        assert_eq!(json["install"]["installed"], false);
    }
    let o = avlt(&["ndi", "launch", "monitor"])
        .env("AVLT_NDI_ROOT", empty.path())
        .output()
        .unwrap();
    // Exit 3 (tool missing) on Windows; elsewhere the whole command is unsupported (5).
    let expected = if cfg!(windows) { 3 } else { 5 };
    assert_eq!(o.status.code(), Some(expected), "{}", stderr(&o));
    assert!(stdout(&o).is_empty(), "{}", stdout(&o));
}

/// Fake `NDI 5 Tools` and `NDI 7 Tools` folders: the newest must be detected. Uses `ndi
/// check`, which only reads; no test may reach the code that starts a process.
#[test]
fn check_detects_the_newest_install_in_a_fake_root() {
    let root = tempfile::tempdir().unwrap();
    for version in [5, 7] {
        std::fs::create_dir_all(root.path().join(format!("NDI {version} Tools"))).unwrap();
    }
    let o = avlt(&["ndi", "check", "--json"])
        .env("AVLT_NDI_ROOT", root.path())
        .output()
        .unwrap();
    if !cfg!(windows) {
        assert_eq!(o.status.code(), Some(5), "{}", stderr(&o));
        return;
    }
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let json: serde_json::Value = serde_json::from_str(&stdout(&o)).unwrap();
    let base = json["install"]["base"].as_str().unwrap();
    assert!(base.ends_with("NDI 7 Tools"), "{base}");
    assert!(base.starts_with(root.path().to_str().unwrap()), "{base}");
    assert_eq!(json["install"]["version"], 7);
    assert_eq!(json["install"]["installed"], true);
}

fn write_config(appdata: &Path, body: &str) {
    let dir = appdata.join("NDI");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("ndi-config.v1.json"), body).unwrap();
}

#[test]
fn config_reads_groups_discovery_and_ips() {
    let appdata = tempfile::tempdir().unwrap();
    write_config(
        appdata.path(),
        r#"{"ndi":{"groups":{"send":"public, show","recv":["a"]},
            "networks":{"discovery":"10.0.0.5","ips":"1.2.3.4"}}}"#,
    );
    let o = avlt(&["ndi", "config", "--json"])
        .env("APPDATA", appdata.path())
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let json: serde_json::Value = serde_json::from_str(&stdout(&o)).unwrap();
    assert_eq!(json["found"], true);
    assert_eq!(json["groups_send"], serde_json::json!(["public", "show"]));
    assert_eq!(json["groups_recv"], serde_json::json!(["a"]));
    assert_eq!(json["discovery"], serde_json::json!(["10.0.0.5"]));
    assert_eq!(json["ips"], serde_json::json!(["1.2.3.4"]));

    let text = avlt(&["ndi", "config"])
        .env("APPDATA", appdata.path())
        .output()
        .unwrap();
    assert!(
        stdout(&text).contains("Groups (sending)"),
        "{}",
        stdout(&text)
    );
    assert!(stdout(&text).contains("show"), "{}", stdout(&text));
}

#[test]
fn config_missing_is_not_an_error_but_garbage_is() {
    let appdata = tempfile::tempdir().unwrap();
    let o = avlt(&["ndi", "config"])
        .env("APPDATA", appdata.path())
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert!(
        stderr(&o).contains("no config file found"),
        "{}",
        stderr(&o)
    );
    assert!(
        stdout(&o).contains("avlt ndi launch manager"),
        "{}",
        stdout(&o)
    );

    write_config(appdata.path(), "{ broken");
    let o = avlt(&["ndi", "config"])
        .env("APPDATA", appdata.path())
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(1), "{}", stderr(&o));
    assert!(stderr(&o).contains("cannot parse"), "{}", stderr(&o));
}

#[cfg(windows)]
#[test]
fn check_and_services_are_read_only_diagnostics() {
    let o = avlt(&["ndi", "check", "--json"]).output().unwrap();
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let json: serde_json::Value = serde_json::from_str(&stdout(&o)).unwrap();
    for key in ["install", "adapters", "notes"] {
        assert!(json.get(key).is_some(), "missing {key}: {json}");
    }

    let o = avlt(&["ndi", "services", "--json"]).output().unwrap();
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let json: serde_json::Value = serde_json::from_str(&stdout(&o)).unwrap();
    assert!(json["services"].is_array() && json["processes"].is_array());

    let o = avlt(&["ndi", "check"]).output().unwrap();
    assert!(
        stdout(&o).contains("=== NDI NETWORK DIAGNOSTIC ==="),
        "{}",
        stdout(&o)
    );
}

#[cfg(not(windows))]
#[test]
fn diagnostics_are_unsupported_off_windows() {
    for cmd in ["check", "services"] {
        let o = avlt(&["ndi", cmd]).output().unwrap();
        assert_eq!(o.status.code(), Some(5), "{cmd}: {}", stderr(&o));
    }
}
