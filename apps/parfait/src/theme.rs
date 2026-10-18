use eframe::egui::{self, Color32};
pub const ACCENT: Color32 = Color32::from_rgb(179, 194, 214);
pub fn apply(ctx: &egui::Context) {
    ctx.options_mut(|o| o.zoom_with_keyboard = false);
    let mut style = (*ctx.style_of(egui::Theme::Dark)).clone();
    style.visuals = egui::Visuals::dark();
    style.visuals.panel_fill = Color32::from_gray(61);
    style.visuals.window_fill = Color32::from_gray(65);
    style.visuals.extreme_bg_color = Color32::from_gray(43);
    style.visuals.faint_bg_color = Color32::from_gray(56);
    style.visuals.override_text_color = Some(Color32::from_gray(209));
    style.visuals.selection.bg_fill = Color32::from_rgb(91, 105, 126);
    style.visuals.selection.stroke.color = ACCENT;
    for widget in [
        &mut style.visuals.widgets.noninteractive,
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.active,
        &mut style.visuals.widgets.open,
    ] {
        widget.corner_radius = egui::CornerRadius::ZERO;
    }
    style.visuals.button_frame = true;
    style.visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1., Color32::from_gray(133));
    style.visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1., Color32::from_gray(183));
    style.visuals.widgets.active.bg_stroke = egui::Stroke::new(1., ACCENT);
    style.visuals.widgets.open.bg_stroke = egui::Stroke::new(1., ACCENT);
    style.visuals.widgets.active.bg_fill = Color32::from_gray(58);
    style.visuals.widgets.active.weak_bg_fill = Color32::from_gray(58);
    style.visuals.widgets.open.bg_fill = Color32::from_rgb(91, 105, 126);
    style.visuals.widgets.open.weak_bg_fill = Color32::from_rgb(91, 105, 126);
    style.visuals.widgets.inactive.bg_fill = Color32::from_gray(87);
    style.visuals.widgets.inactive.weak_bg_fill = Color32::from_gray(87);
    style.visuals.widgets.hovered.bg_fill = Color32::from_gray(88);
    style.visuals.widgets.hovered.weak_bg_fill = Color32::from_gray(82);
    style.visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1., Color32::from_gray(46));
    style.spacing.item_spacing = egui::vec2(4., 4.);
    style.spacing.button_padding = egui::vec2(5., 2.);
    style.spacing.interact_size = egui::vec2(20., 20.);
    style.spacing.slider_width = 100.;
    for text in [
        egui::TextStyle::Body,
        egui::TextStyle::Button,
        egui::TextStyle::Monospace,
    ] {
        style
            .text_styles
            .insert(text, egui::FontId::proportional(12.));
    }
    ctx.set_style_of(egui::Theme::Dark, style);
    ctx.set_theme(egui::ThemePreference::Dark);
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "fontawesome-solid".into(),
        egui::FontData::from_static(include_bytes!("../assets/fontawesome/fa-solid-900.ttf"))
            .into(),
    );
    fonts.families.insert(
        egui::FontFamily::Name("icons".into()),
        vec!["fontawesome-solid".into()],
    );
    // Use a system CJK font if available; no system font binaries are distributed.
    for path in [
        "C:/Windows/Fonts/malgun.ttf",
        "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
        "/System/Library/Fonts/AppleSDGothicNeo.ttc",
    ] {
        if let Ok(bytes) = std::fs::read(path) {
            fonts.font_data.insert(
                "system-cjk".into(),
                egui::FontData::from_owned(bytes).into(),
            );
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .push("system-cjk".into());
            break;
        }
    }
    ctx.set_fonts(fonts);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn control_plus_does_not_change_ui_scale() {
        let ctx = egui::Context::default();
        apply(&ctx);
        let modifiers = egui::Modifiers {
            ctrl: true,
            command: true,
            ..Default::default()
        };
        for events in [
            vec![egui::Event::ModifiersChanged(modifiers)],
            vec![egui::Event::Key {
                key: egui::Key::Plus,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers,
            }],
            vec![],
        ] {
            let mut out = ctx.run_ui(
                egui::RawInput {
                    events,
                    ..Default::default()
                },
                |ui| {
                    ui.label("Scale test");
                },
            );
            out.textures_delta.clear();
        }
        assert_eq!(ctx.zoom_factor(), 1.);
    }
}
