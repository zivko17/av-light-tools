//! `avlt sys ...` integration tests. Read-only commands run for real; commands that
//! change the system (killp, monitors-arrange, services-audio --restart, cleanup) are only
//! driven into their refusals, CDS_TEST validation, or a process the test spawned itself.
#![allow(clippy::unwrap_used)] // helpers outside #[test] functions

use std::process::{Command, Output, Stdio};

use serde_json::Value;

fn avlt(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_avlt"))
        .args(args)
        .stdin(Stdio::null())
        .output()
        .unwrap_or_else(|e| panic!("cannot run avlt: {e}"))
}

#[cfg_attr(not(windows), allow(dead_code))]
fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

#[cfg_attr(not(windows), allow(dead_code))]
fn json(o: &Output) -> Value {
    serde_json::from_slice(&o.stdout)
        .unwrap_or_else(|e| panic!("stdout is not JSON ({e}): {}", stdout(o)))
}

#[cfg_attr(not(windows), allow(dead_code))]
fn json_lines(o: &Output) -> Vec<Value> {
    stdout(o)
        .lines()
        .map(|l| serde_json::from_str(l).unwrap_or_else(|e| panic!("bad JSON line ({e}): {l}")))
        .collect()
}

#[test]
fn bad_arguments_are_usage_errors() {
    for args in [
        &["sys", "top", "--count", "0"][..],
        &["sys", "tele", "-n", "0"],
        &["sys", "killp"],
        &["sys", "info", "extra"],
    ] {
        let o = avlt(args);
        assert_eq!(o.status.code(), Some(2), "{args:?}: {}", stderr(&o));
    }
}

#[cfg(not(windows))]
#[test]
fn unsupported_outside_windows() {
    for args in [
        &["sys", "info"][..],
        &["sys", "disks"],
        &["sys", "startup"],
        &["sys", "usb"],
        &["sys", "battery"],
        &["sys", "cleanup", "--analyze"],
    ] {
        let o = avlt(args);
        assert_eq!(o.status.code(), Some(5), "{args:?}: {}", stderr(&o));
    }
}

#[cfg(windows)]
mod windows {
    use super::*;

    #[test]
    fn info_json_and_text() {
        let o = avlt(&["sys", "info", "--json"]);
        assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
        let v = json(&o);
        assert!(!v["hostname"].as_str().unwrap().is_empty(), "{v}");
        assert!(v["os"]["caption"].as_str().unwrap().contains("Windows"));
        assert!(v["ram"]["total_bytes"].as_u64().unwrap() > 0);
        assert!(
            v["volumes"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["drive"] == "C:")
        );

        let o = avlt(&["sys", "info"]);
        assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
        let text = stdout(&o);
        for s in ["=== SYSTEM INFO ===", "--- CPU ---", "Hostname", "Uptime"] {
            assert!(text.contains(s), "missing {s}:\n{text}");
        }
        assert!(!text.contains('\x1b'), "piped output must be plain");
    }

    #[test]
    fn read_only_listings_succeed() {
        for (args, check) in [
            (&["sys", "disks", "--json"][..], "volumes"),
            (&["sys", "monitors", "--json"], "screens"),
            (&["sys", "cleanup", "--analyze", "--json"], "targets"),
        ] {
            let o = avlt(args);
            assert_eq!(o.status.code(), Some(0), "{args:?}: {}", stderr(&o));
            assert!(json(&o).get(check).is_some(), "{args:?}: {}", stdout(&o));
        }
        for args in [
            &["sys", "startup", "--json"][..],
            &["sys", "usb", "--json"],
            &["sys", "services-audio", "--json"],
        ] {
            let o = avlt(args);
            assert_eq!(o.status.code(), Some(0), "{args:?}: {}", stderr(&o));
            assert!(json(&o).is_array(), "{args:?}: {}", stdout(&o));
        }
        let o = avlt(&["sys", "services-audio", "--json"]);
        assert!(stdout(&o).contains("Audiosrv"), "{}", stdout(&o));
        let o = avlt(&["sys", "battery", "--json"]);
        assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
        let v = json(&o);
        assert!(v.is_null() || v["charge_pct"].is_u64(), "{v}");
    }

    #[test]
    fn cleanup_analyze_never_lists_prefetch() {
        let o = avlt(&["sys", "cleanup", "--analyze", "--json"]);
        assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
        assert!(
            !stdout(&o).to_lowercase().contains("prefetch"),
            "{}",
            stdout(&o)
        );
    }

    #[test]
    fn audio_json_or_readable_error() {
        // CI runners have no audio stack worth the name.
        let o = avlt(&["sys", "audio", "--json"]);
        match o.status.code() {
            Some(0) => assert!(json(&o)["playback"].is_array()),
            Some(1) => assert!(stderr(&o).contains("Core Audio"), "{}", stderr(&o)),
            c => panic!("unexpected exit {c:?}: {}", stderr(&o)),
        }
    }

    #[test]
    fn tele_samples_as_json_lines() {
        let o = avlt(&["sys", "tele", "-n", "2", "--json"]);
        assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
        let lines = json_lines(&o);
        assert_eq!(lines.len(), 2);
        for v in &lines {
            let cpu = v["cpu_pct"].as_f64().unwrap();
            assert!((0.0..=100.0).contains(&cpu), "{v}");
            assert!(v["ram_total_bytes"].as_u64().unwrap() > 0);
            assert_eq!(v["time"].as_str().unwrap().len(), 8);
        }
    }

    #[test]
    fn top_one_sample() {
        let o = avlt(&["sys", "top", "-n", "1", "--count", "3", "--json"]);
        assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
        let lines = json_lines(&o);
        assert_eq!(lines.len(), 1);
        let procs = lines[0]["processes"].as_array().unwrap();
        assert_eq!(procs.len(), 3);
        let cpus: Vec<f64> = procs
            .iter()
            .map(|p| p["cpu_pct"].as_f64().unwrap())
            .collect();
        assert!(
            cpus.windows(2).all(|w| w[0] >= w[1]),
            "not sorted by CPU: {cpus:?}"
        );

        let o = avlt(&["sys", "top", "-n", "1", "-c", "2"]);
        assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
        assert!(stdout(&o).contains("PROCESS"), "{}", stdout(&o));
    }

    /// Spawns a private copy of ping.exe under a unique name; the only process killed.
    fn spawn_victim(name: &str) -> (std::process::Child, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join(format!("{name}.exe"));
        let system = std::env::var("SystemRoot").unwrap();
        std::fs::copy(format!(r"{system}\System32\ping.exe"), &exe).unwrap();
        let child = Command::new(&exe)
            .args(["-n", "60", "127.0.0.1"])
            .stdout(Stdio::null())
            .spawn()
            .unwrap();
        (child, dir)
    }

    #[test]
    fn killp_refuses_without_yes_then_kills_with_yes() {
        let (mut child, _dir) = spawn_victim("avltclivictim");

        let o = avlt(&["sys", "killp", "avltclivictim"]);
        assert_eq!(o.status.code(), Some(2), "{}", stderr(&o));
        assert!(stderr(&o).contains("--yes"), "{}", stderr(&o));
        assert!(child.try_wait().unwrap().is_none(), "killed without --yes");

        // Exact match by default: a partial name matches nothing.
        let o = avlt(&["sys", "killp", "avltcliv", "--yes"]);
        assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
        assert!(stderr(&o).contains("No process found"), "{}", stderr(&o));
        assert!(child.try_wait().unwrap().is_none());

        let o = avlt(&["sys", "killp", "avltcliv", "--wildcard", "--yes", "--json"]);
        assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
        let v = json(&o);
        assert_eq!(v["results"][0]["killed"], true, "{v}");
        assert_eq!(v["results"][0]["pid"], child.id());
        assert_eq!(v["matched"].as_array().unwrap().len(), 1);
        assert!(!child.wait().unwrap().success());
    }

    #[test]
    fn killp_flags_critical_processes_but_refuses_without_yes() {
        let o = avlt(&["sys", "killp", "explorer"]);
        // explorer may not run on a CI runner: no match is exit 0, a match is refused.
        match o.status.code() {
            Some(0) => assert!(stderr(&o).contains("No process found")),
            Some(2) => {
                assert!(stdout(&o).contains("<- CRITICAL"), "{}", stdout(&o));
                assert!(stderr(&o).contains("critical system processes"));
            }
            c => panic!("unexpected exit {c:?}: {}", stderr(&o)),
        }
    }

    #[test]
    fn monitors_arrange_lists_validates_and_refuses() {
        let o = avlt(&["sys", "monitors-arrange", "--json"]);
        if o.status.code() == Some(1) {
            // Headless runner without a desktop display.
            assert!(stderr(&o).contains("no display"), "{}", stderr(&o));
            return;
        }
        assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
        let list = json(&o);
        let first = &list.as_array().unwrap()[0];
        assert_eq!(first["index"], 1);

        let o = avlt(&["sys", "monitors-arrange", "--display", "99"]);
        assert_eq!(o.status.code(), Some(2), "{}", stderr(&o));
        let o = avlt(&["sys", "monitors-arrange", "-d", "1", "-r", "12345x7"]);
        assert_eq!(o.status.code(), Some(2), "{}", stderr(&o));
        let o = avlt(&["sys", "monitors-arrange", "-d", "1", "-r", "axb"]);
        assert_eq!(o.status.code(), Some(2), "{}", stderr(&o));

        let Some(cur) = first["current"].as_object() else {
            return;
        };
        let res = format!("{}x{}", cur["width"], cur["height"]);
        let hz = cur["refresh_hz"].to_string();
        let offered = first["modes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m.as_object() == Some(cur));
        if !offered {
            return;
        }
        // Without --yes and without a terminal: refused before anything is applied.
        let o = avlt(&[
            "sys",
            "monitors-arrange",
            "-d",
            "1",
            "-r",
            &res,
            "--hz",
            &hz,
        ]);
        assert_eq!(o.status.code(), Some(2), "{}", stderr(&o));
        assert!(stderr(&o).contains("--yes"), "{}", stderr(&o));
        // CDS_TEST only: the driver validates the current mode, nothing changes.
        let o = avlt(&[
            "sys",
            "monitors-arrange",
            "-d",
            "1",
            "-r",
            &res,
            "--hz",
            &hz,
            "--test",
            "--json",
        ]);
        assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
        assert_eq!(json(&o)["test_only"], true);
    }

    #[test]
    fn services_restart_is_refused_without_admin_or_confirmation() {
        let o = avlt(&["sys", "services-audio", "--restart"]);
        // 4 = not elevated; 2 = elevated but no terminal and no --yes. Never restarts.
        assert!(
            matches!(o.status.code(), Some(2 | 4)),
            "{:?}: {}",
            o.status.code(),
            stderr(&o)
        );
    }

    #[test]
    fn cleanup_without_yes_is_refused() {
        let o = avlt(&["sys", "cleanup"]);
        assert_eq!(o.status.code(), Some(2), "{}", stderr(&o));
        assert!(stderr(&o).contains("--yes"), "{}", stderr(&o));
        let o = avlt(&["sys", "cleanup", "--json"]);
        assert_eq!(o.status.code(), Some(2), "{}", stderr(&o));
        assert!(stdout(&o).is_empty(), "{}", stdout(&o));
    }
}
