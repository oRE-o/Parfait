//! Portable brush presets and pressure controls; no platform-specific assets.
use crate::{app::Parfait, widgets};
use eframe::egui;
use parfait_core::{Brush, BrushTip};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct Preset {
    pub name: String,
    pub brush: Brush,
}
#[derive(Serialize, Deserialize)]
struct BrushFile {
    version: u32,
    preset: Preset,
}
pub fn defaults() -> Vec<Preset> {
    vec![
        Preset {
            name: "Round pen".into(),
            brush: Brush::default(),
        },
        Preset {
            name: "Soft airbrush".into(),
            brush: Brush {
                tip: BrushTip::Soft,
                size: 70.,
                flow: 0.25,
                pressure_opacity: true,
                ..Default::default()
            },
        },
        Preset {
            name: "Graphite pencil".into(),
            brush: Brush {
                tip: BrushTip::Pencil,
                size: 8.,
                opacity: 0.7,
                ..Default::default()
            },
        },
        Preset {
            name: "Flat marker".into(),
            brush: Brush {
                tip: BrushTip::Flat,
                size: 30.,
                aspect: 0.35,
                angle: -30.,
                pressure: false,
                opacity: 0.65,
                ..Default::default()
            },
        },
        Preset {
            name: "Calligraphy".into(),
            brush: Brush {
                size: 35.,
                aspect: 0.2,
                angle: -45.,
                minimum_size: 0.1,
                ..Default::default()
            },
        },
        Preset {
            name: "Dry pencil".into(),
            brush: Brush {
                tip: BrushTip::Pencil,
                size: 24.,
                spacing: 0.22,
                flow: 0.35,
                pressure_opacity: true,
                ..Default::default()
            },
        },
    ]
}
impl Parfait {
    pub fn brush_library_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui
                .small_button("Import…")
                .on_hover_text("Multiple PNG/JPEG tips or .pbrush presets")
                .clicked()
            {
                self.import_brushes();
            }
            if ui.small_button("Export…").clicked() {
                self.export_brush();
            }
        });
        for i in 0..self.presets.len() {
            let p = &self.presets[i];
            if widgets::brush_row(
                ui,
                &p.name,
                self.tip_name == p.name,
                matches!(p.brush.tip, BrushTip::Soft),
            ) {
                let color = self.brush.color;
                let erase = self.brush.erase;
                self.brush = self.presets[i].brush.clone();
                self.brush.color = color;
                self.brush.erase = erase;
                self.tip_name = self.presets[i].name.clone();
                self.navigation_tool = 0;
            }
        }
        ui.separator();
        ui.add(
            egui::TextEdit::singleline(&mut self.tip_name)
                .char_limit(32)
                .hint_text("Preset name"),
        );
        ui.horizontal(|ui| {
            if ui
                .button("Save preset")
                .on_hover_text("Updates a preset with this name, or adds a new one")
                .clicked()
            {
                let p = Preset {
                    name: self.tip_name.trim().to_owned(),
                    brush: self.brush.clone(),
                };
                if p.name.is_empty() {
                    self.error = Some("Give the brush a name first.".into());
                } else if let Some(old) = self.presets.iter_mut().find(|v| v.name == p.name) {
                    *old = p;
                } else if self.presets.len() < 64 {
                    self.presets.push(p);
                } else {
                    self.error =
                        Some("The brush library is limited to 64 presets in this version.".into());
                }
            }
            if ui
                .button("Reset built-ins")
                .on_hover_text(
                    "Restores missing built-in presets; keeps existing edits and imports",
                )
                .clicked()
            {
                for p in defaults() {
                    if self.presets.len() < 64 && !self.presets.iter().any(|v| v.name == p.name) {
                        self.presets.push(p);
                    }
                }
            }
        });
    }
    fn import_brushes(&mut self) {
        let Some(paths) = rfd::FileDialog::new()
            .add_filter(
                "Brush presets / image tips",
                &["pbrush", "png", "jpg", "jpeg"],
            )
            .pick_files()
        else {
            return;
        };
        let mut errors = Vec::new();
        let mut count = 0;
        for path in paths {
            let result = (|| -> anyhow::Result<Preset> {
                anyhow::ensure!(self.presets.len() < 64, "Library is full");
                anyhow::ensure!(
                    std::fs::metadata(&path)?.len() <= 16 * 1024 * 1024,
                    "File exceeds 16 MiB"
                );
                let mut preset = if path
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("pbrush"))
                {
                    let file: BrushFile = serde_json::from_slice(&std::fs::read(&path)?)?;
                    anyhow::ensure!(file.version == 1, "Unsupported brush format version");
                    file.preset
                } else {
                    let mut reader = image::ImageReader::open(&path)?;
                    let mut limits = image::Limits::default();
                    limits.max_image_width = Some(8192);
                    limits.max_image_height = Some(8192);
                    limits.max_alloc = Some(64 * 1024 * 1024);
                    reader.limits(limits);
                    let img = reader.decode()?.thumbnail(256, 256).to_rgba8();
                    let (width, height) = img.dimensions();
                    let alpha = img.pixels().any(|p| p[3] < 255);
                    let mask = img
                        .pixels()
                        .map(|p| {
                            if alpha {
                                p[3]
                            } else {
                                255 - ((p[0] as u32 + p[1] as u32 + p[2] as u32) / 3) as u8
                            }
                        })
                        .collect();
                    Preset {
                        name: path
                            .file_stem()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .chars()
                            .take(32)
                            .collect(),
                        brush: Brush {
                            tip: BrushTip::Image {
                                width,
                                height,
                                mask: std::sync::Arc::new(mask),
                            },
                            ..Default::default()
                        },
                    }
                };
                anyhow::ensure!(
                    preset.brush.validate() && !preset.name.is_empty() && preset.name.len() <= 128,
                    "Invalid brush settings or tip"
                );
                let base = preset.name.clone();
                let mut n = 2;
                while self.presets.iter().any(|p| p.name == preset.name) {
                    preset.name = format!("{} ({n})", base.chars().take(28).collect::<String>());
                    n += 1;
                }
                Ok(preset)
            })();
            match result {
                Ok(p) => {
                    self.brush = p.brush.clone();
                    self.tip_name = p.name.clone();
                    self.presets.push(p);
                    count += 1;
                }
                Err(e) => errors.push(format!(
                    "{}: {e}",
                    path.file_name().unwrap_or_default().to_string_lossy()
                )),
            }
        }
        self.notice = Some(format!(
            "Imported {count} brush presets. They are kept in your local library."
        ));
        if !errors.is_empty() {
            self.error = Some(errors.join("\n"));
        }
    }
    fn export_brush(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Parfait brush", &["pbrush"])
            .set_file_name("brush.pbrush")
            .save_file()
        {
            let file = BrushFile {
                version: 1,
                preset: Preset {
                    name: self.tip_name.clone(),
                    brush: self.brush.clone(),
                },
            };
            let result = serde_json::to_vec_pretty(&file)
                .map_err(anyhow::Error::from)
                .and_then(|data| std::fs::write(path, data).map_err(Into::into));
            if let Err(e) = result {
                self.error = Some(format!("Brush export failed: {e}"));
            }
        }
    }
    pub fn brush_properties_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.pressure_tab, false, "Brush");
            ui.selectable_value(&mut self.pressure_tab, true, "Pressure curve");
        });
        ui.label(&self.tip_name);
        if !self.pressure_tab {
            ui.add(
                egui::Slider::new(&mut self.brush.size, 1. ..=512.)
                    .logarithmic(true)
                    .text("Size"),
            );
            ui.add(egui::Slider::new(&mut self.brush.opacity, 0. ..=1.).text("Opacity"));
            ui.add(egui::Slider::new(&mut self.brush.flow, 0.01..=1.).text("Flow"));
            ui.add(egui::Slider::new(&mut self.brush.spacing, 0.01..=2.).text("Spacing"));
            ui.add(egui::Slider::new(&mut self.brush.hardness, 0. ..=1.).text("Hardness"))
                .on_hover_text("Round tips only");
            ui.add(egui::Slider::new(&mut self.brush.aspect, 0.1..=1.).text("Roundness"));
            ui.add(egui::Slider::new(&mut self.brush.angle, -180. ..=180.).text("Angle"));
            ui.add(egui::Slider::new(&mut self.brush.smoothing, 0. ..=0.95).text("Smoothing"));
        } else {
            widgets::heading(ui, "Pressure response");
            if ui.button("Calibrate tablet pressure…").clicked() {
                self.calibration_dialog = Some(crate::calibration::CalibrationDialog::new(
                    self.calibration.clone(),
                ));
            }
            ui.checkbox(&mut self.brush.pressure, "Pressure → size");
            ui.checkbox(&mut self.brush.pressure_opacity, "Pressure → opacity");
            ui.add(egui::Slider::new(&mut self.brush.minimum_size, 0. ..=1.).text("Minimum size"));
            ui.label(if self.pressure_seen {
                format!("Pressure received: {:.0}%", self.pressure * 100.)
            } else {
                "No pressure samples yet (mouse = 100%)".into()
            });
            pressure_graph(ui, &mut self.brush.pressure_curve, self.pressure);
            ui.horizontal(|ui| {
                if ui.small_button("Linear").clicked() {
                    self.brush.pressure_curve = [0., 0.25, 0.5, 0.75, 1.];
                }
                if ui.small_button("Soft").clicked() {
                    self.brush.pressure_curve = [0., 0.5, 0.71, 0.87, 1.];
                }
                if ui.small_button("Firm").clicked() {
                    self.brush.pressure_curve = [0., 0.06, 0.25, 0.56, 1.];
                }
            });
            ui.small(
                "Drag graph nodes vertically. Save preset to keep these settings in the library.",
            );
        }
    }
}
fn pressure_graph(ui: &mut egui::Ui, curve: &mut [f32; 5], pressure: f32) {
    let (r, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width().min(300.), 130.),
        egui::Sense::click_and_drag(),
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Other, true, "Pressure response curve")
    });
    let r = r.shrink(7.);
    let node_id = response.id.with("node");
    if (response.drag_started() || response.clicked())
        && let Some(p) = response.interact_pointer_pos()
    {
        ui.data_mut(|d| {
            d.insert_temp(
                node_id,
                (((p.x - r.left()) / r.width() * 4.).round() as usize).min(4),
            )
        });
    }
    if (response.dragged() || response.clicked())
        && let Some(p) = response.interact_pointer_pos()
    {
        let i = ui.data(|d| d.get_temp::<usize>(node_id)).unwrap_or(2);
        curve[i] = ((r.bottom() - p.y) / r.height()).clamp(
            if i == 0 { 0. } else { curve[i - 1] },
            if i == 4 { 1. } else { curve[i + 1] },
        );
    }
    let p = ui.painter();
    p.rect_filled(r, 0., egui::Color32::from_gray(35));
    for i in 0..=4 {
        let t = i as f32 / 4.;
        p.line_segment(
            [
                r.min + egui::vec2(t * r.width(), 0.),
                r.left_bottom() + egui::vec2(t * r.width(), 0.),
            ],
            egui::Stroke::new(1., egui::Color32::from_gray(58)),
        );
    }
    let points: Vec<_> = curve
        .iter()
        .enumerate()
        .map(|(i, v)| {
            egui::pos2(
                r.left() + i as f32 / 4. * r.width(),
                r.bottom() - v * r.height(),
            )
        })
        .collect();
    p.add(egui::Shape::line(
        points.clone(),
        egui::Stroke::new(2., egui::Color32::LIGHT_BLUE),
    ));
    for point in points {
        p.circle_filled(point, 4., egui::Color32::WHITE);
    }
    let x = r.left() + pressure.clamp(0., 1.) * r.width();
    p.line_segment(
        [egui::pos2(x, r.top()), egui::pos2(x, r.bottom())],
        egui::Stroke::new(1., egui::Color32::LIGHT_GREEN),
    );
}
