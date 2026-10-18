use crate::app::Parfait;
use eframe::egui::{self, Color32, Event, Pos2, Sense, Stroke, TouchPhase};
use parfait_core::PressureCalibration;

pub struct CalibrationDialog {
    draft: PressureCalibration,
    #[cfg(test)]
    cancel_rect: Option<egui::Rect>,
    samples: Vec<f32>,
    marks: Vec<([f32; 2], f32, bool)>,
    active: bool,
    connected: bool,
    selected: Option<usize>,
    raw: f32,
    message: String,
}
impl CalibrationDialog {
    pub fn new(draft: PressureCalibration) -> Self {
        Self {
            #[cfg(test)]
            cancel_rect: None,
            draft,
            samples: Vec::new(),
            marks: Vec::new(),
            active: false,
            connected: false,
            selected: None,
            raw: 1.,
            message: "Draw light and firm strokes at your usual pressure. Mouse previews use 100%."
                .into(),
        }
    }
    fn record(&mut self, pos: Pos2, raw: f32, rect: egui::Rect, measured: bool) {
        if !rect.contains(pos) {
            self.connected = false;
            return;
        }
        self.raw = raw.clamp(0., 1.);
        if measured && raw.is_finite() && self.samples.len() < 4096 {
            self.samples.push(self.raw);
        }
        if self.marks.len() < 4096 {
            self.marks.push((
                [
                    (pos.x - rect.left()) / rect.width(),
                    (pos.y - rect.top()) / rect.height(),
                ],
                self.raw,
                self.connected,
            ));
            self.connected = true;
        }
    }
    fn graph(&mut self, ui: &mut egui::Ui) -> egui::Rect {
        crate::curve_editor::edit(
            ui,
            "device_pressure",
            &mut self.draft.points,
            &mut self.selected,
            crate::curve_editor::Options {
                input: "Pressure",
                side: 210.,
                marker: Some(self.raw),
                ..Default::default()
            },
        )
    }
    fn pad(&mut self, ui: &mut egui::Ui) {
        ui.label("Test strokes — preview updates when you edit the curve");
        let (r, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 180.), Sense::drag());
        let events = ui.input(|i| i.events.clone());
        let has_touch = events.iter().any(|e| matches!(e, Event::Touch { .. }));
        for event in events {
            match event {
                Event::Touch {
                    phase, pos, force, ..
                } => {
                    if phase == TouchPhase::Start {
                        self.active = r.contains(pos);
                        self.connected = false;
                    }
                    if self.active && matches!(phase, TouchPhase::Start | TouchPhase::Move) {
                        self.record(
                            pos,
                            force.filter(|v| v.is_finite()).unwrap_or(1.),
                            r,
                            force.is_some_and(|v| v.is_finite()),
                        );
                    }
                    if matches!(phase, TouchPhase::End | TouchPhase::Cancel) {
                        self.active = false;
                        self.connected = false;
                    }
                }
                Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    ..
                } if !has_touch => {
                    self.active = pressed && r.contains(pos);
                    self.connected = false;
                    if self.active {
                        self.record(pos, 1., r, false);
                    }
                }
                Event::PointerMoved(pos) if !has_touch && self.active => {
                    self.record(pos, 1., r, false);
                }
                Event::WindowFocused(false) | Event::PointerGone => {
                    self.active = false;
                    self.connected = false;
                }
                _ => {}
            }
        }
        let p = ui.painter().with_clip_rect(r);
        p.rect_filled(r, 0., Color32::from_gray(245));
        let mut previous = None;
        for (v, raw, connected) in &self.marks {
            let at = egui::pos2(r.left() + v[0] * r.width(), r.top() + v[1] * r.height());
            let width = 1. + self.draft.map(*raw) * 22.;
            if *connected && let Some(last) = previous {
                p.line_segment([last, at], Stroke::new(width, Color32::from_gray(35)));
            }
            p.circle_filled(at, width / 2., Color32::from_gray(35));
            previous = Some(at);
        }
        ui.label(format!(
            "Raw {:.0}% → calibrated {:.0}% · {} pressure samples",
            self.raw * 100.,
            self.draft.map(self.raw) * 100.,
            self.samples.len()
        ));
        if self.marks.len() == 4096 {
            ui.small("Preview limit reached. Clear strokes to collect a new sample set.");
        }
    }
}
impl Parfait {
    pub fn calibration_ui(&mut self, ctx: &egui::Context) {
        let Some(dialog) = &mut self.calibration_dialog else {
            return;
        };
        let mut open = true;
        let mut apply = false;
        let mut cancel = false;
        egui::Window::new("Tablet pressure calibration")
            .open(&mut open).collapsible(false).resizable(false)
            .default_pos(ctx.content_rect().center() - egui::vec2(260., 250.))
            .constrain_to(ctx.content_rect()).default_width(500.).show(ctx,|ui| {
            // Keep dismissal outside the scrolling body, even on small/high-DPI windows.
            ui.horizontal(|ui| {
                apply = ui.add_enabled(dialog.draft.validate(),
                    egui::Button::new("Apply & close").min_size(egui::vec2(120., 30.))).clicked();
                let close = ui.add(egui::Button::new("Cancel / close").min_size(egui::vec2(120., 30.)))
                    .on_hover_text("Close without applying changes (Escape)");
                #[cfg(test)] { dialog.cancel_rect = Some(close.rect); }
                cancel = close.clicked();
            });
            ui.separator();
            egui::ScrollArea::vertical().max_height((ctx.content_rect().height() - 180.).max(100.))
                .show(ui, |ui| {
            ui.label("Device calibration · applies to all brushes before their individual pressure settings.");
            dialog.graph(ui); dialog.pad(ui);
            ui.horizontal(|ui| {
                if ui.button("Suggest from strokes").clicked() {
                    if let Some(curve)=PressureCalibration::suggest(&dialog.samples) {
                        dialog.draft=curve; dialog.selected=None;
                        dialog.message="Suggested from the 95th percentile of measured pressure. Test before applying.".into();
                    } else { dialog.message="Need at least 20 pressure samples with varied light/firm pressure. Mouse or constant input cannot calibrate.".into(); }
                }
                if ui.button("Clear strokes").clicked() { dialog.samples.clear(); dialog.marks.clear(); dialog.connected=false; }
                if ui.button("Reset curve").clicked() { dialog.draft=Default::default(); dialog.selected=None; }
            });
            ui.label(&dialog.message);
            });
        });
        if apply || cancel || !open || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.finish_calibration(apply);
        }
    }
    fn finish_calibration(&mut self, apply: bool) {
        if let Some(dialog) = self.calibration_dialog.take()
            && apply
            && dialog.draft.validate()
        {
            self.calibration = dialog.draft;
            self.persist_preferences(true);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn close_button_is_visible_on_small_window_and_escape_cancels() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open(dir.path().to_path_buf()).unwrap();
        let ctx = egui::Context::default();
        let screen = egui::Rect::from_min_size(Pos2::ZERO, egui::vec2(960., 640.));
        let frame = |app: &mut Parfait, events| {
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(screen),
                    events,
                    ..Default::default()
                },
                |ui| {
                    app.calibration_ui(ui.ctx());
                },
            );
            out.textures_delta.clear();
        };
        app.calibration_dialog = Some(CalibrationDialog::new(Default::default()));
        frame(&mut app, vec![]);
        frame(&mut app, vec![]);
        let rect = app
            .calibration_dialog
            .as_ref()
            .unwrap()
            .cancel_rect
            .unwrap();
        assert!(screen.contains_rect(rect));
        let p = rect.center();
        let button = |pressed| Event::PointerButton {
            pos: p,
            pressed,
            button: egui::PointerButton::Primary,
            modifiers: Default::default(),
        };
        frame(&mut app, vec![Event::PointerMoved(p), button(true)]);
        frame(&mut app, vec![button(false)]);
        assert!(app.calibration_dialog.is_none());
        app.calibration_dialog = Some(CalibrationDialog::new(PressureCalibration {
            points: vec![[0., 0.], [0.5, 1.], [1., 1.]],
        }));
        frame(
            &mut app,
            vec![Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            }],
        );
        assert!(app.calibration_dialog.is_none());
        assert_eq!(app.calibration, PressureCalibration::default());
    }
    #[test]
    fn graph_click_adds_and_fast_drag_moves_control_point() {
        let ctx = egui::Context::default();
        let mut d = CalibrationDialog::new(Default::default());
        let mut frame = |events| {
            let mut rect = egui::Rect::NOTHING;
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        Pos2::ZERO,
                        egui::vec2(500., 400.),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| {
                    egui::CentralPanel::default().show(ui, |ui| {
                        rect = d.graph(ui);
                    });
                },
            );
            out.textures_delta.clear();
            (rect, d.draft.clone())
        };
        let (r, _) = frame(vec![]);
        let start = egui::pos2(r.left() + 0.4 * r.width(), r.bottom() - 0.6 * r.height());
        let button = |pos, pressed| Event::PointerButton {
            pos,
            pressed,
            button: egui::PointerButton::Primary,
            modifiers: Default::default(),
        };
        frame(vec![Event::PointerMoved(start), button(start, true)]);
        let (_, added) = frame(vec![button(start, false)]);
        assert_eq!(added.points.len(), 3);
        frame(vec![button(start, true)]);
        let end = start + egui::vec2(50., -20.);
        let (_, moved) = frame(vec![Event::PointerMoved(end)]);
        assert!(moved.points[1][0] > added.points[1][0] + 0.05);
        assert!(moved.validate());
    }
    #[test]
    fn cancel_preserves_settings_and_apply_survives_restart() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open(dir.path().to_path_buf()).unwrap();
        let original = app.doc.clone();
        let curve = PressureCalibration {
            points: vec![[0., 0.], [0.6, 1.], [1., 1.]],
        };
        app.calibration_dialog = Some(CalibrationDialog::new(curve.clone()));
        app.finish_calibration(false);
        assert_eq!(app.calibration, PressureCalibration::default());
        app.calibration_dialog = Some(CalibrationDialog::new(curve.clone()));
        app.finish_calibration(true);
        assert_eq!(app.doc, original);
        drop(app);
        let restored = Parfait::open(dir.path().to_path_buf()).unwrap();
        assert_eq!(restored.calibration, curve);
        // Old preference JSON without the field still opens with identity calibration.
        let mut json: serde_json::Value =
            serde_json::from_str(&restored.preferences_saved).unwrap();
        json.as_object_mut().unwrap().remove("calibration");
        let old: crate::preferences::Preferences = serde_json::from_value(json).unwrap();
        assert!(old.validate());
        assert_eq!(old.calibration, PressureCalibration::default());
    }
    #[test]
    fn only_measured_pad_samples_calibrate_and_collection_is_bounded() {
        let mut d = CalibrationDialog::new(Default::default());
        let rect = egui::Rect::from_min_size(Pos2::ZERO, egui::vec2(100., 100.));
        d.record(egui::pos2(50., 50.), 1., rect, false);
        d.record(egui::pos2(150., 50.), 0.5, rect, true);
        assert!(d.samples.is_empty());
        for i in 0..5000 {
            d.record(egui::pos2(50., 50.), (i % 100) as f32 / 100., rect, true);
        }
        assert_eq!(d.samples.len(), 4096);
        assert_eq!(d.marks.len(), 4096);
        assert!(PressureCalibration::suggest(&d.samples).is_some());
    }
}
