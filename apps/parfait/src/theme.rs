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
    style.visuals.window_corner_radius = egui::CornerRadius::ZERO;
    style.visuals.menu_corner_radius = egui::CornerRadius::ZERO;
    style.visuals.window_shadow = egui::epaint::Shadow {
        offset: [0, 4],
        blur: 12,
        spread: 0,
        color: Color32::from_black_alpha(72),
    };
    style.visuals.popup_shadow = egui::epaint::Shadow {
        offset: [0, 2],
        blur: 6,
        spread: 0,
        color: Color32::from_black_alpha(64),
    };
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
    style.visuals.widgets.inactive.bg_stroke =
        egui::Stroke::new(1., Color32::from_gray(133));
    style.visuals.widgets.hovered.bg_stroke =
        egui::Stroke::new(1., Color32::from_gray(183));
    style.visuals.widgets.active.bg_stroke = egui::Stroke::new(1., ACCENT);
    style.visuals.widgets.open.bg_stroke = egui::Stroke::new(1., ACCENT);
    style.visuals.widgets.active.bg_fill = Color32::from_gray(58);
    style.visuals.widgets.active.weak_bg_fill = Color32::from_gray(58);
    style.visuals.widgets.open.bg_fill = Color32::from_gray(74);
    style.visuals.widgets.open.weak_bg_fill = Color32::from_gray(74);
    style.visuals.widgets.inactive.bg_fill = Color32::from_gray(87);
    style.visuals.widgets.inactive.weak_bg_fill = Color32::from_gray(87);
    style.visuals.widgets.hovered.bg_fill = Color32::from_gray(88);
    style.visuals.widgets.hovered.weak_bg_fill = Color32::from_gray(82);
    style.visuals.widgets.noninteractive.bg_stroke =
        egui::Stroke::new(1., Color32::from_gray(46));
    style.spacing.item_spacing = egui::vec2(4., 4.);
    style.spacing.button_padding = egui::vec2(5., 2.);
    style.spacing.interact_size = egui::vec2(20., 20.);
    style.spacing.scroll = egui::style::ScrollStyle::solid();
    style.spacing.slider_width = 100.;
    for text in [
        egui::TextStyle::Body,
        egui::TextStyle::Button,
        egui::TextStyle::Monospace,
    ] {
        style.text_styles.insert(text, egui::FontId::proportional(12.));
    }
    ctx.set_style_of(egui::Theme::Dark, style);
    ctx.set_theme(egui::ThemePreference::Dark);
    ctx.set_fonts(font_definitions());
}

pub(crate) fn font_definitions() -> egui::FontDefinitions {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "Pretendard".into(),
        egui::FontData::from_static(include_bytes!(
            "../assets/pretendard/Pretendard-Regular.otf"
        ))
        .into(),
    );
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace]
    {
        fonts
            .families
            .entry(family)
            .or_default()
            .insert(0, "Pretendard".into());
    }
    fonts.font_data.insert(
        "fontawesome-solid".into(),
        egui::FontData::from_static(include_bytes!(
            "../assets/fontawesome/fa-solid-900.ttf"
        ))
        .into(),
    );
    fonts.families.insert(
        egui::FontFamily::Name("icons".into()),
        vec!["fontawesome-solid".into()],
    );
    fonts
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pretendard_is_shared_by_all_text_families_and_icons_remain_separate() {
        let definitions = font_definitions();
        for family in
            [egui::FontFamily::Proportional, egui::FontFamily::Monospace]
        {
            assert_eq!(definitions.families[&family][0], "Pretendard");
        }
        assert_eq!(
            definitions.families[&egui::FontFamily::Name("icons".into())],
            vec!["fontawesome-solid"]
        );
        assert!(!definitions.font_data.contains_key("system-cjk"));
        let ctx = egui::Context::default();
        apply(&ctx);
        let mut output = ctx.run_ui(Default::default(), |ui| {
            ui.label("가나다 브러시 설정 100% 200px");
            ui.ctx().fonts_mut(|fonts| {
                for family in [
                    egui::FontFamily::Proportional,
                    egui::FontFamily::Monospace,
                ] {
                    assert!(fonts.has_glyphs(
                        &egui::FontId::new(12., family),
                        "가나다힣ABC0123%°×"
                    ));
                }
                let icon_font = egui::FontId::new(
                    14.,
                    egui::FontFamily::Name("icons".into()),
                );
                let missing = fonts.layout_no_wrap(
                    "\u{10ffff}".into(),
                    icon_font.clone(),
                    Color32::WHITE,
                );
                for kind in
                    [crate::widgets::Icon::Pen, crate::widgets::Icon::Grip]
                {
                    let icon = fonts.layout_no_wrap(
                        kind.glyph().into(),
                        icon_font.clone(),
                        Color32::WHITE,
                    );
                    assert!(!icon.rows[0].visuals.mesh.vertices.is_empty());
                    assert_ne!(
                        icon.rows[0].glyphs[0].uv_rect,
                        missing.rows[0].glyphs[0].uv_rect
                    );
                }
            });
        });
        assert!(!output.shapes.is_empty());
        output.textures_delta.clear();
    }

    #[test]
    fn control_plus_does_not_change_ui_scale() {
        let ctx = egui::Context::default();
        apply(&ctx);
        let modifiers =
            egui::Modifiers { ctrl: true, command: true, ..Default::default() };
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
                egui::RawInput { events, ..Default::default() },
                |ui| {
                    ui.label("Scale test");
                },
            );
            out.textures_delta.clear();
        }
        assert_eq!(ctx.zoom_factor(), 1.);
    }
}
