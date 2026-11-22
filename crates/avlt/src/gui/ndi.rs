//! NDI section: launch the NDI Tools apps, network diagnostic, services and the Access
//! Manager configuration (after `AVToolbox/NdiTools.cs`).

use std::path::PathBuf;

use avlt_platform::ndi::{self, NdiApp, NdiCheck, NdiConfig, NdiInstall, NdiServices};
use eframe::egui::{self, Ui};

use super::task::Task;
use super::widgets;
use super::{Cx, tabs, theme};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Launch,
    Check,
    Services,
    Config,
}

/// State of the NDI section.
pub struct NdiPage {
    pub tab: Tab,
    pub install: Option<NdiInstall>,
    /// Last launch: app name and the path started, or the error.
    pub launched: Option<Result<(String, PathBuf), String>>,
    pub check: Task<NdiCheck>,
    pub services: Task<NdiServices>,
    pub config: Task<NdiConfig>,
}

impl Default for NdiPage {
    fn default() -> Self {
        NdiPage {
            tab: Tab::Launch,
            install: None,
            launched: None,
            check: Task::default(),
            services: Task::default(),
            config: Task::default(),
        }
    }
}

impl NdiPage {
    /// Records the outcome of starting `app` (the same call as `avlt ndi launch`).
    pub fn launched_with(&mut self, app: NdiApp, r: avlt_core::Result<PathBuf>) {
        self.launched = Some(
            r.map(|p| (app.name().to_string(), p))
                .map_err(|e| e.to_string()),
        );
    }

    /// Advances every job of the section (called each frame by the app).
    pub fn poll(&mut self) {
        self.check.poll();
        self.services.poll();
        self.config.poll();
    }

    pub fn ui(&mut self, ui: &mut Ui, cx: &mut Cx<'_>) {
        theme::heading(ui, "NDI");
        tabs(
            ui,
            &mut self.tab,
            &[
                (Tab::Launch, "Tools"),
                (Tab::Check, "Network check"),
                (Tab::Services, "Services"),
                (Tab::Config, "Access Manager"),
            ],
        );
        match self.tab {
            Tab::Launch => self.launch_tab(ui),
            Tab::Check => {
                if widgets::run_bar(ui, &mut self.check, "Run diagnostic") {
                    self.check.start(cx.runner.run(|_| ndi::check()));
                }
                if let Some(c) = self.check.done() {
                    for n in &c.notes {
                        widgets::error(ui, n);
                    }
                    widgets::result_view(ui, "ndi-check", c);
                }
            }
            Tab::Services => {
                if widgets::run_bar(ui, &mut self.services, "List services and processes") {
                    self.services.start(cx.runner.run(|_| ndi::services()));
                }
                if let Some(s) = self.services.done() {
                    widgets::result_view(ui, "ndi-services", s);
                }
            }
            Tab::Config => {
                if widgets::run_bar(ui, &mut self.config, "Read configuration") {
                    self.config.start(cx.runner.run(|_| ndi::config()));
                }
                if let Some(c) = self.config.done() {
                    widgets::result_view(ui, "ndi-config", c);
                }
            }
        }
    }

    fn launch_tab(&mut self, ui: &mut Ui) {
        let install = self.install.get_or_insert_with(ndi::install);
        if install.installed {
            let version = install
                .version
                .map_or_else(String::new, |v| format!(" (NDI {v})"));
            widgets::ok(
                ui,
                &format!("NDI Tools: {}{version}", install.base.display()),
            );
        } else {
            widgets::error(
                ui,
                &format!(
                    "NDI Tools not found (looked for {})",
                    install.base.display()
                ),
            );
        }
        theme::dim(ui, "Set AVLT_NDI_ROOT to look in another folder.");
        ui.add_space(6.0);
        let mut start = None;
        egui::Grid::new("ndi-apps").striped(true).show(ui, |ui| {
            for app in NdiApp::ALL {
                if ui.button(app.name()).clicked() {
                    start = Some(app);
                }
                theme::dim(ui, app.description());
                ui.end_row();
            }
        });
        if let Some(app) = start {
            let r = ndi::launch(app);
            self.launched_with(app, r);
        }
        match &self.launched {
            Some(Ok((app, path))) => {
                widgets::ok(ui, &format!("Launched: {app} ({})", path.display()));
            }
            Some(Err(e)) => widgets::error(ui, e),
            None => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::task::Status;
    use crate::gui::task::tests::{runner, wait};

    #[test]
    fn check_goes_running_then_failed_or_idle_on_cancel() {
        let rt = runner();
        let mut page = NdiPage::default();
        page.check
            .start(rt.run(|_| Err(avlt_core::Error::Unsupported)));
        assert!(page.check.is_running());
        wait(&mut page.check);
        assert!(matches!(&page.check.status, Status::Failed(_)));

        page.check
            .start(rt.spawn(|_tx| std::future::pending::<()>()));
        page.check.cancel();
        assert!(matches!(page.check.status, Status::Idle));
    }

    #[test]
    fn launch_outcome_is_recorded() {
        let mut page = NdiPage::default();
        let app = NdiApp::ALL[0];
        page.launched_with(app, Err(avlt_core::Error::Other("not installed".into())));
        assert_eq!(page.launched, Some(Err("not installed".into())));
        page.launched_with(app, Ok(PathBuf::from("x.exe")));
        assert_eq!(
            page.launched,
            Some(Ok((app.name().to_string(), PathBuf::from("x.exe"))))
        );
    }
}
