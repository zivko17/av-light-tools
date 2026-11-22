//! `avlt net ...`: Network tools.

use avlt_core::Result;
use clap::Subcommand;

use super::output::Out;
use super::{Pending, not_implemented};

/// `avlt net` subcommands.
#[derive(Debug, Subcommand)]
pub enum NetCmd {
    /// Adapters, addresses, gateway and DNS
    Info(Pending),
    /// Public IP address
    Myip(Pending),
    /// Scan the local subnet for live hosts
    Scan(Pending),
    /// Check whether a TCP port is open
    Portcheck(Pending),
    /// Scan a range of TCP ports
    Portscan(Pending),
    /// Identify a device by MAC vendor
    Whoison(Pending),
    /// Ping several hosts at once
    Pingmulti(Pending),
    /// Latency and jitter to a host
    Latency(Pending),
    /// Live adapter throughput
    Speed(Pending),
    /// Active TCP connections
    Connections(Pending),
    /// Internet speed test
    Speedtest(Pending),
    /// Flush the DNS cache
    Flushdns(Pending),
    /// Restart a network adapter
    ResetNic(Pending),
    /// Set a static IP, a profile or DHCP
    Setip(Pending),
}

/// Runs an `avlt net` subcommand.
pub fn run(cmd: NetCmd, out: &Out) -> Result<()> {
    let _ = out;
    match cmd {
        NetCmd::Info(_) => not_implemented(),
        NetCmd::Myip(_) => not_implemented(),
        NetCmd::Scan(_) => not_implemented(),
        NetCmd::Portcheck(_) => not_implemented(),
        NetCmd::Portscan(_) => not_implemented(),
        NetCmd::Whoison(_) => not_implemented(),
        NetCmd::Pingmulti(_) => not_implemented(),
        NetCmd::Latency(_) => not_implemented(),
        NetCmd::Speed(_) => not_implemented(),
        NetCmd::Connections(_) => not_implemented(),
        NetCmd::Speedtest(_) => not_implemented(),
        NetCmd::Flushdns(_) => not_implemented(),
        NetCmd::ResetNic(_) => not_implemented(),
        NetCmd::Setip(_) => not_implemented(),
    }
}
