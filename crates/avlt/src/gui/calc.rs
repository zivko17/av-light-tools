//! Calculators section.

use eframe::egui;

use super::{Cx, theme};

/// State of the Calculators section.
#[derive(Default)]
pub struct CalcPage {}

impl CalcPage {
    pub fn ui(&mut self, ui: &mut egui::Ui, _cx: &mut Cx<'_>) {
        theme::heading(ui, "Calculators");
    }
}
