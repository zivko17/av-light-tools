//! `avlt net ...`: Network tools (NetTools.ps1, MyIP.ps1, SetIP.ps1, NetMaint).

use std::future::Future;
use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;
use std::time::Duration;

use avlt_core::config::Config;
use avlt_core::net::COMMON_PORTS;
use avlt_core::net::format::{LinkClass, format_link_speed, link_class};
use avlt_core::net::model::{Adapter, MyIp, PortCheck, PortScan, VendorSource};
use avlt_core::net::target::parse_scan_target;
use avlt_core::{Error, Result};
use avlt_platform::net as pnet;
use clap::{Args, Subcommand};
use serde::Serialize;

use super::output::{Out, exit_if_broken_pipe};
use super::{Pending, not_implemented};

/// `avlt net` subcommands.
#[derive(Debug, Subcommand)]
pub enum NetCmd {
    /// Adapters, addresses, gateway and DNS
    Info(IfaceArg),
    /// Local IPs of every active adapter and the public IP
    Myip,
    /// Scan the local subnet for live hosts
    Scan(ScanArgs),
    /// Check whether a TCP port is open
    Portcheck(PortcheckArgs),
    /// Scan the common TCP ports of a host
    Portscan(HostArg),
    /// Identify a device by MAC vendor
    Whoison(WhoisonArgs),
    /// Ping several hosts at once
    Pingmulti(Pending),
    /// Latency and jitter to a host
    Latency(Pending),
    /// Live adapter throughput
    Speed(Pending),
    /// Active TCP connections
    Connections,
    /// Internet speed test
    Speedtest(Pending),
    /// Flush the DNS cache
    Flushdns(Pending),
    /// Restart a network adapter
    ResetNic(Pending),
    /// Set a static IP, a profile or DHCP
    Setip(Pending),
}

/// Optional adapter name.
#[derive(Debug, Args)]
pub struct IfaceArg {
    /// Adapter name, e.g. "Ethernet 2" (default: avlt.toml `iface`, then the adapter
    /// of the default route)
    pub iface: Option<String>,
}

/// `net scan` arguments.
#[derive(Debug, Args)]
pub struct ScanArgs {
    /// Subnet: 192.168.1, 192.168.1.0/24 (/20 to /32) or an address (its /24).
    /// Default: the /24 of the default adapter
    pub subnet: Option<String>,
}

/// `net portcheck` arguments.
#[derive(Debug, Args)]
pub struct PortcheckArgs {
    /// Host name or IPv4 address
    pub host: String,
    /// TCP port (1-65535)
    #[arg(value_parser = clap::value_parser!(u16).range(1..))]
    pub port: u16,
}

/// A single host.
#[derive(Debug, Args)]
pub struct HostArg {
    /// Host name or IPv4 address
    pub host: String,
}

/// `net whoison` arguments.
#[derive(Debug, Args)]
pub struct WhoisonArgs {
    /// Host name or IPv4 address on the local subnet
    pub host: String,
    /// Do not ask api.macvendors.com when the embedded OUI table has no match
    #[arg(long)]
    pub offline: bool,
}

/// Runs an `avlt net` subcommand.
pub fn run(cmd: NetCmd, out: &Out) -> Result<()> {
    match cmd {
        NetCmd::Info(a) => info(a, out),
        NetCmd::Myip => myip(out),
        NetCmd::Scan(a) => scan(a, out),
        NetCmd::Portcheck(a) => portcheck(a, out),
        NetCmd::Portscan(a) => portscan(a, out),
        NetCmd::Whoison(a) => whoison(a, out),
        NetCmd::Pingmulti(_) => not_implemented(),
        NetCmd::Latency(_) => not_implemented(),
        NetCmd::Speed(_) => not_implemented(),
        NetCmd::Connections => connections(out),
        NetCmd::Speedtest(_) => not_implemented(),
        NetCmd::Flushdns(_) => not_implemented(),
        NetCmd::ResetNic(_) => not_implemented(),
        NetCmd::Setip(_) => not_implemented(),
    }
}

// ---------------------------------------------------------------- helpers

/// `avlt.toml` next to the executable (defaults when absent).
fn config() -> Result<Config> {
    let dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."));
    Config::load(&dir)
}

/// The adapter to act on: explicit name, then avlt.toml `iface`, then the default route's.
fn resolve_adapter(explicit: Option<&str>) -> Result<Adapter> {
    let cfg = config()?;
    pnet::default_adapter(explicit.or(cfg.iface.as_deref()))
}

/// Runs `fut` on a fresh multi-threaded Tokio runtime. Blocking lookups still running
/// at the end (slow reverse DNS) are abandoned instead of delaying exit.
fn block_on<F: Future>(fut: F) -> Result<F::Output> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let value = rt.block_on(fut);
    rt.shutdown_background();
    Ok(value)
}

/// A plain stdout line (nothing with `--json`); closed pipes end the process quietly.
fn line(out: &Out, s: &str) {
    if !out.json {
        raw_line(s);
    }
}

fn raw_line(s: &str) {
    if let Err(e) = writeln!(io::stdout().lock(), "{s}") {
        exit_if_broken_pipe(&e);
    }
}

/// One compact JSON document on its own line (live monitors with `--json`).
#[allow(dead_code)]
fn json_line<T: Serialize>(value: &T) {
    match serde_json::to_string(value) {
        Ok(s) => raw_line(&s),
        Err(e) => eprintln!("error: cannot serialize result: {e}"),
    }
}

#[derive(Clone, Copy)]
enum Colour {
    Green,
    Yellow,
    Red,
    Dim,
}

/// `s` in `colour` when stdout takes colour.
fn paint(out: &Out, colour: Colour, s: &str) -> String {
    if !out.color {
        return s.to_string();
    }
    let code = match colour {
        Colour::Green => "32",
        Colour::Yellow => "33",
        Colour::Red => "31",
        Colour::Dim => "2",
    };
    format!("\x1b[{code}m{s}\x1b[0m")
}

fn class_colour(class: LinkClass) -> Colour {
    match class {
        LinkClass::Fast => Colour::Green,
        LinkClass::Medium => Colour::Yellow,
        LinkClass::Slow => Colour::Red,
    }
}

fn join<T: ToString>(items: &[T]) -> String {
    items
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

// ---------------------------------------------------------------- info / myip

/// NetTools.ps1 `netinfo` (lines 24-42).
fn info(args: IfaceArg, out: &Out) -> Result<()> {
    let adapter = match resolve_adapter(args.iface.as_deref()) {
        Ok(a) => a,
        // No adapter at all is a state of the PC, not a failure of avlt: warn, exit 1.
        Err(Error::Other(msg)) => {
            out.warn(&msg);
            std::process::exit(1);
        }
        Err(e) => return Err(e),
    };
    out.emit(&adapter, |a, out| {
        out.title(&format!("=== {} ===", a.name));
        if a.ipv4.is_empty() {
            out.field("IP", &paint(out, Colour::Yellow, "(no address)"));
        }
        for c in &a.ipv4 {
            out.field("IP", &format!("{}/{}", c.address, c.prefix));
        }
        out.field(
            "Gateway",
            &if a.gateways.is_empty() {
                "(none)".to_string()
            } else {
                join(&a.gateways)
            },
        );
        out.field(
            "DNS",
            &if a.dns.is_empty() {
                "(automatic)".to_string()
            } else {
                join(&a.dns)
            },
        );
        let mode = if a.dhcp {
            paint(out, Colour::Green, "DHCP (automatic)")
        } else {
            paint(out, Colour::Yellow, "Manual (static)")
        };
        out.field("Mode", &mode);
        adapter_details(a, out);
    });
    Ok(())
}

fn adapter_details(a: &Adapter, out: &Out) {
    if let Some(mac) = &a.mac {
        out.field("MAC", mac);
    }
    if let Some(bps) = a.link_speed_bps {
        out.field(
            "Link speed",
            &paint(out, class_colour(link_class(bps)), &format_link_speed(bps)),
        );
    }
    if !a.up {
        out.field("Status", &paint(out, Colour::Red, "down"));
    }
}

/// MyIP.ps1 (lines 1-28): every up adapter with IPv4, then the public IP.
fn myip(out: &Out) -> Result<()> {
    let adapters = pnet::active_adapters()?;
    let public_ip = pnet::public_ip(Duration::from_secs(3));
    if public_ip.is_none() {
        out.warn("public IP could not be retrieved (no internet?)");
    }
    let result = MyIp {
        adapters,
        public_ip,
    };
    out.emit(&result, |r, out| {
        out.title("=== NETWORK INFO ===");
        if r.adapters.is_empty() {
            line(
                out,
                &paint(
                    out,
                    Colour::Yellow,
                    "  No active adapter with an IPv4 address.",
                ),
            );
        }
        for a in &r.adapters {
            line(out, "");
            line(out, &format!("  [{}]", a.name));
            for c in &a.ipv4 {
                out.field(
                    "Local IP",
                    &paint(out, Colour::Green, &format!("{}/{}", c.address, c.prefix)),
                );
            }
            if !a.gateways.is_empty() {
                out.field("Gateway", &join(&a.gateways));
            }
            out.field(
                "DNS",
                &if a.dns.is_empty() {
                    "(none)".to_string()
                } else {
                    join(&a.dns)
                },
            );
            adapter_details(a, out);
        }
        line(out, "");
        line(out, "  [Public IP]");
        match &r.public_ip {
            Some(ip) => out.field("Public IP", &paint(out, Colour::Green, ip)),
            None => out.field("Public IP", &paint(out, Colour::Red, "(unavailable)")),
        }
    });
    Ok(())
}

// ---------------------------------------------------------------- scan / whoison

/// NetTools.ps1 `scan` (lines 44-77) and NetMaint ScanPage / NetHelpers.PingSweepAsync.
fn scan(args: ScanArgs, out: &Out) -> Result<()> {
    let (network, prefix) = match args.subnet.as_deref() {
        Some(s) => parse_scan_target(s)?,
        None => {
            let adapter = resolve_adapter(None).map_err(|e| match e {
                Error::Other(msg) => Error::Other(format!(
                    "{msg}; specify a subnet, e.g. avlt net scan 192.168.1"
                )),
                e => e,
            })?;
            let ip = adapter.primary_ipv4().ok_or_else(|| {
                Error::Other(format!(
                    "no IPv4 address on '{}'; specify a subnet, e.g. avlt net scan 192.168.1",
                    adapter.name
                ))
            })?;
            parse_scan_target(&ip.address.to_string())?
        }
    };
    let show_progress = !out.json && io::stderr().is_terminal();
    if !out.json {
        out.title(&format!("Scanning {network}/{prefix} (parallel) ..."));
    }
    let result = block_on(pnet::scan(network, prefix, |done, total| {
        if show_progress {
            let _ = write!(io::stderr(), "\r  {done} / {total}");
        }
    }))??;
    if show_progress {
        let _ = write!(io::stderr(), "\r{:30}\r", "");
    }
    out.emit(&result, |r, out| {
        if r.hosts.is_empty() {
            out.warn("no hosts responded");
            return;
        }
        out.ok(&format!("Found {} hosts:", r.hosts.len()));
        line(out, "");
        line(
            out,
            &paint(
                out,
                Colour::Dim,
                &format!(
                    "  {:<16} {:<18} {:<20} {}",
                    "IP", "MAC", "VENDOR", "HOSTNAME"
                ),
            ),
        );
        for h in &r.hosts {
            let mut name = h.hostname.clone().unwrap_or_default();
            if h.this_pc {
                name.push_str("  (this PC)");
            }
            line(
                out,
                &format!(
                    "  {} {:<18} {:<20} {}",
                    paint(out, Colour::Green, &format!("{:<16}", h.ip)),
                    h.mac.as_deref().unwrap_or("-"),
                    h.vendor.as_deref().unwrap_or("-"),
                    name
                ),
            );
        }
    });
    Ok(())
}

/// NetTools.ps1 `whoison` (lines 109-134); embedded OUI table first, API second.
fn whoison(args: WhoisonArgs, out: &Out) -> Result<()> {
    let ip = pnet::resolve_ipv4(&args.host)?;
    let result = block_on(pnet::whoison(ip, !args.offline))??;
    if result.mac.is_none() {
        out.warn("no ARP response (host offline or outside the local subnet)");
    }
    out.emit(&result, |r, out| {
        out.field("IP", &r.ip.to_string());
        out.field("MAC", r.mac.as_deref().unwrap_or("-"));
        if r.mac.is_some() {
            let vendor = match (&r.vendor, r.vendor_source) {
                (Some(v), Some(VendorSource::Api)) => format!("{v}  (macvendors.com)"),
                (Some(v), _) => v.clone(),
                (None, _) => paint(out, Colour::Dim, "(unknown)"),
            };
            out.field("Vendor", &vendor);
        }
    });
    Ok(())
}

// ---------------------------------------------------------------- ports

/// NetTools.ps1 `portcheck` (lines 79-85), as a 1000 ms TCP connect instead of
/// Test-NetConnection.
fn portcheck(args: PortcheckArgs, out: &Out) -> Result<()> {
    let ip = pnet::resolve_ipv4(&args.host)?;
    let open = block_on(pnet::tcp_open(ip, args.port, pnet::TCP_TIMEOUT))?;
    let result = PortCheck {
        host: args.host,
        ip,
        port: args.port,
        open,
    };
    out.emit(&result, |r, out| {
        if r.open {
            out.ok(&format!("  OPEN    ({} {} responds)", r.ip, r.port));
        } else {
            line(
                out,
                &paint(
                    out,
                    Colour::Yellow,
                    &format!("  CLOSED / no response  ({} {})", r.ip, r.port),
                ),
            );
        }
    });
    Ok(())
}

/// NetTools.ps1 `portscan` (lines 268-302) / NetMaint PortsPage: the 14 common ports.
fn portscan(args: HostArg, out: &Out) -> Result<()> {
    let ip = pnet::resolve_ipv4(&args.host)?;
    let ports = block_on(pnet::port_scan(ip, COMMON_PORTS, pnet::TCP_TIMEOUT))?;
    let result = PortScan {
        host: args.host,
        ip,
        ports,
    };
    out.emit(&result, |r, out| {
        out.title(&format!("Common ports on {}", r.ip));
        line(
            out,
            &paint(
                out,
                Colour::Dim,
                &format!("  {:<8} {:<12} {}", "PORT", "STATUS", "SERVICE"),
            ),
        );
        for p in &r.ports {
            let row = format!(
                "  {:<8} {:<12} {}",
                p.port,
                if p.open { "OPEN" } else { "closed" },
                p.service
            );
            line(
                out,
                &paint(out, if p.open { Colour::Green } else { Colour::Dim }, &row),
            );
        }
    });
    Ok(())
}

// ---------------------------------------------------------------- connections

/// NetTools.ps1 `connections` (lines 179-194) / NetMaint TrafficPage.
fn connections(out: &Out) -> Result<()> {
    let list = pnet::connections()?;
    if list.is_empty() {
        out.warn("no established connections");
    }
    out.emit(&list, |list, out| {
        out.title("Active TCP connections");
        line(
            out,
            &paint(
                out,
                Colour::Dim,
                &format!("  {:<24} {:<28} {}", "PROCESS", "LOCAL", "REMOTE"),
            ),
        );
        for c in list {
            line(
                out,
                &format!(
                    "  {:<24} {:<28} {}",
                    c.process,
                    endpoint(c.local_addr, c.local_port),
                    endpoint(c.remote_addr, c.remote_port)
                ),
            );
        }
    });
    Ok(())
}

fn endpoint(addr: std::net::IpAddr, port: u16) -> String {
    std::net::SocketAddr::new(addr, port).to_string()
}
