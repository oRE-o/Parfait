use crate::{
    app::Parfait,
    brushes::pressure_graph,
    widgets::{self, Icon},
};
use eframe::egui::{self, Color32};
use parfait_core::{Brush, BrushOperation, BrushTip, OpacityMode};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PropertyLayout {
    visible: [bool; 9],
    #[serde(default)]
    blending_visible: [bool; 2],
}
impl Default for PropertyLayout {
    fn default() -> Self {
        Self {
            visible: [true, true, true, true, false, false, false, false, false],
            blending_visible: [false; 2],
        }
    }
}
impl PropertyLayout {
    pub fn blending(blur: bool) -> Self {
        Self {
            blending_visible: [!blur, blur],
            ..Default::default()
        }
    }
    fn visibility(&self, property: Property) -> bool {
        if (property as usize) < 9 {
            self.visible[property as usize]
        } else {
            self.blending_visible[property as usize - 9]
        }
    }
    fn visibility_mut(&mut self, property: Property) -> &mut bool {
        if (property as usize) < 9 {
            &mut self.visible[property as usize]
        } else {
            &mut self.blending_visible[property as usize - 9]
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
    Stretch,
    BlurRadius,
}
impl Property {
    const ALL: [Self; 11] = [
        Self::Size,
        Self::Opacity,
        Self::Flow,
        Self::Smoothing,
        Self::Minimum,
        Self::Hardness,
        Self::Roundness,
        Self::Angle,
        Self::Spacing,
        Self::Stretch,
        Self::BlurRadius,
    ];
    fn label(self) -> &'static str {
        match self {
            Self::Size => "Brush size",
            Self::Opacity => "Opacity",
            Self::Flow => "Brush density",
            Self::Smoothing => "Smoothing",
            Self::Minimum => "Minimum size",
            Self::Hardness => "Hardness",
            Self::Roundness => "Roundness",
            Self::Angle => "Angle",
            Self::Spacing => "Spacing",
            Self::Stretch => "Color stretch",
            Self::BlurRadius => "Fixed blur radius",
        }
    }
    fn help(self) -> &'static str {
        match self {
            Self::Size => {
                "Diameter in document pixels. Zoom does not change the painted size. The preview caps the diameter at 38 px."
            }
            Self::Opacity => {
                "Per dab accumulates each stamp. Per stroke limits this stroke's contribution; another stroke can build up again. Pressure sets the local target without undoing stronger marks. Blending tools retain per-dab strength. This is not layer opacity."
            }
            Self::Flow => {
                "Per-tip deposition (the saved flow value). In Per stroke mode each dab approaches the opacity limit; lower density builds up slowly without exceeding that limit. Blending tools use this as per-dab strength. Timed spraying is separate."
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
                "Fixed-mode distance between dabs as a percentage of maximum brush diameter. Dynamics can vary this interval with pressure using an independent curve. Other Gap modes retain these values but use preset intervals. Small gaps cost more work."
            }
            Self::Stretch => {
                "Color blend retains a sampled average; Smudge carries a spatial pixel patch. Higher values retain the start color or texture farther; neither adds the drawing color. Smudge at 0% or without movement has no effect."
            }
            Self::BlurRadius => {
                "Local blur only. Box-filter radius in document pixels, used when Automatic blur size is off. The automatic mode retains this saved value."
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
            Self::BlurRadius => (1. ..=32., 1., " px"),
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
            Self::Stretch => &mut brush.color_stretch,
            Self::BlurRadius => &mut brush.blur_radius,
        }
    }
    fn enabled(self, brush: &Brush) -> bool {
        match self {
            Self::Hardness => matches!(brush.tip, BrushTip::Round),
            Self::Minimum => brush.pressure,
            Self::Spacing => brush.gap_mode == parfait_core::GapMode::Fixed,
            Self::Stretch => matches!(
                brush.operation(),
                BrushOperation::Blend | BrushOperation::Smudge
            ),
            Self::BlurRadius => brush.operation() == BrushOperation::Blur && !brush.blur_automatic,
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
            Self::Ink => "Choose a pixel operation and its strength.",
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
                "Paint and Erase change pigment coverage. Color blend carries an average, Smudge carries a pixel patch, and Local blur filters nearby pixels. Blending operations use only the active layer, not the drawing color. Pigment-adding wet mixing is not implemented."
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
            Self::Ink => &[Property::Opacity],
            Self::Tip => &[
                Property::Hardness,
                Property::Roundness,
                Property::Angle,
                Property::Flow,
            ],
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
    pub fn clear_help(&mut self) {
        self.help = None;
    }
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
            Property::Spacing => Some(brush.pressure_spacing),
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
                    ui.spacing_mut().interact_size.y = 28.;
                    let visible = layout.visibility_mut(property);
                    let response = widgets::flat_icon(
                        ui,
                        if *visible { Icon::Eye } else { Icon::EyeSlash },
                        *visible,
                        &format!(
                            "{} {} in Tool property",
                            if *visible { "Hide" } else { "Show" },
                            property.label()
                        ),
                        20.,
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
    fn opacity_mode_ui(&mut self, ui: &mut egui::Ui) {
        let supported = matches!(
            self.brush.operation(),
            BrushOperation::Paint | BrushOperation::Erase
        );
        ui.add_enabled_ui(supported, |ui| {
            ui.horizontal(|ui| {
                ui.label("Accumulation");
                let response = egui::ComboBox::from_id_salt("opacity_accumulation")
                    .selected_text(match self.brush.opacity_mode {
                        OpacityMode::PerDab => "Per dab (build up)",
                        OpacityMode::PerStroke => "Per stroke (capped)",
                    }).width(ui.available_width()).show_ui(ui, |ui| {
                        for (mode, label) in [(OpacityMode::PerDab, "Per dab (build up)"), (OpacityMode::PerStroke, "Per stroke (capped)")] {
                            let response = ui.selectable_value(&mut self.brush.opacity_mode, mode, label);
                            #[cfg(test)]
                            ui.data_mut(|data| data.insert_temp(egui::Id::new(("opacity_mode", mode as u8)), response.rect));
                            let _ = response;
                        }
                    }).response.on_hover_text("Per stroke keeps a tiled contribution mask until pen-up and shares the original layer pixels. Tiles are allocated as needed, up to 64 MiB for a fully covered 4096 × 4096 canvas, plus 32 KiB indexing and existing layer copy-on-write. Per dab adds no scratch.");
                #[cfg(test)]
                ui.data_mut(|data| data.insert_temp(egui::Id::new("opacity_mode_combo"), response.rect));
                let _ = response;
            });
        });
        if !supported {
            ui.weak("Blending uses per-dab strength; the saved mode is retained.");
        }
    }
    fn brush_operation_ui(&mut self, ui: &mut egui::Ui) {
        let mut operation = self.brush.operation();
        let label = |operation| match operation {
            BrushOperation::Paint => "Paint",
            BrushOperation::Erase => "Erase",
            BrushOperation::Blend => "Color blend",
            BrushOperation::Blur => "Local blur",
            BrushOperation::Smudge => "Smudge",
        };
        ui.horizontal(|ui| {
            ui.label("Operation");
            let combo = egui::ComboBox::from_id_salt("brush_operation")
                .selected_text(label(operation))
                .width(ui.available_width())
                .show_ui(ui, |ui| {
                    for choice in [
                        BrushOperation::Paint,
                        BrushOperation::Erase,
                        BrushOperation::Blend,
                        BrushOperation::Blur,
                        BrushOperation::Smudge,
                    ] {
                        let response = ui.selectable_value(&mut operation, choice, label(choice));
                        #[cfg(test)]
                        ui.data_mut(|data| {
                            data.insert_temp(
                                egui::Id::new(("operation_choice", label(choice))),
                                response.rect,
                            )
                        });
                        let _ = response;
                    }
                });
            #[cfg(test)]
            ui.data_mut(|data| {
                data.insert_temp(egui::Id::new("operation_combo"), combo.response.rect)
            });
            let _ = combo;
        });
        if operation != self.brush.operation() {
            self.brush.operation = operation;
            self.brush.erase = false;
            self.brush_details.help = None;
        }
    }
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
                .on_hover_text("Neutral ink; blending operations process seeded pixels, with stripes for Smudge · maximum 38 px · varying pressure · erasing, calibration, smoothing and stationary spraying are not simulated");
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
                let mut job = egui::text::LayoutJob::simple_singleline(
                    name,
                    egui::FontId::proportional(11.5),
                    ui.visuals().strong_text_color(),
                );
                job.wrap = egui::text::TextWrapping::truncate_at_width(
                    (preview.rect.width() - 12.).max(1.),
                );
                let galley = ui.painter().layout_job(job);
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
        self.brush_preview(ui, 48., true);
        for property in Property::ALL {
            if self.brush_details.properties.visibility(property)
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
        if !self.brush_details.properties.visible.contains(&true)
            && !self
                .brush_details
                .properties
                .blending_visible
                .contains(&true)
        {
            ui.weak("Use the eyes in Details to show properties here.");
        }
        ui.separator();
        widgets::action_row(ui, |ui| {
            if widgets::flat_icon(ui, Icon::Settings, false, "Sub tool detail", 24.).clicked() {
                self.brush_details.open = true;
            }
            let restorable = self.preset_modified()
                && self
                    .presets
                    .iter()
                    .any(|preset| preset.name == self.tip_name);
            ui.add_enabled_ui(restorable, |ui| {
                if widgets::flat_icon(
                    ui,
                    Icon::Undo,
                    false,
                    "Restore saved brush and visible properties",
                    24.,
                )
                .clicked()
                {
                    self.restore_current_preset();
                }
            });
            if widgets::flat_icon(
                ui,
                Icon::Save,
                false,
                "Save brush settings and visible properties",
                24.,
            )
            .clicked()
            {
                self.save_current_preset();
            }
        });
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
                            ui.set_width(ui.available_width().max(240.));
                            ui.label(self.brush_details.category.summary())
                                .on_hover_text(self.brush_details.category.help());
                            ui.separator();
                            egui::ScrollArea::vertical()
                                .id_salt("brush_detail_controls")
                                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
                                .max_height(controls_height)
                                .min_scrolled_height(controls_height)
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    if self.brush_details.category == Category::Pressure
                                        && ui.button("Calibrate tablet…").clicked()
                                    {
                                        self.calibration_dialog =
                                            Some(crate::calibration::CalibrationDialog::new(
                                                self.calibration.clone(),
                                            ));
                                    }
                                    if self.brush_details.category == Category::Ink {
                                        self.brush_operation_ui(ui);
                                        self.opacity_mode_ui(ui);
                                    }
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
                                            let property = match self.brush.operation() {
                                                BrushOperation::Blend | BrushOperation::Smudge => Some(Property::Stretch),
                                                BrushOperation::Blur => {
                                                    let automatic = ui.checkbox(&mut self.brush.blur_automatic, "Automatic blur size");
                                                    #[cfg(test)]
                                                    ui.data_mut(|data| data.insert_temp(egui::Id::new("automatic_blur_size"), automatic.rect));
                                                    let _ = automatic;
                                                    Some(Property::BlurRadius)
                                                }
                                                _ => None,
                                            };
                                            if let Some(property) = property {
                                                property_control(ui, &mut self.brush, property, Some(&mut self.brush_details.properties), &mut self.brush_details.help);
                                            }
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
                                    egui::ScrollArea::vertical()
                                        .id_salt("brush_detail_help")
                                        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
                                        .max_height(64.)
                                        .min_scrolled_height(64.)
                                        .auto_shrink([false, false])
                                        .show(ui, |ui| {
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
                    });
                    ui.separator();
                    widgets::action_row(ui, |ui| {
                        let close_button = widgets::action_button(ui, "Close", 56.);
                        #[cfg(test)]
                        ui.data_mut(|data| {
                            data.insert_temp(egui::Id::new("detail_close"), close_button.rect)
                        });
                        if close_button.clicked() {
                            close = true;
                        }
                        if widgets::action_button(ui, "Save preset", 82.).clicked() {
                            self.save_current_preset();
                        }
                        let restorable = self.preset_modified() && self.presets.iter().any(|preset| preset.name == self.tip_name);
                        ui.add_enabled_ui(restorable, |ui| {
                            if widgets::action_button(ui, "Restore saved", 96.).clicked() {
                                self.restore_current_preset();
                            }
                        });
                        let options = ui.menu_button(egui::RichText::new(Icon::More.glyph()).font(egui::FontId::new(14., egui::FontFamily::Name("icons".into()))), |ui| {
                            if ui.button("Default property layout").on_hover_text("Reset visible properties only; brush values are retained").clicked() {
                                self.brush_details.properties = Default::default();
                                ui.close();
                            }
                        });
                        options.response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), "Property display options"));
                        options.response.on_hover_text("Property display options");
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
                let spacing = property == Property::Spacing;
                let supported = !spacing || self.brush.gap_mode == parfait_core::GapMode::Fixed;
                if !supported { ui.weak("Select Fixed Gap to edit spacing dynamics. Saved values are retained."); }
                ui.add_enabled_ui(supported, |ui| {
                let active = match property {
                    Property::Spacing => &mut self.brush.pressure_spacing,
                    Property::Opacity => &mut self.brush.pressure_opacity,
                    _ => &mut self.brush.pressure,
                };
                ui.checkbox(active, "Pen pressure");
                if property == Property::Size {
                    property_control(ui, &mut self.brush, Property::Minimum, None, &mut self.brush_details.help);
                }
                if spacing {
                    let mut percent = self.brush.minimum_spacing * 100.;
                    crate::brush_widgets::parameter(ui, &mut percent, crate::brush_widgets::Parameter {
                        label: "Minimum gap", range: 0. ..=100., decimals: 0, unit: "%", logarithmic: false, dynamics: None,
                        layout: crate::brush_widgets::ParameterLayout::Detail,
                    });
                    self.brush.minimum_spacing = percent / 100.;
                }
                ui.separator();
                let curve = if spacing { &mut self.brush.spacing_curve } else { &mut self.brush.pressure_curve };
                let _plot = ui.push_id(property.label(), |ui| pressure_graph(ui, curve, self.calibration.map(self.pressure))).inner;
                ui.horizontal(|ui| {
                    for (name, curve) in [("Linear", [0., 0.25, 0.5, 0.75, 1.]), ("Soft", [0., 0.5, 0.71, 0.87, 1.]), ("Firm", [0., 0.06, 0.25, 0.56, 1.])] {
                        if ui.button(name).clicked() {
                            let target = if spacing { &mut self.brush.spacing_curve } else { &mut self.brush.pressure_curve };
                            *target = if name == "Linear" { Default::default() } else { curve.into() };
                        }
                    }
                    if spacing && ui.button("Reverse").clicked() { self.brush.spacing_curve = [1., 0.75, 0.5, 0.25, 0.].into(); }
                });
                #[cfg(test)] { self.brush_details.dynamics_targets = Some((_plot, egui::Rect::NOTHING)); }
                });
                if spacing { ui.label(format!("Pressure 0%: {:.2} px · 100%: {:.2} px", self.brush.spacing_pixels_at(0.), self.brush.spacing_pixels_at(1.))); }
                ui.separator();
                ui.collapsing("Input behavior", |ui| {
                    ui.label(if spacing { "Square anchors change the response; round handles bend it. This curve affects Fixed Gap only, independently of size and opacity. The interval after each dab uses that dab's pressure, with a 0.5 px minimum." } else { "Square anchors change the response. Round handles change curvature. Size and opacity share this response." });
                    ui.weak("Device calibration is separate. Tilt, velocity and random dynamics are not supported.");
                });
                widgets::action_row(ui, |ui| {
                    let response = widgets::action_button(ui, "Close", 64.);
                    #[cfg(test)] { if let Some(targets) = &mut self.brush_details.dynamics_targets { targets.1 = response.rect; } }
                    if response.clicked() { close = true; }
                    if widgets::action_button(ui, "Save preset", 90.).clicked() { self.save_current_preset(); }
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
    fn detail_footer_does_not_move_with_categories_operations_or_long_help() {
        for size in [egui::vec2(960., 640.), egui::vec2(1440., 920.)] {
            let dir = tempfile::tempdir().unwrap();
            let mut app = Parfait::open(dir.path().to_owned()).unwrap();
            let artwork = app.doc.clone();
            app.brush_details.open = true;
            let ctx = egui::Context::default();
            crate::theme::apply(&ctx);
            let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, size);
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
                ctx.data(|data| data.get_temp::<egui::Rect>(egui::Id::new("detail_close")))
                    .unwrap()
            };
            for _ in 0..4 {
                frame(&mut app, vec![]);
            }
            let close = frame(&mut app, vec![]);
            assert!(screen.contains_rect(close));
            for category in Category::ALL {
                app.brush_details.category = category;
                for operation in [
                    BrushOperation::Paint,
                    BrushOperation::Erase,
                    BrushOperation::Blend,
                    BrushOperation::Blur,
                    BrushOperation::Smudge,
                ] {
                    app.brush.operation = operation;
                    for help in std::iter::once(None).chain(Property::ALL.map(Some)) {
                        app.brush_details.help = help;
                        for _ in 0..2 {
                            let actual = frame(&mut app, vec![]);
                            assert!(screen.contains_rect(actual));
                            assert!(
                                (actual.center() - close.center()).length() < 0.5,
                                "{size:?} {} {operation:?} {}: {close:?} -> {actual:?}",
                                category.label(),
                                help.map_or("Parameter guide", Property::label),
                            );
                        }
                    }
                }
            }
            let event = |pressed| egui::Event::PointerButton {
                pos: close.center(),
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Default::default(),
            };
            frame(
                &mut app,
                vec![egui::Event::PointerMoved(close.center()), event(true)],
            );
            frame(&mut app, vec![event(false)]);
            assert!(!app.brush_details.open);
            assert_eq!(app.doc, artwork);
        }
    }

    #[test]
    fn opacity_mode_clicks_and_blending_retains_disabled_values_without_artwork_edits() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open(dir.path().to_owned()).unwrap();
        let original = app.doc.clone();
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
                |ui| app.opacity_mode_ui(ui),
            );
            output.textures_delta.clear();
        };
        let click = |app: &mut Parfait, id| {
            let rect = ctx.data(|data| data.get_temp::<egui::Rect>(id)).unwrap();
            let event = |pressed| egui::Event::PointerButton {
                pos: rect.center(),
                pressed,
                button: egui::PointerButton::Primary,
                modifiers: Default::default(),
            };
            frame(
                app,
                vec![egui::Event::PointerMoved(rect.center()), event(true)],
            );
            frame(app, vec![event(false)]);
        };
        frame(&mut app, vec![]);
        frame(&mut app, vec![]);
        click(&mut app, egui::Id::new("opacity_mode_combo"));
        frame(&mut app, vec![]);
        click(
            &mut app,
            egui::Id::new(("opacity_mode", OpacityMode::PerStroke as u8)),
        );
        assert_eq!(app.brush.opacity_mode, OpacityMode::PerStroke);
        app.brush.operation = BrushOperation::Blend;
        click(&mut app, egui::Id::new("opacity_mode_combo"));
        assert_eq!(app.brush.opacity_mode, OpacityMode::PerStroke);
        assert!(!ctx.any_popup_open());
        assert_eq!(app.doc, original);
        assert_eq!(Property::Flow.label(), "Brush density");
        assert!(Category::Tip.properties().contains(&Property::Flow));
        assert!(!Category::Ink.properties().contains(&Property::Flow));
    }

    #[test]
    fn spacing_dynamics_button_retains_automatic_values_and_footer_is_visible() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open(dir.path().to_owned()).unwrap();
        app.brush.pressure_spacing = true;
        app.brush.spacing_curve = [1., 0.8, 0.5, 0.2, 0.].into();
        let original = app.brush.clone();
        let artwork = app.doc.clone();
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(960., 640.));
        let mut clicked = false;
        let mut frame = |app: &mut Parfait, events| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(screen),
                    events,
                    ..Default::default()
                },
                |ui| {
                    clicked = property_control(
                        ui,
                        &mut app.brush,
                        Property::Spacing,
                        None,
                        &mut app.brush_details.help,
                    );
                },
            );
            output.textures_delta.clear();
            clicked
        };
        frame(&mut app, vec![]);
        let rect = ctx
            .data(|data| {
                data.get_temp::<egui::Rect>(egui::Id::new(("parameter_dynamics", "Spacing")))
            })
            .unwrap();
        let button = |pressed| egui::Event::PointerButton {
            pos: rect.center(),
            pressed,
            button: egui::PointerButton::Primary,
            modifiers: Default::default(),
        };
        frame(
            &mut app,
            vec![egui::Event::PointerMoved(rect.center()), button(true)],
        );
        assert!(frame(&mut app, vec![button(false)]));
        app.brush.gap_mode = parfait_core::GapMode::Narrow;
        frame(
            &mut app,
            vec![egui::Event::PointerMoved(rect.center()), button(true)],
        );
        assert!(!frame(&mut app, vec![button(false)]));
        app.brush.gap_mode = parfait_core::GapMode::Fixed;
        assert!(app.brush == original);
        app.brush_details.dynamics = Some(Property::Spacing);
        for _ in 0..3 {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(screen),
                    ..Default::default()
                },
                |ui| app.brush_details_ui(ui.ctx()),
            );
            output.textures_delta.clear();
        }
        let (plot, close) = app.brush_details.dynamics_targets.unwrap();
        assert!(screen.contains_rect(plot));
        assert!(screen.contains_rect(close));
        assert!(app.brush == original);
        assert_eq!(app.doc, artwork);
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(screen),
                events: vec![egui::Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Default::default(),
                }],
                ..Default::default()
            },
            |ui| app.brush_details_ui(ui.ctx()),
        );
        output.textures_delta.clear();
        assert!(app.brush_details.dynamics.is_none());
    }

    #[test]
    fn ink_operation_controls_and_pins_survive_preset_save_and_restart() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open(dir.path().to_owned()).unwrap();
        let artwork = app.doc.clone();
        app.brush_details.open = true;
        app.brush_details.category = Category::Ink;
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
        let click = |app: &mut Parfait, rect: egui::Rect| {
            assert!(screen.contains_rect(rect));
            let event = |pressed| egui::Event::PointerButton {
                pos: rect.center(),
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Default::default(),
            };
            frame(
                app,
                vec![egui::Event::PointerMoved(rect.center()), event(true)],
            );
            frame(app, vec![event(false)]);
            frame(app, vec![]);
        };
        frame(&mut app, vec![]);
        frame(&mut app, vec![]);
        for (label, operation, property) in [
            ("Color blend", BrushOperation::Blend, Property::Stretch),
            ("Smudge", BrushOperation::Smudge, Property::Stretch),
            ("Local blur", BrushOperation::Blur, Property::BlurRadius),
        ] {
            let combo = ctx
                .data(|data| data.get_temp::<egui::Rect>(egui::Id::new("operation_combo")))
                .unwrap();
            click(&mut app, combo);
            let target = ctx
                .data(|data| {
                    data.get_temp::<egui::Rect>(egui::Id::new(("operation_choice", label)))
                })
                .unwrap();
            click(&mut app, target);
            assert_eq!(app.brush.operation(), operation);
            let eye = ctx
                .data(|data| {
                    data.get_temp::<egui::Rect>(egui::Id::new((
                        "brush_property_pin",
                        property as usize,
                    )))
                })
                .unwrap();
            let was_visible = app.brush_details.properties.visibility(property);
            click(&mut app, eye);
            assert_ne!(
                app.brush_details.properties.visibility(property),
                was_visible
            );
            if was_visible {
                click(&mut app, eye);
            }
            assert!(app.brush_details.properties.visibility(property));
            if operation == BrushOperation::Blur {
                assert!(!property.enabled(&app.brush));
                app.brush.blur_radius = 17.;
                let automatic = ctx
                    .data(|data| data.get_temp::<egui::Rect>(egui::Id::new("automatic_blur_size")))
                    .unwrap();
                click(&mut app, automatic);
                assert!(!app.brush.blur_automatic);
                assert!(property.enabled(&app.brush));
                assert_eq!(app.brush.blur_radius, 17.);
            }
        }
        app.save_current_preset();
        app.brush_details.properties = Default::default();
        app.brush.blur_radius = 1.;
        app.brush_details.help = Some(Property::Stretch);
        app.restore_current_preset();
        assert!(app.brush_details.help.is_none());
        assert_eq!(app.brush.blur_radius, 17.);
        assert!(app.brush_details.properties.visibility(Property::Stretch));
        assert!(
            app.brush_details
                .properties
                .visibility(Property::BlurRadius)
        );
        assert_eq!(app.doc, artwork);
        app.persist_preferences(true);
        drop(app);
        let restored = Parfait::open(dir.path().to_owned()).unwrap();
        assert_eq!(restored.brush.operation(), BrushOperation::Blur);
        assert_eq!(restored.brush.blur_radius, 17.);
        assert!(
            restored
                .brush_details
                .properties
                .visibility(Property::BlurRadius)
        );
    }

    #[test]
    fn property_actions_are_below_settings_and_detail_footer_stays_on_one_row() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open(dir.path().to_owned()).unwrap();
        let artwork = app.doc.clone();
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(960., 640.),
                )),
                ..Default::default()
            },
            |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    ui.scope_builder(
                        egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(
                            ui.cursor().min,
                            egui::vec2(210., 600.),
                        )),
                        |ui| {
                            ui.set_width(210.);
                            app.brush_properties_ui(ui);
                        },
                    );
                });
            },
        );
        output.textures_delta.clear();
        let actions: Vec<egui::Rect> = [
            "Sub tool detail",
            "Restore saved brush and visible properties",
            "Save brush settings and visible properties",
        ]
        .iter()
        .map(|label| ctx.data(|data| data.get_temp(egui::Id::new(("flat_icon", *label))).unwrap()))
        .collect();
        assert!(
            actions
                .iter()
                .all(|rect| rect.top() > 220. && rect.height() == 24.)
        );
        assert!(
            actions
                .windows(2)
                .all(|pair| pair[1].right() <= pair[0].left())
        );
        app.brush_details.open = true;
        for _ in 0..2 {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(960., 640.),
                    )),
                    ..Default::default()
                },
                |ui| app.brush_details_ui(ui.ctx()),
            );
            output.textures_delta.clear();
        }
        let buttons: Vec<egui::Rect> = ["Close", "Save preset", "Restore saved"]
            .iter()
            .map(|label| {
                ctx.data(|data| {
                    data.get_temp(egui::Id::new(("action_button", *label)))
                        .unwrap()
                })
            })
            .collect();
        assert!(
            buttons
                .iter()
                .all(|rect| ctx.content_rect().contains_rect(*rect)
                    && rect.height() == 24.
                    && rect.center().y == buttons[0].center().y)
        );
        assert!(
            buttons
                .windows(2)
                .all(|pair| pair[1].right() <= pair[0].left())
        );
        assert_eq!(app.doc, artwork);
    }

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
