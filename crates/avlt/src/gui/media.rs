//! Media section: the audio toolkit on top of ffmpeg / ffprobe (convert, normalize,
//! trim, extract, resample, channels, probe). Outputs never overwrite existing files.

use std::path::PathBuf;

use avlt_core::media::{
    self, Channels, ExtractFormat, MediaOp, MediaReport, Preset, ProbeInfo, SAMPLE_RATES,
};
use avlt_core::{Error, Result};
use avlt_platform::media as pmedia;
use avlt_platform::paths::KnownFolder;
use eframe::egui::{self, Ui};

use super::task::{Msg, Task};
use super::widgets;
use super::{Cx, theme};
use crate::cli::env;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpKind {
    Convert,
    Normalize,
    Trim,
    Extract,
    Resample,
    Channels,
    Probe,
}

impl OpKind {
    pub const ALL: [OpKind; 7] = [
        OpKind::Convert,
        OpKind::Normalize,
        OpKind::Trim,
        OpKind::Extract,
        OpKind::Resample,
        OpKind::Channels,
        OpKind::Probe,
    ];

    pub fn title(self) -> &'static str {
        match self {
            OpKind::Convert => "Convert",
            OpKind::Normalize => "Normalize (-23 LUFS)",
            OpKind::Trim => "Trim",
            OpKind::Extract => "Extract audio",
            OpKind::Resample => "Resample",
            OpKind::Channels => "Mono / stereo",
            OpKind::Probe => "Probe",
        }
    }
}

/// State of the Media section.
pub struct MediaPage {
    pub kind: OpKind,
    /// File, or folder (every audio / video file under it).
    pub input: String,
    /// Output folder; empty = `<Music>\AVLT` (or `_OUTPUT` when portable).
    pub out_dir: String,
    pub preset: Preset,
    pub convert_rate: Option<u32>,
    pub convert_channels: Option<Channels>,
    pub trim_start: String,
    pub trim_end: String,
    pub extract: ExtractFormat,
    pub rate: u32,
    pub channels: Channels,
    pub batch: Task<MediaReport>,
    pub probe: Task<ProbeInfo>,
    pub form_error: Option<String>,
}

impl Default for MediaPage {
    fn default() -> Self {
        MediaPage {
            kind: OpKind::Convert,
            input: String::new(),
            out_dir: String::new(),
            preset: Preset::Mp3,
            convert_rate: None,
            convert_channels: None,
            trim_start: "00:00:00".into(),
            trim_end: String::new(),
            extract: ExtractFormat::Mp3,
            rate: 48_000,
            channels: Channels::Stereo,
            batch: Task::default(),
            probe: Task::default(),
            form_error: None,
        }
    }
}

impl MediaPage {
    fn input_path(&self) -> PathBuf {
        media::strip_quotes(&PathBuf::from(self.input.trim()))
    }

    /// The operation the form describes (`None` for Probe), validated as the CLI does.
    pub fn build_op(&self) -> Result<Option<MediaOp>> {
        if self.input.trim().is_empty() {
            return Err(Error::InvalidInput("choose an input file or folder".into()));
        }
        Ok(Some(match self.kind {
            OpKind::Convert => MediaOp::Convert {
                preset: self.preset,
                sample_rate: self.convert_rate,
                channels: self.convert_channels,
            },
            OpKind::Normalize => MediaOp::Normalize,
            OpKind::Trim => {
                if self.input_path().is_dir() {
                    return Err(Error::InvalidInput(
                        "trim takes a single file, not a folder".into(),
                    ));
                }
                let end = self.trim_end.trim();
                MediaOp::Trim {
                    start: self.trim_start.trim().to_string(),
                    end: (!end.is_empty()).then(|| end.to_string()),
                }
            }
            OpKind::Extract => MediaOp::Extract(self.extract),
            OpKind::Resample => MediaOp::Resample(self.rate),
            OpKind::Channels => MediaOp::Channels(self.channels),
            OpKind::Probe => return Ok(None),
        }))
    }

    fn start(&mut self, cx: &Cx<'_>) {
        self.form_error = None;
        let op = match self.build_op() {
            Ok(op) => op,
            Err(e) => {
                self.form_error = Some(e.to_string());
                return;
            }
        };
        let input = self.input_path();
        let Some(op) = op else {
            self.probe
                .start(cx.runner.run(move |_| pmedia::probe(&input)));
            return;
        };
        let out_dir = Some(self.out_dir.trim())
            .filter(|s| !s.is_empty())
            .map(PathBuf::from);
        self.batch.start(cx.runner.run(move |tx| {
            let ffmpeg = pmedia::ffmpeg()?;
            let inputs = media::collect_inputs(&input)?;
            let dir = match out_dir {
                Some(d) => d,
                None => env::output_dir(KnownFolder::Music)?,
            };
            std::fs::create_dir_all(&dir)?;
            let total = inputs.len() as u64;
            let mut results = Vec::new();
            for (i, file) in inputs.iter().enumerate() {
                if tx.cancelled() {
                    return Err(Error::Other("cancelled".into()));
                }
                let name = file
                    .file_name()
                    .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
                tx.send(Msg::Progress {
                    done: i as u64,
                    total,
                    note: name,
                });
                let r = pmedia::run_job(&ffmpeg, &op, file, &dir, tx.cancel_flag(), &mut |_| {});
                results.push(r);
            }
            Ok(MediaReport::new(op.operation(), &dir, results))
        }));
    }

    pub fn ui(&mut self, ui: &mut Ui, cx: &mut Cx<'_>) {
        self.batch.poll();
        self.probe.poll();
        if let Some(p) = cx.dropped.first() {
            self.input = p.display().to_string();
        }
        theme::heading(ui, "Media");
        theme::dim(
            ui,
            "Needs ffmpeg / ffprobe (winget install Gyan.FFmpeg). Existing files are never overwritten.",
        );
        ui.horizontal_wrapped(|ui| {
            for k in OpKind::ALL {
                ui.selectable_value(&mut self.kind, k, k.title());
            }
        });
        ui.separator();
        egui::Grid::new("media-form").num_columns(2).show(ui, |ui| {
            ui.label("Input");
            ui.add(
                egui::TextEdit::singleline(&mut self.input)
                    .desired_width(440.0)
                    .hint_text("file or folder (or drop it on the window)"),
            );
            ui.end_row();
            if self.kind != OpKind::Probe {
                ui.label("Output folder");
                ui.add(
                    egui::TextEdit::singleline(&mut self.out_dir)
                        .desired_width(440.0)
                        .hint_text("default: <Music>\\AVLT"),
                );
                ui.end_row();
            }
            self.options(ui);
        });
        if let Some(e) = &self.form_error {
            widgets::error(ui, e);
        }
        ui.add_space(4.0);
        if self.kind == OpKind::Probe {
            if widgets::run_bar(ui, &mut self.probe, "Probe") {
                self.start(cx);
            }
            if let Some(p) = self.probe.done() {
                widgets::result_view(ui, "media-probe", p);
            }
        } else {
            if widgets::run_bar(ui, &mut self.batch, "Run") {
                self.start(cx);
            }
            if let Some(r) = self.batch.done() {
                let line = format!(
                    "Done: {} ok, {} failed -> {}",
                    r.ok,
                    r.failed,
                    r.output_dir.display()
                );
                if r.failed == 0 {
                    widgets::ok(ui, &line);
                } else {
                    widgets::error(ui, &line);
                }
                widgets::result_view(ui, "media-report", r);
            }
        }
    }

    fn options(&mut self, ui: &mut Ui) {
        match self.kind {
            OpKind::Convert => {
                ui.label("Preset");
                egui::ComboBox::from_id_salt("preset")
                    .selected_text(self.preset.label())
                    .show_ui(ui, |ui| {
                        for p in Preset::ALL {
                            ui.selectable_value(
                                &mut self.preset,
                                p,
                                format!("{}  {}", p.label(), p.description()),
                            );
                        }
                    });
                ui.end_row();
                ui.label("Sample rate");
                egui::ComboBox::from_id_salt("conv-rate")
                    .selected_text(
                        self.convert_rate
                            .map_or("keep".into(), |r| format!("{r} Hz")),
                    )
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.convert_rate, None, "keep");
                        for r in SAMPLE_RATES {
                            ui.selectable_value(&mut self.convert_rate, Some(r), format!("{r} Hz"));
                        }
                    });
                ui.end_row();
                ui.label("Channels");
                egui::ComboBox::from_id_salt("conv-ch")
                    .selected_text(self.convert_channels.map_or("keep", Channels::name))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.convert_channels, None, "keep");
                        for c in [Channels::Mono, Channels::Stereo] {
                            ui.selectable_value(&mut self.convert_channels, Some(c), c.name());
                        }
                    });
                ui.end_row();
            }
            OpKind::Trim => {
                ui.label("Start");
                ui.add(
                    egui::TextEdit::singleline(&mut self.trim_start)
                        .hint_text("HH:MM:SS or seconds"),
                );
                ui.end_row();
                ui.label("End");
                ui.add(
                    egui::TextEdit::singleline(&mut self.trim_end).hint_text("empty = end of file"),
                );
                ui.end_row();
            }
            OpKind::Extract => {
                ui.label("Format");
                egui::ComboBox::from_id_salt("extract")
                    .selected_text(self.extract.name())
                    .show_ui(ui, |ui| {
                        for f in [
                            ExtractFormat::Mp3,
                            ExtractFormat::Flac,
                            ExtractFormat::Wav,
                            ExtractFormat::Copy,
                        ] {
                            ui.selectable_value(&mut self.extract, f, f.name());
                        }
                    });
                ui.end_row();
            }
            OpKind::Resample => {
                ui.label("Rate");
                egui::ComboBox::from_id_salt("rate")
                    .selected_text(format!("{} Hz", self.rate))
                    .show_ui(ui, |ui| {
                        for r in SAMPLE_RATES {
                            ui.selectable_value(&mut self.rate, r, format!("{r} Hz"));
                        }
                    });
                ui.end_row();
            }
            OpKind::Channels => {
                ui.label("Layout");
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.channels, Channels::Mono, "mono");
                    ui.selectable_value(&mut self.channels, Channels::Stereo, "stereo");
                });
                ui.end_row();
            }
            OpKind::Normalize | OpKind::Probe => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::task::Status;
    use crate::gui::task::tests::runner;

    #[test]
    fn the_form_builds_the_cli_operation() {
        let mut page = MediaPage {
            input: "song.wav".into(),
            ..MediaPage::default()
        };
        assert!(matches!(
            page.build_op().unwrap(),
            Some(MediaOp::Convert {
                preset: Preset::Mp3,
                ..
            })
        ));
        page.kind = OpKind::Trim;
        page.trim_end = " ".into();
        assert_eq!(
            page.build_op().unwrap(),
            Some(MediaOp::Trim {
                start: "00:00:00".into(),
                end: None
            })
        );
        page.kind = OpKind::Probe;
        assert_eq!(page.build_op().unwrap(), None);
    }

    #[test]
    fn trim_refuses_a_folder_and_input_is_required() {
        let dir = tempfile::tempdir().unwrap();
        let mut page = MediaPage {
            kind: OpKind::Trim,
            input: dir.path().display().to_string(),
            ..MediaPage::default()
        };
        assert!(
            page.build_op()
                .unwrap_err()
                .to_string()
                .contains("single file")
        );
        page.input.clear();
        assert!(page.build_op().is_err());
    }

    #[test]
    fn cancelling_a_batch_returns_to_idle() {
        let rt = runner();
        let mut page = MediaPage::default();
        page.batch.start(rt.run(|tx| {
            while !tx.cancelled() {
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
            Err(Error::Other("cancelled".into()))
        }));
        page.batch.update(Msg::Progress {
            done: 1,
            total: 4,
            note: "b.wav".into(),
        });
        assert!(matches!(
            page.batch.status,
            Status::Running {
                done: 1,
                total: 4,
                ..
            }
        ));
        page.batch.cancel();
        assert!(matches!(page.batch.status, Status::Idle));
    }
}
