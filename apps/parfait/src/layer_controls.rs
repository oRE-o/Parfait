use eframe::egui::{self, Color32, Rect, Response, Sense, Stroke};
use parfait_core::BlendMode;
pub(crate) const SUPPRESSED_BLEND: Color32 = Color32::from_rgb(232, 102, 102);

pub(crate) fn blend_label(mode: BlendMode) -> &'static str {
    match mode {
        BlendMode::Normal => "표준",
        BlendMode::Darken => "비교(어두움)",
        BlendMode::Multiply => "곱하기",
        BlendMode::ColorBurn => "색상 번",
        BlendMode::Subtract => "감산",
        BlendMode::Lighten => "비교(밝음)",
        BlendMode::Screen => "스크린",
        BlendMode::ColorDodge => "색상 닷지",
        BlendMode::Add => "더하기",
        BlendMode::Overlay => "오버레이",
        BlendMode::SoftLight => "소프트 라이트",
        BlendMode::HardLight => "하드 라이트",
        BlendMode::Difference => "차이",
        BlendMode::Exclusion => "제외",
        BlendMode::Through => "통과",
    }
}

pub(crate) fn blend_select(
    ui: &mut egui::Ui,
    mode: &mut BlendMode,
    folder: bool,
    clipped: bool,
    width: f32,
) -> Response {
    egui::ComboBox::from_id_salt("blend_mode")
        .width(width)
        .height(320.)
        .truncate()
        .selected_text(blend_text(*mode, clipped))
        .show_ui(ui, |ui| {
            if folder {
                let response = ui.selectable_value(mode, BlendMode::Through, "통과");
                #[cfg(test)]
                ui.data_mut(|data| {
                    data.insert_temp(egui::Id::new("blend_choice_through"), response.rect)
                });
                #[cfg(not(test))]
                let _ = response;
            }
            for candidate in [
                BlendMode::Normal,
                BlendMode::Darken,
                BlendMode::Multiply,
                BlendMode::ColorBurn,
                BlendMode::Subtract,
                BlendMode::Lighten,
                BlendMode::Screen,
                BlendMode::ColorDodge,
                BlendMode::Add,
                BlendMode::Overlay,
                BlendMode::SoftLight,
                BlendMode::HardLight,
                BlendMode::Difference,
                BlendMode::Exclusion,
            ] {
                ui.selectable_value(mode, candidate, blend_label(candidate));
            }
        })
        .response
        .on_hover_text(if *mode == BlendMode::Through && clipped {
            "클리핑 중에는 통과 효과가 억제됩니다. 클리핑을 해제하면 다시 적용됩니다."
        } else {
            "합성 모드"
        })
}

pub(crate) fn blend_text(mode: BlendMode, clipped: bool) -> egui::RichText {
    let text = egui::RichText::new(blend_label(mode));
    if mode == BlendMode::Through && clipped {
        text.color(SUPPRESSED_BLEND)
    } else {
        text
    }
}

pub(crate) fn opacity(ui: &mut egui::Ui, value: &mut f32) -> Response {
    let original = *value;
    let mut percent = original * 100.;
    let response = ui
        .horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 3.;
            ui.spacing_mut().slider_width =
                (ui.available_width() - 52.).max(24.);
            let background = ui.painter().add(egui::Shape::Noop);
            let slider = ui
                .scope(|ui| {
                    ui.spacing_mut().slider_rail_height = 10.;
                    ui.visuals_mut().widgets.inactive.bg_fill =
                        Color32::TRANSPARENT;
                    ui.add(
                        egui::Slider::new(&mut percent, 0.0..=100.)
                            .show_value(false)
                            .clamping(egui::SliderClamping::Edits)
                            .step_by(1.)
                            .trailing_fill(false)
                            .handle_shape(egui::style::HandleShape::Rect {
                                aspect_ratio: 0.18,
                            }),
                    )
                })
                .inner;
            slider.widget_info(|| {
                egui::WidgetInfo::slider(
                    ui.is_enabled(),
                    f64::from(percent),
                    "불투명도 (%)",
                )
            });
            let track = Rect::from_center_size(
                slider.rect.center(),
                egui::vec2(slider.rect.width(), 10.),
            );
            let mut mesh = egui::Mesh::default();
            for column in 0..(track.width() / 5.).ceil() as usize {
                for row in 0..2 {
                    let cell = Rect::from_min_size(
                        track.min
                            + egui::vec2(column as f32 * 5., row as f32 * 5.),
                        egui::vec2(5., 5.),
                    )
                    .intersect(track);
                    mesh.add_colored_rect(
                        cell,
                        Color32::from_gray(if (column + row) % 2 == 0 {
                            210
                        } else {
                            162
                        }),
                    );
                }
            }
            mesh.add_colored_rect(
                Rect::from_min_max(
                    egui::pos2(
                        track.left() + track.width() * percent / 100.,
                        track.top(),
                    ),
                    track.max,
                ),
                Color32::from_gray(78),
            );
            ui.painter().set(
                background,
                egui::Shape::Vec(vec![
                    egui::Shape::mesh(mesh),
                    egui::Shape::rect_stroke(
                        track,
                        0.,
                        Stroke::new(0.7, Color32::from_gray(140)),
                        egui::StrokeKind::Inside,
                    ),
                ]),
            );
            let number = ui
                .scope(|ui| {
                    ui.visuals_mut().button_frame = false;
                    ui.add_sized(
                        [33., 20.],
                        egui::DragValue::new(&mut percent)
                            .speed(1.)
                            .range(0.0..=100.)
                            .fixed_decimals(0)
                            .custom_parser(|text| {
                                text.trim()
                                    .trim_end_matches('%')
                                    .trim()
                                    .parse()
                                    .ok()
                            }),
                    )
                })
                .inner
                .on_hover_text("불투명도 (%) · 클릭하여 숫자 입력");
            let (arrows, _) =
                ui.allocate_exact_size(egui::vec2(10., 20.), Sense::hover());
            for (up, rect, delta, label) in [
                (
                    true,
                    Rect::from_min_max(arrows.min, arrows.right_center()),
                    1.,
                    "불투명도 높이기",
                ),
                (
                    false,
                    Rect::from_min_max(arrows.left_center(), arrows.max),
                    -1.,
                    "불투명도 낮추기",
                ),
            ] {
                let button =
                    ui.interact(rect, ui.id().with(up), Sense::click());
                button.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::WidgetType::Button,
                        ui.is_enabled(),
                        label,
                    )
                });
                let visuals = ui.style().interact(&button);
                let direction = if up { -1. } else { 1. };
                ui.painter().add(egui::Shape::line(
                    vec![
                        rect.center() + egui::vec2(-3., -direction),
                        rect.center() + egui::vec2(0., direction),
                        rect.center() + egui::vec2(3., -direction),
                    ],
                    Stroke::new(1., visuals.text_color()),
                ));
                if button.clicked() {
                    percent = (percent.round() + delta).clamp(0., 100.);
                }
                #[cfg(test)]
                ui.data_mut(|data| {
                    data.insert_temp(egui::Id::new(("opacity_arrow", up)), rect)
                });
                button.on_hover_text(label);
            }
            #[cfg(test)]
            ui.data_mut(|data| {
                data.insert_temp(
                    egui::Id::new("layer_opacity_number"),
                    number.rect,
                );
                data.insert_temp(
                    egui::Id::new("layer_opacity"),
                    (slider.rect.union(number.rect), slider.rect.width()),
                );
            });
            if number.gained_focus()
                || number.has_focus()
                || number.lost_focus()
            {
                number.union(slider)
            } else {
                slider.union(number)
            }
        })
        .inner;
    let mut response = response.on_hover_text("불투명도 (%)");
    if percent != original * 100. {
        *value = percent / 100.;
        response.mark_changed();
    }
    #[cfg(test)]
    ui.data_mut(|data| {
        data.insert_temp(egui::Id::new("layer_opacity_focus"), response.id)
    });
    response
}
