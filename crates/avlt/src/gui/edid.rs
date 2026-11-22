//! EDID section (read-only): connected displays, decode, check, compare and dump.
//! Nothing here writes an EDID or touches the display driver.

use std::path::{Path, PathBuf};

use avlt_core::edid::{self, Comparison, Finding, Severity};
use avlt_platform::edid_source::{self, DisplayEntry, DumpOut, ResolvedEdid};
use avlt_platform::paths::KnownFolder;
use eframe::egui::{self, RichText, Ui};

use super::task::{Status, Task};
use super::widgets::{self, to_value};
use super::{Cx, tabs, theme};
use crate::cli::env;

/// Where an EDID comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// Display number, as `avlt edid list` numbers them.
    Display(usize),
    File(PathBuf),
}

impl Source {
    fn label(&self) -> String {
        match self {
            Source::Display(n) => format!("display {n}"),
            Source::File(p) => p.display().to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Info,
    Check,
    Compare,
    Dump,
}

/// State of the EDID section.
pub struct EdidPage {
    pub view: View,
    pub remembered: bool,
    /// The display list (WMI / registry, read in the background).
    pub displays: Task<Vec<DisplayEntry>>,
    /// The EDID shown (side A).
    pub a: Option<(Source, Result<ResolvedEdid, String>)>,
    /// Side B of the comparison.
    pub b: Option<(Source, Result<ResolvedEdid, String>)>,
    /// Text of the "open file" box.
    pub path: String,
    pub dumped: Option<Result<Vec<DumpOut>, String>>,
    loaded_once: bool,
}

impl Default for EdidPage {
    fn default() -> Self {
        EdidPage {
            view: View::Info,
            remembered: false,
            displays: Task::default(),
            a: None,
            b: None,
            path: String::new(),
            dumped: None,
            loaded_once: false,
        }
    }
}

impl EdidPage {
    /// Decodes `source` from the current display list or from disk.
    pub fn resolve(&self, source: &Source) -> Result<ResolvedEdid, String> {
        match source {
            Source::Display(n) => {
                let list = self
                    .displays
                    .done()
                    .ok_or_else(|| "the display list is not loaded".to_string())?;
                let entry = list
                    .iter()
                    .find(|e| e.device.index == *n)
                    .ok_or_else(|| format!("no display with index {n}"))?;
                edid_source::from_device(entry.device.clone()).map_err(|e| e.to_string())
            }
            Source::File(p) => edid_source::resolve_file(p).map_err(|e| e.to_string()),
        }
    }

    /// Shows `source` (side A).
    pub fn select(&mut self, source: Source) {
        let r = self.resolve(&source);
        self.a = Some((source, r));
        self.dumped = None;
    }

    /// Sets side B of the comparison.
    pub fn select_b(&mut self, source: Source) {
        let r = self.resolve(&source);
        self.b = Some((source, r));
    }

    /// The comparison of A and B, when both decoded.
    pub fn comparison(&self) -> Option<Comparison> {
        match (&self.a, &self.b) {
            (Some((_, Ok(x))), Some((_, Ok(y)))) => {
                Some(edid::compare(&x.name, &x.edid, &y.name, &y.edid))
            }
            _ => None,
        }
    }

    fn refresh(&mut self, cx: &Cx<'_>) {
        let remembered = self.remembered;
        self.displays
            .start(cx.runner.run(move |_| edid_source::entries(remembered)));
        self.loaded_once = true;
    }

    /// Saves the EDIDs of `targets` into `<output dir>\EDID` (never overwrites).
    fn dump(&mut self, targets: &[ResolvedEdid]) {
        let r = env::output_dir(KnownFolder::Documents)
            .map(|d| d.join("EDID"))
            .and_then(|dir| {
                std::fs::create_dir_all(&dir)?;
                Ok(edid_source::dump_all(targets, &dir).0)
            })
            .map_err(|e| e.to_string());
        self.dumped = Some(r);
    }

    /// Advances the display-list job (called each frame by the app).
    pub fn poll(&mut self) {
        self.displays.poll();
    }

    pub fn ui(&mut self, ui: &mut Ui, cx: &mut Cx<'_>) {
        if !self.loaded_once {
            self.refresh(cx);
        }
        if let Some(p) = cx.dropped.first() {
            self.path = p.display().to_string();
            self.select(Source::File(p.clone()));
        }

        theme::heading(ui, "EDID");
        theme::dim(
            ui,
            "Read-only. To force an EDID use CRU or a hardware EDID emulator.",
        );
        egui::Panel::left("edid-sources")
            .resizable(true)
            .default_size(330.0)
            .show(ui, |ui| self.sources(ui, cx));
        egui::CentralPanel::default_margins().show(ui, |ui| {
            tabs(
                ui,
                &mut self.view,
                &[
                    (View::Info, "Info"),
                    (View::Check, "Check"),
                    (View::Compare, "Compare"),
                    (View::Dump, "Dump"),
                ],
            );
            match self.view {
                View::Info => self.info(ui),
                View::Check => self.check(ui),
                View::Compare => self.compare(ui),
                View::Dump => self.dump_view(ui),
            }
        });
    }

    fn sources(&mut self, ui: &mut Ui, cx: &mut Cx<'_>) {
        ui.horizontal(|ui| {
            if widgets::run_bar(ui, &mut self.displays, "Refresh displays") {
                self.refresh(cx);
            }
            ui.checkbox(&mut self.remembered, "Remembered");
        });
        let mut pick = None;
        let mut pick_b = None;
        if let Some(list) = self.displays.done() {
            if list.is_empty() {
                widgets::error(
                    ui,
                    "No displays found (some remote sessions expose no EDID).",
                );
            }
            egui::ScrollArea::vertical()
                .id_salt("edid-list")
                .max_height(ui.available_height() - 90.0)
                .show(ui, |ui| {
                    for d in list {
                        let selected =
                            matches!(&self.a, Some((Source::Display(n), _)) if *n == d.device.index);
                        let title = match (&d.manufacturer, &d.model) {
                            (Some(m), Some(model)) => format!("{}  {m} {model}", d.device.index),
                            _ => format!(
                                "{}  EDID unavailable ({})",
                                d.device.index,
                                edid_source::source_name(d.device.source)
                            ),
                        };
                        let mut detail = d.preferred.clone().unwrap_or_default();
                        if !d.caps.is_empty() {
                            detail.push_str(&format!("  {}", d.caps.join(", ")));
                        }
                        if !d.device.active {
                            detail.push_str("  [not connected]");
                        }
                        let r = ui.selectable_label(
                            selected,
                            format!("{title}\n{detail}"),
                        );
                        if r.clicked() {
                            pick = Some(Source::Display(d.device.index));
                        }
                        r.context_menu(|ui| {
                            if ui.button("Compare with this (B)").clicked() {
                                pick_b = Some(Source::Display(d.device.index));
                                ui.close();
                            }
                        });
                    }
                });
            ui.horizontal(|ui| {
                widgets::copy_buttons(ui, &to_value(list));
            });
        }
        if let Some(s) = pick {
            self.select(s);
        }
        if let Some(s) = pick_b {
            self.select_b(s);
            self.view = View::Compare;
        }
        ui.separator();
        ui.label("Open a .bin file (or drop it on the window):");
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut self.path).desired_width(220.0));
            if ui.button("Open").clicked() {
                let p = PathBuf::from(self.path.trim().trim_matches('"'));
                self.select(Source::File(p));
            }
        });
    }

    fn current(&self, ui: &mut Ui) -> Option<&ResolvedEdid> {
        match &self.a {
            None => {
                theme::dim(ui, "Pick a display or open a .bin file.");
                None
            }
            Some((_, Err(e))) => {
                widgets::error(ui, e);
                None
            }
            Some((_, Ok(r))) => Some(r),
        }
    }

    fn info(&self, ui: &mut Ui) {
        let Some(r) = self.current(ui) else { return };
        theme::section(ui, &r.name);
        for p in &r.edid.problems {
            widgets::error(ui, p);
        }
        let value = serde_json::json!({ "source": r.name, "edid": to_value(&r.edid) });
        widgets::result_view(ui, "edid-info", &value);
    }

    fn check(&self, ui: &mut Ui) {
        let Some(r) = self.current(ui) else { return };
        let findings = edid::check(&r.edid);
        theme::section(ui, &format!("Check of {}", r.name));
        findings_view(ui, &findings);
        ui.separator();
        let value = serde_json::json!([{ "source": r.name, "findings": to_value(&findings) }]);
        ui.horizontal(|ui| widgets::copy_buttons(ui, &value));
    }

    fn compare(&mut self, ui: &mut Ui) {
        theme::dim(ui, "A is the EDID picked on the left; choose B below.");
        ui.horizontal(|ui| {
            ui.label(format!(
                "A: {}",
                self.a.as_ref().map_or("(none)".into(), |(s, _)| s.label())
            ));
        });
        let mut pick_b = None;
        ui.horizontal(|ui| {
            ui.label("B:");
            let current = self
                .b
                .as_ref()
                .map_or("(choose)".into(), |(s, _)| s.label());
            egui::ComboBox::from_id_salt("edid-b")
                .selected_text(current)
                .show_ui(ui, |ui| {
                    for d in self.displays.done().into_iter().flatten() {
                        if ui
                            .selectable_label(false, format!("display {}", d.device.index))
                            .clicked()
                        {
                            pick_b = Some(Source::Display(d.device.index));
                        }
                    }
                });
            if ui.button("B from file box").clicked() {
                pick_b = Some(Source::File(PathBuf::from(
                    self.path.trim().trim_matches('"'),
                )));
            }
        });
        if let Some(s) = pick_b {
            self.select_b(s);
        }
        if let Some((_, Err(e))) = &self.b {
            widgets::error(ui, &format!("(B) {e}"));
        }
        let Some(c) = self.comparison() else { return };
        if c.differences.is_empty() {
            widgets::ok(ui, "Identical.");
        } else {
            ui.label(
                RichText::new(format!("{} differences", c.differences.len())).color(theme::WARN),
            );
        }
        let v = to_value(&c);
        ui.horizontal(|ui| widgets::copy_buttons(ui, &v));
        egui::ScrollArea::vertical()
            .id_salt("edid-compare")
            .show(ui, |ui| {
                egui::Grid::new("edid-fields")
                    .striped(true)
                    .num_columns(3)
                    .show(ui, |ui| {
                        ui.label(RichText::new("Field").strong().color(theme::ACCENT));
                        ui.label(RichText::new(&c.a).strong().color(theme::ACCENT));
                        ui.label(RichText::new(&c.b).strong().color(theme::ACCENT));
                        ui.end_row();
                        for f in &c.fields {
                            let colour = if f.same { theme::TEXT } else { theme::WARN };
                            ui.label(&f.field);
                            ui.label(RichText::new(&f.a).color(colour));
                            ui.label(RichText::new(&f.b).color(colour));
                            ui.end_row();
                        }
                    });
                theme::section(ui, "Differences");
                widgets::show_value(ui, "edid-diffs", &to_value(&c.differences));
            });
    }

    fn dump_view(&mut self, ui: &mut Ui) {
        theme::dim(
            ui,
            "Saves .bin files to <Documents>\\AVLT\\EDID (or _OUTPUT when portable). Never overwrites.",
        );
        ui.horizontal(|ui| {
            let current = match &self.a {
                Some((_, Ok(r))) => Some(r.clone()),
                _ => None,
            };
            if ui
                .add_enabled(
                    current.is_some(),
                    egui::Button::new("Save the selected EDID"),
                )
                .clicked()
                && let Some(r) = current
            {
                self.dump(&[r]);
            }
            let any = self
                .displays
                .done()
                .is_some_and(|l| l.iter().any(|d| !d.device.edid.is_empty()));
            if ui
                .add_enabled(any, egui::Button::new("Save every display"))
                .clicked()
            {
                // Decoded only on click, not every frame.
                let all: Vec<ResolvedEdid> = self
                    .displays
                    .done()
                    .into_iter()
                    .flatten()
                    .filter_map(|d| edid_source::from_device(d.device.clone()).ok())
                    .collect();
                self.dump(&all);
            }
        });
        match &self.dumped {
            None => {}
            Some(Err(e)) => widgets::error(ui, e),
            Some(Ok(list)) => {
                for d in list {
                    match (&d.path, &d.error) {
                        (Some(p), _) => widgets::ok(
                            ui,
                            &format!("{} -> {}  ({} bytes)", d.source, show_path(p), d.bytes),
                        ),
                        (None, e) => widgets::error(
                            ui,
                            &format!("{}: {}", d.source, e.as_deref().unwrap_or("failed")),
                        ),
                    }
                }
                ui.horizontal(|ui| widgets::copy_buttons(ui, &to_value(list)));
            }
        }
        if let Status::Failed(e) = &self.displays.status {
            widgets::error(ui, e);
        }
    }
}

fn show_path(p: &Path) -> String {
    p.display().to_string()
}

/// Findings coloured by severity.
fn findings_view(ui: &mut Ui, findings: &[Finding]) {
    for f in findings {
        let (tag, colour) = match f.severity {
            Severity::Ok => ("OK   ", theme::GOOD),
            Severity::Warn => ("WARN ", theme::WARN),
            Severity::Error => ("ERROR", theme::BAD),
        };
        ui.label(RichText::new(format!("[{tag}] {}", f.message)).color(colour));
    }
}

#[cfg(test)]
mod tests {
    use avlt_platform::edid_source::{DisplayDevice, EdidSource};

    use super::*;
    use crate::gui::task::Msg;
    use crate::gui::task::tests::{runner, wait};

    const CTA: &[u8] = include_bytes!("../../../avlt-core/tests/fixtures/edid/synthetic_cta.bin");

    fn device(index: usize, edid: &[u8]) -> DisplayEntry {
        DisplayEntry::new(DisplayDevice {
            index,
            name: format!("D{index}"),
            instance_id: format!("DISPLAY\\X\\{index}"),
            active: true,
            source: EdidSource::Registry,
            edid: edid.to_vec(),
        })
    }

    fn page_with(list: Vec<DisplayEntry>) -> EdidPage {
        let rt = runner();
        let mut page = EdidPage::default();
        page.displays.start(rt.run(move |_| Ok(list)));
        wait(&mut page.displays);
        page
    }

    #[test]
    fn listing_then_selecting_a_display_decodes_it() {
        let mut page = page_with(vec![device(1, CTA), device(2, &[])]);
        assert_eq!(page.displays.done().map(Vec::len), Some(2));

        page.select(Source::Display(1));
        let (_, r) = page.a.as_ref().unwrap();
        let r = r.as_ref().unwrap();
        assert_eq!(r.name, "display 1");
        assert!(r.edid.base.is_some());

        page.select(Source::Display(2));
        let err = page.a.as_ref().unwrap().1.as_ref().unwrap_err();
        assert!(err.contains("reported no EDID"), "{err}");

        page.select(Source::Display(9));
        assert!(page.a.as_ref().unwrap().1.is_err());
    }

    #[test]
    fn compare_needs_both_sides() {
        let mut page = page_with(vec![device(1, CTA), device(2, CTA)]);
        page.select(Source::Display(1));
        assert!(page.comparison().is_none());
        page.select_b(Source::Display(2));
        let c = page.comparison().unwrap();
        assert!(c.differences.is_empty());
    }

    #[test]
    fn missing_file_is_an_error_not_a_panic() {
        let mut page = EdidPage::default();
        page.select(Source::File("no/such/edid.bin".into()));
        assert!(page.a.as_ref().unwrap().1.is_err());
        page.displays.update(Msg::Finished(Ok(Vec::new())));
        assert!(
            matches!(page.displays.status, Status::Idle),
            "not running: ignored"
        );
    }
}
