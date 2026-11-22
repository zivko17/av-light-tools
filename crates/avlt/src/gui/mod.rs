//! GUI (egui / eframe): a sidebar of sections over the same avlt-core / avlt-platform
//! functions the CLI calls. No logic lives here beyond screen state.
//!
//! Each section keeps its state in a plain struct with an `update(msg)` style API
//! (tested without a window) and draws it in `ui`. Long operations run on the
//! [`task::Runner`] runtime and never block the UI thread.

pub mod calc;
pub mod edid;
pub mod media;
pub mod ndi;
pub mod net;
pub mod sys;
pub mod task;
pub mod theme;
pub mod widgets;

use std::path::PathBuf;

use avlt_core::{Error, Result};
use eframe::egui;

use task::{Repaint, Runner};

/// The sidebar sections, in display order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Network,
    Edid,
    Calculators,
    System,
    Media,
    Ndi,
}

impl Section {
    pub const ALL: [Section; 6] = [
        Section::Network,
        Section::Edid,
        Section::Calculators,
        Section::System,
        Section::Media,
        Section::Ndi,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Section::Network => "Network",
            Section::Edid => "EDID",
            Section::Calculators => "Calculators",
            Section::System => "System",
            Section::Media => "Media",
            Section::Ndi => "NDI",
        }
    }
}

/// What a section gets each frame besides its own state.
pub struct Cx<'a> {
    pub runner: &'a Runner,
    /// The process runs elevated.
    pub admin: bool,
    /// Message for the status bar (e.g. a failed relaunch).
    pub notice: &'a mut Option<String>,
    /// Files dropped on the window this frame.
    pub dropped: Vec<PathBuf>,
}

/// The whole application.
pub struct App {
    section: Section,
    runner: Runner,
    admin: bool,
    notice: Option<String>,
    calc: calc::CalcPage,
    edid: edid::EdidPage,
    net: net::NetPage,
    sys: sys::SysPage,
    media: media::MediaPage,
    ndi: ndi::NdiPage,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Result<App> {
        theme::apply(&cc.egui_ctx);
        Ok(App {
            section: Section::Network,
            runner: Runner::new(Repaint::new(cc.egui_ctx.clone()))?,
            admin: avlt_platform::admin::is_admin(),
            notice: None,
            calc: calc::CalcPage::default(),
            edid: edid::EdidPage::default(),
            net: net::NetPage::default(),
            sys: sys::SysPage::default(),
            media: media::MediaPage::default(),
            ndi: ndi::NdiPage::default(),
        })
    }

    fn sidebar(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new("avlt")
                .size(24.0)
                .strong()
                .color(theme::ACCENT),
        );
        theme::dim(ui, "AV / lighting / IT toolbox");
        ui.add_space(12.0);
        for s in Section::ALL {
            let selected = self.section == s;
            let text = egui::RichText::new(s.title()).size(16.0);
            let text = if selected {
                text.color(theme::ACCENT).strong()
            } else {
                text
            };
            let button = egui::Button::selectable(selected, text).min_size(egui::vec2(150.0, 30.0));
            if ui.add(button).clicked() {
                self.section = s;
            }
        }
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            theme::dim(ui, concat!("avlt ", env!("CARGO_PKG_VERSION")));
            ui.separator();
            if self.admin {
                ui.label(egui::RichText::new("Administrator").color(theme::GOOD));
            } else {
                theme::dim(ui, "Standard user");
            }
            if let Some(n) = &self.notice {
                ui.separator();
                ui.label(egui::RichText::new(n).color(theme::WARN));
                if ui.small_button("x").clicked() {
                    self.notice = None;
                }
            }
        });
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let dropped: Vec<PathBuf> = ui.ctx().input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_path_buf())
                .collect()
        });
        egui::Panel::left("avlt-nav")
            .resizable(false)
            .exact_size(170.0)
            .frame(egui::Frame::new().fill(theme::BG).inner_margin(8.0))
            .show(ui, |ui| self.sidebar(ui));
        egui::Panel::bottom("avlt-status")
            .frame(egui::Frame::new().fill(theme::BG).inner_margin(4.0))
            .show(ui, |ui| self.status_bar(ui));
        let mut cx = Cx {
            runner: &self.runner,
            admin: self.admin,
            notice: &mut self.notice,
            dropped,
        };
        egui::CentralPanel::default_margins().show(ui, |ui| match self.section {
            Section::Network => self.net.ui(ui, &mut cx),
            Section::Edid => self.edid.ui(ui, &mut cx),
            Section::Calculators => self.calc.ui(ui, &mut cx),
            Section::System => self.sys.ui(ui, &mut cx),
            Section::Media => self.media.ui(ui, &mut cx),
            Section::Ndi => self.ndi.ui(ui, &mut cx),
        });
    }
}

/// Opens the main window and runs until it closes.
pub fn run() -> Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("avlt")
            .with_inner_size([1180.0, 760.0])
            .with_min_inner_size([820.0, 520.0]),
        ..Default::default()
    };
    eframe::run_native(
        "avlt",
        options,
        Box::new(|cc| {
            let app = App::new(cc).map_err(|e| e.to_string())?;
            Ok(Box::new(app))
        }),
    )
    .map_err(|e| Error::Other(format!("cannot open the window: {e}")))
}

/// Tabs inside a section: a row of selectable labels.
pub fn tabs<T: Copy + PartialEq>(ui: &mut egui::Ui, current: &mut T, all: &[(T, &str)]) {
    ui.horizontal_wrapped(|ui| {
        for (t, name) in all {
            ui.selectable_value(current, *t, *name);
        }
    });
    ui.separator();
}
