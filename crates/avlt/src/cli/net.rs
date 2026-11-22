//! `avlt net ...`: Network tools (NetTools.ps1, MyIP.ps1, SetIP.ps1, NetMaint).

use std::future::Future;
use std::io::{self, BufRead, IsTerminal, Write};
use std::net::Ipv4Addr;
use std::path::PathBuf;
use std::pin::Pin;
use std::time::{Duration, Instant};

use avlt_core::config::Config;
use avlt_core::net::COMMON_PORTS;
use avlt_core::net::format::{
    LatencyStats, LinkClass, format_bps, format_bytes, format_elapsed, format_link_speed,
    latency_bars, latency_class, link_class, traffic_rate,
};
use avlt_core::net::model::{
    Adapter, IpChange, LatencySample, MyIp, PingHost, PingRound, PortCheck, PortScan, SpeedSample,
    VendorSource,
};
use avlt_core::net::target::{IpTarget, parse_ip_target, parse_scan_target};
use avlt_core::{Error, Result};
use avlt_platform::admin::require_admin;
use avlt_platform::net::{self as pnet, PING_TIMEOUT};
use avlt_platform::process::{self, find_tool};
use clap::{Args, Subcommand};
use serde::Serialize;
use tokio::time::MissedTickBehavior;

use super::output::{Out, exit_if_broken_pipe};

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
    /// Ping several hosts at once, every second
    Pingmulti(PingmultiArgs),
    /// Latency and jitter to a host, every second
    Latency(LatencyArgs),
    /// Live adapter throughput, every second
    Speed(SpeedArgs),
    /// Active TCP connections
    Connections,
    /// Internet speed test (Ookla Speedtest CLI)
    Speedtest,
    /// Flush the DNS cache
    Flushdns,
    /// Restart a network adapter (administrator)
    ResetNic(ResetNicArgs),
    /// Set a static IP, a profile or DHCP (administrator)
    Setip(SetipArgs),
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

/// Sample limit of the live monitors.
#[derive(Debug, Args)]
pub struct CountArg {
    /// Stop after N samples (default: run until Ctrl+C)
    #[arg(short = 'n', long, value_parser = clap::value_parser!(u64).range(1..))]
    pub count: Option<u64>,
}

/// `net pingmulti` arguments.
#[derive(Debug, Args)]
pub struct PingmultiArgs {
    /// Hosts or IPv4 addresses, separated by spaces or commas
    #[arg(required = true, num_args = 1..)]
    pub hosts: Vec<String>,
    #[command(flatten)]
    pub count: CountArg,
}

/// `net latency` arguments.
#[derive(Debug, Args)]
pub struct LatencyArgs {
    /// Host name or IPv4 address
    pub host: String,
    #[command(flatten)]
    pub count: CountArg,
}

/// `net speed` arguments.
#[derive(Debug, Args)]
pub struct SpeedArgs {
    /// Adapter name (default: avlt.toml `iface`, then the adapter of the default route)
    pub iface: Option<String>,
    #[command(flatten)]
    pub count: CountArg,
}

/// `net setip` arguments (SetIP.ps1).
#[derive(Debug, Args)]
pub struct SetipArgs {
    /// Profile name from avlt.toml (default profiles: 0 = 192.168.0.99/24,
    /// 1 = 192.168.1.99/24), dhcp, or IP/prefix such as 192.168.1.25/24
    pub target: String,
    /// Default gateway for IP/prefix
    pub gateway: Option<String>,
    /// Adapter to change (default: avlt.toml `iface`, then the adapter of the default route)
    #[arg(long)]
    pub iface: Option<String>,
    /// Do not ask for confirmation
    #[arg(short = 'y', long)]
    pub yes: bool,
}

/// `net reset-nic` arguments.
#[derive(Debug, Args)]
pub struct ResetNicArgs {
    /// Adapter name (default: avlt.toml `iface`, then the adapter of the default route)
    pub iface: Option<String>,
    /// Do not ask for confirmation
    #[arg(short = 'y', long)]
    pub yes: bool,
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
        NetCmd::Pingmulti(a) => pingmulti(a, out),
        NetCmd::Latency(a) => latency(a, out),
        NetCmd::Speed(a) => speed(a, out),
        NetCmd::Connections => connections(out),
        NetCmd::Speedtest => speedtest(out),
        NetCmd::Flushdns => flushdns(out),
        NetCmd::ResetNic(a) => reset_nic(a, out),
        NetCmd::Setip(a) => setip(a, out),
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

// ---------------------------------------------------------------- live monitors

/// Paces a live monitor: one step per second until Ctrl+C or the sample limit.
struct Ticker {
    interval: tokio::time::Interval,
    stop: Pin<Box<dyn Future<Output = ()> + Send>>,
    remaining: Option<u64>,
}

impl Ticker {
    /// `now`: first step immediately; otherwise after one second (rate monitors).
    /// Must be created inside the runtime.
    fn new(count: Option<u64>, now: bool) -> Ticker {
        let period = Duration::from_secs(1);
        let start = tokio::time::Instant::now() + if now { Duration::ZERO } else { period };
        let mut interval = tokio::time::interval_at(start, period);
        interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
        Ticker {
            interval,
            // If Ctrl+C cannot be hooked the monitor just runs until killed.
            stop: Box::pin(async {
                if tokio::signal::ctrl_c().await.is_err() {
                    std::future::pending::<()>().await;
                }
            }),
            remaining: count,
        }
    }

    /// Waits for the next step; `false` once the monitor must stop.
    async fn next(&mut self) -> bool {
        if self.remaining == Some(0) {
            return false;
        }
        tokio::select! {
            biased;
            () = &mut self.stop => return false,
            _ = self.interval.tick() => {}
        }
        if let Some(n) = &mut self.remaining {
            *n -= 1;
        }
        true
    }
}

/// Splits `a,b c` style host lists, dropping blanks and duplicates (order kept).
fn split_hosts(args: &[String]) -> Vec<String> {
    let mut hosts: Vec<String> = Vec::new();
    for h in args.iter().flat_map(|a| a.split(',')).map(str::trim) {
        if !h.is_empty() && !hosts.iter().any(|x| x == h) {
            hosts.push(h.to_string());
        }
    }
    hosts
}

fn millis(d: Duration) -> u64 {
    u64::try_from(d.as_millis()).unwrap_or(u64::MAX)
}

/// NetTools.ps1 `pingmulti` (lines 208-240) / NetMaint PingPage, refreshed every second.
fn pingmulti(args: PingmultiArgs, out: &Out) -> Result<()> {
    if !cfg!(windows) {
        return Err(Error::Unsupported);
    }
    let targets: Vec<(String, Option<Ipv4Addr>)> = split_hosts(&args.hosts)
        .into_iter()
        .map(|h| {
            let ip = pnet::resolve_ipv4(&h).ok();
            (h, ip)
        })
        .collect();
    for (h, ip) in &targets {
        if ip.is_none() {
            out.warn(&format!("cannot resolve '{h}'"));
        }
    }
    if !out.json {
        out.title(&format!(
            "Monitoring {} hosts - Ctrl+C to stop",
            targets.len()
        ));
    }
    block_on(async {
        let start = Instant::now();
        let mut failures = vec![0u64; targets.len()];
        let mut ticker = Ticker::new(args.count.count, true);
        while ticker.next().await {
            let mut set = tokio::task::JoinSet::new();
            for (i, (_, ip)) in targets.iter().enumerate() {
                let ip = *ip;
                set.spawn(async move {
                    let rtt = match ip {
                        Some(ip) => pnet::ping(ip, PING_TIMEOUT).await,
                        None => None,
                    };
                    (i, rtt)
                });
            }
            let mut rtts = vec![None; targets.len()];
            while let Some(Ok((i, rtt))) = set.join_next().await {
                rtts[i] = rtt;
            }
            let hosts = targets
                .iter()
                .zip(rtts)
                .zip(failures.iter_mut())
                .map(|(((host, ip), rtt), fails)| {
                    if rtt.is_none() {
                        *fails += 1;
                    }
                    PingHost {
                        host: host.clone(),
                        ip: *ip,
                        online: rtt.is_some(),
                        latency_ms: rtt.map(millis),
                        failures: *fails,
                    }
                })
                .collect();
            let round = PingRound {
                elapsed_s: start.elapsed().as_secs_f64(),
                hosts,
            };
            if out.json {
                json_line(&round);
            } else {
                render_ping_round(&round, start.elapsed(), out);
            }
        }
    })?;
    Ok(())
}

fn render_ping_round(round: &PingRound, elapsed: Duration, out: &Out) {
    line(out, "");
    line(
        out,
        &paint(
            out,
            Colour::Dim,
            &format!("  Multi-ping monitor - {}", format_elapsed(elapsed)),
        ),
    );
    line(
        out,
        &paint(
            out,
            Colour::Dim,
            &format!(
                "  {:<20} {:<10} {:<10} {}",
                "HOST", "STATUS", "LATENCY", "FAILURES"
            ),
        ),
    );
    for h in &round.hosts {
        let (status, latency, colour) = match (h.ip, h.latency_ms) {
            (None, _) => ("ERROR", "-".to_string(), Colour::Yellow),
            (Some(_), Some(ms)) => ("ONLINE", format!("{ms} ms"), Colour::Green),
            (Some(_), None) => ("OFFLINE", "-".to_string(), Colour::Red),
        };
        let row = format!(
            "  {:<20} {:<10} {:<10} {}",
            h.host, status, latency, h.failures
        );
        line(out, &paint(out, colour, &row));
    }
}

/// NetTools.ps1 `latency` (lines 242-266) plus loss / jitter statistics.
fn latency(args: LatencyArgs, out: &Out) -> Result<()> {
    if !cfg!(windows) {
        return Err(Error::Unsupported);
    }
    let ip = pnet::resolve_ipv4(&args.host)?;
    if !out.json {
        out.title(&format!(
            "Latency monitor to {} ({ip}) - Ctrl+C to stop",
            args.host
        ));
        line(
            out,
            &paint(out, Colour::Dim, "  Time       Latency    Graph"),
        );
        line(
            out,
            &paint(
                out,
                Colour::Dim,
                "  ---------  ---------  ----------------------------------------",
            ),
        );
    }
    let stats = block_on(async {
        let start = Instant::now();
        let mut stats = LatencyStats::default();
        let mut ticker = Ticker::new(args.count.count, true);
        while ticker.next().await {
            let elapsed = start.elapsed();
            let rtt = pnet::ping(ip, PING_TIMEOUT).await.map(millis);
            stats.add(rtt);
            if out.json {
                json_line(&LatencySample {
                    elapsed_s: elapsed.as_secs_f64(),
                    host: args.host.clone(),
                    ip,
                    latency_ms: rtt,
                    stats: stats.summary(),
                });
                continue;
            }
            let t = format_elapsed(elapsed);
            let row = match rtt {
                Some(ms) => paint(
                    out,
                    class_colour(latency_class(ms)),
                    &format!("  {t:<9}  {ms:>5} ms   {}", "#".repeat(latency_bars(ms))),
                ),
                None => paint(out, Colour::Red, &format!("  {t:<9}  TIMEOUT    -")),
            };
            line(out, &row);
        }
        stats
    })?;
    if !out.json {
        let s = stats.summary();
        let float = |v: Option<f64>| v.map_or_else(|| "-".to_string(), |v| format!("{v:.1}"));
        let int = |v: Option<u64>| v.map_or_else(|| "-".to_string(), |v| v.to_string());
        line(out, "");
        out.field(
            "Packets",
            &format!(
                "{} sent, {} received, {:.0}% loss",
                s.sent, s.received, s.loss_pct
            ),
        );
        out.field(
            "Round trip",
            &format!(
                "min {} / avg {} / max {} ms, jitter {} ms",
                int(s.min_ms),
                float(s.avg_ms),
                int(s.max_ms),
                float(s.jitter_ms)
            ),
        );
    }
    Ok(())
}

/// NetTools.ps1 `speed` (lines 136-177) / NetMaint TrafficPage.
fn speed(args: SpeedArgs, out: &Out) -> Result<()> {
    let adapter = resolve_adapter(args.iface.as_deref())?;
    let base = pnet::traffic(&adapter)?;
    if !out.json {
        out.title(&format!(
            "Bandwidth monitor on '{}' - Ctrl+C to stop",
            adapter.name
        ));
        line(
            out,
            &paint(
                out,
                Colour::Dim,
                "  Time       Download       Upload         Total down     Total up",
            ),
        );
        line(
            out,
            &paint(
                out,
                Colour::Dim,
                "  ---------  -------------  -------------  -------------  -------------",
            ),
        );
    }
    block_on(async {
        let start = Instant::now();
        let (mut prev, mut last) = (base, start);
        let mut ticker = Ticker::new(args.count.count, false);
        while ticker.next().await {
            let cur = pnet::traffic(&adapter)?;
            let now = Instant::now();
            let (down_bps, up_bps) = traffic_rate(&prev, &cur, (now - last).as_secs_f64());
            let sample = SpeedSample {
                elapsed_s: (now - start).as_secs_f64(),
                adapter: adapter.name.clone(),
                down_bps,
                up_bps,
                total_down_bytes: cur.rx_bytes.saturating_sub(base.rx_bytes),
                total_up_bytes: cur.tx_bytes.saturating_sub(base.tx_bytes),
            };
            (prev, last) = (cur, now);
            if out.json {
                json_line(&sample);
            } else {
                line(
                    out,
                    &format!(
                        "  {:<9}  {}  {}  {}  {}",
                        format_elapsed(now - start),
                        format_bps(sample.down_bps / 8.0),
                        format_bps(sample.up_bps / 8.0),
                        format_bytes(sample.total_down_bytes),
                        format_bytes(sample.total_up_bytes)
                    ),
                );
            }
        }
        Ok(())
    })?
}

// ---------------------------------------------------------------- speedtest

/// NetTools.ps1 `speedtest` (lines 196-206): runs the Ookla CLI.
fn speedtest(out: &Out) -> Result<()> {
    let exe = find_tool("speedtest", "Ookla.Speedtest.CLI")?;
    let accept = ["--accept-license", "--accept-gdpr"];
    if !out.json {
        out.title("Speed test (Ookla)... approx 30 seconds");
        let status = process::run(&exe, &accept)?;
        return if status.success() {
            Ok(())
        } else {
            Err(Error::Other(format!("speedtest exited with {status}")))
        };
    }
    let text = process::capture(&exe, &[accept[0], accept[1], "--format=json"])?;
    let last = text
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("");
    let value: serde_json::Value = serde_json::from_str(last)
        .map_err(|e| Error::Other(format!("speedtest returned invalid JSON: {e}")))?;
    out.emit(&value, |_, _| {});
    Ok(())
}

// ---------------------------------------------------------------- system changes

/// Asks `question` on stderr unless `yes`; refuses without a terminal to ask on.
fn confirm(yes: bool, question: &str) -> Result<()> {
    let stdin = io::stdin();
    let interactive = stdin.is_terminal();
    confirm_with(yes, interactive, stdin.lock(), question)
}

fn confirm_with(yes: bool, interactive: bool, reader: impl BufRead, question: &str) -> Result<()> {
    if yes {
        return Ok(());
    }
    if !interactive {
        return Err(Error::InvalidInput(format!(
            "{question} needs confirmation: run it in a terminal or pass --yes"
        )));
    }
    let mut err = io::stderr().lock();
    let _ = write!(err, "{question} [y/N] ");
    let _ = err.flush();
    if answered_yes(reader) {
        Ok(())
    } else {
        Err(Error::Other("cancelled".into()))
    }
}

fn answered_yes(mut reader: impl BufRead) -> bool {
    let mut answer = String::new();
    reader.read_line(&mut answer).is_ok()
        && matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes")
}

/// SetIP.ps1 (whole script) / NetMaint IpConfigPage: profile, DHCP or custom address.
///
/// Order: validate the target, require administrator, resolve the adapter, confirm, apply.
fn setip(args: SetipArgs, out: &Out) -> Result<()> {
    let cfg = config()?;
    let target = parse_ip_target(&args.target, args.gateway.as_deref(), &cfg.ip_profiles)?;
    require_admin()?;
    let adapter = pnet::default_adapter(args.iface.as_deref().or(cfg.iface.as_deref()))?;
    let name = adapter.name;
    confirm(args.yes, &format!("Set {target} on '{name}'?"))?;
    match target {
        IpTarget::Dhcp => pnet::set_dhcp(&name)?,
        IpTarget::Static {
            address,
            prefix,
            gateway,
        } => {
            let gateway = gateway.map(|g| g.to_string());
            pnet::set_static(&name, &format!("{address}/{prefix}"), gateway.as_deref())?;
        }
    }
    out.ok(&format!("OK -> {target}"));
    // As SetIP.ps1: wait up to 8 s for a DHCP lease, 800 ms after a static change.
    let after = if target == IpTarget::Dhcp {
        let mut after = pnet::find_adapter(&name)?;
        for _ in 0..8 {
            if after
                .primary_ipv4()
                .is_some_and(|c| !c.address.is_link_local())
            {
                break;
            }
            std::thread::sleep(Duration::from_secs(1));
            after = pnet::find_adapter(&name)?;
        }
        after
    } else {
        std::thread::sleep(Duration::from_millis(800));
        pnet::find_adapter(&name)?
    };
    let result = IpChange {
        adapter: name,
        applied: target,
        ipv4: after.ipv4,
        gateways: after.gateways,
        dhcp: after.dhcp,
    };
    out.emit(&result, |r, out| {
        if r.ipv4.is_empty() {
            line(
                out,
                &paint(
                    out,
                    Colour::Dim,
                    "  (No IP yet, wait and run: avlt net info)",
                ),
            );
        }
        for c in &r.ipv4 {
            out.field("IP", &format!("{}/{}", c.address, c.prefix));
        }
        if !r.gateways.is_empty() {
            out.field("Gateway", &join(&r.gateways));
        }
    });
    Ok(())
}

/// NetTools.ps1 `reset-nic` (lines 94-107): disable, wait 2 s, enable.
fn reset_nic(args: ResetNicArgs, out: &Out) -> Result<()> {
    require_admin()?;
    let adapter = resolve_adapter(args.iface.as_deref())?;
    confirm(
        args.yes,
        &format!(
            "Restart adapter '{}'? Its connections drop for a few seconds.",
            adapter.name
        ),
    )?;
    if !out.json {
        out.title(&format!("Restarting adapter '{}'...", adapter.name));
    }
    pnet::restart_adapter(&adapter.name)?;
    out.emit(
        &serde_json::json!({ "adapter": adapter.name, "restarted": true }),
        |_, out| out.ok("OK -> adapter restarted"),
    );
    Ok(())
}

/// NetTools.ps1 `flushdns` (lines 87-92) / NetMaint `Native.FlushDns`.
fn flushdns(out: &Out) -> Result<()> {
    pnet::flush_dns()?;
    out.emit(&serde_json::json!({ "flushed": true }), |_, out| {
        out.ok("OK -> DNS cache cleared")
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confirmation_answers() {
        for (input, expected) in [
            ("y\n", true),
            ("YES\r\n", true),
            (" y ", true),
            ("n\n", false),
            ("\n", false),
            ("", false),
            ("yep\n", false),
        ] {
            assert_eq!(answered_yes(input.as_bytes()), expected, "{input:?}");
        }
    }

    #[test]
    fn confirmation_paths() {
        assert!(confirm_with(true, false, "".as_bytes(), "Q?").is_ok());
        let err = confirm_with(false, false, "y\n".as_bytes(), "Q?").unwrap_err();
        assert!(matches!(err, Error::InvalidInput(_)), "{err:?}");
        assert!(err.to_string().contains("--yes"), "{err}");
        assert!(confirm_with(false, true, "y\n".as_bytes(), "Q?").is_ok());
        let err = confirm_with(false, true, "n\n".as_bytes(), "Q?").unwrap_err();
        assert_eq!(err.to_string(), "cancelled");
    }

    #[test]
    fn host_lists_split_and_dedupe() {
        let args: Vec<String> = ["192.168.1.1,192.168.1.10", " 8.8.8.8 ", "192.168.1.1,,"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(
            split_hosts(&args),
            ["192.168.1.1", "192.168.1.10", "8.8.8.8"]
        );
    }
}
