//! System section.

use eframe::egui;

use super::{Cx, theme};

/// State of the System section.
#[derive(Default)]
pub struct SysPage {}

impl SysPage {
    pub fn ui(&mut self, ui: &mut egui::Ui, _cx: &mut Cx<'_>) {
        theme::heading(ui, "System");
    }
}
