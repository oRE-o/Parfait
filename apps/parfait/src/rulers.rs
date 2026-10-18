use eframe::egui::{self, Color32, Rect, Stroke, Vec2};

#[derive(Clone, Copy)]
struct Axis {
    screen_start: f32,
    value_start: f64,
    units_per_point: f64,
    minor_step: i64,
    label_step: i64,
    first: i64,
    last: i64,
}

struct Tick {
    value: i64,
    position: f32,
    label: bool,
}

fn nice_step(minimum: f64, base: i64) -> i64 {
    let mut scale = base;
    loop {
        for factor in [1, 2, 5] {
            let candidate = scale * factor;
            if candidate as f64 >= minimum {
                return candidate;
            }
        }
        scale *= 10;
    }
}

impl Axis {
    fn new(
        screen_start: f32,
        screen_end: f32,
        value_start: f32,
        units_per_point: f32,
    ) -> Option<Self> {
        if ![screen_start, screen_end, value_start, units_per_point]
            .into_iter()
            .all(f32::is_finite)
            || screen_end <= screen_start
            || !(0.000001..=1_000_000.).contains(&units_per_point.abs())
            || value_start.abs() > 1_000_000_000.
        {
            return None;
        }
        let units_per_point = f64::from(units_per_point);
        let value_start = f64::from(value_start);
        let value_end = value_start + f64::from(screen_end - screen_start) * units_per_point;
        if value_end.abs() > 1_000_000_000_000. {
            return None;
        }
        let minor_step = nice_step(4. * units_per_point.abs(), 10);
        let label_step = nice_step(64. * units_per_point.abs(), 100);
        Some(Self {
            screen_start,
            value_start,
            units_per_point,
            minor_step,
            label_step,
            first: (value_start.min(value_end) / minor_step as f64).ceil() as i64,
            last: (value_start.max(value_end) / minor_step as f64).floor() as i64,
        })
    }

    fn ticks(self) -> impl Iterator<Item = Tick> {
        (self.first..=self.last).take(4096).map(move |index| {
            let value = index * self.minor_step;
            Tick {
                value,
                position: self.screen_start
                    + ((value as f64 - self.value_start) / self.units_per_point) as f32,
                label: value % self.label_step == 0,
            }
        })
    }
}

pub fn paint(
    ui: &egui::Ui,
    outer: Rect,
    viewport: Rect,
    document_start: Vec2,
    units_per_point: f32,
) {
    let horizontal = Rect::from_min_max(
        egui::pos2(viewport.left(), outer.top()),
        viewport.right_top(),
    );
    let vertical = Rect::from_min_max(
        egui::pos2(outer.left(), viewport.top()),
        viewport.left_bottom(),
    );
    let corner = Rect::from_min_max(outer.min, viewport.min);
    let ink = Color32::from_gray(181);
    let font = egui::FontId::monospace(9.);
    let axes = [
        (
            horizontal,
            viewport.left(),
            viewport.right(),
            document_start.x,
        ),
        (
            vertical,
            viewport.top(),
            viewport.bottom(),
            document_start.y,
        ),
    ];
    for (index, (clip, start, end, origin)) in axes.into_iter().enumerate() {
        let painter = ui.painter().with_clip_rect(clip);
        painter.rect_filled(clip, 0., Color32::from_gray(66));
        for tick in Axis::new(start, end, origin, units_per_point)
            .into_iter()
            .flat_map(Axis::ticks)
        {
            let length = if tick.value % 100 == 0 {
                8.
            } else if tick.value % 50 == 0 {
                5.
            } else {
                3.
            };
            if index == 0 {
                let position = egui::pos2(tick.position, viewport.top());
                painter.line_segment(
                    [position, position - egui::vec2(0., length)],
                    Stroke::new(1., ink),
                );
                if tick.label {
                    let galley = painter.layout_no_wrap(tick.value.to_string(), font.clone(), ink);
                    let label = egui::epaint::TextShape::new(
                        egui::pos2(tick.position + 3., clip.top() + 1.),
                        galley,
                        ink,
                    );
                    if clip.contains_rect(label.visual_bounding_rect()) {
                        painter.add(label);
                    }
                }
            } else {
                let position = egui::pos2(viewport.left(), tick.position);
                painter.line_segment(
                    [position, position - egui::vec2(length, 0.)],
                    Stroke::new(1., ink),
                );
                if tick.label {
                    let galley = painter.layout_no_wrap(tick.value.to_string(), font.clone(), ink);
                    let label = egui::epaint::TextShape::new(
                        egui::pos2(clip.left() + 1., tick.position - 3.),
                        galley,
                        ink,
                    )
                    .with_angle(-std::f32::consts::FRAC_PI_2);
                    if clip.contains_rect(label.visual_bounding_rect()) {
                        painter.add(label);
                    }
                }
            }
        }
    }
    let painter = ui.painter().with_clip_rect(corner);
    painter.rect_filled(corner, 0., Color32::from_gray(56));
    painter.text(
        corner.center(),
        egui::Align2::CENTER_CENTER,
        "px",
        font,
        ink,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::Pos2;

    #[test]
    fn document_grid_keeps_values_and_moves_with_fractional_pan() {
        for zoom in [0.05, 0.367, 0.779, 1., 2., 12.] {
            let before = Axis::new(40., 1400., -300., 1. / zoom).unwrap();
            for delta in [-157.25, -0.5, 0.25, 93.75] {
                let after = Axis::new(40., 1400., -300. - delta / zoom, 1. / zoom).unwrap();
                assert_eq!(before.minor_step, after.minor_step);
                assert_eq!(before.label_step, after.label_step);
                assert_eq!(before.label_step % 100, 0);
                for tick in before.ticks() {
                    assert_eq!(tick.value % 10, 0);
                    if let Some(moved) = after.ticks().find(|moved| moved.value == tick.value) {
                        assert!((moved.position - tick.position - delta).abs() < 0.002);
                        assert_eq!(tick.label, moved.label);
                    }
                }
            }
        }
    }

    #[test]
    fn negative_reversed_degenerate_and_extreme_axes_are_bounded() {
        for slope in [-20., -1., 0.00001, 0.0833333, 1., 20.] {
            let axis = Axis::new(18., 1000., -254., slope).unwrap();
            let ticks: Vec<_> = axis.ticks().collect();
            assert!(ticks.len() <= 247);
            for tick in ticks {
                assert!(tick.position.is_finite());
                assert!((18. - 0.001..=1000. + 0.001).contains(&tick.position));
                assert!(((-254. + (tick.position - 18.) * slope) - tick.value as f32).abs() < 0.01);
            }
        }
        for (origin, slope) in [(0., 0.), (0., f32::NAN), (f32::INFINITY, 1.), (0., 1e20)] {
            assert!(Axis::new(0., 1000., origin, slope).is_none());
        }
        assert!(Axis::new(0., 0., 0., 1.).is_none());
        assert!(Axis::new(0., f32::INFINITY, 0., 1.).is_none());
        assert!(Axis::new(0., f32::MAX, 0., -20.).is_none());
        assert!(Axis::new(0., 1e9, 0., 1.).unwrap().ticks().count() <= 4096);
    }

    #[test]
    fn painted_labels_follow_document_coordinates_and_are_clipped_to_each_ruler() {
        let ctx = egui::Context::default();
        let outer = Rect::from_min_size(Pos2::ZERO, egui::vec2(600., 400.));
        let viewport = Rect::from_min_max(egui::pos2(18., 18.), outer.max);
        for pan in [Vec2::ZERO, egui::vec2(37.25, -42.5)] {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(outer),
                    ..Default::default()
                },
                |ui| paint(ui, outer, viewport, -pan, 1.),
            );
            output.textures_delta.clear();
            let labels: Vec<_> = output
                .shapes
                .iter()
                .filter_map(|shape| {
                    if let egui::Shape::Text(text) = &shape.shape {
                        Some((shape.clip_rect, text))
                    } else {
                        None
                    }
                })
                .collect();
            for (clip, label) in &labels {
                assert!(clip.contains_rect(label.visual_bounding_rect()));
            }
            let horizontal = labels
                .iter()
                .find(|(_, text)| text.galley.job.text == "100" && text.angle == 0.)
                .unwrap();
            assert!((horizontal.1.pos.x - (viewport.left() + 100. + pan.x + 3.)).abs() < 0.001);
            assert_eq!(horizontal.0.bottom(), viewport.top());
            let vertical = labels
                .iter()
                .find(|(_, text)| text.galley.job.text == "100" && text.angle < 0.)
                .unwrap();
            assert!((vertical.1.pos.y - (viewport.top() + 100. + pan.y - 3.)).abs() < 0.001);
            assert_eq!(vertical.0.right(), viewport.left());
            assert!(labels.iter().any(|(_, text)| text.galley.job.text == "px"));
        }
    }
}
