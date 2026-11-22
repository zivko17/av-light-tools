//! System section: summary and device reports, live telemetry and top processes,
//! kill processes, display mode, audio services and cleanup.

use std::time::Duration;

use avlt_core::sys::display::{pick_mode, refresh_rates, resolutions};
use avlt_core::sys::format::{freed_text, mib, round_to};
use avlt_core::sys::model::{
    CleanupAnalysis, CleanupReport, Display, DisplayMode, KillResult, ModeChange, ProcessInfo,
    ServiceStatus, Telemetry, TopSample,
};
use avlt_core::sys::process::{is_protected_process, kill_plan};
use avlt_core::sys::{AUDIO_RESTART_SERVICE, AUDIO_SERVICES};
use avlt_platform::sys as psys;
use eframe::egui::{self, RichText, Ui};
use serde_json::Value;

use super::task::{Live, LiveMsg, Status, Task};
use super::widgets::{self, Confirm, Series, to_value};
use super::{Cx, tabs, theme};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Reports,
    Telemetry,
    Top,
    Kill,
    DisplayMode,
    AudioServices,
    Cleanup,
}

/// Read-only reports, each one platform call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Report {
    Info,
    Usb,
    Disks,
    Monitors,
    Battery,
    Startup,
    Audio,
}

impl Report {
    pub const ALL: [Report; 7] = [
        Report::Info,
        Report::Usb,
        Report::Disks,
        Report::Monitors,
        Report::Battery,
        Report::Startup,
        Report::Audio,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Report::Info => "System info",
            Report::Usb => "USB devices",
            Report::Disks => "Disks",
            Report::Monitors => "Monitors",
            Report::Battery => "Battery",
            Report::Startup => "Startup programs",
            Report::Audio => "Audio devices",
        }
    }

    fn run(self) -> avlt_core::Result<Value> {
        Ok(match self {
            Report::Info => to_value(&psys::info()?),
            Report::Usb => to_value(&psys::usb_devices()?),
            Report::Disks => to_value(&psys::disks()?),
            Report::Monitors => to_value(&psys::monitors()?),
            Report::Battery => match psys::battery()? {
                Some(b) => to_value(&b),
                None => Value::String("No battery (desktop PC).".into()),
            },
            Report::Startup => to_value(&psys::startup_entries()?),
            Report::Audio => to_value(&psys::audio_devices()?),
        })
    }
}

/// A system change that waits for the user's yes.
#[derive(Debug, Clone, PartialEq)]
pub enum SysAction {
    Kill(Vec<ProcessInfo>),
    SetMode {
        device_name: String,
        mode: DisplayMode,
    },
    RestartAudio,
    Cleanup,
}

/// The processes the GUI may offer to kill: never critical or protected ones
/// (they are listed, but without a checkbox or Kill button).
pub fn killable(matched: &[ProcessInfo]) -> Vec<ProcessInfo> {
    let own = std::process::id();
    let (to_kill, _) = kill_plan(matched, false, &[]);
    to_kill
        .into_iter()
        .filter(|p| p.pid != own && !p.critical && !is_protected_process(p.pid, &p.name))
        .cloned()
        .collect()
}

/// State of the System section.
pub struct SysPage {
    pub tab: Tab,
    pub report: Report,
    pub reports: Vec<Task<Value>>,
    pub telemetry: Live<Telemetry>,
    pub top: Live<TopSample>,
    pub kill_name: String,
    pub kill_wildcard: bool,
    pub matches: Task<Vec<ProcessInfo>>,
    /// PIDs ticked in the match list (only killable ones can be).
    pub selected: Vec<u32>,
    pub killed: Task<Vec<KillResult>>,
    pub displays: Task<Vec<Display>>,
    pub display_idx: usize,
    pub resolution: Option<(u32, u32)>,
    pub hz: Option<u32>,
    pub mode_change: Task<ModeChange>,
    pub services: Task<Vec<ServiceStatus>>,
    pub restart: Task<ServiceStatus>,
    pub analysis: Task<CleanupAnalysis>,
    pub cleanup: Task<CleanupReport>,
    pub confirm: Confirm<SysAction>,
    pub error: Option<String>,
    /// Reports, displays and services already loaded once (later loads are manual).
    loaded: Vec<&'static str>,
}

impl Default for SysPage {
    fn default() -> Self {
        SysPage {
            tab: Tab::Reports,
            report: Report::Info,
            reports: Report::ALL.iter().map(|_| Task::default()).collect(),
            telemetry: Live::default(),
            top: Live::default(),
            kill_name: String::new(),
            kill_wildcard: false,
            matches: Task::default(),
            selected: Vec::new(),
            killed: Task::default(),
            displays: Task::default(),
            display_idx: 0,
            resolution: None,
            hz: None,
            mode_change: Task::default(),
            services: Task::default(),
            restart: Task::default(),
            analysis: Task::default(),
            cleanup: Task::default(),
            confirm: Confirm::default(),
            error: None,
            loaded: Vec::new(),
        }
    }
}

/// `true` the first time `key` is seen (auto-load a list once per session).
fn first_time(loaded: &mut Vec<&'static str>, key: &'static str) -> bool {
    if loaded.contains(&key) {
        return false;
    }
    loaded.push(key);
    true
}

fn report_index(r: Report) -> usize {
    Report::ALL.iter().position(|x| *x == r).unwrap_or(0)
}

impl SysPage {
    fn poll(&mut self) {
        for r in &mut self.reports {
            r.poll();
        }
        self.telemetry.poll();
        self.top.poll();
        let finding = self.matches.is_running();
        self.matches.poll();
        if finding && let Some(m) = self.matches.done() {
            self.selected = killable(m).iter().map(|p| p.pid).collect();
        }
        self.killed.poll();
        self.displays.poll();
        self.mode_change.poll();
        self.services.poll();
        self.restart.poll();
        self.analysis.poll();
        self.cleanup.poll();
    }

    /// Asks to kill the ticked processes; critical / protected ones are dropped even if
    /// ticked, and nothing is asked when none remain.
    pub fn request_kill(&mut self) {
        let Some(matched) = self.matches.done() else {
            return;
        };
        let chosen: Vec<ProcessInfo> = killable(matched)
            .into_iter()
            .filter(|p| self.selected.contains(&p.pid))
            .collect();
        self.ask_kill(chosen);
    }

    fn ask_kill(&mut self, chosen: Vec<ProcessInfo>) {
        let chosen = killable(&chosen);
        if chosen.is_empty() {
            return;
        }
        let names: Vec<String> = chosen
            .iter()
            .map(|p| format!("{} ({})", p.name, p.pid))
            .collect();
        self.confirm.ask(
            format!("Kill {} processes?\n{}", chosen.len(), names.join(", ")),
            SysAction::Kill(chosen),
        );
    }

    fn run_action(&mut self, action: SysAction, cx: &Cx<'_>) {
        match action {
            SysAction::Kill(list) => {
                self.killed.start(cx.runner.run(move |_| {
                    let refs: Vec<&ProcessInfo> = list.iter().collect();
                    Ok(psys::kill_all(&refs, &[]))
                }));
            }
            SysAction::SetMode { device_name, mode } => {
                self.mode_change.start(
                    cx.runner
                        .run(move |_| psys::set_mode(&device_name, mode, false)),
                );
            }
            SysAction::RestartAudio => {
                self.restart.start(
                    cx.runner
                        .run(|_| psys::restart_service(AUDIO_RESTART_SERVICE)),
                );
            }
            SysAction::Cleanup => {
                self.cleanup.start(
                    cx.runner
                        .run(|tx| psys::cleanup_cancellable(tx.cancel_flag())),
                );
            }
        }
    }

    pub fn ui(&mut self, ui: &mut Ui, cx: &mut Cx<'_>) {
        self.poll();
        if let Some(a) = self.confirm.show(ui.ctx()) {
            self.run_action(a, cx);
        }
        theme::heading(ui, "System");
        tabs(
            ui,
            &mut self.tab,
            &[
                (Tab::Reports, "Reports"),
                (Tab::Telemetry, "Telemetry"),
                (Tab::Top, "Top"),
                (Tab::Kill, "Kill process"),
                (Tab::DisplayMode, "Display mode"),
                (Tab::AudioServices, "Audio services"),
                (Tab::Cleanup, "Cleanup"),
            ],
        );
        match self.tab {
            Tab::Reports => self.reports_tab(ui, cx),
            Tab::Telemetry => self.telemetry_tab(ui, cx),
            Tab::Top => self.top_tab(ui, cx),
            Tab::Kill => self.kill_tab(ui, cx),
            Tab::DisplayMode => self.display_tab(ui, cx),
            Tab::AudioServices => self.services_tab(ui, cx),
            Tab::Cleanup => self.cleanup_tab(ui, cx),
        }
    }

    fn reports_tab(&mut self, ui: &mut Ui, cx: &mut Cx<'_>) {
        ui.horizontal_wrapped(|ui| {
            for r in Report::ALL {
                ui.selectable_value(&mut self.report, r, r.title());
            }
        });
        let report = self.report;
        let task = &mut self.reports[report_index(report)];
        if widgets::run_bar(ui, task, "Refresh") | first_time(&mut self.loaded, report.title()) {
            task.start(cx.runner.run(move |_| report.run()));
        }
        if let Some(v) = task.done() {
            widgets::result_view(ui, report.title(), v);
        }
    }

    fn telemetry_tab(&mut self, ui: &mut Ui, cx: &mut Cx<'_>) {
        ui.horizontal(|ui| {
            if self.telemetry.is_running() {
                ui.spinner();
                if ui.button("Stop").clicked() {
                    self.telemetry.stop();
                }
            } else if theme::accent_button(ui, "Start").clicked() {
                self.telemetry.start(cx.runner.spawn_blocking(|tx| {
                    let mut sampler = match psys::TelemetrySampler::new() {
                        Ok(s) => s,
                        Err(e) => {
                            tx.send(LiveMsg::Failed(e.to_string()));
                            return;
                        }
                    };
                    loop {
                        std::thread::sleep(Duration::from_secs(1));
                        if tx.cancelled() {
                            break;
                        }
                        let msg = match sampler.sample() {
                            Ok(t) => LiveMsg::Sample(t),
                            Err(e) => LiveMsg::Failed(e.to_string()),
                        };
                        if !tx.send(msg) {
                            break;
                        }
                    }
                }));
            }
            theme::dim(ui, "CPU, RAM, GPU and disk use, every second");
        });
        if let Some(e) = &self.telemetry.error {
            widgets::error(ui, e);
        }
        let Some(last) = self.telemetry.last() else {
            return;
        };
        let s = &self.telemetry.samples;
        let series = [
            Series {
                name: "CPU %",
                colour: theme::ACCENT,
                values: s.iter().map(|t| Some(t.cpu_pct)).collect(),
            },
            Series {
                name: "RAM %",
                colour: theme::GOOD,
                values: s.iter().map(|t| Some(t.ram_pct)).collect(),
            },
            Series {
                name: "GPU %",
                colour: theme::WARN,
                values: s.iter().map(|t| t.gpu_pct).collect(),
            },
        ];
        widgets::graph(ui, &series, |v| format!("{v:.0}%"), 200.0);
        widgets::result_view(ui, "sys-tele", last);
    }

    fn top_tab(&mut self, ui: &mut Ui, cx: &mut Cx<'_>) {
        ui.horizontal(|ui| {
            if self.top.is_running() {
                ui.spinner();
                if ui.button("Stop").clicked() {
                    self.top.stop();
                }
            } else if theme::accent_button(ui, "Start").clicked() {
                self.top.start(cx.runner.spawn_blocking(|tx| {
                    let mut sampler = match psys::TopSampler::new() {
                        Ok(s) => s,
                        Err(e) => {
                            tx.send(LiveMsg::Failed(e.to_string()));
                            return;
                        }
                    };
                    loop {
                        std::thread::sleep(Duration::from_secs(2));
                        if tx.cancelled() {
                            break;
                        }
                        let msg = match sampler.sample(15) {
                            Ok(t) => LiveMsg::Sample(t),
                            Err(e) => LiveMsg::Failed(e.to_string()),
                        };
                        if !tx.send(msg) {
                            break;
                        }
                    }
                }));
            }
            theme::dim(ui, "Top processes by instantaneous CPU, every 2 s");
        });
        if let Some(e) = &self.top.error {
            widgets::error(ui, e);
        }
        let Some(last) = self.top.last().cloned() else {
            return;
        };
        let can_kill = killable(&last.processes);
        let mut kill = None;
        egui::Grid::new("top").striped(true).show(ui, |ui| {
            for h in ["Process", "PID", "CPU %", "Memory (MB)", ""] {
                ui.label(RichText::new(h).strong().color(theme::ACCENT));
            }
            ui.end_row();
            for p in &last.processes {
                ui.label(&p.name);
                ui.label(p.pid.to_string());
                ui.label(format!("{:.1}", p.cpu_pct));
                ui.label(format!("{}", round_to(mib(p.memory_bytes), 1)));
                if can_kill.iter().any(|k| k.pid == p.pid) {
                    if ui.small_button("Kill").clicked() {
                        kill = Some(p.clone());
                    }
                } else {
                    theme::dim(ui, "protected");
                }
                ui.end_row();
            }
        });
        ui.horizontal(|ui| widgets::copy_buttons(ui, &to_value(&last)));
        if let Some(p) = kill {
            self.ask_kill(vec![p]);
        }
        self.kill_results(ui);
    }

    fn kill_tab(&mut self, ui: &mut Ui, cx: &mut Cx<'_>) {
        ui.horizontal(|ui| {
            ui.label("Process name");
            ui.add(egui::TextEdit::singleline(&mut self.kill_name).hint_text("notepad"));
            ui.checkbox(&mut self.kill_wildcard, "Contains (wildcard)");
        });
        if widgets::run_bar(ui, &mut self.matches, "Find") {
            let (name, wildcard) = (self.kill_name.trim().to_string(), self.kill_wildcard);
            self.matches.start(
                cx.runner
                    .run(move |_| psys::find_processes(&name, wildcard)),
            );
        }
        theme::dim(
            ui,
            "Critical system processes are listed but can never be killed from here.",
        );
        let Some(matched) = self.matches.done() else {
            return;
        };
        if matched.is_empty() {
            theme::dim(ui, "No process found.");
            return;
        }
        let can_kill = killable(matched);
        egui::Grid::new("kill").striped(true).show(ui, |ui| {
            for h in ["", "Process", "PID", "Memory (MB)"] {
                ui.label(RichText::new(h).strong().color(theme::ACCENT));
            }
            ui.end_row();
            for p in matched {
                if can_kill.iter().any(|k| k.pid == p.pid) {
                    let mut on = self.selected.contains(&p.pid);
                    if ui.checkbox(&mut on, "").changed() {
                        if on {
                            self.selected.push(p.pid);
                        } else {
                            self.selected.retain(|x| *x != p.pid);
                        }
                    }
                } else {
                    ui.label(RichText::new("CRITICAL").color(theme::BAD));
                }
                ui.label(&p.name);
                ui.label(p.pid.to_string());
                ui.label(format!("{}", round_to(mib(p.memory_bytes), 1)));
                ui.end_row();
            }
        });
        if widgets::run_bar(ui, &mut self.killed, "Kill selected") {
            self.request_kill();
        }
        self.kill_results(ui);
    }

    fn kill_results(&self, ui: &mut Ui) {
        if let Some(results) = self.killed.done() {
            for k in results {
                match &k.error {
                    None if k.killed => {
                        widgets::ok(ui, &format!("Killed: {} (PID {})", k.name, k.pid))
                    }
                    Some(e) => {
                        widgets::error(ui, &format!("Failed: {} (PID {}) - {e}", k.name, k.pid))
                    }
                    None => theme::dim(ui, &format!("Skipped: {} (PID {})", k.name, k.pid)),
                }
            }
        }
    }

    fn display_tab(&mut self, ui: &mut Ui, cx: &mut Cx<'_>) {
        if widgets::run_bar(ui, &mut self.displays, "List displays")
            | first_time(&mut self.loaded, "displays")
        {
            self.displays.start(cx.runner.run(|_| psys::displays()));
        }
        let Some(list) = self.displays.done().cloned() else {
            return;
        };
        if list.is_empty() {
            widgets::error(ui, "No display is attached to the desktop.");
            return;
        }
        self.display_idx = self.display_idx.min(list.len() - 1);
        let d = &list[self.display_idx];
        egui::Grid::new("mode").num_columns(2).show(ui, |ui| {
            ui.label("Display");
            egui::ComboBox::from_id_salt("disp")
                .selected_text(format!("{}: {}", d.index, d.name))
                .show_ui(ui, |ui| {
                    for (i, x) in list.iter().enumerate() {
                        if ui
                            .selectable_value(
                                &mut self.display_idx,
                                i,
                                format!("{}: {}", x.index, x.name),
                            )
                            .changed()
                        {
                            self.resolution = None;
                            self.hz = None;
                        }
                    }
                });
            ui.end_row();
            ui.label("Current");
            ui.label(d.current.map_or("-".into(), mode_text));
            ui.end_row();
            ui.label("Resolution");
            egui::ComboBox::from_id_salt("res")
                .selected_text(
                    self.resolution
                        .map_or("(choose)".into(), |(w, h)| format!("{w}x{h}")),
                )
                .show_ui(ui, |ui| {
                    for (w, h) in resolutions(&d.modes) {
                        if ui
                            .selectable_value(
                                &mut self.resolution,
                                Some((w, h)),
                                format!("{w}x{h}"),
                            )
                            .changed()
                        {
                            self.hz = None;
                        }
                    }
                });
            ui.end_row();
            ui.label("Refresh");
            let rates = self
                .resolution
                .map(|(w, h)| refresh_rates(&d.modes, w, h))
                .unwrap_or_default();
            egui::ComboBox::from_id_salt("hz")
                .selected_text(self.hz.map_or("highest".into(), |r| format!("{r} Hz")))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.hz, None, "highest");
                    for r in rates {
                        ui.selectable_value(&mut self.hz, Some(r), format!("{r} Hz"));
                    }
                });
            ui.end_row();
        });
        self.error = None;
        let mode = self
            .resolution
            .map(|(w, h)| pick_mode(&d.modes, w, h, self.hz).map_err(|e| e.to_string()));
        ui.horizontal(|ui| {
            let ready = matches!(mode, Some(Ok(_)));
            if ui
                .add_enabled(
                    ready && !self.mode_change.is_running(),
                    egui::Button::new("Test (change nothing)"),
                )
                .clicked()
                && let Some(Ok(m)) = &mode
            {
                let (dev, m) = (d.device_name.clone(), *m);
                self.mode_change
                    .start(cx.runner.run(move |_| psys::set_mode(&dev, m, true)));
            }
            if ui
                .add_enabled(
                    ready && !self.mode_change.is_running(),
                    egui::Button::new("Apply"),
                )
                .clicked()
                && let Some(Ok(m)) = &mode
            {
                self.confirm.ask(
                    format!(
                        "Change display {} ({}) to {}?",
                        d.index,
                        d.name,
                        mode_text(*m)
                    ),
                    SysAction::SetMode {
                        device_name: d.device_name.clone(),
                        mode: *m,
                    },
                );
            }
        });
        if let Some(Err(e)) = &mode {
            widgets::error(ui, e);
        }
        if let Status::Failed(e) = &self.mode_change.status {
            widgets::error(ui, e);
        }
        if let Some(c) = self.mode_change.done() {
            if c.test_only {
                widgets::ok(
                    ui,
                    &format!(
                        "The driver accepts {} (test only, nothing changed).",
                        mode_text(c.mode)
                    ),
                );
            } else if c.restart_required {
                widgets::error(
                    ui,
                    &format!(
                        "{} set; Windows needs a restart to apply it.",
                        mode_text(c.mode)
                    ),
                );
            } else {
                widgets::ok(
                    ui,
                    &format!("{} now runs at {}.", c.device_name, mode_text(c.mode)),
                );
            }
        }
    }

    fn services_tab(&mut self, ui: &mut Ui, cx: &mut Cx<'_>) {
        if widgets::run_bar(ui, &mut self.services, "Refresh")
            | first_time(&mut self.loaded, "services")
        {
            self.services
                .start(cx.runner.run(|_| psys::audio_services()));
        }
        if let Some(list) = self.services.done() {
            widgets::result_view(ui, "sys-services", list);
        }
        ui.separator();
        let desc = AUDIO_SERVICES
            .iter()
            .find(|(n, _)| *n == AUDIO_RESTART_SERVICE)
            .map_or("", |(_, d)| d);
        theme::section(ui, &format!("Restart {desc} ({AUDIO_RESTART_SERVICE})"));
        if widgets::admin_gate(ui, cx) {
            if widgets::run_bar(ui, &mut self.restart, "Restart audio") {
                self.confirm.ask(
                    format!("Restart {desc} ({AUDIO_RESTART_SERVICE})? Audio stops for a moment."),
                    SysAction::RestartAudio,
                );
            }
            if let Some(s) = self.restart.done() {
                widgets::ok(ui, &format!("{} restarted: {}", s.name, s.status));
            }
        }
    }

    fn cleanup_tab(&mut self, ui: &mut Ui, cx: &mut Cx<'_>) {
        theme::dim(
            ui,
            "Deletes temp files and browser caches and empties the Recycle Bin. Files in use are skipped.",
        );
        if widgets::run_bar(ui, &mut self.analysis, "Analyze (delete nothing)") {
            self.analysis
                .start(cx.runner.run(|_| psys::analyze_cleanup()));
        }
        if let Some(a) = self.analysis.done() {
            for t in &a.targets {
                ui.label(format!(
                    "{:<25} {:>10}  ({} files)",
                    t.name,
                    freed_text(t.size_bytes),
                    t.files
                ));
            }
            ui.label(format!(
                "{:<25} {:>10}  ({} items)",
                "Recycle Bin",
                freed_text(a.recycle_bin.bytes),
                a.recycle_bin.items
            ));
            ui.horizontal(|ui| widgets::copy_buttons(ui, &to_value(a)));
        }
        ui.separator();
        if widgets::run_bar(ui, &mut self.cleanup, "Clean up") {
            self.confirm.ask(
                "Delete temp files and browser caches and empty the Recycle Bin?",
                SysAction::Cleanup,
            );
        }
        if let Some(r) = self.cleanup.done() {
            widgets::ok(
                ui,
                &format!("Total freed: {}", freed_text(r.total_freed_bytes)),
            );
            widgets::result_view(ui, "sys-cleanup", r);
        }
    }
}

fn mode_text(m: DisplayMode) -> String {
    format!("{}x{} @ {} Hz", m.width, m.height, m.refresh_hz)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::task::Msg;
    use crate::gui::task::tests::runner;

    fn p(name: &str, pid: u32, critical: bool) -> ProcessInfo {
        ProcessInfo {
            name: name.into(),
            pid,
            memory_bytes: 0,
            cpu_pct: 0.0,
            critical,
        }
    }

    #[test]
    fn critical_and_protected_processes_are_never_killable() {
        let matched = [
            p("svchost.exe", 900, true),
            p("csrss.exe", 600, false),
            p("System", 4, false),
            p("notepad.exe", 1234, false),
        ];
        let k = killable(&matched);
        assert_eq!(k.iter().map(|p| p.pid).collect::<Vec<_>>(), [1234]);
    }

    #[test]
    fn kill_request_drops_critical_even_if_ticked() {
        let rt = runner();
        let mut page = SysPage::default();
        let list = vec![p("svchost.exe", 900, true), p("notepad.exe", 1234, false)];
        page.matches.start(rt.run(move |_| Ok(list)));
        crate::gui::task::tests::wait(&mut page.matches);
        page.selected = vec![900];
        page.request_kill();
        assert!(
            !page.confirm.is_open(),
            "only a critical process ticked: nothing asked"
        );
        page.selected = vec![900, 1234];
        page.request_kill();
        match page.confirm.answer(true) {
            Some(SysAction::Kill(list)) => {
                assert_eq!(list.iter().map(|p| p.pid).collect::<Vec<_>>(), [1234]);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn cleanup_cancel_returns_to_idle() {
        let rt = runner();
        let mut page = SysPage::default();
        page.cleanup
            .start(rt.spawn(|_tx| std::future::pending::<()>()));
        assert!(page.cleanup.is_running());
        page.cleanup.cancel();
        assert!(matches!(page.cleanup.status, Status::Idle));
        page.cleanup.update(Msg::Finished(Err("late".into())));
        assert!(matches!(page.cleanup.status, Status::Idle));
    }
}
