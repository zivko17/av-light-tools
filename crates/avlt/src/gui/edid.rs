//! EDID section.

use eframe::egui;

use super::{Cx, theme};

/// State of the EDID section.
#[derive(Default)]
pub struct EdidPage {}

impl EdidPage {
    pub fn ui(&mut self, ui: &mut egui::Ui, _cx: &mut Cx<'_>) {
        theme::heading(ui, "EDID");
    }
}
