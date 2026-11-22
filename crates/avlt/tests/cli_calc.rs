//! `avlt calc`: end-to-end behaviour of the calculators through the real binary.

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

/// Runs `avlt calc ... --json`, asserts success and parses stdout as one JSON document.
fn json(args: &[&str]) -> Value {
    let mut full = vec!["calc"];
    full.extend_from_slice(args);
    full.push("--json");
    let o = avlt(&full);
    assert_eq!(o.status.code(), Some(0), "{full:?}: {}", stderr(&o));
    serde_json::from_slice(&o.stdout)
        .unwrap_or_else(|e| panic!("{full:?}: stdout is not JSON ({e}): {}", stdout(&o)))
}

fn close(v: &Value, expected: f64) {
    let got = v.as_f64().unwrap_or_else(|| panic!("not a number: {v}"));
    assert!((got - expected).abs() < 1e-6, "{got} != {expected}");
}

const LEAVES: [&str; 14] = [
    "subnet",
    "screen",
    "ledwall",
    "bitrate",
    "avoip",
    "projector",
    "pixelpitch",
    "brightness",
    "viewangle",
    "camera",
    "audiobw",
    "speaker",
    "cable",
    "dsp",
];

#[test]
fn every_calculator_has_help() {
    for leaf in LEAVES {
        let o = avlt(&["calc", leaf, "--help"]);
        assert_eq!(o.status.code(), Some(0), "calc {leaf}: {}", stderr(&o));
        assert!(stdout(&o).contains("Usage:"), "calc {leaf}");
    }
}

#[test]
fn subnet_json_has_254_hosts() {
    let v = json(&["subnet", "192.168.10.37/24"]);
    assert_eq!(v["hosts"], 254);
    assert_eq!(v["network"], "192.168.10.0");
    assert_eq!(v["netmask"], "255.255.255.0");
    assert_eq!(v["broadcast"], "192.168.10.255");
    assert_eq!(v["first_usable"], "192.168.10.1");
}

#[test]
fn subnet_text_output() {
    let o = avlt(&[
        "calc",
        "subnet",
        "192.168.10.0/24",
        "--devices",
        "3",
        "--start-host",
        "10",
    ]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let out = stdout(&o);
    assert!(!out.contains('\x1b'), "piped output must be plain: {out:?}");
    assert!(out.contains("=== SUBNET ==="), "{out}");
    assert!(out.contains("192.168.10.1 - 192.168.10.254"), "{out}");
    assert!(out.contains("Device 1    192.168.10.10"), "{out}");
    assert!(out.contains("Device 3    192.168.10.12"), "{out}");
}

#[test]
fn subnet_assignment_table_reports_overflow() {
    let o = avlt(&["calc", "subnet", "192.168.1.0/30", "--devices", "5"]);
    assert_eq!(o.status.code(), Some(0));
    assert!(
        stderr(&o).contains("exceeds usable range at device 3"),
        "{}",
        stderr(&o)
    );
}

#[test]
fn subnet_invalid_input_exits_2() {
    for bad in ["nope", "192.168.1.0", "192.168.1.0/33", "300.1.1.1/24", ""] {
        let o = avlt(&["calc", "subnet", bad]);
        assert_eq!(o.status.code(), Some(2), "{bad:?}: {}", stderr(&o));
        assert!(stderr(&o).starts_with("error: "), "{bad:?}: {}", stderr(&o));
        assert!(stdout(&o).is_empty(), "{bad:?}: {}", stdout(&o));
    }
}

#[test]
fn invalid_input_with_json_keeps_stdout_empty() {
    let o = avlt(&["calc", "subnet", "nope", "--json"]);
    assert_eq!(o.status.code(), Some(2));
    assert!(stdout(&o).is_empty(), "{}", stdout(&o));
}

#[test]
fn screen_forward_and_reverse() {
    let v = json(&["screen", "6", "--content", "detailed", "--aspect", "4:3"]);
    assert_eq!(v["mode"], "forward");
    close(&v["height"], 1.5);
    close(&v["diagonal"], 2.5);
    let v = json(&["screen", "--diagonal", "2.5", "--aspect", "4:3"]);
    assert_eq!(v["mode"], "reverse");
    close(&v["view_distance"], 9.0);
}

#[test]
fn screen_invalid_input_exits_2() {
    for args in [
        vec!["screen", "10", "--aspect", "16:0"],
        vec!["screen"],
        vec!["screen", "-5"],
        vec!["screen", "10", "--aspect", "nonsense"],
    ] {
        let mut full = vec!["calc"];
        full.extend(args.iter());
        let o = avlt(&full);
        assert_eq!(o.status.code(), Some(2), "{args:?}: {}", stderr(&o));
        assert!(!stderr(&o).contains("panicked"), "{args:?}: {}", stderr(&o));
    }
}

#[test]
fn ledwall_list_text_and_json() {
    let o = avlt(&["calc", "ledwall", "--list"]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let out = stdout(&o);
    assert!(out.contains("LED CABINET PRESETS"), "{out}");
    assert!(out.contains("absen-nt-in-1.9"), "{out}");
    assert!(out.contains("in-p2.6-510"), "{out}");

    let o = avlt(&["calc", "ledwall", "--list", "--json"]);
    let v: Value = serde_json::from_slice(&o.stdout).expect("list is JSON");
    assert_eq!(v.as_array().map(Vec::len), Some(51));
}

#[test]
fn ledwall_preset_json() {
    let v = json(&["ledwall", "10", "5", "--cabinet", "in-p2.6"]);
    assert_eq!(v["total_cabinets"], 50);
    assert_eq!(v["resolution"]["width"], 1920);
    assert_eq!(v["resolution"]["aspect"], "2:1");
    assert_eq!(v["power"]["circuits"], 1);
    assert_eq!(v["power"]["cables"], 5);
    assert_eq!(v["signal"]["ports"], 3);
    assert_eq!(
        v["signal"]["sender"]["suggestions"]["mctrl"]["name"],
        "MCTRL660"
    );
    close(&v["weight"]["total_kg"], 375.0);
}

#[test]
fn ledwall_manual_options_use_the_powershell_names() {
    let v = json(&[
        "ledwall",
        "2",
        "2",
        "--cab-px-w",
        "100",
        "--cab-px-h",
        "100",
        "--cab-wmm",
        "500",
        "--cab-hmm",
        "500",
        "--max-watt",
        "200",
        "--power-basis",
        "max",
        "--redundant",
        "--recv-card",
        "A10s Pro",
        "--sender",
        "MX40 Pro",
    ]);
    close(&v["power"]["max_watt_per_cab"], 200.0);
    assert_eq!(v["power"]["basis"], "max");
    assert_eq!(v["signal"]["redundant"], true);
    assert_eq!(v["signal"]["sender"]["selected"]["name"], "MX40 Pro");
    assert_eq!(v["signal"]["sender"]["coex_needs_armor"], true);
}

#[test]
fn ledwall_text_has_sections_and_diagrams() {
    let o = avlt(&["calc", "ledwall", "3", "2", "--cabinet", "in-p2.6"]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let out = stdout(&o);
    assert!(!out.contains('\x1b'), "piped output must be plain");
    for needle in [
        "=== LED WALL (3 x 2 = 6 cabinets",
        "--- Resolution ---",
        "--- Physical ---",
        "--- Power ---",
        "--- Signal (NovaStar) ---",
        "--- Weight ---",
        "Signal map (NovaStar ports, vertical)",
        "Power map (daisy-chains, vertical)",
        "\u{250C}",
        " P 1",
        " C 1",
    ] {
        assert!(out.contains(needle), "missing {needle:?} in:\n{out}");
    }
    let o = avlt(&[
        "calc",
        "ledwall",
        "3",
        "2",
        "--cabinet",
        "in-p2.6",
        "--no-diagram",
    ]);
    assert!(!stdout(&o).contains("Signal map"), "{}", stdout(&o));
}

#[test]
fn ledwall_large_wall_summarises_groups() {
    let o = avlt(&["calc", "ledwall", "30", "20", "--cabinet", "in-p2.6"]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert!(stdout(&o).contains("cabinets"), "{}", stdout(&o));
    assert!(!stdout(&o).contains('\u{250C}'), "too large to draw");
}

#[test]
fn ledwall_invalid_input_exits_2() {
    for args in [
        vec!["ledwall"],
        vec!["ledwall", "10"],
        vec!["ledwall", "0", "5", "--cabinet", "in-p2.6"],
        vec!["ledwall", "10", "5", "--cabinet", "nope"],
        vec!["ledwall", "10", "5"],
        vec![
            "ledwall",
            "10",
            "5",
            "--cabinet",
            "in-p2.6",
            "--voltage",
            "-230",
        ],
        vec![
            "ledwall",
            "10",
            "5",
            "--cabinet",
            "in-p2.6",
            "--avg-factor",
            "0",
        ],
    ] {
        let mut full = vec!["calc"];
        full.extend(args.iter());
        let o = avlt(&full);
        assert_eq!(o.status.code(), Some(2), "{args:?}: {}", stderr(&o));
        assert!(!stderr(&o).contains("panicked"), "{args:?}");
    }
}

#[test]
fn bitrate_json_and_text() {
    let v = json(&["bitrate", "1920", "1080", "--fps", "60"]);
    close(&v["raw_mbps"], 2985.984);
    close(&v["ndi_hb_mbps"], 125.0);
    let o = avlt(&["calc", "bitrate", "1920", "1080", "--hours", "2"]);
    let out = stdout(&o);
    assert!(
        out.contains("VIDEO BITRATE (1920 x 1080 @ 25fps, 8-bit, 3ch)"),
        "{out}"
    );
    assert!(out.contains("1,244 Mbps"), "{out}");
    assert!(out.contains("Uncompressed x 2h:"), "{out}");
}

#[test]
fn avoip_projector_pixelpitch_brightness() {
    close(&json(&["avoip", "3840", "2160"])["ndi_hb_mbps"], 500.0);
    let v = json(&[
        "projector",
        "--width",
        "4",
        "--aspect",
        "4:3",
        "--throw-ratio",
        "1.5",
    ]);
    close(&v["throw_distance_m"], 6.0);
    let v = json(&[
        "pixelpitch",
        "--pitch",
        "2.6",
        "--width-m",
        "5.2",
        "--height-m",
        "2.6",
    ]);
    assert_eq!(v["resolution"]["width"], 2000);
    close(&v["min_distance_m"], 8.9388);
    close(
        &json(&["brightness", "500"])["required_nits"],
        143.239_448_782_706,
    );
}

#[test]
fn viewangle_camera_audiobw() {
    close(&json(&["viewangle", "10"])["usable_width_m"], 20.0);
    let v = json(&["camera", "50", "--distance", "10"]);
    close(&v["fov_width_m"], 7.2);
    assert_eq!(v["dori"].as_array().map(Vec::len), Some(4));
    let v = json(&["audiobw", "--freq", "1000"]);
    close(&v["mbps"], 73.728);
    close(&v["wavelength_m"], 0.343);
}

#[test]
fn speaker_cable_dsp() {
    let v = json(&[
        "speaker",
        "--ceiling-height",
        "3",
        "--room-width",
        "10",
        "--room-length",
        "6",
    ]);
    assert_eq!(v["coverage"]["room"]["total"], 12);
    let v = json(&["speaker", "--speakers", "12", "--watts-each", "10"]);
    close(&v["amp"]["required_w"], 477.728_604_664);
    let v = json(&["cable", "--awg", "14", "--length-m", "30"]);
    assert_eq!(v["rating"], "good");
    let v = json(&[
        "cable",
        "--type",
        "coax",
        "--atten-per-100m",
        "20",
        "--length-m",
        "80",
    ]);
    close(&v["attenuation_db"], 16.0);
    let v = json(&[
        "dsp",
        "--latencies-ms",
        "0.25,1.0,2.0",
        "--inputs",
        "16",
        "--outputs",
        "8",
    ]);
    close(&v["latency"]["total_ms"], 3.25);
    assert_eq!(v["matrix"]["crosspoints"], 128);
}

#[test]
fn negative_dbu_is_accepted() {
    let v = json(&["dsp", "--dbu", "-10"]);
    close(&v["level"]["dbu"], -10.0);
}

#[test]
fn invalid_values_exit_2_without_panic() {
    for args in [
        vec!["bitrate", "0", "1080"],
        vec!["bitrate", "1920", "1080", "--fps", "-25"],
        vec!["bitrate", "1920", "1080", "--hours", "0"],
        vec!["avoip", "1920", "0"],
        vec!["projector"],
        vec!["projector", "--width", "4", "--aspect", "16:0"],
        vec!["projector", "--width", "4", "--gain", "0"],
        vec!["pixelpitch"],
        vec!["pixelpitch", "--pitch", "-1"],
        vec!["brightness", "-50"],
        vec!["brightness", "500", "--reflectance", "2"],
        vec!["viewangle", "0"],
        vec!["viewangle", "10", "--max-angle", "90"],
        vec!["camera", "0"],
        vec!["camera", "-50"],
        vec!["audiobw", "--channels", "0"],
        vec!["speaker"],
        vec!["speaker", "--ceiling-height", "1"],
        vec!["cable"],
        vec!["cable", "--awg", "13", "--length-m", "10"],
        vec!["cable", "--type", "coax"],
        vec!["dsp"],
        vec!["dsp", "--latencies-ms", "1,-2"],
    ] {
        let mut full = vec!["calc"];
        full.extend(args.iter());
        let o = avlt(&full);
        assert_eq!(o.status.code(), Some(2), "{args:?}: {}", stderr(&o));
        assert!(
            stderr(&o).starts_with("error: "),
            "{args:?}: {}",
            stderr(&o)
        );
        assert!(stdout(&o).is_empty(), "{args:?}: {}", stdout(&o));
    }
}
