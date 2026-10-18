use crate::{app::Parfait, theme::ACCENT};
use eframe::egui::{self, Color32, RichText};
use parfait_core::BrushTip;
impl Parfait {
    pub fn top_panel(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("menu").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("parfait").size(23.).strong().color(ACCENT));
                ui.separator();
                ui.menu_button("File", |ui| {
                    if ui.button("New drawing      Ctrl+N").clicked() {
                        self.confirm_new = true;
                        ui.close();
                    }
                    if ui.button("Open project     Ctrl+O").clicked() {
                        self.open_project();
                        ui.close();
                    }
                    if ui.button("Save project     Ctrl+S").clicked() {
                        self.save_project(false);
                        ui.close();
                    }
                    if ui.button("Save as       Ctrl+Shift+S").clicked() {
                        self.save_project(true);
                        ui.close();
                    }
                    if ui.button("Export PNG    Ctrl+Shift+E").clicked() {
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
                if ui.button("Fit canvas").clicked() {
                    self.view = Default::default();
                }
                if ui.button("Shortcuts").clicked() {
                    self.show_help = true;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let (color, label) = if self.saving_failed {
                        (Color32::LIGHT_RED, "Save failed")
                    } else if self.saved >= self.doc.revision {
                        (Color32::from_rgb(145, 205, 171), "Saved locally")
                    } else {
                        (ACCENT, "Saving…")
                    };
                    ui.label(RichText::new(label).color(color));
                });
            });
        });
        egui::Panel::top("document_tab").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!(
                        "{}  ·  {} × {}",
                        self.doc.name, self.doc.width, self.doc.height
                    ))
                    .strong(),
                );
                ui.separator();
                ui.label(if self.brush.erase {
                    "Eraser"
                } else {
                    &self.tip_name
                });
                ui.label(format!("{:.0} px", self.brush.size));
            });
            if let Some(message) = self.notice.clone() {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(message).color(ACCENT));
                    if ui.small_button("Dismiss").clicked() {
                        self.notice = None;
                    }
                });
            }
        });
    }
    pub fn bottom_panel(&mut self, ui: &mut egui::Ui) {
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(format!("{:.0}%", self.view.zoom * 100.));
                ui.separator();
                ui.label(format!("{:.0}°", self.view.angle.to_degrees()));
                ui.separator();
                ui.label("Space: pan   ·   Shift+Space: rotate   ·   Alt: pick color");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label("Parfait 0.1 · local-first");
                });
            });
        });
    }
    pub fn tool_panel(&mut self, ui: &mut egui::Ui) {
        egui::Panel::left("tools").default_size(210.).resizable(true).show(ui,|ui|{
            ui.add_space(6.);ui.label(RichText::new("TOOLS").small().color(ACCENT));
            ui.horizontal(|ui|{if ui.selectable_label(!self.brush.erase,"Pen  B").clicked(){self.brush.erase=false;}
if ui.selectable_label(self.brush.erase,"Eraser  E").clicked(){self.brush.erase=true;}});
            ui.separator();ui.label("SUB TOOL");
            if ui.selectable_label(matches!(self.brush.tip,BrushTip::Round),"●  Round pen").clicked(){self.brush.tip=BrushTip::Round;self.tip_name="Round pen".into();}
            if ui.selectable_label(matches!(self.brush.tip,BrushTip::Soft),"◉  Soft brush").clicked(){self.brush.tip=BrushTip::Soft;self.tip_name="Soft brush".into();}
            if matches!(self.brush.tip,BrushTip::Image{..}){ui.label(RichText::new(&self.tip_name).color(ACCENT));}
            if ui.button("+ Import PNG tip").clicked(){self.import_tip();}
            ui.add_space(14.);ui.separator();ui.label("TOOL PROPERTY");
            ui.add(egui::Slider::new(&mut self.brush.size,1.0..=512.0).logarithmic(true).text("Size"));
            ui.add(egui::Slider::new(&mut self.brush.opacity,0.01..=1.0).text("Opacity"));
            ui.checkbox(&mut self.brush.pressure,"Pressure size").on_hover_text("Uses pressure when supplied by the input backend; mouse uses full pressure.");
            ui.add_space(14.);ui.separator();ui.label("COLOR");
            ui.color_edit_button_srgb(&mut self.brush.color);
            let colors=[[46,43,54],[255,255,255],[196,115,131],[224,168,123],[229,208,147],[135,177,151],[127,166,199],[174,149,204]];
            ui.horizontal_wrapped(|ui|{for color in colors{let b=egui::Button::new("   ").fill(Color32::from_rgb(color[0],color[1],color[2]));if ui.add(b).clicked(){self.brush.color=color;}}});
            ui.add_space(24.);ui.label(RichText::new("Make a little space\nfor your next idea.").small().weak());
        });
    }
    pub fn layer_panel(&mut self, ui: &mut egui::Ui) {
        egui::Panel::right("layers")
            .default_size(245.)
            .resizable(true)
            .show(ui, |ui| {
                ui.add_space(6.);
                ui.label(RichText::new("NAVIGATOR").small().color(ACCENT));
                if let Some(texture) = &self.texture {
                    ui.add(egui::Image::new(texture).fit_to_exact_size(egui::vec2(205., 154.)));
                }
                ui.horizontal(|ui| {
                    if ui.small_button("−").clicked() {
                        self.view.zoom = (self.view.zoom / 1.2).max(0.05);
                    }
                    ui.label(format!("{:.0}%", self.view.zoom * 100.));
                    if ui.small_button("+").clicked() {
                        self.view.zoom = (self.view.zoom * 1.2).min(12.);
                    }
                    if ui.small_button("Fit").clicked() {
                        self.view = Default::default();
                    }
                });
                ui.separator();
                ui.label(RichText::new("LAYERS").small().color(ACCENT));
                ui.horizontal(|ui| {
                    if ui
                        .button("+")
                        .on_hover_text("New layer · Ctrl+Shift+N")
                        .clicked()
                    {
                        self.edit(|d| d.add_layer());
                    }
                    if ui.button("Copy").clicked() {
                        self.edit(|d| {
                            if d.layers.len() < 64 {
                                let mut l = d.layers[d.active].clone();
                                l.name.push_str(" copy");
                                d.active += 1;
                                d.layers.insert(d.active, l);
                            }
                        });
                    }
                    if ui
                        .add_enabled(self.doc.layers.len() > 1, egui::Button::new("Delete"))
                        .clicked()
                    {
                        self.edit(|d| {
                            d.layers.remove(d.active);
                            d.active = d.active.min(d.layers.len() - 1);
                        });
                    }
                });
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(
                            self.doc.active + 1 < self.doc.layers.len(),
                            egui::Button::new("Up"),
                        )
                        .clicked()
                    {
                        self.edit(|d| {
                            d.layers.swap(d.active, d.active + 1);
                            d.active += 1;
                        });
                    }
                    if ui
                        .add_enabled(self.doc.active > 0, egui::Button::new("Down"))
                        .clicked()
                    {
                        self.edit(|d| {
                            d.layers.swap(d.active, d.active - 1);
                            d.active -= 1;
                        });
                    }
                });
                let active = self.doc.active;
                let mut opacity = self.doc.layers[active].opacity;
                let r = ui.add(egui::Slider::new(&mut opacity, 0.0..=1.).text("Opacity"));
                if r.changed() {
                    if r.drag_started() || !r.dragged() {
                        self.history.checkpoint(&self.doc);
                    }
                    self.doc.layers[active].opacity = opacity;
                    self.doc.touch();
                }
                let mut name = self.doc.layers[active].name.clone();
                if ui.text_edit_singleline(&mut name).changed() {
                    self.history.checkpoint(&self.doc);
                    self.doc.layers[active].name = name;
                    self.doc.touch();
                }
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for idx in (0..self.doc.layers.len()).rev() {
                        ui.horizontal(|ui| {
                            let mut visible = self.doc.layers[idx].visible;
                            let mut locked = self.doc.layers[idx].locked;
                            if ui
                                .checkbox(&mut visible, "")
                                .on_hover_text("Visibility")
                                .changed()
                            {
                                self.edit(|d| d.layers[idx].visible = visible);
                            }
                            let label = format!(
                                "{}  {}",
                                if locked { "▣" } else { "▤" },
                                self.doc.layers[idx].name
                            );
                            if ui.selectable_label(self.doc.active == idx, label).clicked() {
                                self.doc.active = idx;
                                self.doc.touch();
                            }
                            if ui
                                .checkbox(&mut locked, "")
                                .on_hover_text("Lock layer")
                                .changed()
                            {
                                self.edit(|d| d.layers[idx].locked = locked);
                            }
                        });
                    }
                });
                ui.add_space(12.);
                ui.label(
                    RichText::new("Left check: visible · Right check: locked")
                        .small()
                        .weak(),
                );
            });
    }
}
