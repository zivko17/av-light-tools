//! Media section.

use eframe::egui;

use super::{Cx, theme};

/// State of the Media section.
#[derive(Default)]
pub struct MediaPage {}

impl MediaPage {
    pub fn ui(&mut self, ui: &mut egui::Ui, _cx: &mut Cx<'_>) {
        theme::heading(ui, "Media");
    }
}
