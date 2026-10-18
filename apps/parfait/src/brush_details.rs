//! Compact brush palette and categorized detail editor sharing the live brush model.
use crate::{app::Parfait, brushes::pressure_graph};
use eframe::egui::{self, Color32};
use parfait_core::{Brush, BrushTip, Document, Point, Stroke};

#[derive(Default, Clone, Copy, PartialEq)]
enum Category {
    #[default]
    Size,
    Ink,
    Tip,
    Stroke,
    Pressure,
}
impl Category {
    const ALL: [Self; 5] = [
        Self::Size,
        Self::Ink,
        Self::Tip,
        Self::Stroke,
        Self::Pressure,
    ];
    fn label(self) -> &'static str {
        match self {
            Self::Size => "Brush size",
            Self::Ink => "Ink",
            Self::Tip => "Brush tip",
            Self::Stroke => "Stroke",
            Self::Pressure => "Pressure",
        }
    }
    fn help(self) -> &'static str {
        match self {
            Self::Size => {
                "Size sets the diameter in pixels. Minimum size is the fraction retained at light pressure. Pressure response is shared with opacity when both are enabled."
            }
            Self::Ink => {
                "Opacity and flow control each dab's coverage. Overlapping dabs can build up paint. Flow is separate from the layer's opacity."
            }
            Self::Tip => {
                "Tip shape, roundness and angle define each dab. Hardness applies to the round tip. Import PNG/JPEG tips from the Sub tools palette."
            }
            Self::Stroke => {
                "Spacing is the distance between dabs relative to brush size. Smoothing filters pointer movement; it can add trailing motion. The preview shows tip deposition, not pointer smoothing."
            }
            Self::Pressure => {
                "The shared curve controls pressure-driven size and opacity. Device calibration runs first and applies to all brushes. Mouse input uses full pressure."
            }
        }
    }
}
#[derive(Default)]
pub struct BrushDetails {
    pub open: bool,
    category: Category,
    preview: Option<(Brush, egui::TextureHandle)>,
}
fn control(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    scale: f32,
    unit: &str,
) {
    ui.push_id(label, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 3.;
            ui.add_sized([76., 20.], egui::Label::new(label));
            let mut shown = *value * scale;
            let bounds = (*range.start() * scale)..=(*range.end() * scale);
            ui.spacing_mut().slider_width = (ui.available_width() - 62.).max(20.);
            ui.add(egui::Slider::new(&mut shown, bounds.clone()).show_value(false));
            ui.add(
                egui::DragValue::new(&mut shown)
                    .range(bounds)
                    .speed(if scale == 100. { 1. } else { 0.2 })
                    .max_decimals(1)
                    .suffix(unit),
            );
            *value = shown / scale;
        });
    });
}
impl Parfait {
    fn brush_preview(&mut self, ui: &mut egui::Ui, height: f32) {
        let changed = self
            .brush_details
            .preview
            .as_ref()
            .is_none_or(|(brush, _)| brush != &self.brush);
        if changed && self.stroke.is_none() {
            let mut doc = Document::new(320, 80);
            for pixel in std::sync::Arc::make_mut(&mut doc.layers[0].pixels)
                .as_chunks_mut::<4>()
                .0
            {
                pixel.copy_from_slice(&[188, 188, 188, 255]);
            }
            let mut brush = self.brush.clone();
            brush.size = brush.size.min(56.);
            brush.erase = false;
            let point = |i: usize| {
                let t = i as f32 / 80.;
                Point {
                    x: 20. + t * 280.,
                    y: 40. - (t * std::f32::consts::TAU).sin() * 15.,
                    pressure: (t * std::f32::consts::PI).sin().max(0.03),
                }
            };
            let mut stroke = Stroke::begin(&mut doc, brush, point(0));
            for i in 1..=80 {
                stroke.extend(&mut doc, point(i));
            }
            let image = egui::ColorImage::from_rgba_unmultiplied([320, 80], &doc.composite());
            if let Some((cached, texture)) = &mut self.brush_details.preview {
                texture.set(image, egui::TextureOptions::LINEAR);
                *cached = self.brush.clone();
            } else {
                self.brush_details.preview = Some((
                    self.brush.clone(),
                    ui.ctx().load_texture(
                        "brush_setting_preview",
                        image,
                        egui::TextureOptions::LINEAR,
                    ),
                ));
            }
        }
        if let Some((_, texture)) = &self.brush_details.preview {
            ui.add(egui::Image::new(texture).maintain_aspect_ratio(false).fit_to_exact_size(egui::vec2(ui.available_width(), height)))
                .on_hover_text("Live tip preview · size capped at 56 px · varying pressure · smoothing is not simulated");
        }
    }
    pub fn brush_properties_ui(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing.y = 2.;
        ui.horizontal(|ui| {
            ui.label(&self.tip_name);
            if ui.small_button("Details…").clicked() {
                self.brush_details.open = true;
            }
        });
        self.brush_preview(ui, 38.);
        control(ui, "Size", &mut self.brush.size, 1. ..=512., 1., " px");
        control(ui, "Opacity", &mut self.brush.opacity, 0. ..=1., 100., "%");
        control(ui, "Flow", &mut self.brush.flow, 0.01..=1., 100., "%");
        control(
            ui,
            "Smoothing",
            &mut self.brush.smoothing,
            0. ..=0.95,
            100.,
            "%",
        );
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.brush.pressure, "Pressure")
                .on_hover_text("Pressure controls brush size");
            if ui.small_button("Curve…").clicked() {
                self.brush_details.category = Category::Pressure;
                self.brush_details.open = true;
            }
        });
    }
    pub fn brush_details_ui(&mut self, ctx: &egui::Context) {
        if !self.brush_details.open {
            return;
        }
        let mut open = true;
        let mut close = false;
        let calibration_open = self.calibration_dialog.is_some();
        egui::Window::new("Brush details")
            .id(egui::Id::new("brush_details"))
            .open(&mut open)
            .collapsible(false)
            .default_size([510., 360.])
            .default_pos(ctx.content_rect().center() - egui::vec2(255., 210.))
            .constrain_to(ctx.content_rect())
            .min_width(390.)
            .show(ctx, |ui| {
                ui.add_enabled_ui(!calibration_open, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(&self.tip_name);
                        if ui.button("Close").clicked() {
                            close = true;
                        }
                        if ui.button("Save preset").clicked() {
                            self.save_current_preset();
                        }
                        ui.weak("Live edits");
                    });
                    self.brush_preview(ui, 78.);
                    ui.separator();
                    ui.horizontal_top(|ui| {
                        ui.vertical(|ui| {
                            ui.set_width(100.);
                            for category in Category::ALL {
                                ui.selectable_value(
                                    &mut self.brush_details.category,
                                    category,
                                    category.label(),
                                );
                            }
                        });
                        ui.separator();
                        ui.vertical(|ui| {
                            ui.set_min_width(250.);
                            crate::widgets::heading(ui, self.brush_details.category.label());
                            egui::ScrollArea::vertical()
                                .id_salt("brush_detail_controls")
                                .max_height(260.)
                                .show(ui, |ui| match self.brush_details.category {
                                    Category::Size => {
                                        control(
                                            ui,
                                            "Size",
                                            &mut self.brush.size,
                                            1. ..=512.,
                                            1.,
                                            " px",
                                        );
                                        ui.checkbox(
                                            &mut self.brush.pressure,
                                            "Pressure controls size",
                                        );
                                        ui.add_enabled_ui(self.brush.pressure, |ui| {
                                            control(
                                                ui,
                                                "Minimum",
                                                &mut self.brush.minimum_size,
                                                0. ..=1.,
                                                100.,
                                                "%",
                                            );
                                        });
                                        if ui.button("Edit response curve…").clicked() {
                                            self.brush_details.category = Category::Pressure;
                                        }
                                    }
                                    Category::Ink => {
                                        control(
                                            ui,
                                            "Opacity",
                                            &mut self.brush.opacity,
                                            0. ..=1.,
                                            100.,
                                            "%",
                                        );
                                        control(
                                            ui,
                                            "Flow",
                                            &mut self.brush.flow,
                                            0.01..=1.,
                                            100.,
                                            "%",
                                        );
                                        ui.checkbox(
                                            &mut self.brush.pressure_opacity,
                                            "Pressure controls opacity",
                                        );
                                    }
                                    Category::Tip => {
                                        let label = match &self.brush.tip {
                                            BrushTip::Round => "Round",
                                            BrushTip::Soft => "Soft",
                                            BrushTip::Pencil => "Pencil",
                                            BrushTip::Flat => "Flat",
                                            BrushTip::Image { .. } => "Imported image",
                                        };
                                        egui::ComboBox::from_id_salt("tip_shape")
                                            .selected_text(label)
                                            .show_ui(ui, |ui| {
                                                for (name, tip) in [
                                                    ("Round", BrushTip::Round),
                                                    ("Soft", BrushTip::Soft),
                                                    ("Pencil", BrushTip::Pencil),
                                                    ("Flat", BrushTip::Flat),
                                                ] {
                                                    if ui
                                                        .selectable_label(label == name, name)
                                                        .clicked()
                                                    {
                                                        self.brush.tip = tip;
                                                    }
                                                }
                                            });
                                        ui.add_enabled_ui(
                                            matches!(self.brush.tip, BrushTip::Round),
                                            |ui| {
                                                control(
                                                    ui,
                                                    "Hardness",
                                                    &mut self.brush.hardness,
                                                    0. ..=1.,
                                                    100.,
                                                    "%",
                                                );
                                            },
                                        );
                                        control(
                                            ui,
                                            "Roundness",
                                            &mut self.brush.aspect,
                                            0.1..=1.,
                                            100.,
                                            "%",
                                        );
                                        control(
                                            ui,
                                            "Angle",
                                            &mut self.brush.angle,
                                            -180. ..=180.,
                                            1.,
                                            "°",
                                        );
                                    }
                                    Category::Stroke => {
                                        control(
                                            ui,
                                            "Spacing",
                                            &mut self.brush.spacing,
                                            0.01..=2.,
                                            100.,
                                            "%",
                                        );
                                        control(
                                            ui,
                                            "Smoothing",
                                            &mut self.brush.smoothing,
                                            0. ..=0.95,
                                            100.,
                                            "%",
                                        );
                                    }
                                    Category::Pressure => {
                                        ui.horizontal(|ui| {
                                            ui.checkbox(&mut self.brush.pressure, "Size");
                                            ui.checkbox(
                                                &mut self.brush.pressure_opacity,
                                                "Opacity",
                                            );
                                        });
                                        pressure_graph(
                                            ui,
                                            &mut self.brush.pressure_curve,
                                            self.calibration.map(self.pressure),
                                        );
                                        ui.horizontal(|ui| {
                                            for (name, curve) in [
                                                ("Linear", [0., 0.25, 0.5, 0.75, 1.]),
                                                ("Soft", [0., 0.5, 0.71, 0.87, 1.]),
                                                ("Firm", [0., 0.06, 0.25, 0.56, 1.]),
                                            ] {
                                                if ui.small_button(name).clicked() {
                                                    self.brush.pressure_curve = curve;
                                                }
                                            }
                                        });
                                        if ui.button("Calibrate tablet…").clicked() {
                                            self.calibration_dialog =
                                                Some(crate::calibration::CalibrationDialog::new(
                                                    self.calibration.clone(),
                                                ));
                                        }
                                    }
                                });
                            ui.separator();
                            egui::Frame::NONE
                                .fill(Color32::from_gray(54))
                                .inner_margin(6)
                                .show(ui, |ui| {
                                    ui.set_min_height(65.);
                                    ui.label(self.brush_details.category.help());
                                });
                        });
                    });
                });
            });
        if !calibration_open && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            close = true;
        }
        self.brush_details.open = open && !close;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn categories_preview_cache_and_escape_preserve_artwork_and_edits() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open(dir.path().to_path_buf()).unwrap();
        app.doc = Document::new(16, 16);
        let before = app.doc.clone();
        let ctx = egui::Context::default();
        let frame = |app: &mut Parfait, events| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(960., 640.),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| {
                    app.brush_details_ui(ui.ctx());
                },
            );
            output.textures_delta.clear();
        };
        app.brush_details.open = true;
        for category in Category::ALL {
            app.brush_details.category = category;
            frame(&mut app, vec![]);
            assert!(app.brush.validate());
            assert_eq!(app.doc, before);
        }
        let texture = app.brush_details.preview.as_ref().unwrap().1.id();
        app.brush.size = 42.;
        frame(&mut app, vec![]);
        let (cached, current_texture) = app.brush_details.preview.as_ref().unwrap();
        assert_eq!(current_texture.id(), texture);
        assert_eq!(cached.size, 42.);
        app.save_current_preset();
        assert_eq!(
            app.presets
                .iter()
                .find(|p| p.name == app.tip_name)
                .unwrap()
                .brush
                .size,
            42.
        );
        frame(
            &mut app,
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            }],
        );
        assert!(!app.brush_details.open);
        assert_eq!(app.brush.size, 42.);
        assert_eq!(app.doc, before);
    }
}
