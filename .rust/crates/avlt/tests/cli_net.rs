//! `avlt net ...` integration tests. Nothing here changes the system: setip and
//! reset-nic are only driven into their validation / admin / confirmation refusals.
#![allow(clippy::unwrap_used)] // helpers outside #[test] functions

use std::net::{Ipv4Addr, TcpListener};
use std::process::{Command, Output};

use serde_json::Value;

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

fn json(o: &Output) -> Value {
    serde_json::from_slice(&o.stdout)
        .unwrap_or_else(|e| panic!("stdout is not JSON ({e}): {}", stdout(o)))
}

/// One JSON document per stdout line (live monitors).
fn json_lines(o: &Output) -> Vec<Value> {
    stdout(o)
        .lines()
        .map(|l| serde_json::from_str(l).unwrap_or_else(|e| panic!("bad JSON line ({e}): {l}")))
        .collect()
}

fn free_port() -> u16 {
    TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

#[test]
fn portcheck_open_port_json() {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let port = listener.local_addr().unwrap().port().to_string();
    let o = avlt(&["net", "portcheck", "127.0.0.1", &port, "--json"]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let v = json(&o);
    assert_eq!(v["open"], true, "{v}");
    assert_eq!(v["port"].to_string(), port);
    assert_eq!(v["ip"], "127.0.0.1");
}

#[test]
fn portcheck_closed_port_text() {
    let port = free_port().to_string();
    let o = avlt(&["net", "portcheck", "127.0.0.1", &port]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert!(stdout(&o).contains("CLOSED"), "{}", stdout(&o));
}

#[test]
fn portcheck_rejects_bad_port_and_host() {
    for port in ["0", "65536", "http"] {
        let o = avlt(&["net", "portcheck", "127.0.0.1", port]);
        assert_eq!(o.status.code(), Some(2), "port {port}: {}", stderr(&o));
    }
    let o = avlt(&["net", "portcheck", "no such host.invalid", "80"]);
    assert_eq!(o.status.code(), Some(2), "{}", stderr(&o));
}

#[test]
fn portscan_localhost_lists_the_14_common_ports() {
    let o = avlt(&["net", "portscan", "127.0.0.1", "--json"]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let v = json(&o);
    let ports = v["ports"].as_array().unwrap();
    assert_eq!(ports.len(), 14);
    assert!(
        ports
            .iter()
            .any(|p| p["port"] == 3389 && p["service"] == "RDP")
    );
    assert!(ports.iter().all(|p| p["open"].is_boolean()));
}

#[test]
fn scan_rejects_bad_subnet() {
    for bad in ["192.168", "192.168.1.0/16", "a.b.c"] {
        let o = avlt(&["net", "scan", bad]);
        assert_eq!(o.status.code(), Some(2), "{bad}: {}", stderr(&o));
    }
}

#[test]
fn whoison_rejects_unresolvable_host() {
    let o = avlt(&["net", "whoison", "no such host.invalid"]);
    assert_eq!(o.status.code(), Some(2), "{}", stderr(&o));
}

#[test]
fn info_and_myip() {
    let info = avlt(&["net", "info", "--json"]);
    let myip = avlt(&["net", "myip", "--json"]);
    if cfg!(windows) {
        match info.status.code() {
            Some(0) => assert!(json(&info)["name"].is_string()),
            // A PC without any active adapter: warning only, exit 1.
            Some(1) => {
                assert!(stderr(&info).contains("warning: no active network adapter"));
                assert!(stdout(&info).is_empty());
            }
            other => panic!("net info exit {other:?}: {}", stderr(&info)),
        }
        assert_eq!(myip.status.code(), Some(0), "{}", stderr(&myip));
        let v = json(&myip);
        assert!(v["adapters"].is_array());
        assert!(v.get("public_ip").is_some(), "{v}");
    } else {
        assert_eq!(info.status.code(), Some(5));
        assert_eq!(myip.status.code(), Some(5));
    }
}

#[test]
fn info_unknown_adapter_is_usage_error() {
    let o = avlt(&["net", "info", "avlt-test-no-such-adapter"]);
    let expected = if cfg!(windows) { 2 } else { 5 };
    assert_eq!(o.status.code(), Some(expected), "{}", stderr(&o));
}

#[test]
fn connections_lists_a_local_connection() {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    let _client = std::net::TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
    let _server = listener.accept().unwrap();
    let o = avlt(&["net", "connections", "--json"]);
    if cfg!(windows) {
        assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
        let v = json(&o);
        assert!(
            v.as_array()
                .unwrap()
                .iter()
                .any(|c| c["remote_port"] == port || c["local_port"] == port),
            "{v}"
        );
    } else {
        assert_eq!(o.status.code(), Some(5));
    }
}

#[test]
fn latency_emits_one_json_line_per_sample() {
    let o = avlt(&["net", "latency", "127.0.0.1", "-n", "2", "--json"]);
    if !cfg!(windows) {
        assert_eq!(o.status.code(), Some(5), "{}", stderr(&o));
        return;
    }
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let lines = json_lines(&o);
    assert_eq!(lines.len(), 2, "{}", stdout(&o));
    assert!(lines[0]["latency_ms"].is_u64(), "{}", lines[0]);
    assert_eq!(lines[1]["stats"]["sent"], 2);
    assert_eq!(lines[1]["stats"]["loss_pct"], 0.0);
}

#[test]
fn latency_text_has_summary() {
    let o = avlt(&["net", "latency", "127.0.0.1", "-n", "1"]);
    if cfg!(windows) {
        assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
        assert!(stdout(&o).contains(" ms"), "{}", stdout(&o));
        assert!(stdout(&o).contains("jitter"), "{}", stdout(&o));
    }
}

#[test]
fn pingmulti_round_with_dedup_and_unresolvable_host() {
    let o = avlt(&[
        "net",
        "pingmulti",
        "127.0.0.1,127.0.0.1",
        "localhost",
        "no such host.invalid",
        "-n",
        "1",
        "--json",
    ]);
    if !cfg!(windows) {
        assert_eq!(o.status.code(), Some(5), "{}", stderr(&o));
        return;
    }
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let lines = json_lines(&o);
    assert_eq!(lines.len(), 1, "{}", stdout(&o));
    let hosts = lines[0]["hosts"].as_array().unwrap();
    assert_eq!(hosts.len(), 3, "{}", lines[0]);
    assert_eq!(hosts[0]["online"], true);
    assert_eq!(hosts[1]["online"], true);
    assert_eq!(hosts[2]["ip"], Value::Null);
    assert_eq!(hosts[2]["online"], false);
    assert_eq!(hosts[2]["failures"], 1);
}

#[test]
fn pingmulti_needs_a_host() {
    let o = avlt(&["net", "pingmulti"]);
    assert_eq!(o.status.code(), Some(2), "{}", stderr(&o));
}

#[test]
fn speed_samples_default_adapter() {
    let o = avlt(&["net", "speed", "-n", "1", "--json"]);
    if !cfg!(windows) {
        assert_eq!(o.status.code(), Some(5), "{}", stderr(&o));
        return;
    }
    match o.status.code() {
        Some(0) => {
            let lines = json_lines(&o);
            assert_eq!(lines.len(), 1, "{}", stdout(&o));
            assert!(lines[0]["down_bps"].is_number());
            assert!(lines[0]["total_up_bytes"].is_u64());
        }
        Some(1) => assert!(stderr(&o).contains("no active network adapter")),
        other => panic!("exit {other:?}: {}", stderr(&o)),
    }
    let o = avlt(&["net", "speed", "avlt-test-no-such-adapter", "-n", "1"]);
    assert_eq!(o.status.code(), Some(2), "{}", stderr(&o));
}

#[test]
fn speedtest_without_cli_suggests_winget() {
    let o = Command::new(env!("CARGO_BIN_EXE_avlt"))
        .args(["net", "speedtest"])
        .env("PATH", "")
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(3), "{}", stderr(&o));
    assert!(
        stderr(&o).contains("winget install Ookla.Speedtest.CLI"),
        "{}",
        stderr(&o)
    );
}

/// Exit code expected when a system-changing command passes validation: 4 without
/// elevation; elevated, the bogus adapter name is a usage error (2); 5 off Windows.
fn refusal_code() -> i32 {
    if !cfg!(windows) {
        5
    } else if avlt_platform::admin::is_admin() {
        2
    } else {
        4
    }
}

#[test]
fn setip_rejects_bad_targets_with_usage() {
    for args in [
        &["bogus"][..],
        &["192.168.1.25"],
        &["192.168.1.25/33"],
        &["192.168.1.25/24", "192.168.1.300"],
        &["dhcp", "192.168.1.1"],
    ] {
        let mut full = vec!["net", "setip"];
        full.extend_from_slice(args);
        full.extend_from_slice(&["--iface", "avlt-test-no-such-adapter"]);
        let o = avlt(&full);
        assert_eq!(o.status.code(), Some(2), "{args:?}: {}", stderr(&o));
        assert!(stderr(&o).contains("usage:"), "{args:?}: {}", stderr(&o));
        assert!(stdout(&o).is_empty(), "{}", stdout(&o));
    }
}

#[test]
fn setip_and_reset_nic_never_reach_netsh_in_tests() {
    for args in [
        &[
            "setip",
            "10.0.0.5/24",
            "--iface",
            "avlt-test-no-such-adapter",
        ][..],
        &["setip", "0", "--iface", "avlt-test-no-such-adapter"],
        &[
            "setip",
            "dhcp",
            "--iface",
            "avlt-test-no-such-adapter",
            "--yes",
        ],
        &["reset-nic", "avlt-test-no-such-adapter"],
        &["reset-nic", "avlt-test-no-such-adapter", "--yes"],
    ] {
        let mut full = vec!["net"];
        full.extend_from_slice(args);
        let o = avlt(&full);
        assert_eq!(
            o.status.code(),
            Some(refusal_code()),
            "{args:?}: {}",
            stderr(&o)
        );
        assert!(!stdout(&o).contains("OK"), "{}", stdout(&o));
        if refusal_code() == 4 {
            assert!(stderr(&o).contains("administrator"), "{}", stderr(&o));
        }
    }
}

#[test]
fn flushdns_is_wired_up() {
    // Not executed: it changes system state. Only the command surface is checked.
    let o = avlt(&["net", "flushdns", "--help"]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert!(stdout(&o).contains("DNS"), "{}", stdout(&o));
}
