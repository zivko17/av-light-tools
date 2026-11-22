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
