//! Portable brush presets and pressure controls; no platform-specific assets.
use crate::brush_details::PropertyLayout;
use crate::{app::Parfait, widgets};
use eframe::egui;
use parfait_core::{Brush, BrushOperation, BrushTip, Document, Point, Stroke};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct Preset {
    pub name: String,
    pub brush: Brush,
    #[serde(default)]
    pub favorite: bool,
    #[serde(default)]
    pub properties: PropertyLayout,
}
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BrushGroup {
    #[default]
    Ink,
    Sketch,
    Paint,
    Airbrush,
    Eraser,
    Blend,
    Materials,
}
impl BrushGroup {
    fn icon(self) -> widgets::Icon {
        match self {
            Self::Ink => widgets::Icon::Pen,
            Self::Sketch => widgets::Icon::Pencil,
            Self::Paint => widgets::Icon::Brush,
            Self::Airbrush => widgets::Icon::Airbrush,
            Self::Eraser => widgets::Icon::Eraser,
            Self::Blend => widgets::Icon::Blend,
            Self::Materials => widgets::Icon::Material,
        }
    }
    pub const ALL: [Self; 7] = [
        Self::Ink,
        Self::Sketch,
        Self::Paint,
        Self::Airbrush,
        Self::Eraser,
        Self::Blend,
        Self::Materials,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Ink => "Ink",
            Self::Sketch => "Sketch",
            Self::Paint => "Paint",
            Self::Airbrush => "Airbrush",
            Self::Eraser => "Eraser",
            Self::Blend => "Blend",
            Self::Materials => "Materials",
        }
    }
    pub fn of(brush: &Brush) -> Self {
        if matches!(
            brush.operation(),
            BrushOperation::Blend | BrushOperation::Blur | BrushOperation::Smudge
        ) {
            return Self::Blend;
        }
        if brush.operation() == BrushOperation::Erase {
            return Self::Eraser;
        }
        match brush.tip {
            BrushTip::Round => Self::Ink,
            BrushTip::Pencil => Self::Sketch,
            BrushTip::Flat => Self::Paint,
            BrushTip::Soft => Self::Airbrush,
            BrushTip::Image { .. } => Self::Materials,
        }
    }
}
#[derive(Serialize, Deserialize)]
pub struct BrushLibrary {
    pub group: Option<BrushGroup>,
    pub favorites_only: bool,
    #[serde(skip)]
    pub query: String,
    #[serde(skip)]
    remembered: Vec<(BrushGroup, Brush, String, PropertyLayout)>,
    #[serde(skip)]
    previews: Vec<Option<(Brush, egui::TextureHandle)>>,
    #[cfg(test)]
    #[serde(skip)]
    targets: Vec<(String, egui::Rect)>,
}
impl Default for BrushLibrary {
    fn default() -> Self {
        Self {
            group: Some(BrushGroup::Ink),
            favorites_only: false,
            query: String::new(),
            remembered: Vec::new(),
            previews: Vec::new(),
            #[cfg(test)]
            targets: Vec::new(),
        }
    }
}
impl BrushLibrary {
    pub fn preferences(&self) -> Self {
        Self {
            group: self.group,
            favorites_only: self.favorites_only,
            ..Default::default()
        }
    }
    fn matches(&self, preset: &Preset) -> bool {
        self.group
            .is_none_or(|group| group == BrushGroup::of(&preset.brush))
            && (!self.favorites_only || preset.favorite)
            && preset
                .name
                .to_lowercase()
                .contains(&self.query.trim().to_lowercase())
    }
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
            favorite: false,
            properties: Default::default(),
            brush: Brush::default(),
        },
        Preset {
            name: "Soft airbrush".into(),
            favorite: false,
            properties: Default::default(),
            brush: Brush {
                tip: BrushTip::Soft,
                size: 70.,
                flow: 0.25,
                pressure_opacity: true,
                ..Default::default()
            },
        },
        Preset {
            name: "Dotted pen".into(),
            favorite: false,
            properties: Default::default(),
            brush: Brush {
                size: 8.,
                spacing: 1.5,
                pressure: false,
                ..Default::default()
            },
        },
        Preset {
            name: "Continuous airbrush".into(),
            favorite: false,
            properties: Default::default(),
            brush: Brush {
                tip: BrushTip::Soft,
                size: 70.,
                flow: 0.06,
                pressure: false,
                continuous_spray: true,
                ..Default::default()
            },
        },
        Preset {
            name: "Graphite pencil".into(),
            favorite: false,
            properties: Default::default(),
            brush: Brush {
                tip: BrushTip::Pencil,
                size: 8.,
                opacity: 0.7,
                ..Default::default()
            },
        },
        Preset {
            name: "Flat marker".into(),
            favorite: false,
            properties: Default::default(),
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
            favorite: false,
            properties: Default::default(),
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
            favorite: false,
            properties: Default::default(),
            brush: Brush {
                tip: BrushTip::Pencil,
                size: 24.,
                spacing: 0.22,
                flow: 0.35,
                pressure_opacity: true,
                ..Default::default()
            },
        },
        Preset {
            name: "Hard eraser".into(),
            favorite: false,
            properties: Default::default(),
            brush: Brush {
                size: 30.,
                erase: true,
                ..Default::default()
            },
        },
        Preset {
            name: "Soft eraser".into(),
            favorite: false,
            properties: Default::default(),
            brush: Brush {
                size: 60.,
                erase: true,
                tip: BrushTip::Soft,
                ..Default::default()
            },
        },
        Preset {
            name: "Color blend".into(),
            favorite: false,
            properties: PropertyLayout::blending(false),
            brush: Brush {
                operation: BrushOperation::Blend,
                size: 48.,
                flow: 0.35,
                pressure: false,
                hardness: 0.5,
                ..Default::default()
            },
        },
        Preset {
            name: "Local blur".into(),
            favorite: false,
            properties: PropertyLayout::blending(true),
            brush: Brush {
                operation: BrushOperation::Blur,
                size: 64.,
                flow: 0.5,
                pressure: false,
                hardness: 0.5,
                ..Default::default()
            },
        },
        Preset {
            name: "Smudge".into(),
            favorite: false,
            properties: PropertyLayout::blending(false),
            brush: Brush {
                operation: BrushOperation::Smudge,
                size: 30.,
                flow: 0.75,
                color_stretch: 0.7,
                pressure: false,
                hardness: 0.6,
                ..Default::default()
            },
        },
        Preset {
            name: "Capped marker".into(),
            favorite: false,
            properties: Default::default(),
            brush: Brush {
                opacity_mode: parfait_core::OpacityMode::PerStroke,
                tip: BrushTip::Flat,
                size: 30.,
                opacity: 0.4,
                flow: 0.35,
                pressure: false,
                ..Default::default()
            },
        },
    ]
}

pub(crate) fn preview_image(brush: &Brush) -> egui::ColorImage {
    stroke_preview(brush, [188, 188, 188, 255], [35, 35, 38])
}

fn library_preview_image(brush: &Brush) -> egui::ColorImage {
    stroke_preview(brush, [0, 0, 0, 0], [235, 235, 235])
}

fn stroke_preview(brush: &Brush, background: [u8; 4], ink: [u8; 3]) -> egui::ColorImage {
    let mut doc = Document::new(240, 56);
    for pixel in std::sync::Arc::make_mut(&mut doc.layers[0].pixels)
        .as_chunks_mut::<4>()
        .0
    {
        pixel.copy_from_slice(&background);
    }
    let mut preview = brush.clone();
    preview.size = preview.size.min(38.);
    preview.color = ink;
    preview.erase = false;
    if matches!(
        preview.operation(),
        BrushOperation::Blend | BrushOperation::Blur | BrushOperation::Smudge
    ) {
        let pixels = Arc::make_mut(&mut doc.layers[1].pixels);
        for row in 18..38 {
            for column in 16..224 {
                let index = ((row * 240 + column) * 4) as usize;
                let color = if column < 120 {
                    if preview.operation() == BrushOperation::Smudge && (row / 3) % 2 == 0 {
                        ink.map(|component| 255 - component)
                    } else {
                        ink
                    }
                } else {
                    ink.map(|component| component / 2)
                };
                pixels[index..index + 4].copy_from_slice(&[color[0], color[1], color[2], 255]);
            }
        }
    } else {
        preview.operation = BrushOperation::Paint;
    }
    let point = |step: usize| {
        let t = step as f32 / 64.;
        Point {
            x: 16. + t * 208.,
            y: 28. - (t * std::f32::consts::TAU).sin() * 9.,
            pressure: (t * std::f32::consts::PI).sin().max(0.03),
        }
    };
    let mut stroke = Stroke::begin(&mut doc, preview, point(0));
    for step in 1..=64 {
        stroke.extend(&mut doc, point(step));
    }
    egui::ColorImage::from_rgba_unmultiplied([240, 56], &doc.composite())
}

impl Parfait {
    fn remember_brush(&mut self) {
        let group = BrushGroup::of(&self.brush);
        if let Some(saved) = self
            .brush_library
            .remembered
            .iter_mut()
            .find(|entry| entry.0 == group)
        {
            *saved = (
                group,
                self.brush.clone(),
                self.tip_name.clone(),
                self.brush_details.properties,
            );
        } else {
            self.brush_library.remembered.push((
                group,
                self.brush.clone(),
                self.tip_name.clone(),
                self.brush_details.properties,
            ));
        }
    }
    pub fn select_brush_group(&mut self, group: BrushGroup) {
        if group == BrushGroup::Blend {
            for preset in defaults()
                .into_iter()
                .filter(|preset| BrushGroup::of(&preset.brush) == BrushGroup::Blend)
            {
                if self.presets.len() < 64
                    && !self
                        .presets
                        .iter()
                        .any(|existing| existing.name == preset.name)
                {
                    self.presets.push(preset);
                }
            }
        }
        self.navigation_tool = 0;
        self.brush_library.group = Some(group);
        self.brush_library.query.clear();
        self.brush_library.favorites_only = false;
        if BrushGroup::of(&self.brush) == group {
            return;
        }
        self.brush_details.clear_help();
        self.remember_brush();
        let color = self.brush.color;
        if let Some((_, brush, name, properties)) = self
            .brush_library
            .remembered
            .iter()
            .find(|entry| entry.0 == group)
        {
            self.brush = brush.clone();
            self.tip_name = name.clone();
            self.brush_details.properties = *properties;
        } else if let Some(preset) = self
            .presets
            .iter()
            .chain(defaults().iter())
            .find(|preset| BrushGroup::of(&preset.brush) == group)
            .cloned()
        {
            self.brush = preset.brush.clone();
            self.tip_name = preset.name.clone();
            self.brush_details.properties = preset.properties;
            if self.presets.len() < 64
                && !self
                    .presets
                    .iter()
                    .any(|existing| existing.name == preset.name)
            {
                self.presets.push(preset);
            }
        }
        self.brush.color = color;
    }
    fn select_preset(&mut self, index: usize) {
        self.brush_details.clear_help();
        self.remember_brush();
        let color = self.brush.color;
        self.brush = self.presets[index].brush.clone();
        self.brush.color = color;
        self.tip_name = self.presets[index].name.clone();
        self.brush_details.properties = self.presets[index].properties;
        self.navigation_tool = 0;
    }
    pub(crate) fn preset_modified(&self) -> bool {
        self.presets
            .iter()
            .find(|preset| preset.name == self.tip_name)
            .is_none_or(|preset| {
                let mut saved = preset.brush.clone();
                saved.color = self.brush.color;
                saved != self.brush || preset.properties != self.brush_details.properties
            })
    }
    pub(crate) fn restore_current_preset(&mut self) {
        if let Some(index) = self
            .presets
            .iter()
            .position(|preset| preset.name == self.tip_name)
        {
            self.select_preset(index);
        }
    }
    pub fn brush_library_ui(&mut self, ui: &mut egui::Ui) {
        #[cfg(test)]
        self.brush_library.targets.clear();
        ui.spacing_mut().item_spacing = egui::vec2(2., 1.);
        let search_id = ui.id().with("library_search_visible");
        let mut show_search = ui
            .data(|data| data.get_temp::<bool>(search_id))
            .unwrap_or(false)
            || !self.brush_library.query.is_empty();
        let mut focus_search = false;
        ui.horizontal(|ui| {
            ui.spacing_mut().interact_size.y = 22.;
            let filter = egui::ComboBox::from_id_salt("brush_family_filter")
                .selected_text(
                    self.brush_library
                        .group
                        .map_or("All brushes", BrushGroup::label),
                )
                .width((ui.available_width() - 76.).max(64.))
                .truncate()
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.brush_library.group, None, "All brushes");
                    for group in BrushGroup::ALL {
                        let choice = ui.selectable_value(
                            &mut self.brush_library.group,
                            Some(group),
                            group.label(),
                        );
                        #[cfg(test)]
                        self.brush_library
                            .targets
                            .push((format!("Filter {}", group.label()), choice.rect));
                        #[cfg(not(test))]
                        let _ = choice;
                    }
                })
                .response
                .on_hover_text("Browse a brush family without changing the active drawing tool");
            #[cfg(test)]
            self.brush_library
                .targets
                .push(("Family filter".into(), filter.rect));
            #[cfg(not(test))]
            let _ = filter;
            let search = widgets::flat_icon(
                ui,
                widgets::Icon::Search,
                show_search,
                if show_search {
                    "Close search and clear filter"
                } else {
                    "Search brushes"
                },
                22.,
            );
            #[cfg(test)]
            self.brush_library
                .targets
                .push(("Search brushes".into(), search.rect));
            if search.clicked() {
                show_search = !show_search;
                if show_search {
                    focus_search = true;
                } else {
                    self.brush_library.query.clear();
                }
            }
            let favorites = widgets::flat_icon(
                ui,
                widgets::Icon::Favorite,
                self.brush_library.favorites_only,
                "Show favorites only",
                22.,
            );
            #[cfg(test)]
            self.brush_library
                .targets
                .push(("Show favorites only".into(), favorites.rect));
            if favorites.clicked() {
                self.brush_library.favorites_only = !self.brush_library.favorites_only;
            }
            self.brush_library_menu(ui);
        });
        if show_search {
            let search = ui.add(
                egui::TextEdit::singleline(&mut self.brush_library.query)
                    .char_limit(64)
                    .hint_text("Search brushes…")
                    .desired_width(ui.available_width()),
            );
            if focus_search {
                search.request_focus();
            }
            #[cfg(test)]
            self.brush_library
                .targets
                .push(("Search input".into(), search.rect));
        }
        ui.data_mut(|data| data.insert_temp(search_id, show_search));
        let count = self
            .presets
            .iter()
            .filter(|preset| self.brush_library.matches(preset))
            .count();
        if count == 0 {
            ui.label("No matching brushes.");
            if ui.button("Show all brushes").clicked() {
                self.brush_library.group = None;
                self.brush_library.favorites_only = false;
                self.brush_library.query.clear();
            }
        }
        self.brush_library
            .previews
            .resize_with(self.presets.len(), || None);
        let mut regenerated = false;
        for index in 0..self.presets.len() {
            if !self.brush_library.matches(&self.presets[index]) {
                continue;
            }
            ui.push_id(index, |ui| {
                let (rect, _) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), 40.),
                    egui::Sense::hover(),
                );
                if ui.is_rect_visible(rect) && self.stroke.is_none() {
                    let changed = self.brush_library.previews[index]
                        .as_ref()
                        .is_none_or(|(cached, _)| cached != &self.presets[index].brush);
                    if changed && !regenerated {
                        let image = library_preview_image(&self.presets[index].brush);
                        if let Some((cached, texture)) = &mut self.brush_library.previews[index] {
                            texture.set(image, egui::TextureOptions::LINEAR);
                            *cached = self.presets[index].brush.clone();
                        } else {
                            self.brush_library.previews[index] = Some((
                                self.presets[index].brush.clone(),
                                ui.ctx().load_texture(
                                    format!("preset-{index}"),
                                    image,
                                    egui::TextureOptions::LINEAR,
                                ),
                            ));
                        }
                        regenerated = true;
                        ui.ctx().request_repaint();
                    } else if changed {
                        ui.ctx().request_repaint();
                    }
                }
                let preview = self.brush_library.previews[index]
                    .as_ref()
                    .map(|(_, texture)| texture.id());
                let row = widgets::brush_row(
                    ui,
                    rect,
                    &self.presets[index].name,
                    self.tip_name == self.presets[index].name
                        && self.brush.operation() == self.presets[index].brush.operation(),
                    BrushGroup::of(&self.presets[index].brush).icon(),
                    preview,
                );
                #[cfg(test)]
                self.brush_library
                    .targets
                    .push((self.presets[index].name.clone(), row.rect));
                if row.clicked() {
                    self.select_preset(index);
                }
                ui.scope_builder(
                    egui::UiBuilder::new().max_rect(egui::Rect::from_min_max(
                        egui::pos2(rect.right() - 23., rect.top() + 9.),
                        egui::pos2(rect.right() - 1., rect.top() + 31.),
                    )),
                    |ui| {
                        let label = format!("Favorite {}", self.presets[index].name);
                        let favorite = &mut self.presets[index].favorite;
                        let response =
                            widgets::flat_icon(ui, widgets::Icon::Favorite, *favorite, &label, 22.);
                        #[cfg(test)]
                        self.brush_library.targets.push((label, response.rect));
                        if response.clicked() {
                            *favorite = !*favorite;
                        }
                    },
                );
            });
        }
    }
    fn brush_library_menu(&mut self, ui: &mut egui::Ui) {
        let menu = ui
            .scope(|ui| {
                ui.visuals_mut().button_frame = false;
                ui.menu_button(
                    egui::RichText::new(widgets::Icon::More.glyph()).font(egui::FontId::new(
                        14.,
                        egui::FontFamily::Name("icons".into()),
                    )),
                    |ui| {
                        ui.strong("Brush library");
                        if ui.button("Import tips / presets…").clicked() {
                            self.import_brushes();
                            ui.close();
                        }
                        if ui.button("Export active preset…").clicked() {
                            self.export_brush();
                            ui.close();
                        }
                        ui.separator();
                        ui.label("Preset name");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.tip_name)
                                .char_limit(32)
                                .desired_width(160.),
                        );
                        if ui.button("Save preset").clicked() {
                            self.save_current_preset();
                            ui.close();
                        }
                        if ui.button("Restore missing built-ins").clicked() {
                            for preset in defaults() {
                                if self.presets.len() < 64
                                    && !self
                                        .presets
                                        .iter()
                                        .any(|existing| existing.name == preset.name)
                                {
                                    self.presets.push(preset);
                                }
                            }
                            ui.close();
                        }
                    },
                )
            })
            .inner
            .response;
        menu.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), "Manage brushes")
        });
        #[cfg(test)]
        self.brush_library
            .targets
            .push(("Manage brushes".into(), menu.rect));
        menu.on_hover_text("Manage brushes · import, export and preset settings");
    }
    pub(crate) fn save_current_preset(&mut self) {
        self.tip_name = self.tip_name.trim().to_owned();
        let p = Preset {
            name: self.tip_name.trim().to_owned(),
            brush: self.brush.clone(),
            properties: self.brush_details.properties,
            favorite: self
                .presets
                .iter()
                .find(|preset| preset.name == self.tip_name.trim())
                .is_some_and(|preset| preset.favorite),
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
                    anyhow::ensure!(
                        (1..=8).contains(&file.version),
                        "Unsupported brush format version"
                    );
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
                        favorite: false,
                        properties: Default::default(),
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
                    self.presets.push(p);
                    self.select_preset(self.presets.len() - 1);
                    self.brush_library.group = None;
                    self.brush_library.query.clear();
                    self.brush_library.favorites_only = false;
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
                version: 8,
                preset: Preset {
                    name: self.tip_name.clone(),
                    brush: self.brush.clone(),
                    favorite: false,
                    properties: self.brush_details.properties,
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

pub(crate) fn pressure_graph(
    ui: &mut egui::Ui,
    curve: &mut parfait_core::ResponseCurve,
    pressure: f32,
) -> egui::Rect {
    let id = ui.id().with("brush_pressure_selection");
    let mut selected = ui
        .data(|d| d.get_temp::<Option<usize>>(id))
        .unwrap_or(Some(0));
    let plot = crate::curve_editor::edit_brush(
        ui,
        "brush_pressure",
        curve,
        &mut selected,
        crate::curve_editor::Options {
            input: "Pressure",
            side: 200.,
            fixed_x: false,
            fixed_endpoints: false,
            increasing: false,
            max_points: parfait_core::MAX_CURVE_POINTS,
            delete_on_drag_out: true,
            marker: Some(pressure),
            ..Default::default()
        },
    );
    ui.data_mut(|d| d.insert_temp(id, selected));
    ui.weak(format!(
        "{} / {} anchors · select an anchor for handles",
        curve.points.len(),
        parfait_core::MAX_CURVE_POINTS
    ));
    plot
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn smudge_preset_drafts_portable_v6_and_restart_keep_spatial_operation() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open(dir.path().to_owned()).unwrap();
        let artwork = app.doc.clone();
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        app.select_brush_group(BrushGroup::Blend);
        library_frame(&mut app, &ctx, 210., vec![]);
        library_click(&mut app, &ctx, "Smudge");
        assert_eq!(app.brush.operation(), BrushOperation::Smudge);
        app.brush.color_stretch = 0.91;
        let color = app.brush.color;
        app.select_brush_group(BrushGroup::Ink);
        app.select_brush_group(BrushGroup::Blend);
        assert_eq!(app.brush.operation(), BrushOperation::Smudge);
        assert_eq!(app.brush.color_stretch, 0.91);
        assert_eq!(app.brush.color, color);
        app.save_current_preset();
        let preset = app
            .presets
            .iter()
            .find(|preset| preset.name == "Smudge")
            .unwrap()
            .clone();
        let file: BrushFile = serde_json::from_str(
            &serde_json::to_string(&BrushFile { version: 6, preset }).unwrap(),
        )
        .unwrap();
        assert_eq!(file.version, 6);
        assert!(file.preset.brush == app.brush);
        assert_eq!(file.preset.properties, app.brush_details.properties);
        let mut changed = app.brush.clone();
        changed.operation = BrushOperation::Blend;
        assert_ne!(
            preview_image(&app.brush).pixels,
            preview_image(&changed).pixels
        );
        changed = app.brush.clone();
        changed.color = [220, 10, 200];
        assert_eq!(
            preview_image(&app.brush).pixels,
            preview_image(&changed).pixels
        );
        assert_eq!(app.doc, artwork);
        app.persist_preferences(true);
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&app.saved_preferences_json()).unwrap()["version"],
            18
        );
        drop(app);
        let app = Parfait::open(dir.path().to_owned()).unwrap();
        assert_eq!(app.brush.operation(), BrushOperation::Smudge);
        assert_eq!(app.brush.color_stretch, 0.91);
        for version in 1..=5 {
            let encoded = serde_json::json!({"version":version,"preset":{"name":"old","brush":{}}});
            let old: BrushFile = serde_json::from_value(encoded).unwrap();
            assert_eq!(old.preset.brush.operation(), BrushOperation::Paint);
        }
    }

    #[test]
    fn blend_group_restores_drafts_and_exchange_preserves_operations() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open(dir.path().to_owned()).unwrap();
        app.presets
            .retain(|preset| BrushGroup::of(&preset.brush) != BrushGroup::Blend);
        let color = [15, 210, 98];
        app.brush.color = color;
        app.select_brush_group(BrushGroup::Blend);
        assert_eq!(app.brush.operation(), BrushOperation::Blend);
        assert_eq!(app.brush.color, color);
        assert_eq!(
            app.presets
                .iter()
                .filter(|preset| BrushGroup::of(&preset.brush) == BrushGroup::Blend)
                .count(),
            3
        );
        app.brush.color_stretch = 0.19;
        app.select_brush_group(BrushGroup::Ink);
        app.select_brush_group(BrushGroup::Blend);
        assert_eq!(app.brush.color_stretch, 0.19);
        app.brush.operation = BrushOperation::Blur;
        app.brush.blur_automatic = false;
        app.brush.blur_radius = 13.;
        app.save_current_preset();
        let preset = app
            .presets
            .iter()
            .find(|preset| preset.name == app.tip_name)
            .unwrap()
            .clone();
        let encoded = serde_json::to_value(BrushFile { version: 5, preset }).unwrap();
        let decoded: BrushFile = serde_json::from_value(encoded.clone()).unwrap();
        assert_eq!(decoded.version, 5);
        assert!(decoded.preset.brush == app.brush);
        let sample = preview_image(&app.brush);
        let mut recolored = app.brush.clone();
        recolored.color = [250, 0, 20];
        assert_eq!(sample.pixels, preview_image(&recolored).pixels);
        recolored.operation = BrushOperation::Blend;
        assert_ne!(sample.pixels, preview_image(&recolored).pixels);
        app.persist_preferences(true);
        drop(app);
        let restored = Parfait::open(dir.path().to_owned()).unwrap();
        assert_eq!(restored.brush.operation(), BrushOperation::Blur);
        assert!(!restored.brush.blur_automatic);
        assert_eq!(restored.brush.blur_radius, 13.);
        for version in 1..=4 {
            let mut legacy = encoded.clone();
            legacy["version"] = version.into();
            for name in [
                "operation",
                "color_stretch",
                "blur_automatic",
                "blur_radius",
            ] {
                legacy["preset"]["brush"]
                    .as_object_mut()
                    .unwrap()
                    .remove(name);
            }
            let decoded: BrushFile = serde_json::from_value(legacy.clone()).unwrap();
            assert_eq!(decoded.preset.brush.operation(), BrushOperation::Paint);
            assert!(decoded.preset.brush.validate());
            legacy["preset"]["brush"]["erase"] = true.into();
            let decoded: BrushFile = serde_json::from_value(legacy).unwrap();
            assert_eq!(decoded.preset.brush.operation(), BrushOperation::Erase);
        }
        let mut invalid = encoded;
        invalid["preset"]["brush"]["operation"] = "FutureOperation".into();
        assert!(serde_json::from_value::<BrushFile>(invalid).is_err());
    }

    fn library_frame(
        app: &mut Parfait,
        ctx: &egui::Context,
        width: f32,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
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
                egui::CentralPanel::default().show(ui, |ui| {
                    ui.scope_builder(
                        egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(
                            ui.cursor().min,
                            egui::vec2(width, 600.),
                        )),
                        |ui| {
                            ui.set_width(width);
                            app.brush_library_ui(ui);
                        },
                    );
                });
            },
        );
        output.textures_delta.clear();
        output
    }

    fn library_target(app: &Parfait, name: &str) -> egui::Rect {
        app.brush_library
            .targets
            .iter()
            .find(|(label, _)| label == name)
            .unwrap_or_else(|| panic!("Missing target: {name}"))
            .1
    }

    fn library_click(app: &mut Parfait, ctx: &egui::Context, name: &str) {
        let position = library_target(app, name).center();
        let events = vec![
            egui::Event::PointerMoved(position),
            egui::Event::PointerButton {
                pos: position,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            },
            egui::Event::PointerButton {
                pos: position,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            },
        ];
        library_frame(app, ctx, 210., events);
        library_frame(app, ctx, 210., vec![]);
    }

    #[test]
    fn compact_library_header_and_rows_fit_narrow_palettes() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open(dir.path().to_owned()).unwrap();
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let artwork = app.doc.clone();
        for width in [190., 210., 246., 360.] {
            library_frame(&mut app, &ctx, width, vec![]);
            let header: Vec<_> = [
                "Family filter",
                "Search brushes",
                "Show favorites only",
                "Manage brushes",
            ]
            .map(|name| library_target(&app, name))
            .into();
            assert!(
                header
                    .windows(2)
                    .all(|pair| pair[0].right() <= pair[1].left())
            );
            assert!(
                header
                    .iter()
                    .all(|rect| rect.right() <= header[0].left() + width + 0.1),
                "width={width}, header={header:?}"
            );
            assert!(
                header
                    .iter()
                    .all(|rect| rect.center().y == header[0].center().y)
            );
            let row = library_target(&app, "Round pen");
            let favorite = library_target(&app, "Favorite Round pen");
            assert_eq!(row.height(), 40.);
            assert!(row.top() - header[0].top() <= 25.);
            assert!(row.right() <= favorite.left());
            assert!(favorite.right() <= header[0].left() + width);
            assert!(
                !app.brush_library
                    .targets
                    .iter()
                    .any(|(name, _)| name == "Search input")
            );
        }
        assert_eq!(app.doc, artwork);
    }

    #[test]
    fn compact_library_favorites_search_and_preset_targets_remain_independent() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open(dir.path().to_owned()).unwrap();
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let artwork = app.doc.clone();
        let original = app.brush.clone();
        library_frame(&mut app, &ctx, 210., vec![]);
        library_click(&mut app, &ctx, "Favorite Dotted pen");
        assert!(
            app.presets
                .iter()
                .find(|preset| preset.name == "Dotted pen")
                .unwrap()
                .favorite
        );
        assert_eq!(app.tip_name, "Round pen");
        assert!(app.brush == original);
        library_click(&mut app, &ctx, "Dotted pen");
        assert_eq!(app.tip_name, "Dotted pen");
        let dotted = app.brush.clone();
        library_click(&mut app, &ctx, "Search brushes");
        assert!(ctx.egui_wants_keyboard_input());
        library_frame(
            &mut app,
            &ctx,
            210.,
            vec![egui::Event::Text("ROUND".into())],
        );
        assert_eq!(app.brush_library.query, "ROUND");
        assert!(
            app.brush_library
                .targets
                .iter()
                .any(|(name, _)| name == "Round pen")
        );
        assert!(
            !app.brush_library
                .targets
                .iter()
                .any(|(name, _)| name == "Dotted pen")
        );
        library_click(&mut app, &ctx, "Show favorites only");
        assert!(app.brush_library.favorites_only);
        assert!(
            !app.brush_library
                .targets
                .iter()
                .any(|(name, _)| name == "Round pen")
        );
        library_click(&mut app, &ctx, "Show favorites only");
        library_click(&mut app, &ctx, "Search brushes");
        assert!(app.brush_library.query.is_empty());
        assert!(
            !app.brush_library
                .targets
                .iter()
                .any(|(name, _)| name == "Search input")
        );
        assert!(app.brush == dotted);
        assert_eq!(app.doc, artwork);
    }

    #[test]
    fn family_dropdown_only_filters_and_manage_menu_keeps_its_actions() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open(dir.path().to_owned()).unwrap();
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let brush = app.brush.clone();
        let artwork = app.doc.clone();
        library_frame(&mut app, &ctx, 210., vec![]);
        library_click(&mut app, &ctx, "Family filter");
        library_click(&mut app, &ctx, "Filter Eraser");
        assert_eq!(app.brush_library.group, Some(BrushGroup::Eraser));
        assert!(app.brush == brush);
        assert_eq!(app.tip_name, "Round pen");
        library_click(&mut app, &ctx, "Manage brushes");
        let output = library_frame(&mut app, &ctx, 210., vec![]);
        fn contains_text(shape: &egui::Shape, label: &str) -> bool {
            match shape {
                egui::Shape::Text(text) => text.galley.job.text == label,
                egui::Shape::Vec(shapes) => shapes.iter().any(|shape| contains_text(shape, label)),
                _ => false,
            }
        }
        for label in [
            "Import tips / presets…",
            "Export active preset…",
            "Save preset",
            "Restore missing built-ins",
        ] {
            assert!(
                output
                    .shapes
                    .iter()
                    .any(|shape| contains_text(&shape.shape, label)),
                "{label}"
            );
        }
        assert_eq!(app.doc, artwork);
    }

    #[test]
    fn library_samples_use_transparent_light_ink_without_changing_property_samples() {
        let brush = Brush::default();
        let library = library_preview_image(&brush);
        let property = preview_image(&brush);
        assert_eq!(library.size, [240, 56]);
        assert_eq!(library.pixels[0], egui::Color32::TRANSPARENT);
        assert_eq!(property.pixels[0], egui::Color32::from_gray(188));
        assert!(
            library
                .pixels
                .iter()
                .any(|pixel| pixel.a() == 255 && pixel.r() == 235)
        );
        assert!(
            property
                .pixels
                .iter()
                .any(|pixel| pixel.r() == 35 && pixel.g() == 35 && pixel.b() == 38)
        );
        for pixel in library.pixels.iter().filter(|pixel| pixel.a() > 0) {
            assert_eq!(pixel.r(), pixel.g());
            assert_eq!(pixel.g(), pixel.b());
        }
        let mut recolored = brush;
        recolored.color = [255, 0, 120];
        assert_eq!(library.pixels, library_preview_image(&recolored).pixels);
    }

    #[test]
    fn library_preview_updates_remain_bounded_and_pause_during_drawing() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open(dir.path().to_owned()).unwrap();
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        library_frame(&mut app, &ctx, 210., vec![]);
        assert_eq!(app.brush_library.previews.iter().flatten().count(), 1);
        let original_texture = app.brush_library.previews[0].as_ref().unwrap().1.id();
        app.presets[0].brush.opacity = 0.3;
        library_frame(&mut app, &ctx, 210., vec![]);
        assert_eq!(app.brush_library.previews.iter().flatten().count(), 1);
        assert_eq!(
            app.brush_library.previews[0].as_ref().unwrap().1.id(),
            original_texture
        );
        app.stroke = Some(Stroke::begin(
            &mut app.doc,
            app.brush.clone(),
            Point {
                x: 20.,
                y: 20.,
                pressure: 1.,
            },
        ));
        for _ in 0..3 {
            library_frame(&mut app, &ctx, 210., vec![]);
            assert_eq!(app.brush_library.previews.iter().flatten().count(), 1);
        }
        app.stroke = None;
        library_frame(&mut app, &ctx, 210., vec![]);
        assert_eq!(app.brush_library.previews.iter().flatten().count(), 2);
    }

    #[test]
    fn legacy_samples_migrate_and_arbitrary_curve_coordinates_survive_every_preset_path() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open(dir.path().to_owned()).unwrap();
        app.persist_preferences(true);
        let mut legacy: serde_json::Value =
            serde_json::from_str(&app.saved_preferences_json()).unwrap();
        legacy["version"] = serde_json::json!(1);
        legacy["brush"]["pressure_curve"] = serde_json::json!([0., 0.1, 0.4, 0.8, 1.]);
        legacy["presets"][0]["brush"]["pressure_curve"] = legacy["brush"]["pressure_curve"].clone();
        drop(app);
        parfait_storage::save_preferences(
            &dir.path().join("preferences.sqlite"),
            &legacy.to_string(),
        )
        .unwrap();
        let mut app = Parfait::open(dir.path().to_owned()).unwrap();
        assert!(!app.preferences_read_failed);
        assert_eq!(
            app.brush.pressure_curve.points,
            vec![[0., 0.], [0.25, 0.1], [0.5, 0.4], [0.75, 0.8], [1., 1.]]
        );
        let original = app.doc.clone();
        let curve = parfait_core::ResponseCurve {
            points: vec![
                [0., 0.15],
                [0.17, 0.9],
                [0.62, 0.2],
                [0.81, 0.7],
                [1., 0.95],
            ],
            controls: Some(vec![
                [[0.04, 0.4], [0.13, 0.8]],
                [[0.25, 0.8], [0.5, 0.3]],
                [[0.7, 0.3], [0.75, 0.6]],
                [[0.85, 0.7], [0.95, 0.9]],
            ]),
        };
        app.brush.pressure_curve = curve.clone();
        app.brush.gap_mode = parfait_core::GapMode::Narrow;
        app.brush.spacing = 2.5;
        app.brush.continuous_spray = true;
        app.brush.spray_rate = 48.;
        app.save_current_preset();
        app.select_brush_group(BrushGroup::Eraser);
        app.select_brush_group(BrushGroup::Ink);
        assert_eq!(app.brush.pressure_curve, curve);
        app.brush.pressure_curve = Default::default();
        app.brush.gap_mode = parfait_core::GapMode::Fixed;
        app.brush.continuous_spray = false;
        app.restore_current_preset();
        assert_eq!(app.brush.pressure_curve, curve);
        assert_eq!(app.doc, original);
        let portable = BrushFile {
            version: 4,
            preset: app.presets[0].clone(),
        };
        let encoded = serde_json::to_vec(&portable).unwrap();
        let decoded: BrushFile = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(decoded.version, 4);
        assert_eq!(decoded.preset.brush.pressure_curve, curve);
        assert_eq!(decoded.preset.brush.gap_mode, parfait_core::GapMode::Narrow);
        assert_eq!(decoded.preset.brush.spacing, 2.5);
        assert!(decoded.preset.brush.continuous_spray);
        assert_eq!(decoded.preset.brush.spray_rate, 48.);
        let old_file = serde_json::json!({"version":1,"preset":{"name":"Legacy","brush":{"pressure_curve":[0.,0.1,0.4,0.8,1.]}}});
        let decoded: BrushFile = serde_json::from_value(old_file).unwrap();
        assert_eq!(decoded.preset.brush.pressure_curve.points[1], [0.25, 0.1]);
        let version_two = serde_json::json!({"version":2,"preset":{"name":"Coordinates","brush":{"pressure_curve":{"points":[[0.,0.],[0.37,0.7],[1.,1.]]}}}});
        let decoded: BrushFile = serde_json::from_value(version_two).unwrap();
        assert_eq!(decoded.preset.brush.pressure_curve.points[1], [0.37, 0.7]);
        assert!(decoded.preset.brush.pressure_curve.controls.is_none());
        for malformed in [
            serde_json::json!({"points":[[0.,0.],[0.,1.]]}),
            serde_json::json!({"points":[[0.,0.],[1.,2.]]}),
            serde_json::json!({"points":vec![[0.,0.];17]}),
            serde_json::json!({"points":[[0.,0.],[1.,1.]],"controls":[[[0.8,0.2],[0.2,0.8]]]}),
            serde_json::json!({"points":[[0.,0.],[1.,1.]],"controls":[]}),
        ] {
            assert!(serde_json::from_value::<parfait_core::ResponseCurve>(malformed).is_err());
        }
        app.persist_preferences(true);
        assert!(app.error.is_none());
        let saved: serde_json::Value = serde_json::from_str(&app.saved_preferences_json()).unwrap();
        assert_eq!(saved["version"], 18);
        drop(app);
        let restored = Parfait::open(dir.path().to_owned()).unwrap();
        assert_eq!(restored.brush.pressure_curve, curve);
        assert_eq!(restored.presets[0].brush.pressure_curve, curve);
        assert_eq!(restored.brush.gap_mode, parfait_core::GapMode::Narrow);
        assert!(restored.brush.continuous_spray);
        assert_eq!(restored.brush.spray_rate, 48.);
    }

    #[test]
    fn previous_preference_versions_default_to_fixed_gap_and_no_stationary_spray() {
        for version in 1..=3 {
            let dir = tempfile::tempdir().unwrap();
            let mut app = Parfait::open(dir.path().to_owned()).unwrap();
            app.brush.spacing = 0.67;
            app.persist_preferences(true);
            let mut encoded: serde_json::Value =
                serde_json::from_str(&app.saved_preferences_json()).unwrap();
            encoded["version"] = serde_json::json!(version);
            for key in ["gap_mode", "continuous_spray", "spray_rate"] {
                encoded["brush"].as_object_mut().unwrap().remove(key);
                for preset in encoded["presets"].as_array_mut().unwrap() {
                    preset["brush"].as_object_mut().unwrap().remove(key);
                }
            }
            drop(app);
            parfait_storage::save_preferences(
                &dir.path().join("preferences.sqlite"),
                &encoded.to_string(),
            )
            .unwrap();
            let restored = Parfait::open(dir.path().to_owned()).unwrap();
            assert!(!restored.preferences_read_failed);
            assert_eq!(restored.brush.gap_mode, parfait_core::GapMode::Fixed);
            assert_eq!(restored.brush.spacing, 0.67);
            assert!(!restored.brush.continuous_spray);
            assert_eq!(restored.brush.spray_rate, 30.);
        }
    }

    #[test]
    fn tool_switch_restores_unsaved_settings_and_preserves_color_and_artwork() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open(dir.path().to_owned()).unwrap();
        let artwork = app.doc.clone();
        app.brush.size = 23.;
        app.brush.color = [12, 80, 165];
        app.select_brush_group(BrushGroup::Eraser);
        assert!(app.brush.erase);
        app.brush.size = 64.;
        app.select_brush_group(BrushGroup::Ink);
        assert!(!app.brush.erase);
        assert_eq!(app.brush.size, 23.);
        assert!(app.preset_modified());
        app.restore_current_preset();
        assert!(!app.preset_modified());
        assert_eq!(app.brush.size, 12.);
        app.select_brush_group(BrushGroup::Eraser);
        assert_eq!(app.brush.size, 64.);
        assert_eq!(app.brush.color, [12, 80, 165]);
        let pencil = app
            .presets
            .iter()
            .position(|preset| preset.name == "Graphite pencil")
            .unwrap();
        app.select_preset(pencil);
        assert!(!app.brush.erase);
        assert_eq!(BrushGroup::of(&app.brush), BrushGroup::Sketch);
        assert_eq!(app.doc, artwork);
    }

    #[test]
    fn favorites_filters_and_old_preferences_survive_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open(dir.path().to_owned()).unwrap();
        app.presets[0].favorite = true;
        app.brush_library.group = None;
        app.brush_library.favorites_only = true;
        app.brush_library.query = "ROUND".into();
        assert_eq!(
            app.presets
                .iter()
                .filter(|preset| app.brush_library.matches(preset))
                .count(),
            1
        );
        app.brush_library.group = Some(BrushGroup::Eraser);
        assert!(
            !app.presets
                .iter()
                .any(|preset| app.brush_library.matches(preset))
        );
        app.brush_library.group = None;
        app.brush.size = 41.;
        app.save_current_preset();
        assert!(app.presets[0].favorite);
        app.persist_preferences(true);
        assert!(app.error.is_none());
        drop(app);
        let restored = Parfait::open(dir.path().to_owned()).unwrap();
        assert!(restored.presets[0].favorite);
        assert!(restored.brush_library.favorites_only);
        assert_eq!(restored.brush.size, 41.);
        assert!(restored.brush_library.query.is_empty());
        let mut old: serde_json::Value =
            serde_json::from_str(&restored.saved_preferences_json()).unwrap();
        old.as_object_mut().unwrap().remove("brush_library");
        for preset in old["presets"].as_array_mut().unwrap() {
            preset.as_object_mut().unwrap().remove("favorite");
        }
        drop(restored);
        parfait_storage::save_preferences(
            &dir.path().join("preferences.sqlite"),
            &serde_json::to_string(&old).unwrap(),
        )
        .unwrap();
        let legacy = Parfait::open(dir.path().to_owned()).unwrap();
        assert!(!legacy.preferences_read_failed);
        assert_eq!(legacy.brush.size, 41.);
        assert!(legacy.presets.iter().all(|preset| !preset.favorite));
        assert_eq!(legacy.brush_library.group, Some(BrushGroup::Ink));
    }

    #[test]
    fn old_brush_files_and_missing_erasers_remain_usable() {
        let json = serde_json::json!({ "version": 1, "preset": { "name": "Old pencil", "brush": Brush { tip: BrushTip::Pencil, ..Default::default() } } });
        let file: BrushFile = serde_json::from_value(json).unwrap();
        assert!(!file.preset.favorite);
        assert_eq!(BrushGroup::of(&file.preset.brush), BrushGroup::Sketch);
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open(dir.path().to_owned()).unwrap();
        app.presets.retain(|preset| !preset.brush.erase);
        app.select_brush_group(BrushGroup::Eraser);
        assert!(app.brush.erase);
        assert!(app.presets.iter().any(|preset| preset.name == app.tip_name));
        assert_ne!(
            preview_image(&Brush::default()).pixels,
            preview_image(&file.preset.brush).pixels
        );
    }
}
#[cfg(test)]
mod pressure_spacing_tests {
    use super::*;
    #[test]
    fn opacity_mode_portable_restart_and_legacy_preferences_keep_explicit_semantics() {
        use parfait_core::OpacityMode;
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open(dir.path().to_owned()).unwrap();
        let marker = app
            .presets
            .iter()
            .find(|preset| preset.name == "Capped marker")
            .unwrap()
            .clone();
        assert_eq!(marker.brush.opacity_mode, OpacityMode::PerStroke);
        let encoded = serde_json::to_value(BrushFile {
            version: 8,
            preset: marker.clone(),
        })
        .unwrap();
        let decoded: BrushFile = serde_json::from_value(encoded.clone()).unwrap();
        assert!(decoded.preset == marker);
        for version in 1..=7 {
            let mut legacy = encoded.clone();
            legacy["version"] = version.into();
            legacy["preset"]["brush"]
                .as_object_mut()
                .unwrap()
                .remove("opacity_mode");
            let decoded: BrushFile = serde_json::from_value(legacy).unwrap();
            assert_eq!(decoded.preset.brush.opacity_mode, OpacityMode::PerDab);
        }
        app.brush = marker.brush;
        app.tip_name = marker.name;
        app.persist_preferences(true);
        let json = app.saved_preferences_json();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&json).unwrap()["version"],
            18
        );
        drop(app);
        let mut app = Parfait::open(dir.path().to_owned()).unwrap();
        assert_eq!(app.brush.opacity_mode, OpacityMode::PerStroke);
        let path = dir.path().join("preferences.sqlite");
        for version in 1..=8 {
            let mut legacy: serde_json::Value = serde_json::from_str(&json).unwrap();
            legacy["version"] = version.into();
            legacy["brush"]
                .as_object_mut()
                .unwrap()
                .remove("opacity_mode");
            for preset in legacy["presets"].as_array_mut().unwrap() {
                preset["brush"]
                    .as_object_mut()
                    .unwrap()
                    .remove("opacity_mode");
            }
            parfait_storage::save_preferences(&path, &legacy.to_string()).unwrap();
            app.restore_preferences();
            assert!(!app.preferences_read_failed);
            assert_eq!(app.brush.opacity_mode, OpacityMode::PerDab);
        }
        let mut invalid: serde_json::Value = serde_json::from_str(&json).unwrap();
        invalid["brush"]["opacity_mode"] = "Unknown mode".into();
        let invalid = invalid.to_string();
        parfait_storage::save_preferences(&path, &invalid).unwrap();
        app.restore_preferences();
        assert!(app.preferences_read_failed);
        app.persist_preferences(true);
        assert_eq!(app.saved_preferences_json(), invalid);
    }
    #[test]
    fn pressure_spacing_exchange_restart_and_old_file_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open(dir.path().to_owned()).unwrap();
        app.brush.pressure_spacing = true;
        app.brush.minimum_spacing = 0.32;
        app.brush.spacing_curve = [1., 0.9, 0.3, 0.1, 0.].into();
        app.save_current_preset();
        let expected = app.brush.clone();
        let file = BrushFile {
            version: 7,
            preset: app
                .presets
                .iter()
                .find(|preset| preset.name == app.tip_name)
                .unwrap()
                .clone(),
        };
        let encoded = serde_json::to_value(file).unwrap();
        let decoded: BrushFile = serde_json::from_value(encoded.clone()).unwrap();
        assert!(decoded.preset.brush == expected);
        app.persist_preferences(true);
        drop(app);
        let app = Parfait::open(dir.path().to_owned()).unwrap();
        assert!(app.brush == expected);
        for version in 1..=6 {
            let mut legacy = encoded.clone();
            legacy["version"] = version.into();
            for field in ["pressure_spacing", "minimum_spacing", "spacing_curve"] {
                legacy["preset"]["brush"]
                    .as_object_mut()
                    .unwrap()
                    .remove(field);
            }
            let restored: BrushFile = serde_json::from_value(legacy).unwrap();
            assert!(!restored.preset.brush.pressure_spacing);
            assert_eq!(restored.preset.brush.minimum_spacing, 0.1);
            assert_eq!(restored.preset.brush.spacing_curve, Default::default());
        }
        for invalid in [serde_json::json!(-0.1), serde_json::json!(1.1)] {
            let mut file = encoded.clone();
            file["preset"]["brush"]["minimum_spacing"] = invalid;
            let restored: BrushFile = serde_json::from_value(file).unwrap();
            assert!(!restored.preset.brush.validate());
        }
    }
}
