//! Network section.

use eframe::egui;

use super::{Cx, theme};

/// State of the Network section.
#[derive(Default)]
pub struct NetPage {}

impl NetPage {
    pub fn ui(&mut self, ui: &mut egui::Ui, _cx: &mut Cx<'_>) {
        theme::heading(ui, "Network");
    }
}
