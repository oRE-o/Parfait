//! Shared normalized response-curve widget. Callers own curve data and persistence.
use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke};

pub struct Options<'a> {
    pub input: &'a str,
    pub output: &'a str,
    pub side: f32,
    pub fixed_x: bool,
    pub fixed_endpoints: bool,
    pub increasing: bool,
    pub max_points: usize,
    pub marker: Option<f32>,
}
impl Default for Options<'_> {
    fn default() -> Self {
        Self {
            input: "Input",
            output: "Output",
            side: 180.,
            fixed_x: false,
            fixed_endpoints: true,
            increasing: true,
            max_points: 16,
            marker: None,
        }
    }
}
fn y_bounds(points: &[[f32; 2]], i: usize, increasing: bool) -> (f32, f32) {
    if increasing {
        (
            if i == 0 { 0. } else { points[i - 1][1] },
            if i + 1 == points.len() {
                1.
            } else {
                points[i + 1][1]
            },
        )
    } else {
        (0., 1.)
    }
}
fn move_node(points: &mut [[f32; 2]], i: usize, value: [f32; 2], options: &Options<'_>) {
    if options.fixed_endpoints && (i == 0 || i + 1 == points.len()) {
        return;
    }
    let (lo, hi) = y_bounds(points, i, options.increasing);
    points[i][1] = value[1].clamp(lo, hi);
    if !options.fixed_x && i > 0 && i + 1 < points.len() {
        let lo = points[i - 1][0] + 0.002;
        let hi = points[i + 1][0] - 0.002;
        if lo <= hi {
            points[i][0] = value[0].clamp(lo, hi);
        }
    }
}
fn sample(points: &[[f32; 2]], x: f32) -> f32 {
    for pair in points.windows(2) {
        if x <= pair[1][0] {
            return pair[0][1]
                + (pair[1][1] - pair[0][1])
                    * ((x - pair[0][0]) / (pair[1][0] - pair[0][0])).clamp(0., 1.);
        }
    }
    points.last().map_or(0., |p| p[1])
}
/// Points must be finite, sorted by x, with distinct x in [0,1] and y in [0,1].
/// Returns the plot rectangle for embedding/tests. Lines reflect actual linear interpolation.
pub fn edit(
    ui: &mut egui::Ui,
    id: impl egui::AsIdSalt,
    points: &mut Vec<[f32; 2]>,
    selected: &mut Option<usize>,
    options: Options<'_>,
) -> Rect {
    let valid = points.len() >= 2
        && points
            .iter()
            .flatten()
            .all(|v| v.is_finite() && (0. ..=1.).contains(v))
        && points
            .windows(2)
            .all(|w| w[1][0] > w[0][0] && (!options.increasing || w[1][1] >= w[0][1]));
    if !valid {
        return ui
            .colored_label(Color32::LIGHT_RED, "Invalid response curve")
            .rect;
    }
    ui.push_id(id, |ui| {
        if selected.is_some_and(|i| i >= points.len()) {
            *selected = None;
        }
        let side = (ui.available_width() - 42.).min(options.side).max(40.);
        let (outer, response) =
            ui.allocate_exact_size(egui::vec2(side + 42., side + 36.), Sense::click_and_drag());
        response.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::WidgetType::Other,
                true,
                format!("{} response curve", options.input),
            )
        });
        let r = Rect::from_min_size(outer.min + egui::vec2(30., 12.), egui::vec2(side, side));
        let pos =
            |v: [f32; 2]| egui::pos2(r.left() + v[0] * r.width(), r.bottom() - v[1] * r.height());
        let value = |p: Pos2| {
            [
                ((p.x - r.left()) / r.width()).clamp(0., 1.),
                ((r.bottom() - p.y) / r.height()).clamp(0., 1.),
            ]
        };
        if ((response.is_pointer_button_down_on() && ui.input(|i| i.pointer.any_pressed()))
            || response.clicked())
            && let Some(p) = response.interact_pointer_pos()
        {
            *selected = points
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| {
                    pos(**a).distance_sq(p).total_cmp(&pos(**b).distance_sq(p))
                })
                .and_then(|(i, v)| (pos(*v).distance(p) < 12.).then_some(i));
            if selected.is_none()
                && response.clicked()
                && r.contains(p)
                && !options.fixed_x
                && points.len() < options.max_points
            {
                let [x, mut y] = value(p);
                let i = points.partition_point(|v| v[0] < x);
                if i > 0
                    && i < points.len()
                    && x - points[i - 1][0] > 0.005
                    && points[i][0] - x > 0.005
                {
                    if options.increasing {
                        y = y.clamp(points[i - 1][1], points[i][1]);
                    }
                    points.insert(i, [x, y]);
                    *selected = Some(i);
                }
            }
            response.request_focus();
        }
        if response.dragged()
            && let (Some(i), Some(p)) = (*selected, response.interact_pointer_pos())
        {
            move_node(points, i, value(p), &options);
        }
        let removable = !options.fixed_x && selected.is_some_and(|i| i > 0 && i + 1 < points.len());
        let delete = response.has_focus()
            && ui
                .input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace));
        let painter = ui.painter();
        painter.rect_filled(r, 0., Color32::from_gray(65));
        for i in 0..=4 {
            let t = i as f32 / 4.;
            let stroke = Stroke::new(1., Color32::from_gray(125));
            painter.line_segment([pos([t, 0.]), pos([t, 1.])], stroke);
            painter.line_segment([pos([0., t]), pos([1., t])], stroke);
        }
        let label = |at, text, align| {
            painter.text(
                at,
                align,
                text,
                egui::FontId::proportional(10.),
                Color32::LIGHT_GRAY,
            );
        };
        label(
            r.left_top() - egui::vec2(3., 0.),
            "100%",
            egui::Align2::RIGHT_TOP,
        );
        label(
            r.left_bottom() - egui::vec2(3., 0.),
            "0%",
            egui::Align2::RIGHT_BOTTOM,
        );
        label(
            r.left_bottom() + egui::vec2(0., 3.),
            "0%",
            egui::Align2::LEFT_TOP,
        );
        label(
            r.right_bottom() + egui::vec2(0., 3.),
            "100%",
            egui::Align2::RIGHT_TOP,
        );
        label(
            r.center_bottom() + egui::vec2(0., 3.),
            options.input,
            egui::Align2::CENTER_TOP,
        );
        label(
            r.left_center() - egui::vec2(3., 0.),
            options.output,
            egui::Align2::RIGHT_CENTER,
        );
        painter.add(egui::Shape::line(
            points.iter().map(|p| pos(*p)).collect(),
            Stroke::new(1.2, Color32::WHITE),
        ));
        for (i, p) in points.iter().enumerate() {
            painter.rect_filled(
                Rect::from_center_size(pos(*p), egui::vec2(5., 5.)),
                0.,
                if *selected == Some(i) {
                    Color32::WHITE
                } else {
                    Color32::from_rgb(159, 150, 230)
                },
            );
        }
        if let Some(raw) = options.marker.filter(|p| p.is_finite()) {
            let x = raw.clamp(0., 1.);
            painter.circle_stroke(
                pos([x, sample(points, x)]),
                3.,
                Stroke::new(1., Color32::LIGHT_GREEN),
            );
        }
        response.on_hover_text(if options.fixed_x {
            "Drag a square vertically to edit its response."
        } else {
            "Click to add a point; drag to move. Delete removes the selected interior point."
        });
        if !options.fixed_x {
            let clicked = ui
                .add_enabled(removable, egui::Button::new("Delete selected point"))
                .clicked();
            if removable
                && (delete || clicked)
                && let Some(i) = selected.take()
            {
                points.remove(i);
            }
        }
        r
    })
    .inner
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn adjacent_editors_keep_selection_and_edits_independent() {
        let ctx = egui::Context::default();
        let mut a = vec![[0., 0.], [0.5, 0.5], [1., 1.]];
        let mut b = vec![[0., 1.], [0.5, 0.5], [1., 0.]];
        let mut sa = None;
        let mut sb = None;
        let mut frame = |events| {
            let mut plot = Rect::NOTHING;
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(700., 400.))),
                    events,
                    ..Default::default()
                },
                |ui| {
                    ui.horizontal(|ui| {
                        plot = edit(ui, "pressure", &mut a, &mut sa, Options::default());
                        edit(
                            ui,
                            "tilt",
                            &mut b,
                            &mut sb,
                            Options {
                                increasing: false,
                                ..Default::default()
                            },
                        );
                    });
                },
            );
            out.textures_delta.clear();
            (plot, a.clone(), b.clone())
        };
        let (r, _, original_b) = frame(vec![]);
        let p = r.center();
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            pressed,
            button: egui::PointerButton::Primary,
            modifiers: Default::default(),
        };
        frame(vec![egui::Event::PointerMoved(p), button(p, true)]);
        let (_, changed_a, changed_b) =
            frame(vec![egui::Event::PointerMoved(p + egui::vec2(20., -30.))]);
        assert!(changed_a[1][1] > 0.5);
        assert_eq!(changed_b, original_b);
        frame(vec![button(p + egui::vec2(20., -30.), false)]);
        let (_, deleted_a, unchanged_b) = frame(vec![egui::Event::Key {
            key: egui::Key::Delete,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        }]);
        assert_eq!(deleted_a.len(), 2);
        assert_eq!(unchanged_b, original_b);
    }
    #[test]
    fn constraints_allow_falling_curves_without_breaking_pressure_rules() {
        let mut falling = vec![[0., 1.], [0.5, 0.5], [1., 0.]];
        move_node(
            &mut falling,
            1,
            [0.8, 0.1],
            &Options {
                increasing: false,
                ..Default::default()
            },
        );
        assert_eq!(falling[1], [0.8, 0.1]);
        assert!((sample(&falling, 0.4) - 0.55).abs() < 0.001);
        let mut pressure = vec![[0., 0.], [0.25, 0.3], [0.5, 0.5], [0.75, 0.7], [1., 1.]];
        move_node(
            &mut pressure,
            2,
            [0.9, 1.],
            &Options {
                fixed_x: true,
                ..Default::default()
            },
        );
        assert_eq!(pressure[2], [0.5, 0.7]);
        move_node(&mut pressure, 0, [0.8, 0.9], &Options::default());
        assert_eq!(pressure[0], [0., 0.]);
    }
}
