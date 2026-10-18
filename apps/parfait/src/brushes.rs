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
                self.save_current_preset();
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
    pub(crate) fn save_current_preset(&mut self) {
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
            self.error = Some("The brush library is limited to 64 presets in this version.".into());
        }
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
}

pub(crate) fn pressure_graph(ui: &mut egui::Ui, curve: &mut [f32; 5], pressure: f32) {
    let id = ui.id().with("brush_pressure_selection");
    let mut selected = ui.data(|d| d.get_temp::<Option<usize>>(id)).flatten();
    let mut points: Vec<_> = curve
        .iter()
        .enumerate()
        .map(|(i, y)| [i as f32 / 4., *y])
        .collect();
    crate::curve_editor::edit(
        ui,
        "brush_pressure",
        &mut points,
        &mut selected,
        crate::curve_editor::Options {
            input: "Pressure",
            side: 160.,
            fixed_x: true,
            fixed_endpoints: false,
            max_points: 5,
            marker: Some(pressure),
            ..Default::default()
        },
    );
    for (value, p) in curve.iter_mut().zip(points) {
        *value = p[1];
    }
    ui.data_mut(|d| d.insert_temp(id, selected));
}
