use crate::shortcuts::Command;
use crate::{
    app::Parfait,
    theme::ACCENT,
    widgets::{self, Icon},
};
use eframe::egui::{self, Color32, RichText};

impl Parfait {
    pub fn top_panel(&mut self, ui: &mut egui::Ui) {
        self.menu_bar(ui);
        egui::Panel::top("command_bar").show(ui, |ui| {
            ui.horizontal(|ui| {
                self.command_icon(ui, Icon::Plus, Command::NewDrawing);
                self.command_icon(ui, Icon::Folder, Command::OpenProject);
                self.command_icon(ui, Icon::Save, Command::SaveProject);
                ui.separator();
                self.command_icon(ui, Icon::Undo, Command::Undo);
                self.command_icon(ui, Icon::Redo, Command::Redo);
                ui.separator();
                self.command_icon(ui, Icon::Fit, Command::FitCanvas);
                ui.separator();
                ui.label(
                    RichText::new(format!(
                        "{} × {} px",
                        self.doc.width, self.doc.height
                    ))
                    .weak(),
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
                    ui.add_sized(
                        [180_f32.min(ui.available_width() * 0.25), 20.],
                        egui::Button::new(format!(
                            "{}{}",
                            self.doc.name,
                            if self.project_is_modified() { " *" } else { "" }
                        ))
                        .truncate()
                        .fill(Color32::from_rgb(81, 88, 101)),
                    )
                    .on_hover_text(&self.doc.name);
                    ui.label(
                        RichText::new(format!("{:.1}%", self.view.zoom * 100.))
                            .small()
                            .weak(),
                    );
                    if let Some(message) = self.notice.clone() {
                        ui.separator();
                        ui.add_sized(
                            [(ui.available_width() - 26.).max(1.), 20.],
                            egui::Label::new(RichText::new(&message).size(11.))
                                .truncate(),
                        )
                        .on_hover_text(message);
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
                if self.view.flip_horizontal {
                    ui.label(RichText::new("H-flip").color(ACCENT));
                }
                if self.view.flip_vertical {
                    ui.label(RichText::new("V-flip").color(ACCENT));
                }
                ui.separator();
                ui.label(
                    RichText::new(
                        "Space: pan    Shift+Space: rotate    Alt: pick color",
                    )
                    .size(11.)
                    .weak(),
                );
                ui.with_layout(
                    egui::Layout::right_to_left(egui::Align::Center),
                    |ui| {
                        ui.label(RichText::new("Parfait").size(11.).weak());
                    },
                );
            });
        });
    }
    pub fn tool_rail(&mut self, ui: &mut egui::Ui) {
        egui::Panel::left("tool_rail")
            .exact_size(38.)
            .resizable(false)
            .frame(
                egui::Frame::NONE.fill(Color32::from_gray(66)).inner_margin(4),
            )
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 1.;
                use crate::brushes::BrushGroup;
                for (group, icon, command) in [
                    (BrushGroup::Ink, Icon::Pen, Command::ToolInk),
                    (BrushGroup::Sketch, Icon::Pencil, Command::ToolSketch),
                    (BrushGroup::Paint, Icon::Brush, Command::ToolPaint),
                    (
                        BrushGroup::Airbrush,
                        Icon::Airbrush,
                        Command::ToolAirbrush,
                    ),
                    (BrushGroup::Eraser, Icon::Eraser, Command::ToolEraser),
                    (BrushGroup::Blend, Icon::Blend, Command::ToolBlend),
                ] {
                    if widgets::flat_icon(
                        ui,
                        icon,
                        self.navigation_tool == 0
                            && BrushGroup::of(&self.brush) == group,
                        self.keymap.tooltip(command),
                        30.,
                    )
                    .clicked()
                    {
                        self.select_brush_group(group);
                    }
                }
                ui.separator();
                if widgets::flat_icon(
                    ui,
                    Icon::Hand,
                    self.navigation_tool == 1,
                    self.keymap.tooltip(Command::ToolHand),
                    30.,
                )
                .clicked()
                {
                    self.navigation_tool = 1;
                }
                if widgets::flat_icon(
                    ui,
                    Icon::Picker,
                    self.navigation_tool == 2,
                    self.keymap.tooltip(Command::ToolPicker),
                    30.,
                )
                .clicked()
                {
                    self.navigation_tool = 2;
                }
                ui.separator();
                if widgets::flat_icon(ui, Icon::Fit, false, "Fit canvas", 30.)
                    .clicked()
                {
                    self.execute_command(Command::FitCanvas, ui.ctx());
                }
                ui.add_space(10.);
                ui.color_edit_button_srgb(&mut self.brush.color);
            });
    }
    pub(crate) fn duplicate_layer(&mut self) {
        if self.doc.has_non_raster_layers() {
            self.execute_tree_collection(
                Command::DuplicateLayer,
                1 << self.doc.active,
            );
            return;
        }
        if let Some(reason) = parfait_storage::layer_duplication_error(
            &self.doc,
            1 << self.doc.active,
        ) {
            self.notice = Some(reason.into());
            return;
        }
        self.edit_named("Duplicate layer", |document| {
            let mut layer = document.layers[document.active].clone();
            layer.id = document.allocate_layer_id().unwrap();
            layer.name.push_str(" copy");
            document.active += 1;
            document.layers.insert(document.active, layer);
        });
    }
    pub fn update_previews(&mut self, ctx: &egui::Context) {
        // Thumbnails catch up on pen-up; the canvas/navigator remain live.
        if self.stroke.is_some() || self.preview_revision == self.doc.revision {
            return;
        }
        self.layer_previews.truncate(self.doc.layers.len());
        self.mask_previews.resize_with(self.doc.layers.len(), || None);
        for (i, layer) in self.doc.layers.iter().enumerate() {
            if let Some(mask) = &layer.mask {
                let mut pixels = Vec::with_capacity(40 * 40 * 4);
                for row in 0..40 {
                    for column in 0..40 {
                        let index = (row * self.doc.height / 40
                            * self.doc.width
                            + column * self.doc.width / 40)
                            as usize;
                        let value = mask.coverage.value(index);
                        pixels.extend_from_slice(&[value, value, value, 255]);
                    }
                }
                let image =
                    egui::ColorImage::from_rgba_unmultiplied([40, 40], &pixels);
                if let Some(texture) = &mut self.mask_previews[i] {
                    texture.set(image, egui::TextureOptions::LINEAR);
                } else {
                    self.mask_previews[i] = Some(ctx.load_texture(
                        format!("mask-{i}"),
                        image,
                        egui::TextureOptions::LINEAR,
                    ));
                }
            } else {
                self.mask_previews[i] = None;
            }
            let mut pixels = Vec::with_capacity(40 * 40 * 4);
            for y in 0..40 {
                for x in 0..40 {
                    let sx = x * self.doc.width / 40;
                    let sy = y * self.doc.height / 40;
                    let pixel =
                        layer.source_pixel((sy * self.doc.width + sx) as usize);
                    let a = pixel[3] as f32 / 255.;
                    let bg = if (x / 5 + y / 5) % 2 == 0 { 210. } else { 165. };
                    for component in &pixel[..3] {
                        pixels.push(
                            (f32::from(*component) * a + bg * (1. - a)) as u8,
                        );
                    }
                    pixels.push(255);
                }
            }
            let img =
                egui::ColorImage::from_rgba_unmultiplied([40, 40], &pixels);
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
    pub fn palette(
        &mut self,
        ui: &mut egui::Ui,
        palette: crate::workspace::Palette,
        body_height: f32,
    ) {
        use crate::workspace::Palette;
        match palette {
            Palette::History => self.history_palette(ui, body_height),
            Palette::Brushes => self.brush_library_ui(ui),
            Palette::Properties => self.brush_properties_ui(ui),
            Palette::Sizes => {
                let sizes = [
                    1., 2., 3., 4., 5., 7., 10., 12., 15., 20., 25., 30., 40.,
                    50., 70., 100., 150., 200.,
                ];
                let cell = ((ui.available_width() - 5. * 2.) / 6.).max(20.);
                egui::Grid::new("brush_size_grid")
                    .spacing(egui::vec2(2., 2.))
                    .show(ui, |ui| {
                        for (i, size) in sizes.into_iter().enumerate() {
                            let (r, response) = ui.allocate_exact_size(
                                egui::vec2(cell, 36.),
                                egui::Sense::click(),
                            );
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
                ui.vertical_centered(|ui| {
                    widgets::color_wheel_sized(
                        ui,
                        &mut self.brush.color,
                        body_height - 30.,
                    );
                });
                ui.color_edit_button_srgb(&mut self.brush.color);
            }
            Palette::Navigator => {
                if let Some(texture) = &self.texture {
                    let w = ui.available_width();
                    let h = (w * self.doc.height as f32
                        / self.doc.width as f32)
                        .min((body_height - 66.).clamp(30., 180.));
                    let size = egui::vec2(
                        h * self.doc.width as f32 / self.doc.height as f32,
                        h,
                    );
                    ui.vertical_centered(|ui| {
                        ui.add(
                            egui::Image::new(texture).fit_to_exact_size(size),
                        );
                    });
                }
                ui.horizontal(|ui| {
                    ui.spacing_mut().slider_width =
                        (ui.available_width() - 108.).max(24.);
                    ui.add(
                        egui::Slider::new(&mut self.view.zoom, 0.05..=12.)
                            .logarithmic(true)
                            .show_value(false),
                    );
                    ui.add_sized(
                        [44., 24.],
                        egui::Label::new(format!(
                            "{:.1}%",
                            self.view.zoom * 100.
                        ))
                        .halign(egui::Align::RIGHT),
                    );
                    if widgets::flat_icon(
                        ui,
                        Icon::Minus,
                        false,
                        "Zoom out",
                        24.,
                    )
                    .clicked()
                    {
                        self.view.zoom = (self.view.zoom / 1.2).max(0.05);
                    }
                    if widgets::flat_icon(ui, Icon::Plus, false, "Zoom in", 24.)
                        .clicked()
                    {
                        self.view.zoom = (self.view.zoom * 1.2).min(12.);
                    }
                });
                ui.horizontal(|ui| {
                    let mut deg = self.view.angle.to_degrees();
                    if ui
                        .add_sized(
                            [64., 24.],
                            egui::DragValue::new(&mut deg)
                                .speed(0.5)
                                .suffix("°"),
                        )
                        .changed()
                    {
                        self.view.angle = deg.to_radians();
                    }
                    widgets::action_row(ui, |ui| {
                        if widgets::flat_icon(
                            ui,
                            Icon::Fit,
                            false,
                            "Fit canvas",
                            24.,
                        )
                        .clicked()
                        {
                            self.view_command(crate::display::ViewCommand::Fit);
                        }
                        if widgets::flat_icon(
                            ui,
                            Icon::Undo,
                            false,
                            "Reset rotation",
                            24.,
                        )
                        .clicked()
                        {
                            self.view.angle = 0.;
                        }
                    });
                });
            }
            Palette::Layers => {
                let controls_top = ui.cursor().top();
                let active = self.doc.active;
                let mut opacity = self.doc.layers[active].opacity;
                ui.horizontal(|ui| {
                    let mut mode = self.doc.layers[active].blend_mode;
                    if self.doc.layers[active].kind.is_paper() {
                        ui.disable();
                    }
                    let width = ui.available_width();
                    let blend = crate::layer_controls::blend_select(
                        ui,
                        &mut mode,
                        self.doc.layers[active].kind
                            == parfait_core::LayerKind::Folder,
                        self.doc.layers[active].clipped,
                        (width * 0.4).clamp(72., 120.),
                    );
                    #[cfg(test)]
                    ui.data_mut(|data| {
                        data.insert_temp(
                            egui::Id::new("layer_blend"),
                            blend.rect,
                        )
                    });
                    #[cfg(not(test))]
                    let _ = blend;
                    if mode != self.doc.layers[active].blend_mode {
                        self.edit_named("Layer blend mode", |document| {
                            document.layers[active].blend_mode = mode
                        });
                    }
                    let r = ui
                        .add_enabled_ui(!self.cancelled_layer_drag, |ui| {
                            crate::layer_controls::opacity(ui, &mut opacity)
                        })
                        .inner;
                    if r.changed() && opacity != self.doc.layers[active].opacity
                    {
                        self.begin_layer_edit(
                            crate::editing::LayerEditKind::Opacity,
                            active,
                            ui.input(|input| input.pointer.any_down()),
                        );
                        self.doc.layers[active].opacity = opacity;
                        self.doc.touch();
                    }
                    self.layer_edit_response(
                        crate::editing::LayerEditKind::Opacity,
                        &r,
                    );
                });
                ui.horizontal_wrapped(|ui| {
                    for (icon, command, selected) in [
                        (
                            Icon::Clipping,
                            Command::LayerClipping,
                            self.doc.layers[active].clipped,
                        ),
                        (
                            Icon::Reference,
                            Command::LayerReference,
                            self.doc.layers[active].reference,
                        ),
                        (
                            Icon::Pencil,
                            Command::LayerDraft,
                            self.doc.layers[active].draft,
                        ),
                        (
                            Icon::Lock,
                            Command::LayerLock,
                            self.doc.layers[active].locked,
                        ),
                        (
                            Icon::AlphaLock,
                            Command::LayerAlphaLock,
                            self.doc.layers[active].alpha_locked,
                        ),
                    ] {
                        let reason = self.command_disabled_reason(command);
                        let response = ui
                            .add_enabled_ui(reason.is_none(), |ui| {
                                widgets::flat_icon(
                                    ui,
                                    icon,
                                    selected,
                                    self.keymap.tooltip(command),
                                    22.,
                                )
                            })
                            .inner;
                        #[cfg(test)]
                        ui.data_mut(|data| {
                            data.insert_temp(
                                egui::Id::new(("layer_property", command)),
                                response.rect,
                            )
                        });
                        if response.clicked() {
                            self.execute_command(command, ui.ctx());
                        }
                        if let Some(reason) = reason {
                            response.on_disabled_hover_text(reason);
                        }
                    }
                });
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.;
                    ui.menu_button(
                        RichText::new(Icon::More.glyph())
                            .family(egui::FontFamily::Name("icons".into())),
                        |ui| {
                            self.layer_operations_menu(ui);
                            ui.separator();
                            self.command_menu_item(
                                ui,
                                Command::GroupLayersInFolder,
                            );
                            self.command_menu_item(
                                ui,
                                Command::UngroupLayerFolder,
                            );
                            ui.separator();
                            ui.menu_button("레이어 마스크", |ui| {
                                self.layer_mask_menu(ui)
                            });
                        },
                    )
                    .response
                    .on_hover_text("레이어 조작");
                    self.layer_command_icon(
                        ui,
                        Icon::NewLayer,
                        Command::NewLayer,
                    );
                    self.layer_command_icon(
                        ui,
                        Icon::Folder,
                        Command::NewLayerFolder,
                    );
                    self.layer_command_icon(
                        ui,
                        Icon::Copy,
                        Command::DuplicateLayer,
                    );
                    self.layer_command_icon(ui, Icon::Up, Command::MoveLayerUp);
                    self.layer_command_icon(
                        ui,
                        Icon::Down,
                        Command::MoveLayerDown,
                    );
                    self.layer_command_icon(
                        ui,
                        Icon::Transfer,
                        Command::TransferLayerDown,
                    );
                    self.layer_command_icon(
                        ui,
                        Icon::Merge,
                        Command::MergeLayerDown,
                    );
                    self.layer_command_icon(
                        ui,
                        Icon::LayerMask,
                        Command::LayerMaskRevealAll,
                    );
                    self.layer_command_icon(
                        ui,
                        Icon::Trash,
                        Command::DeleteLayer,
                    );
                });
                ui.separator();
                let list_height =
                    body_height - (ui.cursor().top() - controls_top);
                self.layer_rows(ui, list_height);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::Palette;

    #[test]
    fn paper_controls_and_commands_cannot_write_incompatible_metadata() {
        let directory = tempfile::tempdir().unwrap();
        let mut app = Parfait::open(directory.path().to_owned()).unwrap();
        app.doc = parfait_core::Document::new_with_paper(8, 5);
        app.select_layer(0);
        let before = app.doc.clone();
        let context = egui::Context::default();
        crate::theme::apply(&context);
        let frame = |app: &mut Parfait, events| {
            let mut output = context.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(320., 600.),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| app.palette(ui, Palette::Layers, 400.),
            );
            output.textures_delta.clear();
        };
        frame(&mut app, vec![]);
        frame(&mut app, vec![]);
        let slider = context
            .data(|data| {
                data.get_temp::<(egui::Rect, f32)>(egui::Id::new(
                    "layer_opacity",
                ))
            })
            .unwrap()
            .0;
        for pressed in [true, false] {
            frame(
                &mut app,
                vec![
                    egui::Event::PointerMoved(slider.center()),
                    egui::Event::PointerButton {
                        pos: slider.center(),
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: Default::default(),
                    },
                ],
            );
        }
        for command in [
            Command::LayerAlphaLock,
            Command::LayerClipping,
            Command::LayerReference,
            Command::LayerDraft,
            Command::DuplicateLayer,
            Command::MoveActiveLayerUp,
            Command::MoveActiveLayerToTop,
            Command::GroupLayersInFolder,
            Command::LayerDetails,
            Command::LayerMaskRevealAll,
        ] {
            assert!(
                app.command_disabled_reason(command).is_some(),
                "{command:?}"
            );
            app.execute_command(command, &context);
            assert!(app.doc.same_edit_state(&before), "{command:?}");
            assert_eq!(app.doc.revision, before.revision);
            assert!(app.doc.validate());
        }
        app.set_layer_selection(3, 0);
        assert!(app.command_disabled_reason(Command::LayerDraft).is_some());
        assert!(app.command_disabled_reason(Command::LayerAlphaLock).is_some());
    }

    #[test]
    fn opacity_drag_from_existing_handle_is_one_undoable_edit() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open(dir.path().to_owned()).unwrap();
        app.doc = parfait_core::Document::new(4, 4);
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let frame = |app: &mut Parfait, events| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(320., 600.),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| app.palette(ui, Palette::Layers, 400.),
            );
            output.textures_delta.clear();
            ctx.data(|data| {
                data.get_temp::<(egui::Rect, f32)>(egui::Id::new(
                    "layer_opacity",
                ))
            })
            .unwrap()
        };
        frame(&mut app, vec![]);
        let (slider, width) = frame(&mut app, vec![]);
        let start = egui::pos2(slider.left() + width - 2., slider.center().y);
        let end = start - egui::vec2(width * 0.5, 0.);
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            pressed,
            button: egui::PointerButton::Primary,
            modifiers: Default::default(),
        };
        frame(&mut app, vec![egui::Event::PointerMoved(start)]);
        frame(&mut app, vec![button(start, true)]);
        assert_eq!(app.doc.layers[1].opacity, 1.);
        frame(&mut app, vec![egui::Event::PointerMoved(end)]);
        frame(
            &mut app,
            vec![egui::Event::PointerMoved(end + egui::vec2(8., 0.))],
        );
        frame(&mut app, vec![egui::Event::PointerMoved(end)]);
        frame(&mut app, vec![button(end, false)]);
        let changed = app.doc.layers[1].opacity;
        assert!(changed < 0.8);
        assert_eq!(app.history.rows().count(), 2);
        app.undo();
        assert_eq!(app.doc.layers[1].opacity, 1.);
        app.redo();
        assert_eq!(app.doc.layers[1].opacity, changed);
        app.undo();
        frame(&mut app, vec![]);
        let revision = app.doc.revision;
        frame(
            &mut app,
            vec![egui::Event::PointerMoved(start), button(start, true)],
        );
        frame(&mut app, vec![button(start, false)]);
        assert_eq!(app.doc.revision, revision);
        assert!(app.history.can_redo());
    }

    #[test]
    fn palette_contents_fit_narrow_widths_without_changing_artwork() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open(dir.path().to_owned()).unwrap();
        app.doc = parfait_core::Document::new(32, 24);
        app.doc.layers[1].name =
            "A layer with a very long name that must remain inside its row"
                .into();
        let artwork = app.doc.clone();
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        app.update_previews(&ctx);
        app.texture = Some(ctx.load_texture(
            "palette_layout_fixture",
            egui::ColorImage::new([32, 24], vec![Color32::WHITE; 32 * 24]),
            egui::TextureOptions::NEAREST,
        ));
        for palette in [
            Palette::Brushes,
            Palette::Properties,
            Palette::Sizes,
            Palette::Color,
            Palette::Navigator,
            Palette::Layers,
            Palette::History,
        ] {
            for width in [180., 210., 260., 360.] {
                let mut measured = egui::Rect::NOTHING;
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(960., 900.),
                        )),
                        ..Default::default()
                    },
                    |ui| {
                        egui::CentralPanel::default().show(ui, |ui| {
                            ui.scope_builder(
                                egui::UiBuilder::new()
                                    .id_salt((palette, width as u32))
                                    .max_rect(egui::Rect::from_min_size(
                                        ui.cursor().min,
                                        egui::vec2(width, 800.),
                                    )),
                                |ui| {
                                    ui.set_width(width);
                                    app.palette(ui, palette, 190.);
                                    measured = ui.min_rect();
                                },
                            );
                        });
                    },
                );
                output.textures_delta.clear();
                assert!(
                    measured.width() <= width + 0.1,
                    "{palette:?}: width={width}, content={measured:?}"
                );
                assert_eq!(app.doc, artwork);
                if palette == Palette::Layers {
                    let blend: egui::Rect = ctx
                        .data(|data| {
                            data.get_temp(egui::Id::new("layer_blend"))
                        })
                        .unwrap();
                    let (opacity, _): (egui::Rect, f32) = ctx
                        .data(|data| {
                            data.get_temp(egui::Id::new("layer_opacity"))
                        })
                        .unwrap();
                    assert!((blend.center().y - opacity.center().y).abs() < 1.);
                    assert!(blend.right() < opacity.left());
                    let clipping: egui::Rect = ctx
                        .data(|data| {
                            data.get_temp(egui::Id::new((
                                "layer_property",
                                Command::LayerClipping,
                            )))
                        })
                        .unwrap();
                    let reference: egui::Rect = ctx
                        .data(|data| {
                            data.get_temp(egui::Id::new((
                                "layer_property",
                                Command::LayerReference,
                            )))
                        })
                        .unwrap();
                    assert!(clipping.top() > opacity.bottom());
                    assert!(clipping.right() < reference.left());
                }
            }
            if matches!(palette, Palette::Color | Palette::Navigator) {
                let mut previous_height = 0.;
                for body_height in [120., 190., 300.] {
                    let mut measured = egui::Rect::NOTHING;
                    let mut output = ctx.run_ui(
                        egui::RawInput {
                            screen_rect: Some(egui::Rect::from_min_size(
                                egui::Pos2::ZERO,
                                egui::vec2(960., 900.),
                            )),
                            ..Default::default()
                        },
                        |ui| {
                            egui::CentralPanel::default().show(ui, |ui| {
                                ui.scope_builder(
                                    egui::UiBuilder::new()
                                        .id_salt((
                                            "palette_height",
                                            palette,
                                            body_height as u32,
                                        ))
                                        .max_rect(egui::Rect::from_min_size(
                                            ui.cursor().min,
                                            egui::vec2(260., 800.),
                                        )),
                                    |ui| {
                                        ui.set_width(260.);
                                        app.palette(ui, palette, body_height);
                                        measured = ui.min_rect();
                                    },
                                );
                            });
                        },
                    );
                    output.textures_delta.clear();
                    assert!(
                        measured.height() > previous_height,
                        "{palette:?} did not follow the host height {body_height}"
                    );
                    previous_height = measured.height();
                    assert_eq!(app.doc, artwork);
                }
            }
        }
    }

    #[test]
    fn opacity_arrows_commit_single_steps_and_unchanged_display_preserves_redo()
    {
        let directory = tempfile::tempdir().unwrap();
        let mut app = Parfait::open(directory.path().to_owned()).unwrap();
        app.doc = parfait_core::Document::new(8, 8);
        app.doc.layers[1].opacity = 0.33333334;
        let context = egui::Context::default();
        crate::theme::apply(&context);
        let render = |app: &mut Parfait, events| {
            let mut output = context.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(240., 500.),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| app.palette(ui, crate::workspace::Palette::Layers, 400.),
            );
            output.textures_delta.clear();
        };
        for _ in 0..3 {
            render(&mut app, vec![]);
        }
        assert_eq!(app.doc.layers[1].opacity, 0.33333334);
        let arrow: egui::Rect = context
            .data(|data| data.get_temp(egui::Id::new(("opacity_arrow", true))))
            .unwrap();
        let pointer = |pressed| egui::Event::PointerButton {
            pos: arrow.center(),
            pressed,
            button: egui::PointerButton::Primary,
            modifiers: Default::default(),
        };
        render(
            &mut app,
            vec![egui::Event::PointerMoved(arrow.center()), pointer(true)],
        );
        render(&mut app, vec![pointer(false)]);
        assert_eq!(app.doc.layers[1].opacity, 0.34);
        assert_eq!(app.history.rows().count(), 2);
        app.undo();
        assert_eq!(app.doc.layers[1].opacity, 0.33333334);
        render(&mut app, vec![]);
        assert_eq!(app.doc.layers[1].opacity, 0.33333334);
        assert!(app.history.can_redo());
        app.redo();
        assert_eq!(app.doc.layers[1].opacity, 0.34);
    }
}
