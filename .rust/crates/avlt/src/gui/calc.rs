//! Calculators: one form per calculator, built from its `XInput` struct (starting from
//! `XInput::default()`), recalculated whenever a value changes.

use avlt_core::calc::{
    audiobw::{self, AudioBwInput},
    avoip::{self, AvoipInput},
    bitrate::{self, BitrateInput},
    brightness::{self, BrightnessContent, BrightnessInput},
    cable::{self, CableInput, CableType},
    camera::{self, CameraInput},
    dsp::{self, DspInput},
    led_cabinets::led_cabinets,
    ledwall::{self, Distribution, LedWallInput, NOVA_RECV_CARDS, NOVA_SENDERS, PowerBasis},
    pixelpitch::{self, PixelPitchInput},
    projector::{self, ProjectorInput},
    screen::{self, Content, ScreenInput},
    speaker::{self, SpeakerInput},
    subnet,
    viewangle::{self, ViewAngleInput},
};
use eframe::egui::{self, Ui};
use serde::Serialize;
use serde_json::Value;

use super::widgets::{self, to_value};
use super::{Cx, theme};

/// The calculators, in sidebar order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Calc {
    Subnet,
    Screen,
    Ledwall,
    Bitrate,
    Avoip,
    Projector,
    Pixelpitch,
    Brightness,
    Viewangle,
    Camera,
    Audiobw,
    Speaker,
    Cable,
    Dsp,
}

impl Calc {
    pub const ALL: [Calc; 14] = [
        Calc::Subnet,
        Calc::Screen,
        Calc::Ledwall,
        Calc::Bitrate,
        Calc::Avoip,
        Calc::Projector,
        Calc::Pixelpitch,
        Calc::Brightness,
        Calc::Viewangle,
        Calc::Camera,
        Calc::Audiobw,
        Calc::Speaker,
        Calc::Cable,
        Calc::Dsp,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Calc::Subnet => "IPv4 subnet",
            Calc::Screen => "Screen size",
            Calc::Ledwall => "LED wall",
            Calc::Bitrate => "Video bitrate",
            Calc::Avoip => "AV-over-IP",
            Calc::Projector => "Projector",
            Calc::Pixelpitch => "Pixel pitch",
            Calc::Brightness => "Brightness",
            Calc::Viewangle => "Viewing angle",
            Calc::Camera => "Camera / DORI",
            Calc::Audiobw => "Audio bandwidth",
            Calc::Speaker => "Ceiling speakers",
            Calc::Cable => "Cable loss",
            Calc::Dsp => "DSP",
        }
    }

    /// The one-line description `avlt calc --help` shows.
    pub fn help(self) -> &'static str {
        match self {
            Calc::Subnet => "IPv4 subnet calculator (CIDR)",
            Calc::Screen => "Screen size from viewing distance (AVIXA 4/6/8:1), or the reverse",
            Calc::Ledwall => "LED wall size, resolution, power and NovaStar signal",
            Calc::Bitrate => "Uncompressed video bitrate, NDI estimate and storage",
            Calc::Avoip => "AV-over-IP bandwidth per codec",
            Calc::Projector => "Projector throw distance and brightness",
            Calc::Pixelpitch => "DvLED pixel pitch and viewing distance",
            Calc::Brightness => "Required display brightness versus ambient light",
            Calc::Viewangle => "Usable seating width from the off-axis viewing limit",
            Calc::Camera => "Camera field of view and DORI distances",
            Calc::Audiobw => "Digital audio data rate and wavelength",
            Calc::Speaker => "Ceiling speaker coverage and amplifier sizing",
            Calc::Cable => "Speaker wire loss and coax attenuation",
            Calc::Dsp => "dBu conversion, latency budget and matrix size",
        }
    }
}

/// `calc subnet` takes plain arguments rather than an input struct.
#[derive(Debug, Clone, PartialEq)]
pub struct SubnetForm {
    pub cidr: String,
    pub devices: u32,
    pub start_host: u32,
}

/// [`DspInput`] with its optional / list fields as editable text.
#[derive(Debug, Clone, PartialEq)]
pub struct DspForm {
    pub dbu: String,
    pub latencies_ms: String,
    pub inputs: u32,
    pub outputs: u32,
}

/// Every calculator's input, each starting from its `XInput::default()` with example
/// values for the fields the CLI requires.
#[derive(Debug, Clone, PartialEq)]
pub struct Inputs {
    pub subnet: SubnetForm,
    pub screen: ScreenInput,
    pub ledwall: LedWallInput,
    pub bitrate: BitrateInput,
    pub avoip: AvoipInput,
    pub projector: ProjectorInput,
    pub pixelpitch: PixelPitchInput,
    pub brightness: BrightnessInput,
    pub viewangle: ViewAngleInput,
    pub camera: CameraInput,
    pub audiobw: AudioBwInput,
    pub speaker: SpeakerInput,
    pub cable: CableInput,
    pub dsp: DspForm,
}

impl Default for Inputs {
    fn default() -> Self {
        Inputs {
            subnet: SubnetForm {
                cidr: "192.168.1.0/24".into(),
                devices: 0,
                start_host: 1,
            },
            screen: ScreenInput {
                distance: 6.0,
                ..ScreenInput::default()
            },
            ledwall: LedWallInput {
                cols: 10,
                rows: 5,
                cabinet: led_cabinets().first().map(|c| c.key.clone()),
                ..LedWallInput::default()
            },
            bitrate: BitrateInput {
                width: 1920,
                height: 1080,
                ..BitrateInput::default()
            },
            avoip: AvoipInput {
                width: 1920,
                height: 1080,
                ..AvoipInput::default()
            },
            projector: ProjectorInput {
                width: 3.0,
                throw_ratio: 1.5,
                ..ProjectorInput::default()
            },
            pixelpitch: PixelPitchInput {
                pitch: 2.6,
                ..PixelPitchInput::default()
            },
            brightness: BrightnessInput {
                ambient_lux: 300.0,
                ..BrightnessInput::default()
            },
            viewangle: ViewAngleInput {
                distance: 5.0,
                ..ViewAngleInput::default()
            },
            camera: CameraInput {
                focal_length: 4.0,
                ..CameraInput::default()
            },
            audiobw: AudioBwInput::default(),
            speaker: SpeakerInput {
                ceiling_height: 3.0,
                room_width: 10.0,
                room_length: 8.0,
                ..SpeakerInput::default()
            },
            cable: CableInput {
                length_m: 20.0,
                ..CableInput::default()
            },
            dsp: {
                let d = DspInput::default();
                DspForm {
                    dbu: "4".into(),
                    latencies_ms: String::new(),
                    inputs: d.inputs,
                    outputs: d.outputs,
                }
            },
        }
    }
}

fn ok<T: Serialize>(r: avlt_core::Result<T>) -> Result<Value, String> {
    r.map(|v| to_value(&v)).map_err(|e| e.to_string())
}

impl Inputs {
    /// Runs the calculator on its current input (the same core function as the CLI).
    pub fn compute(&self, calc: Calc) -> Result<Value, String> {
        match calc {
            Calc::Subnet => ok(subnet::subnet(
                &self.subnet.cidr,
                self.subnet.devices,
                self.subnet.start_host,
            )),
            Calc::Screen => ok(screen::screen(&self.screen)),
            Calc::Ledwall => ok(ledwall::ledwall(&self.ledwall)),
            Calc::Bitrate => ok(bitrate::bitrate(&self.bitrate)),
            Calc::Avoip => ok(avoip::avoip(&self.avoip)),
            Calc::Projector => ok(projector::projector(&self.projector)),
            Calc::Pixelpitch => ok(pixelpitch::pixelpitch(&self.pixelpitch)),
            Calc::Brightness => ok(brightness::brightness(&self.brightness)),
            Calc::Viewangle => ok(viewangle::viewangle(&self.viewangle)),
            Calc::Camera => ok(camera::camera(&self.camera)),
            Calc::Audiobw => ok(audiobw::audiobw(&self.audiobw)),
            Calc::Speaker => ok(speaker::speaker(&self.speaker)),
            Calc::Cable => ok(cable::cable(&self.cable)),
            Calc::Dsp => {
                let input = self.dsp_input()?;
                ok(dsp::dsp(&input))
            }
        }
    }

    fn dsp_input(&self) -> Result<DspInput, String> {
        let dbu = match self.dsp.dbu.trim() {
            "" => None,
            s => Some(
                s.parse::<f64>()
                    .map_err(|_| format!("dBu: '{s}' is not a number"))?,
            ),
        };
        let latencies_ms = self
            .dsp
            .latencies_ms
            .split([',', ' '])
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| {
                s.parse::<f64>()
                    .map_err(|_| format!("latency: '{s}' is not a number"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(DspInput {
            dbu,
            latencies_ms,
            inputs: self.dsp.inputs,
            outputs: self.dsp.outputs,
        })
    }
}

/// What changes the calculator screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalcMsg {
    Select(Calc),
    /// A form value changed.
    Edited,
}

/// State of the Calculators section.
pub struct CalcPage {
    pub current: Calc,
    pub inputs: Inputs,
    /// Result of the current calculator on the current input.
    pub result: Result<Value, String>,
}

impl Default for CalcPage {
    fn default() -> Self {
        let inputs = Inputs::default();
        let result = inputs.compute(Calc::Subnet);
        CalcPage {
            current: Calc::Subnet,
            inputs,
            result,
        }
    }
}

impl CalcPage {
    pub fn update(&mut self, msg: CalcMsg) {
        if let CalcMsg::Select(c) = msg {
            self.current = c;
        }
        self.result = self.inputs.compute(self.current);
    }

    pub fn ui(&mut self, ui: &mut Ui, _cx: &mut Cx<'_>) {
        let mut msg = None;
        egui::Panel::left("calc-list")
            .resizable(false)
            .exact_size(170.0)
            .show(ui, |ui| {
                theme::heading(ui, "Calculators");
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for c in Calc::ALL {
                        if ui.selectable_label(self.current == c, c.title()).clicked() {
                            msg = Some(CalcMsg::Select(c));
                        }
                    }
                });
            });
        egui::Panel::left("calc-form")
            .resizable(true)
            .min_size(330.0)
            .show(ui, |ui| {
                theme::section(ui, self.current.title());
                theme::dim(ui, self.current.help());
                ui.add_space(6.0);
                egui::ScrollArea::vertical().show(ui, |ui| {
                    if form(ui, self.current, &mut self.inputs) {
                        msg = Some(CalcMsg::Edited);
                    }
                    ui.add_space(8.0);
                    if ui.button("Reset to defaults").clicked() {
                        self.inputs = Inputs::default();
                        msg = Some(CalcMsg::Edited);
                    }
                });
            });
        if let Some(m) = msg {
            self.update(m);
        }
        egui::CentralPanel::default_margins().show(ui, |ui| match &self.result {
            Ok(v) => widgets::result_view(ui, "calc-result", v),
            Err(e) => widgets::error(ui, e),
        });
    }
}

// ---------------------------------------------------------------- forms

/// A two-column form that records whether any value changed.
struct Form<'u> {
    ui: &'u mut Ui,
    changed: bool,
}

impl Form<'_> {
    fn row(&mut self, label: &str, add: impl FnOnce(&mut Ui) -> egui::Response) {
        self.ui.label(label);
        let r = add(self.ui);
        self.changed |= r.changed();
        self.ui.end_row();
    }

    fn num(&mut self, label: &str, v: &mut f64) {
        self.row(label, |ui| {
            ui.add(egui::DragValue::new(v).speed(0.1).max_decimals(4))
        });
    }

    fn int(&mut self, label: &str, v: &mut u32) {
        self.row(label, |ui| ui.add(egui::DragValue::new(v).speed(1.0)));
    }

    fn text(&mut self, label: &str, v: &mut String) {
        self.row(label, |ui| {
            ui.add(egui::TextEdit::singleline(v).desired_width(160.0))
        });
    }

    /// Optional text: empty means `None`.
    fn opt_text(&mut self, label: &str, v: &mut Option<String>) {
        let mut s = v.clone().unwrap_or_default();
        let before = s.clone();
        self.text(label, &mut s);
        if s != before {
            *v = (!s.trim().is_empty()).then_some(s);
        }
    }

    fn flag(&mut self, label: &str, v: &mut bool) {
        self.row(label, |ui| ui.checkbox(v, ""));
    }

    fn choice<E: Copy + PartialEq>(&mut self, label: &str, v: &mut E, options: &[(E, &str)]) {
        let current = options.iter().find(|(e, _)| e == v).map_or("", |(_, n)| *n);
        let mut changed = false;
        self.ui.label(label);
        egui::ComboBox::from_id_salt(label)
            .selected_text(current)
            .show_ui(self.ui, |ui| {
                for (e, name) in options {
                    changed |= ui.selectable_value(v, *e, *name).changed();
                }
            });
        self.ui.end_row();
        self.changed |= changed;
    }

    /// Optional value picked from a list (or "none").
    fn opt_choice(&mut self, label: &str, v: &mut Option<String>, options: &[String]) {
        let mut changed = false;
        self.ui.label(label);
        egui::ComboBox::from_id_salt(label)
            .selected_text(v.as_deref().unwrap_or("(none)"))
            .width(200.0)
            .show_ui(self.ui, |ui| {
                changed |= ui.selectable_value(v, None, "(none)").changed();
                for o in options {
                    changed |= ui.selectable_value(v, Some(o.clone()), o).changed();
                }
            });
        self.ui.end_row();
        self.changed |= changed;
    }
}

/// Draws the form of `calc`; `true` if a value changed.
fn form(ui: &mut Ui, calc: Calc, i: &mut Inputs) -> bool {
    egui::Grid::new(("calc-form", calc.title()))
        .num_columns(2)
        .spacing([12.0, 6.0])
        .show(ui, |ui| {
            let mut f = Form { ui, changed: false };
            fields(&mut f, calc, i);
            f.changed
        })
        .inner
}

fn fields(f: &mut Form<'_>, calc: Calc, i: &mut Inputs) {
    match calc {
        Calc::Subnet => {
            let s = &mut i.subnet;
            f.text("CIDR", &mut s.cidr);
            f.int("Devices", &mut s.devices);
            f.int("Start host", &mut s.start_host);
        }
        Calc::Screen => {
            let s = &mut i.screen;
            f.num("Farthest viewer", &mut s.distance);
            f.num("Diagonal (reverse)", &mut s.diagonal);
            f.choice(
                "Content",
                &mut s.content,
                &[
                    (Content::Detailed, "detailed (4:1)"),
                    (Content::Basic, "basic (6:1)"),
                    (Content::Passive, "passive (8:1)"),
                ],
            );
            f.text("Aspect (w:h)", &mut s.aspect);
            f.text("Unit (m / ft)", &mut s.unit);
        }
        Calc::Ledwall => ledwall_fields(f, &mut i.ledwall),
        Calc::Bitrate => {
            let b = &mut i.bitrate;
            f.int("Width (px)", &mut b.width);
            f.int("Height (px)", &mut b.height);
            f.num("FPS", &mut b.fps);
            f.int("Bit depth", &mut b.bit_depth);
            f.int("Channels", &mut b.channels);
            f.num("Hours", &mut b.hours);
        }
        Calc::Avoip => {
            let a = &mut i.avoip;
            f.int("Width (px)", &mut a.width);
            f.int("Height (px)", &mut a.height);
            f.num("FPS", &mut a.fps);
            f.int("Bit depth", &mut a.bit_depth);
        }
        Calc::Projector => {
            let p = &mut i.projector;
            f.num("Image width (m)", &mut p.width);
            f.num("Diagonal (m)", &mut p.diagonal);
            f.text("Aspect (w:h)", &mut p.aspect);
            f.num("Throw ratio", &mut p.throw_ratio);
            f.num("Distance (m)", &mut p.distance);
            f.num("Screen gain", &mut p.gain);
            f.num("Target fL", &mut p.target_fl);
        }
        Calc::Pixelpitch => {
            let p = &mut i.pixelpitch;
            f.num("Pitch (mm)", &mut p.pitch);
            f.num("Viewing distance (m)", &mut p.distance);
            f.num("Wall width (m)", &mut p.width_m);
            f.num("Wall height (m)", &mut p.height_m);
        }
        Calc::Brightness => {
            let b = &mut i.brightness;
            f.num("Ambient (lux)", &mut b.ambient_lux);
            f.num("Reflectance (0-1)", &mut b.reflectance);
            f.choice(
                "Content",
                &mut b.content,
                &[
                    (BrightnessContent::Basic, "basic (7:1)"),
                    (BrightnessContent::Detailed, "detailed (15:1)"),
                ],
            );
        }
        Calc::Viewangle => {
            let v = &mut i.viewangle;
            f.num("Viewer distance (m)", &mut v.distance);
            f.num("Max angle (deg)", &mut v.max_angle);
            f.num("Screen width (m)", &mut v.screen_width);
        }
        Calc::Camera => {
            let c = &mut i.camera;
            f.num("Focal length (mm)", &mut c.focal_length);
            f.num("Sensor width (mm)", &mut c.sensor_width);
            f.int("H resolution (px)", &mut c.h_resolution);
            f.num("Distance (m)", &mut c.distance);
        }
        Calc::Audiobw => {
            let a = &mut i.audiobw;
            f.int("Channels", &mut a.channels);
            f.int("Sample rate (Hz)", &mut a.sample_rate);
            f.int("Bit depth", &mut a.bit_depth);
            f.num("Frequency (Hz)", &mut a.freq);
        }
        Calc::Speaker => {
            let s = &mut i.speaker;
            f.num("Ceiling height (m)", &mut s.ceiling_height);
            f.num("Ear height (m)", &mut s.ear_height);
            f.num("Coverage angle (deg)", &mut s.coverage_angle);
            f.num("Room width (m)", &mut s.room_width);
            f.num("Room length (m)", &mut s.room_length);
            f.int("Speakers (amp mode)", &mut s.speakers);
            f.num("Watts each", &mut s.watts_each);
            f.num("Headroom (dB)", &mut s.headroom_db);
        }
        Calc::Cable => {
            let c = &mut i.cable;
            f.choice(
                "Type",
                &mut c.kind,
                &[(CableType::Speaker, "speaker"), (CableType::Coax, "coax")],
            );
            f.int("AWG", &mut c.awg);
            f.num("Length (m)", &mut c.length_m);
            f.num("Impedance (ohm)", &mut c.impedance);
            f.num("Atten. dB/100 m", &mut c.atten_per_100m);
            f.num("Budget (dB)", &mut c.max_db);
        }
        Calc::Dsp => {
            let d = &mut i.dsp;
            f.text("Level (dBu)", &mut d.dbu);
            f.text("Latencies (ms, a,b,c)", &mut d.latencies_ms);
            f.int("Matrix inputs", &mut d.inputs);
            f.int("Matrix outputs", &mut d.outputs);
        }
    }
}

fn ledwall_fields(f: &mut Form<'_>, l: &mut LedWallInput) {
    let cabinets: Vec<String> = led_cabinets().iter().map(|c| c.key.clone()).collect();
    let cards: Vec<String> = NOVA_RECV_CARDS.iter().map(|c| c.name.to_string()).collect();
    let senders: Vec<String> = NOVA_SENDERS.iter().map(|s| s.name.to_string()).collect();
    f.int("Cabinets wide", &mut l.cols);
    f.int("Cabinets high", &mut l.rows);
    f.opt_choice("Cabinet preset", &mut l.cabinet, &cabinets);
    f.int("Cabinet px W", &mut l.cab_px_w);
    f.int("Cabinet px H", &mut l.cab_px_h);
    f.num("Cabinet W (mm)", &mut l.cab_wmm);
    f.num("Cabinet H (mm)", &mut l.cab_hmm);
    f.opt_text("Environment", &mut l.env);
    f.num("Peak W / cabinet", &mut l.max_watt);
    f.num("Peak W/m2", &mut l.max_wm2);
    f.num("Average factor", &mut l.avg_factor);
    f.num("Weight (kg)", &mut l.weight_kg);
    f.num("Brightness (nits)", &mut l.nits);
    f.int("Recv cards / cabinet", &mut l.recv_per_cab);
    f.opt_choice("Receiving card", &mut l.recv_card, &cards);
    f.opt_choice("Sender", &mut l.sender, &senders);
    f.num("Voltage", &mut l.voltage);
    f.num("Breaker (A)", &mut l.breaker_a);
    f.int("Max chain (0 = none)", &mut l.max_chain);
    f.choice(
        "Power basis",
        &mut l.power_basis,
        &[(PowerBasis::Avg, "average"), (PowerBasis::Max, "peak")],
    );
    f.num("Px per port", &mut l.px_per_port);
    f.choice(
        "Distribution",
        &mut l.distribution,
        &[
            (Distribution::Vertical, "vertical"),
            (Distribution::Horizontal, "horizontal"),
        ],
    );
    f.flag("Redundant", &mut l.redundant);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_calculator_works_on_its_default_form() {
        let inputs = Inputs::default();
        for c in Calc::ALL {
            let r = inputs.compute(c);
            assert!(r.is_ok(), "{c:?}: {r:?}");
        }
    }

    #[test]
    fn select_and_edit_recalculate() {
        let mut page = CalcPage::default();
        let hosts = |p: &CalcPage| p.result.as_ref().ok().and_then(|v| v["hosts"].as_u64());
        assert_eq!(hosts(&page), Some(254));

        page.inputs.subnet.cidr = "10.0.0.0/30".into();
        page.update(CalcMsg::Edited);
        assert_eq!(hosts(&page), Some(2));

        page.inputs.subnet.cidr = "10.0.0.0/33".into();
        page.update(CalcMsg::Edited);
        assert!(
            page.result.is_err(),
            "invalid CIDR is an error, not a panic"
        );

        page.update(CalcMsg::Select(Calc::Screen));
        assert_eq!(page.current, Calc::Screen);
        let v = page.result.as_ref().unwrap();
        assert_eq!(v["view_distance"].as_f64(), Some(6.0));
    }

    #[test]
    fn dsp_text_fields_parse_or_explain() {
        let mut i = Inputs::default();
        i.dsp.latencies_ms = "1.5, 2 0.5".into();
        assert_eq!(i.dsp_input().unwrap().latencies_ms, [1.5, 2.0, 0.5]);
        i.dsp.dbu = "loud".into();
        assert!(i.compute(Calc::Dsp).unwrap_err().contains("not a number"));
    }
}
