//! CLI contract: command tree, help and exit codes.

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

/// Groups whose subcommands are still stubs (`calc`, `edid`, `net`, `media` and `ndi` are implemented; see `cli_*.rs`).
const GROUPS: [(&str, &[&str]); 2] = [
    (
        "sys",
        &[
            "info",
            "tele",
            "top",
            "killp",
            "usb",
            "disks",
            "monitors",
            "monitors-arrange",
            "battery",
            "startup",
            "audio",
            "services-audio",
            "cleanup",
        ],
    ),
    ("util", &["backup", "search", "weather", "timer"]),
];

#[test]
fn help_lists_all_groups() {
    let o = avlt(&["--help"]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let out = stdout(&o);
    for group in ["calc", "edid", "net", "sys", "media", "ndi", "util", "gui"] {
        assert!(out.contains(group), "help is missing '{group}':\n{out}");
    }
    assert!(out.contains("--json"), "help is missing --json:\n{out}");
}

#[test]
fn no_args_prints_help_and_exits_0() {
    let o = avlt(&[]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert!(stdout(&o).contains("Usage:"), "{}", stdout(&o));
}

#[test]
fn every_subcommand_exists_and_is_a_stub() {
    for (group, leaves) in GROUPS {
        for leaf in leaves {
            let o = avlt(&[group, leaf]);
            assert_eq!(
                o.status.code(),
                Some(1),
                "avlt {group} {leaf}: {}",
                stderr(&o)
            );
            assert!(
                stderr(&o).contains("not implemented"),
                "avlt {group} {leaf}: {}",
                stderr(&o)
            );
        }
    }
}

#[test]
fn json_flag_is_global() {
    for args in [["--json", "gui"], ["gui", "--json"]] {
        let o = avlt(&args);
        assert_eq!(o.status.code(), Some(1), "{args:?}: {}", stderr(&o));
    }
}

#[test]
fn gui_is_not_implemented_yet() {
    let o = avlt(&["gui"]);
    assert_eq!(o.status.code(), Some(1));
    assert!(stderr(&o).contains("not implemented"), "{}", stderr(&o));
    assert!(stdout(&o).is_empty(), "{}", stdout(&o));
}

#[test]
fn unknown_subcommand_is_usage_error() {
    let o = avlt(&["calc", "nope"]);
    assert_eq!(o.status.code(), Some(2), "{}", stderr(&o));
    let o = avlt(&["frobnicate"]);
    assert_eq!(o.status.code(), Some(2), "{}", stderr(&o));
}

#[test]
fn piped_output_has_no_colour_and_json_stdout_stays_clean() {
    let o = avlt(&["--json", "gui"]);
    assert!(!stderr(&o).contains('\x1b'), "{:?}", stderr(&o));
    assert!(stderr(&o).starts_with("error: "), "{:?}", stderr(&o));
    assert!(stdout(&o).is_empty(), "{:?}", stdout(&o));
}
