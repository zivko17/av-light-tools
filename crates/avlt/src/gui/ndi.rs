//! NDI section.

use eframe::egui;

use super::{Cx, theme};

/// State of the NDI section.
#[derive(Default)]
pub struct NdiPage {}

impl NdiPage {
    pub fn ui(&mut self, ui: &mut egui::Ui, _cx: &mut Cx<'_>) {
        theme::heading(ui, "NDI");
    }
}
