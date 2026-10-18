use eframe::egui::{self, Color32, Rect, Response, Sense, Stroke};
use std::ops::RangeInclusive;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ParameterLayout {
    Palette,
    Detail,
}

pub struct Parameter<'a> {
    pub label: &'a str,
    pub range: RangeInclusive<f32>,
    pub decimals: usize,
    pub unit: &'a str,
    pub logarithmic: bool,
    pub dynamics: Option<bool>,
    pub layout: ParameterLayout,
}

pub struct ParameterResponse {
    pub response: Response,
    pub dynamics_clicked: bool,
}

struct ParameterGeometry {
    number: Rect,
    track: Rect,
    arrows: Rect,
    dynamics: Rect,
}

impl ParameterLayout {
    fn height(self) -> f32 {
        match self {
            Self::Palette => 42.,
            Self::Detail => 28.,
        }
    }

    fn geometry(self, rect: Rect) -> ParameterGeometry {
        let right = rect.right() - 2.;
        let number_right = right - 36.;
        let (number_width, top, bottom, track_top, dynamics_top, dynamics_height) = match self {
            Self::Palette => (49., 8., 33., 25., 6., 28.),
            Self::Detail => (42., 2., 25., 18., 2., 24.),
        };
        let number = Rect::from_min_max(
            egui::pos2(number_right - number_width, rect.top() + top),
            egui::pos2(number_right, rect.top() + bottom),
        );
        ParameterGeometry {
            number,
            track: Rect::from_min_max(
                rect.min + egui::vec2(24., track_top),
                egui::pos2(
                    (number.left() - 10.).max(rect.left() + 44.),
                    rect.top() + bottom,
                ),
            ),
            arrows: Rect::from_min_max(
                egui::pos2(number_right + 1., rect.top() + top),
                egui::pos2(number_right + 14., rect.top() + bottom),
            ),
            dynamics: Rect::from_min_size(
                egui::pos2(right - 19., rect.top() + dynamics_top),
                egui::vec2(19., dynamics_height),
            ),
        }
    }
}

fn fraction(value: f32, range: &RangeInclusive<f32>, logarithmic: bool) -> f32 {
    let minimum = *range.start();
    let maximum = *range.end();
    if logarithmic {
        (value.clamp(minimum, maximum) / minimum).ln() / (maximum / minimum).ln()
    } else {
        (value.clamp(minimum, maximum) - minimum) / (maximum - minimum)
    }
}

fn from_fraction(fraction: f32, range: &RangeInclusive<f32>, logarithmic: bool) -> f32 {
    let minimum = *range.start();
    let maximum = *range.end();
    let fraction = fraction.clamp(0., 1.);
    if logarithmic {
        minimum * (maximum / minimum).powf(fraction)
    } else {
        minimum + fraction * (maximum - minimum)
    }
}

fn chevron(ui: &mut egui::Ui, rect: Rect, upward: bool, label: &str) -> Response {
    let response = ui.interact(rect, ui.id().with(upward), Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label)
    });
    let visuals = ui.style().interact(&response);
    if response.hovered() || response.has_focus() {
        ui.painter().rect_filled(rect, 1., visuals.bg_fill);
    }
    let center = rect.center();
    let direction = if upward { -1. } else { 1. };
    ui.painter().add(egui::Shape::line(
        vec![
            center + egui::vec2(-3.5, -direction * 1.5),
            center + egui::vec2(0., direction * 1.5),
            center + egui::vec2(3.5, -direction * 1.5),
        ],
        Stroke::new(1., visuals.text_color()),
    ));
    response.on_hover_text(label)
}

pub fn parameter(ui: &mut egui::Ui, value: &mut f32, options: Parameter<'_>) -> ParameterResponse {
    let original = *value;
    let (rect, mut response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), options.layout.height()),
        Sense::hover(),
    );
    let geometry = options.layout.geometry(rect);
    let compact = options.layout == ParameterLayout::Detail;
    let number_rect = geometry.number;
    ui.painter().text(
        rect.min + egui::vec2(6., 2.),
        egui::Align2::LEFT_TOP,
        options.label,
        egui::FontId::proportional(if compact { 11.5 } else { 13. }),
        ui.visuals().strong_text_color(),
    );
    let track = geometry.track;
    let slider = ui.interact(
        track.expand2(egui::vec2(1., 6.)),
        ui.id().with("bar"),
        Sense::drag(),
    );
    if (slider.clicked() || slider.dragged() || slider.drag_stopped())
        && let Some(pointer) = slider.interact_pointer_pos()
    {
        *value = from_fraction(
            (pointer.x - track.left()) / track.width(),
            &options.range,
            options.logarithmic,
        );
        let multiplier = 10_f32.powi(options.decimals as i32);
        *value = (*value * multiplier).round() / multiplier;
        slider.request_focus();
    }
    let step = 10_f32.powi(-(options.decimals as i32));
    if slider.has_focus() {
        ui.input_mut(|input| {
            for (key, delta) in [
                (egui::Key::ArrowRight, step),
                (egui::Key::ArrowUp, step),
                (egui::Key::ArrowLeft, -step),
                (egui::Key::ArrowDown, -step),
            ] {
                if input.consume_key(egui::Modifiers::NONE, key) {
                    *value += delta;
                }
            }
            if input.consume_key(egui::Modifiers::NONE, egui::Key::Home) {
                *value = *options.range.start();
            }
            if input.consume_key(egui::Modifiers::NONE, egui::Key::End) {
                *value = *options.range.end();
            }
        });
    }
    *value = value.clamp(*options.range.start(), *options.range.end());
    slider.widget_info(|| {
        egui::WidgetInfo::slider(ui.is_enabled(), f64::from(*value), options.label)
    });
    let border = if slider.hovered() || slider.has_focus() {
        Color32::from_gray(185)
    } else {
        Color32::from_gray(126)
    };
    let filled = Rect::from_min_max(
        track.min,
        egui::pos2(
            track.left() + track.width() * fraction(*value, &options.range, options.logarithmic),
            track.bottom(),
        ),
    );
    ui.painter().rect_filled(filled, 0., Color32::from_gray(88));
    ui.painter().rect_stroke(
        track,
        0.,
        Stroke::new(0.7, border),
        egui::StrokeKind::Inside,
    );
    slider.on_hover_text(format!(
        "{} · {} · drag or use arrow keys",
        options.label,
        options.unit.trim()
    ));
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(number_rect)
            .layout(egui::Layout::right_to_left(egui::Align::Center)),
        |ui| {
            ui.style_mut().override_font_id =
                Some(egui::FontId::proportional(if compact { 12.5 } else { 16. }));
            ui.visuals_mut().button_frame = false;
            ui.add_sized(
                number_rect.size(),
                egui::DragValue::new(value)
                    .range(options.range.clone())
                    .update_while_editing(false)
                    .speed(step)
                    .fixed_decimals(options.decimals),
            )
            .on_hover_text(format!(
                "{} ({}) · drag to adjust; click to type",
                options.label,
                options.unit.trim()
            ));
        },
    );
    let arrows = geometry.arrows;
    if chevron(
        ui,
        Rect::from_min_max(arrows.min, arrows.right_center()),
        true,
        &format!("Increase {}", options.label),
    )
    .clicked()
    {
        *value += step;
    }
    if chevron(
        ui,
        Rect::from_min_max(arrows.left_center(), arrows.max),
        false,
        &format!("Decrease {}", options.label),
    )
    .clicked()
    {
        *value -= step;
    }
    let mut dynamics_clicked = false;
    if let Some(active) = options.dynamics {
        let button_rect = geometry.dynamics;
        let button = ui.interact(button_rect, ui.id().with("dynamics"), Sense::click());
        button.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::WidgetType::Button,
                ui.is_enabled(),
                format!("{} dynamics", options.label),
            )
        });
        let visuals = ui.style().interact(&button);
        ui.painter().rect(
            button_rect,
            2.,
            if button.hovered() {
                visuals.bg_fill
            } else {
                Color32::from_gray(76)
            },
            Stroke::new(0.7, Color32::from_gray(130)),
            egui::StrokeKind::Inside,
        );
        let center = button_rect.center();
        let ink = Stroke::new(1.2, visuals.text_color());
        if active {
            ui.painter().line_segment(
                [center + egui::vec2(0., -6.), center + egui::vec2(0., 3.)],
                ink,
            );
            ui.painter().add(egui::Shape::line(
                vec![
                    center + egui::vec2(-3., 0.),
                    center + egui::vec2(0., 3.),
                    center + egui::vec2(3., 0.),
                ],
                ink,
            ));
            ui.painter().line_segment(
                [center + egui::vec2(-5., 6.), center + egui::vec2(5., 6.)],
                ink,
            );
        } else {
            ui.painter().rect_stroke(
                Rect::from_center_size(center, egui::vec2(9., 10.)),
                1.,
                ink,
                egui::StrokeKind::Inside,
            );
            ui.painter().line_segment(
                [center + egui::vec2(-4., 5.), center + egui::vec2(4., -5.)],
                ink,
            );
        }
        dynamics_clicked = button.clicked();
        button.on_hover_text(format!(
            "{} dynamics · pen pressure {}",
            options.label,
            if active { "on" } else { "off" }
        ));
    }
    *value = value.clamp(*options.range.start(), *options.range.end());
    if *value != original {
        response.mark_changed();
    }
    ui.painter().line_segment(
        [rect.left_bottom(), rect.right_bottom()],
        Stroke::new(0.7, Color32::from_gray(107)),
    );
    ParameterResponse {
        response,
        dynamics_clicked,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn row_arrows_track_keyboard_and_dynamics_are_independent() {
        for layout in [ParameterLayout::Palette, ParameterLayout::Detail] {
            check_row_interactions(layout);
        }
    }

    fn check_row_interactions(layout: ParameterLayout) {
        let ctx = egui::Context::default();
        let mut value = 12.;
        let mut frame = |events| {
            let mut rect = Rect::NOTHING;
            let mut clicked = false;
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(260., 200.),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| {
                    let row = parameter(
                        ui,
                        &mut value,
                        Parameter {
                            label: "Brush size",
                            range: 1. ..=512.,
                            decimals: 1,
                            unit: "px",
                            logarithmic: true,
                            dynamics: Some(true),
                            layout,
                        },
                    );
                    rect = row.response.rect;
                    clicked = row.dynamics_clicked;
                },
            );
            output.textures_delta.clear();
            (rect, value, clicked)
        };
        let (rect, _, _) = frame(vec![]);
        assert_eq!(rect.height(), layout.height());
        let geometry = layout.geometry(rect);
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            pressed,
            button: egui::PointerButton::Primary,
            modifiers: Default::default(),
        };
        let increase =
            geometry.arrows.center_top() + egui::vec2(0., geometry.arrows.height() * 0.25);
        frame(vec![
            egui::Event::PointerMoved(increase),
            button(increase, true),
        ]);
        let (_, increased, _) = frame(vec![button(increase, false)]);
        assert!((increased - 12.1).abs() < 0.001);
        let dynamics = geometry.dynamics.center();
        frame(vec![
            egui::Event::PointerMoved(dynamics),
            button(dynamics, true),
        ]);
        let (_, unchanged, clicked) = frame(vec![button(dynamics, false)]);
        assert!(clicked);
        assert_eq!(unchanged, increased);
        let track_end = geometry.track.right_center();
        frame(vec![
            egui::Event::PointerMoved(track_end),
            button(track_end, true),
        ]);
        let (_, maximum, _) = frame(vec![button(track_end, false)]);
        assert_eq!(maximum, 512.);
        frame(vec![
            egui::Event::PointerMoved(track_end),
            button(track_end, true),
        ]);
        let middle = geometry.track.center();
        frame(vec![egui::Event::PointerMoved(middle)]);
        let (_, dragged, _) = frame(vec![button(middle, false)]);
        assert!((dragged - 22.6).abs() < 0.001);
        let (_, minimum, _) = frame(vec![egui::Event::Key {
            key: egui::Key::Home,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        }]);
        assert_eq!(minimum, 1.);
        let number = geometry.number.center();
        frame(vec![
            egui::Event::PointerMoved(number),
            button(number, true),
        ]);
        frame(vec![button(number, false)]);
        frame(vec![egui::Event::Key {
            key: egui::Key::A,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers {
                ctrl: true,
                command: true,
                ..Default::default()
            },
        }]);
        let (_, partial, _) = frame(vec![egui::Event::Text("3".into())]);
        assert_eq!(partial, 1.);
        frame(vec![egui::Event::Text("7.0".into())]);
        let (_, committed, _) = frame(vec![egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        }]);
        assert_eq!(committed, 37.);
    }

    #[test]
    fn both_row_densities_keep_targets_separate_and_disabled_values_unchanged() {
        for layout in [ParameterLayout::Palette, ParameterLayout::Detail] {
            let ctx = egui::Context::default();
            let mut value = 100.;
            let mut row_rect = Rect::NOTHING;
            let mut frame = |events| {
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(210., 160.),
                        )),
                        events,
                        ..Default::default()
                    },
                    |ui| {
                        ui.add_enabled_ui(false, |ui| {
                            let row = parameter(
                                ui,
                                &mut value,
                                Parameter {
                                    label: "Opacity",
                                    range: 0. ..=100.,
                                    decimals: 0,
                                    unit: "%",
                                    logarithmic: false,
                                    dynamics: Some(false),
                                    layout,
                                },
                            );
                            row_rect = row.response.rect;
                            assert!(!row.dynamics_clicked);
                            assert!(!row.response.changed());
                        });
                    },
                );
                output.textures_delta.clear();
                (row_rect, value)
            };
            let (rect, _) = frame(vec![]);
            let geometry = layout.geometry(rect);
            for target in [
                geometry.track,
                geometry.number,
                geometry.arrows,
                geometry.dynamics,
            ] {
                assert!(rect.contains_rect(target));
            }
            assert!(geometry.track.right() + 1. < geometry.number.left());
            assert!(geometry.number.right() < geometry.arrows.left());
            assert!(geometry.arrows.right() < geometry.dynamics.left());
            for target in [
                geometry.track,
                geometry.number,
                geometry.arrows,
                geometry.dynamics,
            ] {
                for pressed in [true, false] {
                    let (_, unchanged) = frame(vec![
                        egui::Event::PointerMoved(target.center()),
                        egui::Event::PointerButton {
                            pos: target.center(),
                            pressed,
                            button: egui::PointerButton::Primary,
                            modifiers: Default::default(),
                        },
                    ]);
                    assert_eq!(unchanged, 100.);
                }
            }
        }
    }

    #[test]
    fn tracks_round_trip_and_clamp() {
        for logarithmic in [false, true] {
            for value in [1., 7., 32., 256., 512.] {
                let position = fraction(value, &(1. ..=512.), logarithmic);
                assert!(
                    (from_fraction(position, &(1. ..=512.), logarithmic) - value).abs() < 0.001
                );
            }
            assert_eq!(from_fraction(-1., &(1. ..=512.), logarithmic), 1.);
            assert_eq!(from_fraction(2., &(1. ..=512.), logarithmic), 512.);
        }
    }
}
