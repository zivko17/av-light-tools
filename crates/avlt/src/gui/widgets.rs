//! Widgets every section shares: the result viewer with "Copy as text" / "Copy as JSON",
//! the run / cancel bar, the confirmation dialog, the administrator notice and the graph.

use eframe::egui::{self, Color32, RichText, Ui};
use serde::Serialize;
use serde_json::Value;

use super::task::{HISTORY, Status, Task};
use super::{Cx, theme};

// ---------------------------------------------------------------- values as text

/// A number as the GUI shows it: integers as is, others with at most 3 decimals.
pub fn number(n: &serde_json::Number) -> String {
    if let Some(i) = n.as_i64() {
        return i.to_string();
    }
    if let Some(u) = n.as_u64() {
        return u.to_string();
    }
    let f = n.as_f64().unwrap_or(0.0);
    let s = format!("{f:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".into() } else { s.into() }
}

/// A scalar as one line of text; containers as compact JSON.
pub fn scalar(v: &Value) -> String {
    match v {
        Value::Null => "-".into(),
        Value::Bool(b) => if *b { "yes" } else { "no" }.into(),
        Value::Number(n) => number(n),
        Value::String(s) => s.clone(),
        Value::Array(a) if a.iter().all(is_scalar) => {
            if a.is_empty() {
                "(none)".into()
            } else {
                a.iter().map(scalar).collect::<Vec<_>>().join(", ")
            }
        }
        other => other.to_string(),
    }
}

fn is_scalar(v: &Value) -> bool {
    !matches!(v, Value::Array(_) | Value::Object(_))
}

/// `snake_case` field names as labels: `max_watt` -> `Max watt`.
pub fn label(key: &str) -> String {
    let s = key.replace('_', " ");
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect(),
        None => s,
    }
}

/// The plain-text rendering used by "Copy as text": `Label: value`, nested values
/// indented, list items with `-`.
pub fn value_text(v: &Value) -> String {
    let mut out = String::new();
    write_value(&mut out, v, 0);
    out
}

fn write_value(out: &mut String, v: &Value, depth: usize) {
    let pad = "  ".repeat(depth);
    match v {
        Value::Object(map) => {
            for (k, v) in map {
                if is_scalar(v) || matches!(v, Value::Array(a) if a.iter().all(is_scalar)) {
                    out.push_str(&format!("{pad}{}: {}\n", label(k), scalar(v)));
                } else {
                    out.push_str(&format!("{pad}{}:\n", label(k)));
                    write_value(out, v, depth + 1);
                }
            }
        }
        Value::Array(items) => {
            if items.is_empty() {
                out.push_str(&format!("{pad}(none)\n"));
            }
            for item in items {
                if is_scalar(item) {
                    out.push_str(&format!("{pad}- {}\n", scalar(item)));
                } else {
                    out.push_str(&format!("{pad}-\n"));
                    write_value(out, item, depth + 1);
                }
            }
        }
        scalar_value => out.push_str(&format!("{pad}{}\n", scalar(scalar_value))),
    }
}

/// Columns of a table of objects: every key, in order of first appearance.
pub fn columns(rows: &[Value]) -> Vec<String> {
    let mut cols: Vec<String> = Vec::new();
    for r in rows {
        if let Value::Object(m) = r {
            for k in m.keys() {
                if !cols.iter().any(|c| c == k) {
                    cols.push(k.clone());
                }
            }
        }
    }
    cols
}

// ---------------------------------------------------------------- result viewer

/// Serializes `value` for display; a failure shows as a string.
pub fn to_value<T: Serialize>(value: &T) -> Value {
    serde_json::to_value(value).unwrap_or_else(|e| Value::String(format!("cannot show: {e}")))
}

/// "Copy as text" and "Copy as JSON" buttons for `value`.
pub fn copy_buttons(ui: &mut Ui, value: &Value) {
    if ui.button("Copy as text").clicked() {
        ui.ctx().copy_text(value_text(value));
    }
    if ui.button("Copy as JSON").clicked() {
        let json = serde_json::to_string_pretty(value).unwrap_or_default();
        ui.ctx().copy_text(json);
    }
}

/// A result: copy buttons, then the value as a tree / table in a scroll area.
pub fn result_view<T: Serialize>(ui: &mut Ui, id: &str, value: &T) {
    let v = to_value(value);
    ui.horizontal(|ui| copy_buttons(ui, &v));
    ui.separator();
    egui::ScrollArea::both()
        .id_salt(id)
        .auto_shrink([false, false])
        .show(ui, |ui| show_value(ui, id, &v));
}

/// Draws any value: objects as a key / value grid, lists of objects as tables.
pub fn show_value(ui: &mut Ui, id: &str, v: &Value) {
    match v {
        Value::Object(map) => {
            egui::Grid::new(id)
                .num_columns(2)
                .striped(true)
                .spacing([16.0, 4.0])
                .show(ui, |ui| {
                    for (k, val) in map {
                        if is_scalar(val)
                            || matches!(val, Value::Array(a) if a.iter().all(is_scalar))
                        {
                            ui.label(RichText::new(label(k)).color(theme::TEXT_DIM));
                            ui.label(scalar(val));
                            ui.end_row();
                        }
                    }
                });
            for (k, val) in map {
                if !(is_scalar(val) || matches!(val, Value::Array(a) if a.iter().all(is_scalar))) {
                    let child = format!("{id}/{k}");
                    egui::CollapsingHeader::new(RichText::new(label(k)).color(theme::WARN))
                        .id_salt(&child)
                        .default_open(true)
                        .show(ui, |ui| show_value(ui, &child, val));
                }
            }
        }
        Value::Array(rows) if !rows.is_empty() && rows.iter().all(Value::is_object) => {
            table(ui, id, rows);
        }
        Value::Array(items) => {
            if items.is_empty() {
                theme::dim(ui, "(none)");
            }
            for (i, item) in items.iter().enumerate() {
                if is_scalar(item) {
                    ui.label(scalar(item));
                } else {
                    show_value(ui, &format!("{id}/{i}"), item);
                }
            }
        }
        other => {
            ui.label(scalar(other));
        }
    }
}

/// A table of objects; nested values show as compact JSON.
pub fn table(ui: &mut Ui, id: &str, rows: &[Value]) {
    let cols = columns(rows);
    egui::Grid::new(id)
        .num_columns(cols.len())
        .striped(true)
        .spacing([14.0, 4.0])
        .show(ui, |ui| {
            for c in &cols {
                ui.label(RichText::new(label(c)).strong().color(theme::ACCENT));
            }
            ui.end_row();
            for r in rows {
                for c in &cols {
                    ui.label(r.get(c).map(scalar).unwrap_or_default());
                }
                ui.end_row();
            }
        });
}

// ---------------------------------------------------------------- run / cancel

/// The run button of a one-shot task, or its progress and Cancel while it runs.
/// Returns `true` when the user clicked `label` (the caller starts the task).
pub fn run_bar<T>(ui: &mut Ui, task: &mut Task<T>, label: &str) -> bool {
    let mut start = false;
    ui.horizontal(|ui| {
        if let Status::Running { done, total, note } = &task.status {
            ui.spinner();
            let text = match (*total, note.is_empty()) {
                (0, true) => "Working...".to_string(),
                (0, false) => note.clone(),
                (t, _) => format!("{done} / {t}  {note}"),
            };
            ui.label(text);
            if *total > 0 {
                ui.add(
                    egui::ProgressBar::new(*done as f32 / *total as f32)
                        .desired_width(160.0)
                        .show_percentage(),
                );
            }
            if ui.button("Cancel").clicked() {
                task.cancel();
            }
        } else {
            start = theme::accent_button(ui, label).clicked();
        }
    });
    if let Status::Failed(e) = &task.status {
        error(ui, e);
    }
    start
}

/// An error line.
pub fn error(ui: &mut Ui, msg: &str) {
    ui.label(RichText::new(msg).color(theme::BAD));
}

/// A success line.
pub fn ok(ui: &mut Ui, msg: &str) {
    ui.label(RichText::new(msg).color(theme::GOOD));
}

// ---------------------------------------------------------------- confirmation

/// A pending yes / no question guarding a destructive action `A`.
pub struct Confirm<A> {
    pending: Option<(String, A)>,
}

impl<A> Default for Confirm<A> {
    fn default() -> Self {
        Confirm { pending: None }
    }
}

impl<A> Confirm<A> {
    /// Opens the dialog for `action`.
    pub fn ask(&mut self, question: impl Into<String>, action: A) {
        self.pending = Some((question.into(), action));
    }

    pub fn is_open(&self) -> bool {
        self.pending.is_some()
    }

    /// Closes the dialog; the action comes back only on yes.
    pub fn answer(&mut self, yes: bool) -> Option<A> {
        let (_, action) = self.pending.take()?;
        yes.then_some(action)
    }

    /// Draws the modal; returns the action once the user confirms it.
    pub fn show(&mut self, ctx: &egui::Context) -> Option<A> {
        if !self.is_open() {
            return None;
        }
        let (question, _) = self.pending.as_ref()?;
        let question = question.clone();
        let mut answer = None;
        let modal = egui::Modal::new(egui::Id::unique("avlt-confirm")).show(ctx, |ui| {
            ui.set_max_width(420.0);
            theme::section(ui, "Confirm");
            ui.label(question);
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Yes, continue").clicked() {
                    answer = Some(true);
                }
                if ui.button("Cancel").clicked() {
                    answer = Some(false);
                }
            });
        });
        if modal.should_close() && answer.is_none() {
            answer = Some(false);
        }
        answer.and_then(|yes| self.answer(yes))
    }
}

// ---------------------------------------------------------------- administrator

/// For actions that need administrator rights: `true` when elevated; otherwise shows
/// the notice and a "Relaunch as administrator" button and returns `false`.
pub fn admin_gate(ui: &mut Ui, cx: &mut Cx<'_>) -> bool {
    if cx.admin {
        return true;
    }
    egui::Frame::new()
        .fill(theme::CARD)
        .stroke(egui::Stroke::new(1.0, theme::WARN))
        .inner_margin(8.0)
        .show(ui, |ui| {
            ui.label(RichText::new("This action needs administrator rights.").color(theme::WARN));
            if ui.button("Relaunch as administrator").clicked() {
                match avlt_platform::admin::relaunch_elevated(&["gui"]) {
                    Ok(()) => ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close),
                    Err(e) => *cx.notice = Some(format!("Could not relaunch: {e}")),
                }
            }
        });
    false
}

// ---------------------------------------------------------------- graph

/// One line of a [`graph`]: name, colour and up to [`HISTORY`] values (`None` = gap).
pub struct Series<'a> {
    pub name: &'a str,
    pub colour: Color32,
    pub values: Vec<Option<f64>>,
}

/// A line graph after `NetMaint/GraphPanel.cs`: dark background, four grid lines
/// labelled with `fmt`, newest sample on the right, legend top-left.
pub fn graph(ui: &mut Ui, series: &[Series<'_>], fmt: fn(f64) -> String, height: f32) {
    let width = ui.available_width().max(200.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 3.0, theme::INPUT_BG);
    let max = series
        .iter()
        .flat_map(|s| s.values.iter().flatten())
        .fold(0.0_f64, |m, v| m.max(*v));
    let max = if max <= 0.0 { 1.0 } else { max * 1.15 };
    let area = rect.shrink2(egui::vec2(6.0, 14.0));
    let small = egui::FontId::proportional(11.0);
    for i in 0..=4 {
        let y = area.bottom() - area.height() * i as f32 / 4.0;
        p.line_segment(
            [egui::pos2(area.left(), y), egui::pos2(area.right(), y)],
            egui::Stroke::new(1.0, theme::GRID),
        );
        if i > 0 {
            p.text(
                egui::pos2(area.left() + 2.0, y - 1.0),
                egui::Align2::LEFT_BOTTOM,
                fmt(max * f64::from(i) / 4.0),
                small.clone(),
                theme::TEXT_DIM,
            );
        }
    }
    let dx = area.width() / (HISTORY - 1) as f32;
    for s in series {
        let offset = HISTORY.saturating_sub(s.values.len());
        let mut run: Vec<egui::Pos2> = Vec::new();
        let flush = |run: &mut Vec<egui::Pos2>| {
            if run.len() >= 2 {
                p.add(egui::Shape::line(
                    std::mem::take(run),
                    egui::Stroke::new(1.8, s.colour),
                ));
            }
            run.clear();
        };
        for (i, v) in s.values.iter().enumerate() {
            match v {
                Some(v) => {
                    let x = area.left() + (offset + i) as f32 * dx;
                    let y = area.bottom() - (*v / max) as f32 * area.height();
                    run.push(egui::pos2(x, y));
                }
                None => flush(&mut run),
            }
        }
        flush(&mut run);
    }
    let mut x = area.right() - 6.0;
    for s in series.iter().rev() {
        let galley = p.layout_no_wrap(s.name.to_string(), small.clone(), s.colour);
        x -= galley.size().x;
        p.galley(egui::pos2(x, rect.top() + 2.0), galley, s.colour);
        x -= 12.0;
        p.rect_filled(
            egui::Rect::from_min_size(egui::pos2(x, rect.top() + 5.0), egui::vec2(8.0, 8.0)),
            1.0,
            s.colour,
        );
        x -= 10.0;
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn numbers_and_labels() {
        assert_eq!(scalar(&json!(3)), "3");
        assert_eq!(scalar(&json!(1.23456)), "1.235");
        assert_eq!(scalar(&json!(2.5)), "2.5");
        assert_eq!(scalar(&json!(4.0)), "4");
        assert_eq!(scalar(&json!(null)), "-");
        assert_eq!(scalar(&json!(true)), "yes");
        assert_eq!(scalar(&json!(["a", "b"])), "a, b");
        assert_eq!(label("max_watt"), "Max watt");
    }

    #[test]
    fn text_copy_nests_objects_and_lists() {
        let v = json!({"cidr": "10.0.0.0/24", "hosts": 254, "ips": ["a", "b"],
                       "power": {"watts": 10.5}, "rows": [{"ip": "1"}]});
        assert_eq!(
            value_text(&v),
            "Cidr: 10.0.0.0/24\nHosts: 254\nIps: a, b\nPower:\n  Watts: 10.5\nRows:\n  -\n    Ip: 1\n"
        );
    }

    #[test]
    fn table_columns_in_first_appearance_order() {
        let rows = [json!({"a": 1, "b": 2}), json!({"a": 1, "c": 3})];
        assert_eq!(columns(&rows), ["a", "b", "c"]);
    }

    #[test]
    fn confirm_returns_the_action_only_on_yes() {
        let mut c: Confirm<u32> = Confirm::default();
        assert!(!c.is_open());
        c.ask("Kill?", 5);
        assert!(c.is_open());
        assert_eq!(c.answer(false), None);
        assert!(!c.is_open());
        c.ask("Kill?", 6);
        assert_eq!(c.answer(true), Some(6));
        assert_eq!(c.answer(true), None);
    }
}
