use crate::{
    app::Parfait,
    brushes::pressure_graph,
    widgets::{self, Icon},
};
use eframe::egui::{self, Color32};
use parfait_core::{Brush, BrushTip};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PropertyLayout {
    visible: [bool; 9],
}
impl Default for PropertyLayout {
    fn default() -> Self {
        Self {
            visible: [true, true, true, true, false, false, false, false, false],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Property {
    Size,
    Opacity,
    Flow,
    Smoothing,
    Minimum,
    Hardness,
    Roundness,
    Angle,
    Spacing,
}
impl Property {
    const ALL: [Self; 9] = [
        Self::Size,
        Self::Opacity,
        Self::Flow,
        Self::Smoothing,
        Self::Minimum,
        Self::Hardness,
        Self::Roundness,
        Self::Angle,
        Self::Spacing,
    ];
    fn label(self) -> &'static str {
        match self {
            Self::Size => "Brush size",
            Self::Opacity => "Opacity",
            Self::Flow => "Flow",
            Self::Smoothing => "Smoothing",
            Self::Minimum => "Minimum size",
            Self::Hardness => "Hardness",
            Self::Roundness => "Roundness",
            Self::Angle => "Angle",
            Self::Spacing => "Spacing",
        }
    }
    fn help(self) -> &'static str {
        match self {
            Self::Size => {
                "Diameter in document pixels. Zoom does not change the painted size. The preview caps the diameter at 38 px."
            }
            Self::Opacity => {
                "Maximum coverage of each dab, before flow and tip coverage. Overlapping dabs can build up paint; this is not layer opacity."
            }
            Self::Flow => {
                "Amount deposited by each dab. Lower flow builds up gradually. Continuous spraying and its Rate are separate settings under Stroke."
            }
            Self::Smoothing => {
                "Filters pointer movement. Higher values smooth movement but can trail the cursor. The static tip preview does not simulate this."
            }
            Self::Minimum => {
                "Fraction of brush size retained at the lightest pressure. Only used when pressure controls size."
            }
            Self::Hardness => {
                "Controls the edge of the Round tip. Soft, Pencil, Flat and imported image tips use their own coverage and do not use this setting."
            }
            Self::Roundness => {
                "Compresses the tip into an ellipse. 100% keeps its original proportions. Use Angle to rotate the compressed tip."
            }
            Self::Angle => {
                "Rotation of the brush tip in degrees. Most visible on flat, compressed or asymmetric image tips."
            }
            Self::Spacing => {
                "Fixed-mode distance between dabs as a percentage of maximum brush diameter. The other Gap modes retain this value but use their own preset interval. Small gaps cost more work."
            }
        }
    }
    fn settings(self) -> (std::ops::RangeInclusive<f32>, f32, &'static str) {
        match self {
            Self::Size => (1. ..=512., 1., " px"),
            Self::Flow => (0.01..=1., 100., "%"),
            Self::Smoothing => (0. ..=0.95, 100., "%"),
            Self::Roundness => (0.1..=1., 100., "%"),
            Self::Angle => (-180. ..=180., 1., "°"),
            Self::Spacing => (0.01..=10., 100., "%"),
            _ => (0. ..=1., 100., "%"),
        }
    }
    fn value(self, brush: &mut Brush) -> &mut f32 {
        match self {
            Self::Size => &mut brush.size,
            Self::Opacity => &mut brush.opacity,
            Self::Flow => &mut brush.flow,
            Self::Smoothing => &mut brush.smoothing,
            Self::Minimum => &mut brush.minimum_size,
            Self::Hardness => &mut brush.hardness,
            Self::Roundness => &mut brush.aspect,
            Self::Angle => &mut brush.angle,
            Self::Spacing => &mut brush.spacing,
        }
    }
    fn enabled(self, brush: &Brush) -> bool {
        match self {
            Self::Hardness => matches!(brush.tip, BrushTip::Round),
            Self::Minimum => brush.pressure,
            Self::Spacing => brush.gap_mode == parfait_core::GapMode::Fixed,
            _ => true,
        }
    }
}

#[derive(Default, Clone, Copy, PartialEq)]
enum Category {
    #[default]
    Size,
    Ink,
    Tip,
    Stroke,
    Correction,
    Pressure,
}
impl Category {
    fn summary(self) -> &'static str {
        match self {
            Self::Size => "Set brush size and pressure response.",
            Self::Ink => "Adjust opacity and paint deposition.",
            Self::Tip => "Set the brush tip shape and orientation.",
            Self::Stroke => "Set dab intervals and continuous spraying.",
            Self::Correction => "Smooth pointer movement.",
            Self::Pressure => "Shape the response to pen pressure.",
        }
    }
    const ALL: [Self; 6] = [
        Self::Size,
        Self::Ink,
        Self::Tip,
        Self::Stroke,
        Self::Correction,
        Self::Pressure,
    ];
    fn label(self) -> &'static str {
        match self {
            Self::Size => "Brush size",
            Self::Ink => "Ink",
            Self::Tip => "Brush tip",
            Self::Stroke => "Stroke",
            Self::Correction => "Correction",
            Self::Pressure => "Pressure",
        }
    }
    fn help(self) -> &'static str {
        match self {
            Self::Size => {
                "Set the brush diameter and the minimum size retained at light pressure. The eye buttons choose which values appear in Tool property."
            }
            Self::Ink => {
                "Control opacity and deposition per dab. Pressure can also control opacity. These controls do not implement wet color mixing."
            }
            Self::Tip => {
                "Choose a procedural tip or retain an imported image. Adjust edge hardness, roundness and angle. Import image tips from Sub tools → Manage."
            }
            Self::Stroke => {
                "Choose a Gap mode, edit Fixed spacing, or keep depositing paint while stationary with Continuous spraying. Narrow gaps and higher rates increase work; pointer smoothing is separate."
            }
            Self::Correction => {
                "Smooth pointer movement before dabs are placed. This filter can trail the cursor; predictive stabilization and stroke taper are not implemented."
            }
            Self::Pressure => {
                "Device calibration is applied before this brush response. Size and opacity share this curve. Mouse input uses full pressure."
            }
        }
    }
    fn properties(self) -> &'static [Property] {
        match self {
            Self::Size => &[Property::Size, Property::Minimum],
            Self::Ink => &[Property::Opacity, Property::Flow],
            Self::Tip => &[Property::Hardness, Property::Roundness, Property::Angle],
            Self::Stroke => &[Property::Spacing],
            Self::Correction => &[Property::Smoothing],
            Self::Pressure => &[],
        }
    }
}

#[derive(Default)]
pub struct BrushDetails {
    pub open: bool,
    pub properties: PropertyLayout,
    dynamics: Option<Property>,
    category: Category,
    help: Option<Property>,
    preview: Option<(Brush, egui::TextureHandle)>,
    #[cfg(test)]
    pin_targets: Vec<(Property, egui::Rect)>,
    #[cfg(test)]
    category_targets: Vec<egui::Rect>,
    #[cfg(test)]
    dynamics_targets: Option<(egui::Rect, egui::Rect)>,
}
impl BrushDetails {
    pub fn open_stroke_settings(&mut self) {
        self.open = true;
        self.category = Category::Stroke;
    }
    pub fn dialog_open(&self) -> bool {
        self.open || self.dynamics.is_some()
    }
    pub fn open_pressure_dynamics(&mut self) {
        self.dynamics = Some(Property::Size);
    }
}

fn property_control(
    ui: &mut egui::Ui,
    brush: &mut Brush,
    property: Property,
    layout: Option<&mut PropertyLayout>,
    help: &mut Option<Property>,
) -> bool {
    let parameter_layout = if layout.is_some() {
        crate::brush_widgets::ParameterLayout::Detail
    } else {
        crate::brush_widgets::ParameterLayout::Palette
    };
    ui.push_id(property.label(), |ui| {
        if property == Property::Spacing {
            crate::brush_stroke::gap_selector(ui, brush);
        }
        let enabled = property.enabled(brush);
        let dynamics = match property {
            Property::Size => Some(brush.pressure),
            Property::Opacity => Some(brush.pressure_opacity),
            _ => None,
        };
        let (range, scale, unit) = property.settings();
        let value = property.value(brush);
        let mut shown = *value * scale;
        let bounds = (*range.start() * scale)..=(*range.end() * scale);
        let mut dynamics_clicked = false;
        let row = ui
            .horizontal(|ui| {
                if let Some(layout) = layout {
                    let visible = &mut layout.visible[property as usize];
                    let response = widgets::icon(
                        ui,
                        if *visible { Icon::Eye } else { Icon::EyeSlash },
                        *visible,
                        &format!(
                            "{} {} in Tool property",
                            if *visible { "Hide" } else { "Show" },
                            property.label()
                        ),
                    );
                    #[cfg(test)]
                    ui.data_mut(|data| {
                        data.insert_temp(
                            egui::Id::new(("brush_property_pin", property as usize)),
                            response.rect,
                        )
                    });
                    if response.clicked() {
                        *visible = !*visible;
                    }
                }
                ui.add_enabled_ui(enabled, |ui| {
                    let control = crate::brush_widgets::parameter(
                        ui,
                        &mut shown,
                        crate::brush_widgets::Parameter {
                            label: property.label(),
                            range: bounds,
                            decimals: usize::from(
                                property == Property::Size || property == Property::Angle,
                            ),
                            unit,
                            logarithmic: property == Property::Size,
                            dynamics,
                            layout: parameter_layout,
                        },
                    );
                    dynamics_clicked = control.dynamics_clicked;
                    if control.response.changed() {
                        *help = Some(property);
                    }
                });
            })
            .response
            .on_hover_text(property.help());
        if row.hovered() || row.contains_pointer() {
            *help = Some(property);
        }
        *value = shown / scale;
        dynamics_clicked
    })
    .inner
}

impl Parfait {
    fn brush_preview(&mut self, ui: &mut egui::Ui, height: f32, show_name: bool) {
        let changed = self
            .brush_details
            .preview
            .as_ref()
            .is_none_or(|(brush, _)| brush != &self.brush);
        if changed && self.stroke.is_none() {
            let image = crate::brushes::preview_image(&self.brush);
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
            let preview = ui.add(egui::Image::new(texture).maintain_aspect_ratio(false).fit_to_exact_size(egui::vec2(ui.available_width(), height)))
                .on_hover_text("Neutral ink · maximum 38 px · varying pressure · erasing, calibration, smoothing and stationary spraying are not simulated");
            if show_name {
                let name = format!(
                    "{}{}",
                    self.tip_name,
                    if self.preset_modified() {
                        " · Modified"
                    } else {
                        ""
                    },
                );
                let galley = ui.painter().layout_no_wrap(
                    name,
                    egui::FontId::proportional(11.5),
                    ui.visuals().strong_text_color(),
                );
                let label = egui::Rect::from_min_size(
                    preview.rect.min + egui::vec2(3., 3.),
                    galley.size() + egui::vec2(6., 2.),
                );
                let painter = ui.painter().with_clip_rect(preview.rect);
                painter.rect_filled(label, 1., Color32::from_gray(93));
                painter.galley(label.min + egui::vec2(3., 1.), galley, Color32::WHITE);
            }
        }
    }
    pub fn brush_properties_ui(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing.y = 2.;
        if self.navigation_tool != 0 {
            ui.strong(if self.navigation_tool == 1 {
                "Hand"
            } else {
                "Eyedropper"
            });
            ui.label(if self.navigation_tool == 1 { "Drag to pan the canvas. Space temporarily pans while a drawing tool is selected." } else { "Click the canvas to sample its composited color. Alt temporarily samples while drawing." });
            if ui.button("Return to drawing tool").clicked() {
                self.navigation_tool = 0;
            }
            return;
        }
        ui.horizontal(|ui| {
            ui.label(&self.tip_name);
            if self.preset_modified() {
                ui.weak("*").on_hover_text(
                    "Modified brush or visible properties · Save to update the preset",
                );
            }
        });
        ui.horizontal(|ui| {
            if ui.small_button("Details…").clicked() {
                self.brush_details.open = true;
            }
            if ui
                .small_button("Save")
                .on_hover_text("Save brush settings and visible properties")
                .clicked()
            {
                self.save_current_preset();
            }
            if ui
                .add_enabled(
                    self.preset_modified()
                        && self
                            .presets
                            .iter()
                            .any(|preset| preset.name == self.tip_name),
                    egui::Button::new("Restore").small(),
                )
                .on_hover_text(
                    "Restore the saved brush and visible properties; keep the current color",
                )
                .clicked()
            {
                self.restore_current_preset();
            }
        });
        self.brush_preview(ui, 38., false);
        for property in Property::ALL {
            if self.brush_details.properties.visible[property as usize]
                && property_control(
                    ui,
                    &mut self.brush,
                    property,
                    None,
                    &mut self.brush_details.help,
                )
            {
                self.brush_details.dynamics = Some(property);
            }
        }
        if !self.brush_details.properties.visible.contains(&true) {
            ui.weak("Use the eyes in Details to show properties here.");
        }
    }
    pub fn brush_details_ui(&mut self, ctx: &egui::Context) {
        let dynamics_was_open = self.brush_details.dynamics.is_some();
        if !self.brush_details.open {
            self.brush_dynamics_ui(ctx);
            return;
        }
        let mut open = true;
        let mut close = false;
        let calibration_open = self.calibration_dialog.is_some();
        let controls_height = (ctx.content_rect().height() - 350.).clamp(100., 240.);
        #[cfg(test)]
        self.brush_details.pin_targets.clear();
        #[cfg(test)]
        self.brush_details.category_targets.clear();
        egui::Window::new(egui::RichText::new("Sub tool detail").size(11.5))
            .id(egui::Id::new("brush_details"))
            .open(&mut open)
            .collapsible(false)
            .frame(
                egui::Frame::window(&ctx.style_of(egui::Theme::Dark))
                    .corner_radius(0)
                    .fill(Color32::from_gray(65)),
            )
            .default_size([410., 480.])
            .title_frame(
                egui::Frame::window(&ctx.style_of(egui::Theme::Dark))
                    .corner_radius(0)
                    .inner_margin(egui::Margin::symmetric(6, 0))
                    .fill(Color32::from_gray(65)),
            )
            .default_pos(ctx.content_rect().center() - egui::vec2(205., 240.))
            .constrain_to(ctx.content_rect())
            .min_width(410.)
            .show(ctx, |ui| {
                ui.add_enabled_ui(!calibration_open && !dynamics_was_open, |ui| {
                    self.brush_preview(ui, 100., true);
                    ui.add_space(4.);
                    ui.horizontal_top(|ui| {
                        egui::Frame::NONE
                            .fill(Color32::from_gray(53))
                            .inner_margin(4)
                            .show(ui, |ui| {
                                ui.vertical(|ui| {
                                    ui.set_width(130.);
                                    for category in Category::ALL {
                                        let response =
                                            ui.add_sized(
                                                [130., 20.],
                                                egui::Button::selectable(
                                                    self.brush_details.category == category,
                                                    "",
                                                )
                                                .left_text(category.label())
                                                .fill(if self.brush_details.category == category {
                                                    ui.visuals().selection.bg_fill
                                                } else {
                                                    Color32::from_gray(53)
                                                }),
                                            );
                                        #[cfg(test)]
                                        self.brush_details.category_targets.push(response.rect);
                                        if response.clicked() {
                                            self.brush_details.category = category;
                                            self.brush_details.help = None;
                                        }
                                    }
                                });
                            });
                        ui.vertical(|ui| {
                            ui.set_min_width(240.);
                            ui.label(self.brush_details.category.summary())
                                .on_hover_text(self.brush_details.category.help());
                            ui.separator();
                            if self.brush_details.category == Category::Pressure
                                && ui.button("Calibrate tablet…").clicked()
                            {
                                self.calibration_dialog =
                                    Some(crate::calibration::CalibrationDialog::new(
                                        self.calibration.clone(),
                                    ));
                            }
                            egui::ScrollArea::vertical()
                                .id_salt("brush_detail_controls")
                                .max_height(controls_height)
                                .min_scrolled_height(100.)
                                .show(ui, |ui| {
                                    if self.brush_details.category == Category::Tip {
                                        self.tip_shape_ui(ui);
                                    }
                                    for &property in self.brush_details.category.properties() {
                                        egui::Frame::NONE
                                            .fill(Color32::from_gray(69))
                                            .inner_margin(egui::Margin::symmetric(2, 0))
                                            .show(ui, |ui| {
                                                if property_control(
                                                    ui,
                                                    &mut self.brush,
                                                    property,
                                                    Some(&mut self.brush_details.properties),
                                                    &mut self.brush_details.help,
                                                ) {
                                                    self.brush_details.dynamics = Some(property);
                                                }
                                                #[cfg(test)]
                                                if let Some(rect) = ui.data(|data| {
                                                    data.get_temp::<egui::Rect>(egui::Id::new((
                                                        "brush_property_pin",
                                                        property as usize,
                                                    )))
                                                }) {
                                                    self.brush_details
                                                        .pin_targets
                                                        .push((property, rect));
                                                }
                                            });
                                    }
                                    match self.brush_details.category {
                                        Category::Size => {
                                            ui.checkbox(
                                                &mut self.brush.pressure,
                                                "Pressure controls size",
                                            );
                                            if ui.button("Pressure response…").clicked() {
                                                self.brush_details.category = Category::Pressure;
                                                self.brush_details.help = None;
                                            }
                                        }
                                        Category::Ink => {
                                            ui.checkbox(
                                                &mut self.brush.pressure_opacity,
                                                "Pressure controls opacity",
                                            );
                                        }
                                        Category::Stroke => {
                                            crate::brush_stroke::spray_controls(ui, &mut self.brush)
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
                                                        self.brush.pressure_curve =
                                                            if name == "Linear" {
                                                                Default::default()
                                                            } else {
                                                                curve.into()
                                                            };
                                                    }
                                                }
                                            });
                                        }
                                        _ => {}
                                    }
                                });
                            ui.separator();
                            egui::Frame::NONE
                                .fill(Color32::from_gray(54))
                                .inner_margin(7)
                                .show(ui, |ui| {
                                    ui.set_min_height(64.);
                                    ui.set_min_width(ui.available_width());
                                    ui.strong(
                                        self.brush_details
                                            .help
                                            .map_or("Parameter guide", Property::label),
                                    );
                                    ui.label(self.brush_details.help.map_or(
                                        "Point to a setting to read its description. Edits apply immediately.",
                                        Property::help,
                                    ));
                                });
                        });
                    });
                    ui.separator();
                    ui.horizontal_wrapped(|ui| {
                        if ui
                            .button("Default property layout")
                            .on_hover_text(
                                "Reset visible properties only; brush values are retained",
                            )
                            .clicked()
                        {
                            self.brush_details.properties = Default::default();
                        }
                        if ui.button("Save preset").clicked() {
                            self.save_current_preset();
                        }
                        if ui
                            .add_enabled(
                                self.preset_modified()
                                    && self
                                        .presets
                                        .iter()
                                        .any(|preset| preset.name == self.tip_name),
                                egui::Button::new("Restore saved"),
                            )
                            .clicked()
                        {
                            self.restore_current_preset();
                        }
                        let close_button = ui.button("Close");
                        #[cfg(test)]
                        ui.data_mut(|data| {
                            data.insert_temp(egui::Id::new("detail_close"), close_button.rect)
                        });
                        if close_button.clicked() {
                            close = true;
                        }
                    });
                    ui.weak("Eye: show in Tool property · edits apply immediately");
                });
            });
        if !calibration_open
            && !dynamics_was_open
            && ctx.input(|input| input.key_pressed(egui::Key::Escape))
        {
            close = true;
        }
        self.brush_details.open = open && !close;
        self.brush_dynamics_ui(ctx);
    }
    fn brush_dynamics_ui(&mut self, ctx: &egui::Context) {
        let Some(property) = self.brush_details.dynamics else {
            return;
        };
        let mut open = true;
        let mut close = false;
        egui::Window::new(format!("{} dynamics", property.label()))
            .id(egui::Id::new("brush_dynamics"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(320.)
            .default_pos(ctx.content_rect().center() - egui::vec2(160., 210.))
            .frame(egui::Frame::window(&ctx.style_of(egui::Theme::Dark)).corner_radius(0).fill(Color32::from_gray(65)))
            .show(ctx, |ui| {
                let active = if property == Property::Opacity { &mut self.brush.pressure_opacity } else { &mut self.brush.pressure };
                ui.checkbox(active, "Pen pressure");
                if property == Property::Size {
                    property_control(ui, &mut self.brush, Property::Minimum, None, &mut self.brush_details.help);
                }
                ui.separator();
                let _plot = pressure_graph(ui, &mut self.brush.pressure_curve, self.calibration.map(self.pressure));
                ui.horizontal(|ui| {
                    for (name, curve) in [("Linear", [0., 0.25, 0.5, 0.75, 1.]), ("Soft", [0., 0.5, 0.71, 0.87, 1.]), ("Firm", [0., 0.06, 0.25, 0.56, 1.])] {
                        if ui.button(name).clicked() { self.brush.pressure_curve = if name == "Linear" { Default::default() } else { curve.into() }; }
                    }
                });
                ui.separator();
                ui.label("Square: move anchor. Round handle: bend the curve without moving the anchor. Select an anchor to show its handles. Size and opacity share this response.");
                ui.weak("Device calibration is separate. Tilt, velocity and random dynamics are not supported.");
                ui.horizontal(|ui| {
                    if ui.button("Save preset").clicked() { self.save_current_preset(); }
                    let response = ui.button("Close");
                    #[cfg(test)] { self.brush_details.dynamics_targets = Some((_plot, response.rect)); }
                    if response.clicked() { close = true; }
                });
            });
        if self.calibration_dialog.is_none()
            && ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            close = true;
        }
        if !open || close {
            self.brush_details.dynamics = None;
        }
    }
    fn tip_shape_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Tip shape");
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
                        if ui.selectable_label(label == name, name).clicked() {
                            self.brush.tip = tip;
                        }
                    }
                });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use parfait_core::Document;

    #[test]
    fn stroke_controls_and_close_are_reachable_at_minimum_window_size() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open(dir.path().to_owned()).unwrap();
        app.brush_details.open_stroke_settings();
        let artwork = app.doc.clone();
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(960., 640.));
        let frame = |app: &mut Parfait, events| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(screen),
                    events,
                    ..Default::default()
                },
                |ui| app.brush_details_ui(ui.ctx()),
            );
            output.textures_delta.clear();
        };
        frame(&mut app, vec![]);
        frame(&mut app, vec![]);
        for label in ["Fixed", "Wide", "Normal", "Narrow"] {
            assert!(screen.contains_rect(ctx.data(|data| data.get_temp::<egui::Rect>(egui::Id::new(("gap_mode", label)))).unwrap()));
        }
        let close = ctx
            .data(|data| data.get_temp::<egui::Rect>(egui::Id::new("detail_close")))
            .unwrap();
        assert!(screen.contains_rect(close));
        let event = |pressed| egui::Event::PointerButton {
            pos: close.center(),
            pressed,
            button: egui::PointerButton::Primary,
            modifiers: Default::default(),
        };
        frame(
            &mut app,
            vec![egui::Event::PointerMoved(close.center()), event(true)],
        );
        frame(&mut app, vec![event(false)]);
        assert!(!app.brush_details.dialog_open());
        assert_eq!(app.doc, artwork);
    }

    #[test]
    fn free_curve_dialog_keeps_close_visible_after_adding_a_point_at_minimum_window_size() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open(dir.path().to_owned()).unwrap();
        app.brush_details.open_pressure_dynamics();
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(960., 640.));
        let frame = |app: &mut Parfait, events| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(screen),
                    events,
                    ..Default::default()
                },
                |ui| app.brush_details_ui(ui.ctx()),
            );
            output.textures_delta.clear();
        };
        frame(&mut app, vec![]);
        frame(&mut app, vec![]);
        let (plot, close) = app.brush_details.dynamics_targets.unwrap();
        assert!(screen.contains_rect(close));
        let position = plot.center();
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            pressed,
            button: egui::PointerButton::Primary,
            modifiers: Default::default(),
        };
        frame(
            &mut app,
            vec![egui::Event::PointerMoved(position), button(position, true)],
        );
        frame(&mut app, vec![button(position, false)]);
        frame(&mut app, vec![]);
        assert_eq!(app.brush.pressure_curve.points.len(), 3);
        let (_, close) = app.brush_details.dynamics_targets.unwrap();
        assert!(
            screen.contains_rect(close),
            "close button outside screen: {close:?}"
        );
        frame(
            &mut app,
            vec![
                egui::Event::PointerMoved(close.center()),
                button(close.center(), true),
            ],
        );
        frame(&mut app, vec![button(close.center(), false)]);
        assert!(!app.brush_details.dialog_open());
    }

    #[test]
    fn dynamics_escape_dismisses_child_before_details_and_preserves_saved_response() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open(dir.path().to_owned()).unwrap();
        let artwork = app.doc.clone();
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        app.brush_details.open = true;
        app.brush_details.open_pressure_dynamics();
        app.brush.pressure_curve = [0., 0.1, 0.4, 0.8, 1.].into();
        app.save_current_preset();
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
                |ui| app.brush_details_ui(ui.ctx()),
            );
            output.textures_delta.clear();
        };
        frame(&mut app, vec![]);
        let escape = || egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        };
        frame(&mut app, vec![escape()]);
        assert!(app.brush_details.open);
        assert!(app.brush_details.dynamics.is_none());
        frame(&mut app, vec![]);
        frame(&mut app, vec![escape()]);
        assert!(!app.brush_details.dialog_open());
        assert_eq!(app.doc, artwork);
        app.brush.pressure_curve = Default::default();
        app.restore_current_preset();
        assert_eq!(
            app.brush.pressure_curve,
            parfait_core::ResponseCurve::from([0., 0.1, 0.4, 0.8, 1.])
        );
        app.persist_preferences(true);
        drop(app);
        let reopened = Parfait::open(dir.path().to_owned()).unwrap();
        assert_eq!(
            reopened.brush.pressure_curve,
            parfait_core::ResponseCurve::from([0., 0.1, 0.4, 0.8, 1.])
        );
    }

    #[test]
    fn eye_button_changes_only_layout_and_saved_presets_restore_it_after_restart() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open(dir.path().to_owned()).unwrap();
        let artwork = app.doc.clone();
        let brush = app.brush.clone();
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        app.brush_details.open = true;
        app.brush_details.category = Category::Stroke;
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
                |ui| app.brush_details_ui(ui.ctx()),
            );
            output.textures_delta.clear();
        };
        frame(&mut app, vec![]);
        frame(&mut app, vec![]);
        let target = app
            .brush_details
            .pin_targets
            .iter()
            .find(|(property, _)| *property == Property::Spacing)
            .unwrap()
            .1;
        assert!(ctx.content_rect().contains_rect(target));
        let event = |pressed| egui::Event::PointerButton {
            pos: target.center(),
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        frame(
            &mut app,
            vec![egui::Event::PointerMoved(target.center()), event(true)],
        );
        frame(&mut app, vec![event(false)]);
        assert!(app.brush_details.properties.visible[Property::Spacing as usize]);
        assert!(app.brush == brush);
        assert_eq!(app.doc, artwork);
        assert!(app.preset_modified());
        app.save_current_preset();
        app.brush_details.properties = Default::default();
        app.restore_current_preset();
        assert!(app.brush_details.properties.visible[Property::Spacing as usize]);
        app.select_brush_group(crate::brushes::BrushGroup::Eraser);
        assert!(!app.brush_details.properties.visible[Property::Spacing as usize]);
        app.select_brush_group(crate::brushes::BrushGroup::Ink);
        assert!(app.brush_details.properties.visible[Property::Spacing as usize]);
        app.persist_preferences(true);
        assert!(app.error.is_none());
        drop(app);
        let restored = Parfait::open(dir.path().to_owned()).unwrap();
        assert!(restored.brush_details.properties.visible[Property::Spacing as usize]);
        assert!(restored.presets[0].properties.visible[Property::Spacing as usize]);
        let mut old: serde_json::Value =
            serde_json::from_str(&restored.saved_preferences_json()).unwrap();
        old.as_object_mut().unwrap().remove("property_layout");
        for preset in old["presets"].as_array_mut().unwrap() {
            preset.as_object_mut().unwrap().remove("properties");
        }
        drop(restored);
        parfait_storage::save_preferences(
            &dir.path().join("preferences.sqlite"),
            &serde_json::to_string(&old).unwrap(),
        )
        .unwrap();
        let legacy = Parfait::open(dir.path().to_owned()).unwrap();
        assert!(!legacy.preferences_read_failed);
        assert_eq!(legacy.brush_details.properties, PropertyLayout::default());
        assert_eq!(legacy.presets[0].properties, PropertyLayout::default());
    }

    #[test]
    fn categories_preview_cache_and_escape_preserve_artwork_and_edits() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open(dir.path().to_path_buf()).unwrap();
        app.doc = Document::new(16, 16);
        let before = app.doc.clone();
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
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
                |ui| app.brush_details_ui(ui.ctx()),
            );
            output.textures_delta.clear();
        };
        app.brush_details.open = true;
        for category in Category::ALL {
            app.brush_details.category = category;
            frame(&mut app, vec![]);
            assert!(app.brush.validate());
            assert_eq!(app.doc, before);
            assert_eq!(app.brush_details.category_targets.len(), 6);
            assert!(
                app.brush_details
                    .category_targets
                    .iter()
                    .all(|rect| ctx.content_rect().contains_rect(*rect))
            );
            assert!(
                app.brush_details
                    .category_targets
                    .windows(2)
                    .all(|pair| pair[0].bottom() <= pair[1].top())
            );
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
                .find(|preset| preset.name == app.tip_name)
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
