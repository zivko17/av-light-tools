//! `avlt ndi ...`: NDI tools. Port of `NDITools.ps1` (`Invoke-NDILaunch`,
//! `Test-NDINetwork`, `Get-NDIServices`, `Get-NDIConfig`) and `AVToolbox/NdiTools.cs`.
//! The interactive `ndi` menu is not ported; the GUI covers it.

use std::path::PathBuf;

use avlt_core::Result;
use avlt_platform::ndi::{self, LinkGrade, NdiApp, NdiCheck, NdiConfig, NdiServices};
use clap::builder::{PossibleValue, PossibleValuesParser};
use clap::{Args, Subcommand};
use serde::Serialize;

use super::line;
use super::output::Out;

/// `avlt ndi` subcommands.
#[derive(Debug, Subcommand)]
pub enum NdiCmd {
    /// Launch an NDI Tools app
    #[command(
        after_help = r"Uses the newest 'NDI N Tools' folder under Program Files\NDI. Set AVLT_NDI_ROOT to look in another folder."
    )]
    Launch(LaunchArgs),
    /// Diagnose the network for NDI (link speed, mDNS, firewall, ports, multicast)
    Check,
    /// List NDI services and running NDI processes
    Services,
    /// Show the NDI Access Manager configuration (groups, discovery servers, IPs)
    Config,
}

/// `ndi launch` arguments.
#[derive(Debug, Args)]
pub struct LaunchArgs {
    /// App to start
    #[arg(value_parser = app_values())]
    pub app: String,
}

/// The app aliases with their descriptions, for `--help` and validation.
fn app_values() -> PossibleValuesParser {
    PossibleValuesParser::new(
        NdiApp::ALL.map(|a| PossibleValue::new(a.name()).help(a.description())),
    )
}

/// Runs an `avlt ndi` subcommand.
pub fn run(cmd: NdiCmd, out: &Out) -> Result<()> {
    match cmd {
        NdiCmd::Launch(a) => launch(&a, out),
        NdiCmd::Check => check(out),
        NdiCmd::Services => services(out),
        NdiCmd::Config => config(out),
    }
}

/// Result of `ndi launch`.
#[derive(Debug, Serialize)]
struct Launched {
    app: String,
    path: PathBuf,
}

fn launch(a: &LaunchArgs, out: &Out) -> Result<()> {
    let app: NdiApp = a.app.parse()?;
    let path = ndi::launch(app)?;
    out.emit(
        &Launched {
            app: app.name().to_string(),
            path,
        },
        |l, out| out.ok(&format!("Launched: {} ({})", l.app, l.path.display())),
    );
    Ok(())
}

fn yes_no(v: Option<bool>) -> &'static str {
    match v {
        Some(true) => "YES",
        Some(false) => "NO",
        None => "unknown",
    }
}

fn check(out: &Out) -> Result<()> {
    let c = ndi::check()?;
    out.emit(&c, render_check);
    Ok(())
}

fn render_check(c: &NdiCheck, out: &Out) {
    out.title("=== NDI NETWORK DIAGNOSTIC ===");
    line(out, "");
    if c.install.installed {
        let version = c
            .install
            .version
            .map_or_else(String::new, |v| format!(" (NDI {v})"));
        out.field(
            "NDI Tools",
            &format!("{}{version}", c.install.base.display()),
        );
    } else {
        out.warn(&format!(
            "NDI Tools not found (looked for {})",
            c.install.base.display()
        ));
    }
    line(out, "");

    out.title("--- Network adapter ---");
    if c.adapters.is_empty() {
        out.field("", "no active adapter");
    }
    for a in &c.adapters {
        let speed = a.link_mbps.map_or_else(
            || "unknown".to_string(),
            |m| {
                if m >= 1000 && m % 1000 == 0 {
                    format!("{} Gbps", m / 1000)
                } else if m >= 1000 {
                    format!("{} Gbps", m as f64 / 1000.0)
                } else {
                    format!("{m} Mbps")
                }
            },
        );
        line(
            out,
            &format!(
                "    {:<25} {:<12} {}",
                a.name,
                speed,
                a.mac.as_deref().unwrap_or("")
            ),
        );
        if a.grade == LinkGrade::Poor {
            out.warn(&format!(
                "{}: {speed} is NOT enough for NDI Full Bandwidth (need 1 Gbps+)",
                a.name
            ));
        }
    }
    line(out, "");

    out.title("--- mDNS service (port 5353) ---");
    out.field(
        "DNS Client service",
        c.dns_client.as_deref().unwrap_or("not found"),
    );
    out.field("UDP 5353 listening", yes_no(c.mdns_listening));
    if c.mdns_listening == Some(false) {
        out.warn("nothing listens on UDP 5353: mDNS discovery may not work");
    }
    line(out, "");

    out.title("--- Firewall ---");
    match c.firewall {
        Some(f) if f.allow + f.block > 0 => {
            out.field(
                "NDI firewall rules",
                &format!("{} allow / {} block", f.allow, f.block),
            );
            if f.block > 0 {
                out.warn(&format!(
                    "{} blocking NDI rules detected, check Windows Firewall",
                    f.block
                ));
            }
        }
        Some(_) => out.field("NDI firewall rules", "none found (NDI may be blocked)"),
        None => out.field("NDI firewall rules", "unknown"),
    }
    line(out, "");

    out.title("--- NDI listening ports (5960-5990) ---");
    match &c.ports {
        Some(p) if !p.is_empty() => {
            out.field("Found", &format!("{} NDI port(s) listening", p.len()));
            for port in p {
                line(
                    out,
                    &format!("      Port {:>5}  <-  {}", port.port, port.process),
                );
            }
        }
        Some(_) => out.field("Ports", "none listening (no NDI app is sending right now)"),
        None => out.field("Ports", "unknown"),
    }
    line(out, "");

    out.title("--- Multicast ---");
    match c.multicast_routes {
        Some(0) => {
            out.field("Multicast routes", "NONE (multicast disabled)");
            out.warn("no route to 224.0.0.0/4: multicast is disabled");
        }
        Some(n) => out.field("Multicast routes", &format!("{n} configured")),
        None => out.field("Multicast routes", "unknown"),
    }
    for note in &c.notes {
        out.warn(note);
    }
}

fn services(out: &Out) -> Result<()> {
    let s = ndi::services()?;
    out.emit(&s, render_services);
    Ok(())
}

fn render_services(s: &NdiServices, out: &Out) {
    out.title("=== NDI SERVICES & PROCESSES ===");
    line(out, "");
    out.title("--- Services ---");
    if s.services.is_empty() {
        line(out, "    No NDI services registered.");
    }
    for svc in &s.services {
        line(out, &format!("    {:<40} {}", svc.display_name, svc.status));
    }
    line(out, "");
    out.title("--- Running NDI processes ---");
    if s.processes.is_empty() {
        line(out, "    No NDI processes running.");
    } else {
        line(
            out,
            &format!("    {:<40} {:<10} {}", "PROCESS", "PID", "MEMORY (MB)"),
        );
        for p in &s.processes {
            line(
                out,
                &format!("    {:<40} {:<10} {:.1}", p.name, p.pid, p.memory_mb),
            );
        }
    }
}

fn config(out: &Out) -> Result<()> {
    let c = ndi::config()?;
    out.emit(&c, render_config);
    Ok(())
}

fn list_section(out: &Out, title: &str, items: &[String], empty: &str) {
    out.title(title);
    if items.is_empty() {
        line(out, &format!("    {empty}"));
    }
    for i in items {
        line(out, &format!("    {i}"));
    }
    line(out, "");
}

fn render_config(c: &NdiConfig, out: &Out) {
    out.title("=== NDI ACCESS MANAGER CONFIG ===");
    line(out, "");
    if !c.found {
        out.warn(&format!("no config file found at {}", c.path.display()));
        line(
            out,
            "  Open Access Manager once to generate it: avlt ndi launch manager",
        );
        return;
    }
    out.field("File", &c.path.display().to_string());
    line(out, "");
    list_section(
        out,
        "--- Groups (sending) ---",
        &c.groups_send,
        "(default: public)",
    );
    list_section(
        out,
        "--- Groups (receiving) ---",
        &c.groups_recv,
        "(default: public)",
    );
    list_section(
        out,
        "--- Discovery Servers ---",
        &c.discovery,
        "(none configured - using mDNS)",
    );
    list_section(
        out,
        "--- External IPs (manual sources) ---",
        &c.ips,
        "(none)",
    );
}
