use crate::{
    app::Parfait,
    theme::ACCENT,
    widgets::{self, Icon},
};
use eframe::egui::{self, Color32, RichText};

impl Parfait {
    pub fn top_panel(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("menu").show(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("New drawing       Ctrl+N").clicked() {
                        self.confirm_new = true;
                        ui.close();
                    }
                    if ui.button("Open project      Ctrl+O").clicked() {
                        self.open_project();
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Save project      Ctrl+S").clicked() {
                        self.save_project(false);
                        ui.close();
                    }
                    if ui.button("Save as     Ctrl+Shift+S").clicked() {
                        self.save_project(true);
                        ui.close();
                    }
                    if ui.button("Export PNG  Ctrl+Shift+E").clicked() {
                        self.export();
                        ui.close();
                    }
                });
                ui.menu_button("Edit", |ui| {
                    if ui.button("Undo    Ctrl+Z").clicked() {
                        self.undo();
                        ui.close();
                    }
                    if ui.button("Redo    Ctrl+Y").clicked() {
                        self.redo();
                        ui.close();
                    }
                });
                ui.menu_button("Layer", |ui| {
                    if ui.button("New raster layer  Ctrl+Shift+N").clicked() {
                        self.edit(|d| d.add_layer());
                        ui.close();
                    }
                    if ui.button("Duplicate layer").clicked() {
                        self.duplicate_layer();
                        ui.close();
                    }
                });
                ui.menu_button("View", |ui| {
                    ui.menu_button("Palettes", |ui| {
                        for p in &mut self.workspace.panels {
                            ui.checkbox(&mut p.visible, p.palette.title());
                        }
                    });
                    if ui.button("Reset palette layout").clicked() {
                        self.workspace = Default::default();
                        ui.ctx().data_mut(|d| {
                            d.remove::<egui::containers::panel::PanelState>(egui::Id::new(
                                "dock_left",
                            ));
                            d.remove::<egui::containers::panel::PanelState>(egui::Id::new(
                                "dock_right",
                            ));
                        });
                        ui.close();
                    }
                    if ui.button("Fit canvas   Ctrl+0").clicked() {
                        self.view = Default::default();
                        ui.close();
                    }
                    if ui
                        .checkbox(&mut self.hide_panels, "Hide palettes   Tab")
                        .changed()
                    {
                        ui.close();
                    }
                });
                ui.menu_button("Help", |ui| {
                    if ui.button("Third-party notices").clicked() {
                        self.show_licenses = true;
                        ui.close();
                    }
                    if ui.button("Keyboard shortcuts   F1").clicked() {
                        self.show_help = true;
                        ui.close();
                    }
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let (color, label) = if self.saving_failed {
                        (Color32::LIGHT_RED, "Save failed")
                    } else if self.saved >= self.doc.revision {
                        (Color32::from_gray(184), "Saved locally")
                    } else {
                        (ACCENT, "Saving…")
                    };
                    ui.label(RichText::new(label).color(color).size(11.));
                });
            });
        });
        egui::Panel::top("command_bar").show(ui, |ui| {
            ui.horizontal(|ui| {
                if widgets::icon(ui, Icon::Plus, false, "New drawing · Ctrl+N").clicked() {
                    self.confirm_new = true;
                }
                if widgets::icon(ui, Icon::Folder, false, "Open project · Ctrl+O").clicked() {
                    self.open_project();
                }
                if widgets::icon(ui, Icon::Save, false, "Save project · Ctrl+S").clicked() {
                    self.save_project(false);
                }
                ui.separator();
                if widgets::icon(ui, Icon::Undo, false, "Undo · Ctrl+Z").clicked() {
                    self.undo();
                }
                if widgets::icon(ui, Icon::Redo, false, "Redo · Ctrl+Y").clicked() {
                    self.redo();
                }
                ui.separator();
                if widgets::icon(ui, Icon::Fit, false, "Fit canvas · Ctrl+0").clicked() {
                    self.view = Default::default();
                }
                ui.separator();
                ui.label(
                    RichText::new(format!("{} × {} px", self.doc.width, self.doc.height)).weak(),
                );
            });
        });
        egui::Panel::top("document_tab")
            .frame(
                egui::Frame::NONE
                    .fill(Color32::from_gray(46))
                    .inner_margin(egui::Margin::symmetric(5, 0)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.add(
                        egui::Button::new(format!("   {}   ", self.doc.name))
                            .fill(Color32::from_rgb(81, 88, 101)),
                    );
                    ui.label(
                        RichText::new(format!("{:.1}%", self.view.zoom * 100.))
                            .small()
                            .weak(),
                    );
                    if let Some(message) = self.notice.clone() {
                        ui.separator();
                        ui.label(RichText::new(message).size(11.));
                        if ui
                            .small_button("×")
                            .on_hover_text("Dismiss notification")
                            .clicked()
                        {
                            self.notice = None;
                        }
                    }
                });
            });
    }
    pub fn bottom_panel(&mut self, ui: &mut egui::Ui) {
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(format!("{:.1}%", self.view.zoom * 100.));
                ui.separator();
                ui.label(format!("{:.1}°", self.view.angle.to_degrees()));
                ui.separator();
                ui.label(
                    RichText::new("Space: pan    Shift+Space: rotate    Alt: pick color")
                        .size(11.)
                        .weak(),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new("Parfait").size(11.).weak());
                });
            });
        });
    }
    pub fn tool_rail(&mut self, ui: &mut egui::Ui) {
        egui::Panel::left("tool_rail")
            .exact_size(34.)
            .resizable(false)
            .frame(
                egui::Frame::NONE
                    .fill(Color32::from_gray(66))
                    .inner_margin(4),
            )
            .show(ui, |ui| {
                if widgets::icon(
                    ui,
                    Icon::Pen,
                    !self.brush.erase && self.navigation_tool == 0,
                    "Pen · B / P",
                )
                .clicked()
                {
                    self.brush.erase = false;
                    self.navigation_tool = 0;
                }
                if widgets::icon(
                    ui,
                    Icon::Eraser,
                    self.brush.erase && self.navigation_tool == 0,
                    "Eraser · E",
                )
                .clicked()
                {
                    self.brush.erase = true;
                    self.navigation_tool = 0;
                }
                ui.separator();
                if widgets::icon(
                    ui,
                    Icon::Hand,
                    self.navigation_tool == 1,
                    "Hand · drag to pan; Space temporarily",
                )
                .clicked()
                {
                    self.navigation_tool = 1;
                }
                if widgets::icon(
                    ui,
                    Icon::Picker,
                    self.navigation_tool == 2,
                    "Eyedropper · Alt temporarily",
                )
                .clicked()
                {
                    self.navigation_tool = 2;
                }
                ui.separator();
                if widgets::icon(ui, Icon::Fit, false, "Fit canvas").clicked() {
                    self.view = Default::default();
                }
                ui.add_space(10.);
                ui.color_edit_button_srgb(&mut self.brush.color);
            });
    }
    fn duplicate_layer(&mut self) {
        self.edit(|d| {
            if d.layers.len() < 64 {
                let mut l = d.layers[d.active].clone();
                l.name.push_str(" copy");
                d.active += 1;
                d.layers.insert(d.active, l);
            }
        });
    }
    pub fn update_previews(&mut self, ctx: &egui::Context) {
        // Thumbnails catch up on pen-up; the canvas/navigator remain live.
        if self.stroke.is_some() || self.preview_revision == self.doc.revision {
            return;
        }
        self.layer_previews.truncate(self.doc.layers.len());
        for (i, layer) in self.doc.layers.iter().enumerate() {
            let mut pixels = Vec::with_capacity(40 * 40 * 4);
            for y in 0..40 {
                for x in 0..40 {
                    let sx = x * self.doc.width / 40;
                    let sy = y * self.doc.height / 40;
                    let at = ((sy * self.doc.width + sx) * 4) as usize;
                    let a = layer.pixels[at + 3] as f32 / 255.;
                    let bg = if (x / 5 + y / 5) % 2 == 0 { 210. } else { 165. };
                    for c in 0..3 {
                        pixels.push((layer.pixels[at + c] as f32 * a + bg * (1. - a)) as u8);
                    }
                    pixels.push(255);
                }
            }
            let img = egui::ColorImage::from_rgba_unmultiplied([40, 40], &pixels);
            if let Some(t) = self.layer_previews.get_mut(i) {
                t.set(img, egui::TextureOptions::LINEAR);
            } else {
                self.layer_previews.push(ctx.load_texture(
                    format!("layer-{i}"),
                    img,
                    egui::TextureOptions::LINEAR,
                ));
            }
        }
        self.preview_revision = self.doc.revision;
    }
    pub fn palette(&mut self, ui: &mut egui::Ui, palette: crate::workspace::Palette) {
        use crate::workspace::Palette;
        match palette {
            Palette::Brushes => self.brush_library_ui(ui),
            Palette::Properties => self.brush_properties_ui(ui),
            Palette::Sizes => {
                let sizes = [
                    1., 2., 3., 4., 5., 7., 10., 12., 15., 20., 25., 30., 40., 50., 70., 100.,
                    150., 200.,
                ];
                let cell = ((ui.available_width() - 5. * 2.) / 6.).max(20.);
                egui::Grid::new("brush_size_grid")
                    .spacing(egui::vec2(2., 2.))
                    .show(ui, |ui| {
                        for (i, size) in sizes.into_iter().enumerate() {
                            let (r, response) =
                                ui.allocate_exact_size(egui::vec2(cell, 36.), egui::Sense::click());
                            response.widget_info(|| {
                                egui::WidgetInfo::labeled(
                                    egui::WidgetType::Button,
                                    true,
                                    format!("Brush size {size} pixels"),
                                )
                            });
                            let selected = (self.brush.size - size).abs() < 0.1;
                            ui.painter().rect_filled(
                                r,
                                0.,
                                if selected {
                                    ui.visuals().selection.bg_fill
                                } else {
                                    Color32::from_gray(68)
                                },
                            );
                            ui.painter().circle_filled(
                                r.center_top() + egui::vec2(0., 12.),
                                size.sqrt().clamp(1., 10.),
                                Color32::from_gray(224),
                            );
                            ui.painter().text(
                                r.center_bottom() - egui::vec2(0., 3.),
                                egui::Align2::CENTER_BOTTOM,
                                format!("{size}"),
                                egui::FontId::proportional(10.),
                                Color32::from_gray(206),
                            );
                            if response.clicked() {
                                self.brush.size = size;
                            }
                            if i % 6 == 5 {
                                ui.end_row();
                            }
                        }
                    });
            }
            Palette::Color => {
                widgets::color_wheel(ui, &mut self.brush.color);
                ui.color_edit_button_srgb(&mut self.brush.color);
            }
            Palette::Navigator => {
                if let Some(texture) = &self.texture {
                    let w = ui.available_width();
                    let h = (w * self.doc.height as f32 / self.doc.width as f32).min(180.);
                    let size = egui::vec2(h * self.doc.width as f32 / self.doc.height as f32, h);
                    ui.vertical_centered(|ui| {
                        ui.add(egui::Image::new(texture).fit_to_exact_size(size));
                    });
                }
                ui.horizontal(|ui| {
                    ui.add(
                        egui::Slider::new(&mut self.view.zoom, 0.05..=12.)
                            .logarithmic(true)
                            .show_value(false),
                    );
                    ui.label(format!("{:.1}%", self.view.zoom * 100.));
                    if widgets::icon(ui, Icon::Minus, false, "Zoom out").clicked() {
                        self.view.zoom = (self.view.zoom / 1.2).max(0.05);
                    }
                    if widgets::icon(ui, Icon::Plus, false, "Zoom in").clicked() {
                        self.view.zoom = (self.view.zoom * 1.2).min(12.);
                    }
                });
                ui.horizontal(|ui| {
                    let mut deg = self.view.angle.to_degrees();
                    if ui
                        .add(egui::DragValue::new(&mut deg).speed(0.5).suffix("°"))
                        .changed()
                    {
                        self.view.angle = deg.to_radians();
                    }
                    if ui.small_button("Reset rotation").clicked() {
                        self.view.angle = 0.;
                    }
                    if widgets::icon(ui, Icon::Fit, false, "Fit canvas").clicked() {
                        self.view = Default::default();
                    }
                });
            }
            Palette::Layers => {
                let active = self.doc.active;
                let mut opacity = self.doc.layers[active].opacity;
                ui.horizontal(|ui| {
                    ui.label("Normal");
                    let r = ui.add(
                        egui::Slider::new(&mut opacity, 0.0..=1.)
                            .custom_formatter(|v, _| format!("{:.0}%", v * 100.)),
                    );
                    if r.changed() {
                        if r.drag_started() || !r.dragged() {
                            self.history.checkpoint(&self.doc);
                        }
                        self.doc.layers[active].opacity = opacity;
                        self.doc.touch();
                    }
                });
                let mut name = self.doc.layers[active].name.clone();
                if ui
                    .add(egui::TextEdit::singleline(&mut name).desired_width(f32::INFINITY))
                    .on_hover_text("Rename active layer")
                    .changed()
                {
                    self.history.checkpoint(&self.doc);
                    self.doc.layers[active].name = name;
                    self.doc.touch();
                }
                ui.horizontal(|ui| {
                    if widgets::icon(ui, Icon::Plus, false, "New raster layer · Ctrl+Shift+N")
                        .clicked()
                    {
                        self.edit(|d| d.add_layer());
                    }
                    if widgets::icon(ui, Icon::Copy, false, "Duplicate layer").clicked() {
                        self.duplicate_layer();
                    }
                    ui.add_enabled_ui(self.doc.active + 1 < self.doc.layers.len(), |ui| {
                        if widgets::icon(ui, Icon::Up, false, "Move layer up").clicked() {
                            self.edit(|d| {
                                d.layers.swap(d.active, d.active + 1);
                                d.active += 1;
                            });
                        }
                    });
                    ui.add_enabled_ui(self.doc.active > 0, |ui| {
                        if widgets::icon(ui, Icon::Down, false, "Move layer down").clicked() {
                            self.edit(|d| {
                                d.layers.swap(d.active, d.active - 1);
                                d.active -= 1;
                            });
                        }
                    });
                    ui.add_enabled_ui(self.doc.layers.len() > 1, |ui| {
                        if widgets::icon(ui, Icon::Trash, false, "Delete layer").clicked() {
                            self.edit(|d| {
                                d.layers.remove(d.active);
                                d.active = d.active.min(d.layers.len() - 1);
                            });
                        }
                    });
                });
                ui.separator();
                egui::ScrollArea::vertical()
                    .id_salt("layer_list")
                    .show(ui, |ui| {
                        for idx in (0..self.doc.layers.len()).rev() {
                            let selected = self.doc.active == idx;
                            egui::Frame::NONE
                                .fill(if selected {
                                    ui.visuals().selection.bg_fill
                                } else {
                                    Color32::from_gray(59)
                                })
                                .inner_margin(3)
                                .show(ui, |ui| {
                                    ui.set_min_width(ui.available_width());
                                    ui.horizontal(|ui| {
                                        let mut visible = self.doc.layers[idx].visible;
                                        if ui
                                            .checkbox(&mut visible, "")
                                            .on_hover_text("Show layer")
                                            .changed()
                                        {
                                            self.edit(|d| d.layers[idx].visible = visible);
                                        }
                                        if let Some(t) = self.layer_previews.get(idx) {
                                            let aspect =
                                                self.doc.width as f32 / self.doc.height as f32;
                                            let size = if aspect >= 1. {
                                                egui::vec2(36., 36. / aspect)
                                            } else {
                                                egui::vec2(36. * aspect, 36.)
                                            };
                                            if ui
                                                .add(
                                                    egui::Image::new(t)
                                                        .fit_to_exact_size(size)
                                                        .sense(egui::Sense::click()),
                                                )
                                                .clicked()
                                            {
                                                self.doc.active = idx;
                                                self.doc.touch();
                                            }
                                        }
                                        ui.vertical(|ui| {
                                            ui.label(
                                                RichText::new(format!(
                                                    "{:.0}%  Normal",
                                                    self.doc.layers[idx].opacity * 100.
                                                ))
                                                .size(10.),
                                            );
                                            if ui
                                                .selectable_label(
                                                    selected,
                                                    &self.doc.layers[idx].name,
                                                )
                                                .clicked()
                                            {
                                                self.doc.active = idx;
                                                self.doc.touch();
                                            }
                                        });
                                        let mut locked = self.doc.layers[idx].locked;
                                        if ui
                                            .checkbox(&mut locked, "")
                                            .on_hover_text("Lock layer")
                                            .changed()
                                        {
                                            self.edit(|d| d.layers[idx].locked = locked);
                                        }
                                    });
                                });
                        }
                    });
            }
        }
    }
}
