use eframe::egui::{self, Color32};
pub const ACCENT: Color32 = Color32::from_rgb(191, 173, 246);
pub fn apply(ctx: &egui::Context) {
    let mut style = (*ctx.style_of(egui::Theme::Dark)).clone();
    style.visuals = egui::Visuals::dark();
    style.visuals.panel_fill = Color32::from_rgb(34, 35, 42);
    style.visuals.window_fill = Color32::from_rgb(38, 39, 47);
    style.visuals.selection.bg_fill = Color32::from_rgb(79, 65, 108);
    style.visuals.selection.stroke.color = ACCENT;
    style.spacing.item_spacing = egui::vec2(8., 8.);
    style.spacing.button_padding = egui::vec2(10., 7.);
    ctx.set_style_of(egui::Theme::Dark, style);
    ctx.set_theme(egui::ThemePreference::Dark);
    // Use a system CJK font if available; no font binaries are distributed.
    for path in [
        "C:/Windows/Fonts/malgun.ttf",
        "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
        "/System/Library/Fonts/AppleSDGothicNeo.ttc",
    ] {
        if let Ok(bytes) = std::fs::read(path) {
            let mut fonts = egui::FontDefinitions::default();
            fonts.font_data.insert(
                "system-cjk".into(),
                egui::FontData::from_owned(bytes).into(),
            );
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .push("system-cjk".into());
            ctx.set_fonts(fonts);
            break;
        }
    }
}
