//! Network section: adapters, scan, ports, MAC vendor, multi-ping, latency and traffic
//! graphs, connections, speed test and IP configuration (after NetMaint's pages).

use std::net::Ipv4Addr;
use std::time::Instant;

use avlt_core::Result;
use avlt_core::net::COMMON_PORTS;
use avlt_core::net::format::{LatencyStats, format_bps, format_link_speed};
use avlt_core::net::model::{
    Adapter, Connection, IpChange, LatencySample, MyIp, PingRound, PortCheck, PortScan, ScanResult,
    SpeedSample, WhoIsOn,
};
use avlt_core::net::target::{IpTarget, parse_ip_target, parse_scan_target, split_hosts};
use avlt_platform::net::{self as pnet, TrafficMeter};
use eframe::egui::{self, RichText, Ui};
use serde_json::Value;

use super::task::{Live, LiveMsg, Msg, Status, Task};
use super::widgets::{self, Confirm, Series, to_value};
use super::{Cx, tabs, theme};
use crate::cli::env;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Info,
    Scan,
    Ports,
    WhoIsOn,
    Ping,
    Latency,
    Traffic,
    Connections,
    Speedtest,
    IpConfig,
}

/// A change to the system that waits for the user's yes.
#[derive(Debug, Clone, PartialEq)]
pub enum NetAction {
    SetIp { adapter: String, target: IpTarget },
    ResetNic { adapter: String },
}

/// State of the Network section.
pub struct NetPage {
    pub tab: Tab,
    pub info: Task<MyIp>,
    pub scan_target: String,
    pub scan: Task<ScanResult>,
    pub host: String,
    pub port: u16,
    pub portcheck: Task<PortCheck>,
    pub portscan: Task<PortScan>,
    pub whois_host: String,
    pub whois_offline: bool,
    pub whois: Task<WhoIsOn>,
    pub ping_hosts: String,
    pub ping: Live<PingRound>,
    pub latency_host: String,
    pub latency: Live<LatencySample>,
    pub traffic_iface: String,
    pub traffic: Live<SpeedSample>,
    pub connections: Task<Vec<Connection>>,
    pub speedtest: Task<Value>,
    pub ip_target: String,
    pub ip_gateway: String,
    pub ip_iface: String,
    pub ip_change: Task<IpChange>,
    pub reset: Task<String>,
    pub flush: Task<()>,
    pub confirm: Confirm<NetAction>,
    pub form_error: Option<String>,
    /// avlt.toml IP profile names, read once (and again on Apply).
    profiles: Option<Vec<String>>,
    started: bool,
}

impl Default for NetPage {
    fn default() -> Self {
        NetPage {
            tab: Tab::Info,
            info: Task::default(),
            scan_target: String::new(),
            scan: Task::default(),
            host: String::new(),
            port: 80,
            portcheck: Task::default(),
            portscan: Task::default(),
            whois_host: String::new(),
            whois_offline: false,
            whois: Task::default(),
            ping_hosts: "8.8.8.8, 1.1.1.1".into(),
            ping: Live::default(),
            latency_host: "8.8.8.8".into(),
            latency: Live::default(),
            traffic_iface: String::new(),
            traffic: Live::default(),
            connections: Task::default(),
            speedtest: Task::default(),
            ip_target: "dhcp".into(),
            ip_gateway: String::new(),
            ip_iface: String::new(),
            ip_change: Task::default(),
            reset: Task::default(),
            flush: Task::default(),
            confirm: Confirm::default(),
            form_error: None,
            profiles: None,
            started: false,
        }
    }
}

/// The adapter to act on: `explicit` (if not blank), then avlt.toml `iface`, then the
/// adapter of the default route (as the CLI).
fn adapter_for(explicit: &str) -> Result<Adapter> {
    let cfg = env::load_config()?;
    let explicit = Some(explicit.trim()).filter(|s| !s.is_empty());
    pnet::default_adapter(explicit.or(cfg.iface.as_deref()))
}

/// `a.b.c` of an address: the /24 `net scan` takes by default.
pub fn default_scan_target(ip: Ipv4Addr) -> String {
    let o = ip.octets();
    format!("{}.{}.{}", o[0], o[1], o[2])
}

impl NetPage {
    /// Advances every job of the section (called each frame by the app).
    pub fn poll(&mut self) {
        let was_loading = self.info.is_running();
        self.info.poll();
        if was_loading
            && self.scan_target.is_empty()
            && let Some(ip) = self
                .info
                .done()
                .and_then(|i| i.adapters.first())
                .and_then(Adapter::primary_ipv4)
        {
            self.scan_target = default_scan_target(ip.address);
        }
        self.scan.poll();
        self.portcheck.poll();
        self.portscan.poll();
        self.whois.poll();
        self.ping.poll();
        self.latency.poll();
        self.traffic.poll();
        self.connections.poll();
        self.speedtest.poll();
        self.ip_change.poll();
        self.reset.poll();
        self.flush.poll();
    }

    fn load_info(&mut self, cx: &Cx<'_>) {
        self.info.start(cx.runner.run(|_| {
            Ok(MyIp {
                adapters: pnet::active_adapters()?,
                public_ip: pnet::public_ip(std::time::Duration::from_secs(3)),
            })
        }));
    }

    /// Starts a scan of the subnet in the box (progress per host, cancellable).
    pub fn start_scan(&mut self, cx: &Cx<'_>) {
        let target = self.scan_target.clone();
        self.scan.start(cx.runner.spawn(|tx| async move {
            let r = async {
                let (network, prefix) = parse_scan_target(&target)?;
                pnet::scan(network, prefix, |done, total| {
                    tx.send(Msg::Progress {
                        done: done as u64,
                        total: total as u64,
                        note: "hosts pinged".into(),
                    });
                })
                .await
            }
            .await;
            tx.send(Msg::Finished(r.map_err(|e| e.to_string())));
        }));
    }

    fn start_ping(&mut self, cx: &Cx<'_>) {
        // The box takes spaces as well as commas between hosts.
        let words: Vec<String> = self
            .ping_hosts
            .split_whitespace()
            .map(String::from)
            .collect();
        let hosts = split_hosts(&words);
        self.ping.start(cx.runner.spawn(|tx| async move {
            let targets = tokio::task::spawn_blocking(move || pnet::ping_targets(&hosts))
                .await
                .unwrap_or_default();
            let start = Instant::now();
            let mut failures = vec![0u64; targets.len()];
            let mut tick = tokio::time::interval(std::time::Duration::from_secs(1));
            loop {
                tick.tick().await;
                let round = pnet::ping_round(&targets, &mut failures, start).await;
                if !tx.send(LiveMsg::Sample(round)) {
                    break;
                }
            }
        }));
    }

    fn start_latency(&mut self, cx: &Cx<'_>) {
        let host = self.latency_host.trim().to_string();
        self.latency.start(cx.runner.spawn(|tx| async move {
            let h = host.clone();
            let ip = match tokio::task::spawn_blocking(move || pnet::resolve_ipv4(&h)).await {
                Ok(Ok(ip)) => ip,
                Ok(Err(e)) => {
                    tx.send(LiveMsg::Failed(e.to_string()));
                    return;
                }
                Err(e) => {
                    tx.send(LiveMsg::Failed(e.to_string()));
                    return;
                }
            };
            let start = Instant::now();
            let mut stats = LatencyStats::default();
            let mut tick = tokio::time::interval(std::time::Duration::from_secs(1));
            loop {
                tick.tick().await;
                let elapsed = start.elapsed();
                let rtt = pnet::latency_ms(ip).await;
                stats.add(rtt);
                let sample = LatencySample {
                    elapsed_s: elapsed.as_secs_f64(),
                    host: host.clone(),
                    ip,
                    latency_ms: rtt,
                    stats: stats.summary(),
                };
                if !tx.send(LiveMsg::Sample(sample)) {
                    break;
                }
            }
        }));
    }

    fn start_traffic(&mut self, cx: &Cx<'_>) {
        let iface = self.traffic_iface.clone();
        self.traffic.start(cx.runner.spawn_blocking(move |tx| {
            let mut meter = match adapter_for(&iface).and_then(TrafficMeter::new) {
                Ok(m) => m,
                Err(e) => {
                    tx.send(LiveMsg::Failed(e.to_string()));
                    return;
                }
            };
            loop {
                std::thread::sleep(std::time::Duration::from_secs(1));
                if tx.cancelled() {
                    break;
                }
                let msg = match meter.sample() {
                    Ok(s) => LiveMsg::Sample(s),
                    Err(e) => LiveMsg::Failed(e.to_string()),
                };
                if !tx.send(msg) {
                    break;
                }
            }
        }));
    }

    /// Validates the IP form and asks for confirmation (the change itself runs on yes).
    pub fn request_setip(&mut self) {
        self.form_error = None;
        self.profiles = None;
        let r = (|| {
            let cfg = env::load_config()?;
            let gateway = Some(self.ip_gateway.trim()).filter(|g| !g.is_empty());
            let target = parse_ip_target(self.ip_target.trim(), gateway, &cfg.ip_profiles)?;
            let iface = Some(self.ip_iface.trim()).filter(|s| !s.is_empty());
            let adapter = pnet::setip_adapter(iface.or(cfg.iface.as_deref()))?;
            Ok::<_, avlt_core::Error>((adapter.name, target))
        })();
        match r {
            Ok((adapter, target)) => self.confirm.ask(
                format!("Set {target} on '{adapter}'?"),
                NetAction::SetIp { adapter, target },
            ),
            Err(e) => self.form_error = Some(e.to_string()),
        }
    }

    fn request_reset(&mut self) {
        self.form_error = None;
        match adapter_for(&self.ip_iface) {
            Ok(a) => self.confirm.ask(
                format!(
                    "Restart adapter '{}'? Its connections drop for a few seconds.",
                    a.name
                ),
                NetAction::ResetNic { adapter: a.name },
            ),
            Err(e) => self.form_error = Some(e.to_string()),
        }
    }

    fn run_action(&mut self, action: NetAction, cx: &Cx<'_>) {
        match action {
            NetAction::SetIp { adapter, target } => {
                self.ip_change.start(cx.runner.run(move |tx| {
                    tx.send(Msg::Progress {
                        done: 0,
                        total: 0,
                        note: format!("Applying {target}..."),
                    });
                    pnet::apply_ip_target(&adapter, &target)?;
                    let after = pnet::wait_for_change(&adapter, target == IpTarget::Dhcp);
                    Ok(IpChange::new(&adapter, target, after.as_ref()))
                }));
            }
            NetAction::ResetNic { adapter } => {
                self.reset.start(cx.runner.run(move |_| {
                    pnet::restart_adapter(&adapter)?;
                    Ok(adapter)
                }));
            }
        }
    }

    pub fn ui(&mut self, ui: &mut Ui, cx: &mut Cx<'_>) {
        if !self.started {
            self.started = true;
            self.load_info(cx);
        }
        if let Some(action) = self.confirm.show(ui.ctx()) {
            self.run_action(action, cx);
        }
        theme::heading(ui, "Network");
        tabs(
            ui,
            &mut self.tab,
            &[
                (Tab::Info, "Info"),
                (Tab::Scan, "Scan"),
                (Tab::Ports, "Ports"),
                (Tab::WhoIsOn, "Who is on"),
                (Tab::Ping, "Ping"),
                (Tab::Latency, "Latency"),
                (Tab::Traffic, "Traffic"),
                (Tab::Connections, "Connections"),
                (Tab::Speedtest, "Speed test"),
                (Tab::IpConfig, "IP config"),
            ],
        );
        match self.tab {
            Tab::Info => self.info_tab(ui, cx),
            Tab::Scan => self.scan_tab(ui, cx),
            Tab::Ports => self.ports_tab(ui, cx),
            Tab::WhoIsOn => self.whois_tab(ui, cx),
            Tab::Ping => self.ping_tab(ui, cx),
            Tab::Latency => self.latency_tab(ui, cx),
            Tab::Traffic => self.traffic_tab(ui, cx),
            Tab::Connections => self.connections_tab(ui, cx),
            Tab::Speedtest => self.speedtest_tab(ui, cx),
            Tab::IpConfig => self.ip_tab(ui, cx),
        }
    }

    fn info_tab(&mut self, ui: &mut Ui, cx: &mut Cx<'_>) {
        if widgets::run_bar(ui, &mut self.info, "Refresh") {
            self.load_info(cx);
        }
        let Some(info) = self.info.done() else { return };
        if info.adapters.is_empty() {
            widgets::error(ui, "No active adapter with an IPv4 address.");
        }
        let v = to_value(info);
        ui.horizontal(|ui| widgets::copy_buttons(ui, &v));
        egui::ScrollArea::vertical()
            .id_salt("net-info")
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Public IP").color(theme::TEXT_DIM));
                    match &info.public_ip {
                        Some(ip) => ui.label(RichText::new(ip).color(theme::GOOD)),
                        None => ui.label(RichText::new("(unavailable)").color(theme::BAD)),
                    };
                });
                for a in &info.adapters {
                    ui.add_space(6.0);
                    egui::Frame::new()
                        .fill(theme::CARD)
                        .inner_margin(8.0)
                        .corner_radius(3.0)
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            theme::section(ui, &a.name);
                            theme::dim(ui, &a.description);
                            egui::Grid::new(("adapter", a.index)).show(ui, |ui| {
                                let row = |ui: &mut Ui, k: &str, v: String| {
                                    ui.label(RichText::new(k).color(theme::TEXT_DIM));
                                    ui.label(v);
                                    ui.end_row();
                                };
                                for c in &a.ipv4 {
                                    row(ui, "IP", format!("{}/{}", c.address, c.prefix));
                                }
                                row(ui, "Gateway", join(&a.gateways, "(none)"));
                                row(ui, "DNS", join(&a.dns, "(automatic)"));
                                row(
                                    ui,
                                    "Mode",
                                    if a.dhcp {
                                        "DHCP (automatic)"
                                    } else {
                                        "Manual (static)"
                                    }
                                    .into(),
                                );
                                if let Some(mac) = &a.mac {
                                    row(ui, "MAC", mac.clone());
                                }
                                if let Some(bps) = a.link_speed_bps {
                                    row(ui, "Link speed", format_link_speed(bps));
                                }
                            });
                        });
                }
            });
    }

    fn scan_tab(&mut self, ui: &mut Ui, cx: &mut Cx<'_>) {
        ui.horizontal(|ui| {
            ui.label("Subnet");
            ui.add(egui::TextEdit::singleline(&mut self.scan_target).hint_text("192.168.1"));
            theme::dim(ui, "192.168.1, 192.168.1.0/24 (/20-/32) or an address");
        });
        if widgets::run_bar(ui, &mut self.scan, "Scan") {
            self.start_scan(cx);
        }
        if let Some(r) = self.scan.done() {
            if r.hosts.is_empty() {
                widgets::error(ui, "No hosts responded.");
            } else {
                widgets::ok(
                    ui,
                    &format!("Found {} hosts on {}", r.hosts.len(), r.network),
                );
            }
            widgets::result_view(ui, "net-scan", &r.hosts);
        }
    }

    fn ports_tab(&mut self, ui: &mut Ui, cx: &mut Cx<'_>) {
        ui.horizontal(|ui| {
            ui.label("Host");
            ui.add(egui::TextEdit::singleline(&mut self.host).hint_text("192.168.1.10"));
            ui.label("Port");
            ui.add(egui::DragValue::new(&mut self.port).range(1..=65535));
        });
        ui.horizontal(|ui| {
            if widgets::run_bar(ui, &mut self.portcheck, "Check port") {
                let (host, port) = (self.host.trim().to_string(), self.port);
                self.portcheck.start(cx.runner.spawn(move |tx| async move {
                    let r = async {
                        let h = host.clone();
                        let ip = tokio::task::spawn_blocking(move || pnet::resolve_ipv4(&h))
                            .await
                            .map_err(|e| avlt_core::Error::Other(e.to_string()))??;
                        let open = pnet::tcp_open(ip, port, pnet::TCP_TIMEOUT).await;
                        Ok::<_, avlt_core::Error>(PortCheck {
                            host,
                            ip,
                            port,
                            open,
                        })
                    }
                    .await;
                    tx.send(Msg::Finished(r.map_err(|e| e.to_string())));
                }));
            }
            if widgets::run_bar(ui, &mut self.portscan, "Scan common ports") {
                let host = self.host.trim().to_string();
                self.portscan.start(cx.runner.spawn(move |tx| async move {
                    let r = async {
                        let h = host.clone();
                        let ip = tokio::task::spawn_blocking(move || pnet::resolve_ipv4(&h))
                            .await
                            .map_err(|e| avlt_core::Error::Other(e.to_string()))??;
                        let ports = pnet::port_scan(ip, COMMON_PORTS, pnet::TCP_TIMEOUT).await;
                        Ok::<_, avlt_core::Error>(PortScan { host, ip, ports })
                    }
                    .await;
                    tx.send(Msg::Finished(r.map_err(|e| e.to_string())));
                }));
            }
        });
        if let Some(c) = self.portcheck.done() {
            ui.horizontal(|ui| {
                if c.open {
                    widgets::ok(ui, &format!("OPEN ({} {} responds)", c.ip, c.port));
                } else {
                    widgets::error(ui, &format!("CLOSED / no response ({} {})", c.ip, c.port));
                }
                widgets::copy_buttons(ui, &to_value(c));
            });
        }
        if let Some(p) = self.portscan.done() {
            widgets::result_view(ui, "net-ports", p);
        }
    }

    fn whois_tab(&mut self, ui: &mut Ui, cx: &mut Cx<'_>) {
        ui.horizontal(|ui| {
            ui.label("Host on the local subnet");
            ui.text_edit_singleline(&mut self.whois_host);
            ui.checkbox(&mut self.whois_offline, "Offline (embedded OUI table only)");
        });
        if widgets::run_bar(ui, &mut self.whois, "Identify") {
            let (host, online) = (self.whois_host.trim().to_string(), !self.whois_offline);
            self.whois.start(cx.runner.spawn(move |tx| async move {
                let r = async {
                    let ip = tokio::task::spawn_blocking(move || pnet::resolve_ipv4(&host))
                        .await
                        .map_err(|e| avlt_core::Error::Other(e.to_string()))??;
                    pnet::whoison(ip, online).await
                }
                .await;
                tx.send(Msg::Finished(r.map_err(|e| e.to_string())));
            }));
        }
        if let Some(w) = self.whois.done() {
            if w.mac.is_none() {
                widgets::error(
                    ui,
                    "No ARP response (host offline or outside the local subnet).",
                );
            }
            widgets::result_view(ui, "net-whois", w);
        }
    }

    fn ping_tab(&mut self, ui: &mut Ui, cx: &mut Cx<'_>) {
        ui.horizontal(|ui| {
            ui.label("Hosts");
            ui.add(
                egui::TextEdit::singleline(&mut self.ping_hosts)
                    .desired_width(360.0)
                    .hint_text("192.168.1.1, 8.8.8.8"),
            );
            if live_buttons(ui, &mut self.ping) {
                self.start_ping(cx);
            }
        });
        live_error(ui, &self.ping.error);
        let Some(round) = self.ping.last() else {
            return;
        };
        let colours = [theme::ACCENT, theme::GOOD, theme::WARN, theme::BAD];
        let series: Vec<Series<'_>> = round
            .hosts
            .iter()
            .enumerate()
            .map(|(i, h)| Series {
                name: &h.host,
                colour: colours[i % colours.len()],
                values: self
                    .ping
                    .samples
                    .iter()
                    .map(|r| r.hosts.get(i).and_then(|h| h.latency_ms).map(|v| v as f64))
                    .collect(),
            })
            .collect();
        widgets::graph(ui, &series, |v| format!("{v:.0} ms"), 160.0);
        egui::Grid::new("ping-table").striped(true).show(ui, |ui| {
            for t in ["Host", "Status", "Latency", "Failures"] {
                ui.label(RichText::new(t).strong().color(theme::ACCENT));
            }
            ui.end_row();
            for h in &round.hosts {
                let (status, colour) = match (h.ip, h.latency_ms) {
                    (None, _) => ("ERROR", theme::WARN),
                    (Some(_), Some(_)) => ("ONLINE", theme::GOOD),
                    (Some(_), None) => ("OFFLINE", theme::BAD),
                };
                ui.label(&h.host);
                ui.label(RichText::new(status).color(colour));
                ui.label(h.latency_ms.map_or("-".into(), |ms| format!("{ms} ms")));
                ui.label(h.failures.to_string());
                ui.end_row();
            }
        });
        ui.horizontal(|ui| widgets::copy_buttons(ui, &to_value(round)));
    }

    fn latency_tab(&mut self, ui: &mut Ui, cx: &mut Cx<'_>) {
        ui.horizontal(|ui| {
            ui.label("Host");
            ui.text_edit_singleline(&mut self.latency_host);
            if live_buttons(ui, &mut self.latency) {
                self.start_latency(cx);
            }
        });
        live_error(ui, &self.latency.error);
        let Some(last) = self.latency.last() else {
            return;
        };
        let series = [Series {
            name: "latency",
            colour: theme::ACCENT,
            values: self
                .latency
                .samples
                .iter()
                .map(|s| s.latency_ms.map(|v| v as f64))
                .collect(),
        }];
        widgets::graph(ui, &series, |v| format!("{v:.0} ms"), 180.0);
        let s = &last.stats;
        let float = |v: Option<f64>| v.map_or_else(|| "-".to_string(), |v| format!("{v:.1}"));
        let int = |v: Option<u64>| v.map_or_else(|| "-".to_string(), |v| v.to_string());
        ui.label(format!(
            "{} ({})   now {}   packets {} sent, {} received, {:.0}% loss",
            last.host,
            last.ip,
            last.latency_ms
                .map_or("TIMEOUT".into(), |ms| format!("{ms} ms")),
            s.sent,
            s.received,
            s.loss_pct
        ));
        ui.label(format!(
            "min {} / avg {} / max {} ms, jitter {} ms",
            int(s.min_ms),
            float(s.avg_ms),
            int(s.max_ms),
            float(s.jitter_ms)
        ));
        ui.horizontal(|ui| widgets::copy_buttons(ui, &to_value(last)));
    }

    fn traffic_tab(&mut self, ui: &mut Ui, cx: &mut Cx<'_>) {
        ui.horizontal(|ui| {
            ui.label("Adapter");
            ui.add(
                egui::TextEdit::singleline(&mut self.traffic_iface)
                    .hint_text("default route adapter"),
            );
            if live_buttons(ui, &mut self.traffic) {
                self.start_traffic(cx);
            }
        });
        live_error(ui, &self.traffic.error);
        let Some(last) = self.traffic.last() else {
            return;
        };
        let series = [
            Series {
                name: "download",
                colour: theme::ACCENT,
                values: self
                    .traffic
                    .samples
                    .iter()
                    .map(|s| Some(s.down_bps))
                    .collect(),
            },
            Series {
                name: "upload",
                colour: theme::GOOD,
                values: self
                    .traffic
                    .samples
                    .iter()
                    .map(|s| Some(s.up_bps))
                    .collect(),
            },
        ];
        widgets::graph(ui, &series, |v| format_bps(v / 8.0), 200.0);
        ui.label(format!(
            "{}   down {}   up {}",
            last.adapter,
            format_bps(last.down_bps / 8.0),
            format_bps(last.up_bps / 8.0)
        ));
        ui.horizontal(|ui| widgets::copy_buttons(ui, &to_value(last)));
    }

    fn connections_tab(&mut self, ui: &mut Ui, cx: &mut Cx<'_>) {
        if widgets::run_bar(ui, &mut self.connections, "List connections") {
            self.connections
                .start(cx.runner.run(|_| pnet::connections()));
        }
        if let Some(list) = self.connections.done() {
            if list.is_empty() {
                theme::dim(ui, "No established connections.");
            }
            widgets::result_view(ui, "net-conns", list);
        }
    }

    fn speedtest_tab(&mut self, ui: &mut Ui, cx: &mut Cx<'_>) {
        theme::dim(
            ui,
            "Runs the Ookla Speedtest CLI (about 30 s). Needs: winget install Ookla.Speedtest.CLI",
        );
        if widgets::run_bar(ui, &mut self.speedtest, "Run speed test") {
            self.speedtest.start(cx.runner.run(|tx| {
                tx.send(Msg::Progress {
                    done: 0,
                    total: 0,
                    note: "Testing (about 30 s)...".into(),
                });
                pnet::speedtest(tx.cancel_flag())
            }));
        }
        if let Some(v) = self.speedtest.done() {
            let mbps = |key: &str| v[key]["bandwidth"].as_f64().map(|b| b * 8.0 / 1e6);
            if let (Some(d), Some(u)) = (mbps("download"), mbps("upload")) {
                ui.label(
                    RichText::new(format!("Download {d:.1} Mbps   Upload {u:.1} Mbps"))
                        .size(20.0)
                        .color(theme::ACCENT),
                );
            }
            widgets::result_view(ui, "net-speedtest", v);
        }
    }

    fn ip_tab(&mut self, ui: &mut Ui, cx: &mut Cx<'_>) {
        let profiles = self
            .profiles
            .get_or_insert_with(|| {
                env::load_config()
                    .map(|c| c.ip_profiles.keys().cloned().collect())
                    .unwrap_or_default()
            })
            .clone();
        theme::section(ui, "Set IP (SetIP)");
        egui::Grid::new("setip").num_columns(2).show(ui, |ui| {
            ui.label("Target");
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.ip_target)
                        .hint_text("dhcp, profile or 192.168.1.25/24"),
                );
                egui::ComboBox::from_id_salt("ip-profiles")
                    .selected_text("profiles")
                    .show_ui(ui, |ui| {
                        for p in profiles.iter().map(String::as_str).chain(["dhcp"]) {
                            if ui.selectable_label(self.ip_target == p, p).clicked() {
                                self.ip_target = p.to_string();
                            }
                        }
                    });
            });
            ui.end_row();
            ui.label("Gateway");
            ui.add(egui::TextEdit::singleline(&mut self.ip_gateway).hint_text("optional"));
            ui.end_row();
            ui.label("Adapter");
            ui.add(
                egui::TextEdit::singleline(&mut self.ip_iface)
                    .hint_text("avlt.toml iface, then the default route's"),
            );
            ui.end_row();
        });
        if let Some(e) = &self.form_error {
            widgets::error(ui, e);
        }
        if widgets::admin_gate(ui, cx) {
            if widgets::run_bar(ui, &mut self.ip_change, "Apply IP") {
                self.request_setip();
            }
            if let Some(c) = self.ip_change.done() {
                widgets::ok(ui, &format!("OK -> {} on '{}'", c.applied, c.adapter));
                widgets::result_view(ui, "net-setip", c);
            }
            ui.separator();
            theme::section(ui, "Restart adapter");
            if widgets::run_bar(ui, &mut self.reset, "Restart adapter") {
                self.request_reset();
            }
            if let Some(a) = self.reset.done() {
                ui.horizontal(|ui| {
                    widgets::ok(ui, &format!("OK -> '{a}' restarted"));
                    let v = serde_json::json!({ "adapter": a, "restarted": true });
                    widgets::copy_buttons(ui, &v);
                });
            }
        }
        ui.separator();
        theme::section(ui, "DNS cache");
        if widgets::run_bar(ui, &mut self.flush, "Flush DNS") {
            self.flush.start(cx.runner.run(|_| pnet::flush_dns()));
        }
        if matches!(self.flush.status, Status::Done(())) {
            widgets::ok(ui, "OK -> DNS cache cleared");
        }
    }
}

fn join<T: ToString>(items: &[T], empty: &str) -> String {
    if items.is_empty() {
        return empty.to_string();
    }
    items
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

/// Start / Stop of a live monitor; `true` when Start was clicked.
fn live_buttons<S>(ui: &mut Ui, live: &mut Live<S>) -> bool {
    if live.is_running() {
        ui.spinner();
        if ui.button("Stop").clicked() {
            live.stop();
        }
        false
    } else {
        theme::accent_button(ui, "Start").clicked()
    }
}

fn live_error(ui: &mut Ui, error: &Option<String>) {
    if let Some(e) = error {
        widgets::error(ui, e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::task::tests::{runner, wait};

    fn result(n: usize) -> ScanResult {
        ScanResult {
            network: "192.168.1.0/24".into(),
            scanned: 254,
            hosts: (0..n)
                .map(|i| avlt_core::net::model::ScanHost {
                    ip: Ipv4Addr::new(192, 168, 1, i as u8 + 1),
                    mac: None,
                    vendor: None,
                    hostname: None,
                    latency_ms: 1,
                    this_pc: false,
                })
                .collect(),
        }
    }

    #[test]
    fn scan_goes_idle_running_done() {
        let rt = runner();
        let mut page = NetPage::default();
        assert!(matches!(page.scan.status, Status::Idle));
        page.scan.start(rt.run(|tx| {
            tx.send(Msg::Progress {
                done: 10,
                total: 254,
                note: String::new(),
            });
            Ok(result(3))
        }));
        assert!(matches!(page.scan.status, Status::Running { .. }));
        wait(&mut page.scan);
        assert_eq!(page.scan.done().map(|r| r.hosts.len()), Some(3));
    }

    #[test]
    fn cancel_during_a_scan_returns_to_idle() {
        let rt = runner();
        let mut page = NetPage::default();
        page.scan
            .start(rt.spawn(|_tx| std::future::pending::<()>()));
        page.scan.update(Msg::Progress {
            done: 5,
            total: 254,
            note: String::new(),
        });
        assert!(matches!(page.scan.status, Status::Running { done: 5, .. }));
        page.scan.cancel();
        assert!(matches!(page.scan.status, Status::Idle));
        page.scan.update(Msg::Finished(Ok(result(1))));
        assert!(matches!(page.scan.status, Status::Idle));
    }

    #[test]
    fn invalid_subnet_fails_the_scan_without_panicking() {
        let rt = runner();
        let mut page = NetPage::default();
        page.scan.start(rt.spawn(|tx| async move {
            let r = parse_scan_target("10.0.0.0/8").map(|_| result(0));
            tx.send(Msg::Finished(r.map_err(|e| e.to_string())));
        }));
        wait(&mut page.scan);
        assert!(matches!(&page.scan.status, Status::Failed(e) if e.contains("too large")));
    }

    #[test]
    fn destructive_actions_wait_for_confirmation() {
        let mut page = NetPage {
            ip_target: "not an ip".into(),
            ..NetPage::default()
        };
        page.request_setip();
        assert!(
            page.form_error.is_some(),
            "invalid target is refused before asking"
        );
        assert!(!page.confirm.is_open());
        page.confirm.ask(
            "Restart?",
            NetAction::ResetNic {
                adapter: "Ethernet".into(),
            },
        );
        assert_eq!(page.confirm.answer(false), None, "No never runs the action");
    }

    #[test]
    fn default_scan_target_is_the_24() {
        assert_eq!(default_scan_target(Ipv4Addr::new(10, 1, 2, 3)), "10.1.2");
    }
}
