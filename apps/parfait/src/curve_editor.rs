//! Shared normalized response-curve widget. Callers own curve data and persistence.
use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke};
use parfait_core::sample_response as sample;

pub struct Options<'a> {
    pub input: &'a str,
    pub output: &'a str,
    pub side: f32,
    pub fixed_x: bool,
    pub fixed_endpoints: bool,
    pub increasing: bool,
    pub max_points: usize,
    pub marker: Option<f32>,
    pub delete_on_drag_out: bool,
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
            delete_on_drag_out: false,
        }
    }
}
fn y_bounds(points: &[[f32; 2]], i: usize, increasing: bool) -> (f32, f32) {
    if increasing {
        (
            if i == 0 { 0. } else { points[i - 1][1] },
            if i + 1 == points.len() { 1. } else { points[i + 1][1] },
        )
    } else {
        (0., 1.)
    }
}
fn move_node(
    points: &mut [[f32; 2]],
    i: usize,
    value: [f32; 2],
    options: &Options<'_>,
) {
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
/// Points must be finite, sorted by x, with distinct x in [0,1] and y in [0,1].
/// Returns the plot rectangle for embedding/tests. The curve uses the same smooth interpolation as the brush engine.
pub fn edit(
    ui: &mut egui::Ui,
    id: impl egui::AsIdSalt,
    points: &mut Vec<[f32; 2]>,
    selected: &mut Option<usize>,
    options: Options<'_>,
) -> Rect {
    edit_inner(ui, id, points, selected, options, None, None).0
}

pub fn edit_tone(
    ui: &mut egui::Ui,
    id: impl egui::AsIdSalt,
    points: &mut Vec<[f32; 2]>,
    selected: &mut Option<usize>,
    histogram: &[u64; 256],
    options: Options<'_>,
) -> Rect {
    edit_inner(ui, id, points, selected, options, None, Some(histogram)).0
}

pub fn edit_brush(
    ui: &mut egui::Ui,
    id: impl egui::AsIdSalt,
    curve: &mut parfait_core::ResponseCurve,
    selected: &mut Option<usize>,
    options: Options<'_>,
) -> Rect {
    let mut controls = curve.controls.clone().unwrap_or_else(|| {
        parfait_core::automatic_curve_controls(&curve.points)
    });
    let (plot, edited) = edit_inner(
        ui,
        id,
        &mut curve.points,
        selected,
        options,
        Some(&mut controls),
        None,
    );
    if edited || curve.controls.is_some() {
        curve.controls = Some(controls);
    }
    if ui
        .add_enabled(
            curve.controls.is_some(),
            egui::Button::new("Auto curvature"),
        )
        .on_hover_text("Reset tangent handles without moving anchors")
        .clicked()
    {
        curve.controls = None;
    }
    plot
}

fn edit_inner(
    ui: &mut egui::Ui,
    id: impl egui::AsIdSalt,
    points: &mut Vec<[f32; 2]>,
    selected: &mut Option<usize>,
    options: Options<'_>,
    mut controls: Option<&mut Vec<[[f32; 2]; 2]>>,
    histogram: Option<&[u64; 256]>,
) -> (Rect, bool) {
    let valid = (2..=options.max_points).contains(&points.len())
        && points
            .iter()
            .flatten()
            .all(|v| v.is_finite() && (0. ..=1.).contains(v))
        && points.windows(2).all(|w| {
            w[1][0] > w[0][0] && (!options.increasing || w[1][1] >= w[0][1])
        });
    if !valid {
        return (
            ui.colored_label(Color32::LIGHT_RED, "Invalid response curve").rect,
            false,
        );
    }
    ui.push_id(id, |ui| {
        let mut controls_edited = false;
        let mut previous_points = points.clone();
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
        let dragging_id = ui.id().with("curve_dragging");
        let handle_id = ui.id().with("handle_dragging");
        let active_handle_id = ui.id().with("active_handle");
        let mut active_handle = ui.data(|data| data.get_temp::<(usize, usize)>(active_handle_id));
        if active_handle.is_some_and(|(segment, _)| segment + 1 >= points.len()) { active_handle = None; }
        let mut handle_dragging = ui.data(|data| data.get_temp::<(usize, usize, egui::Vec2)>(handle_id));
        let mut dragging = ui.data(|data| data.get_temp::<egui::Vec2>(dragging_id));
        if !ui.is_enabled() { dragging = None; handle_dragging = None; }
        let mut outside = false;
        let transform = ui.ctx().layer_transform_from_global(ui.layer_id());
        let local = |position| transform.map_or(position, |transform| transform * position);
        let events = ui.input(|input| input.events.clone());
        for event in events {
            match event {
                egui::Event::PointerButton { pos: position, button: egui::PointerButton::Primary, pressed: true, .. }
                    if ui.is_enabled() && ui.clip_rect().contains(local(position)) && outer.contains(local(position))
                        && ui.ctx().layer_id_at(position).is_none_or(|layer| layer == ui.layer_id()) => {
                    let pointer = local(position);
                    handle_dragging = controls.as_ref().and_then(|handles| {
                        selected.and_then(|anchor| {
                            [(anchor.checked_sub(1), 1), ((anchor + 1 < points.len()).then_some(anchor), 0)]
                                .into_iter().filter_map(|(segment, side)| segment.map(|segment| (segment, side)))
                                .find(|(segment, side)| pos(handles[*segment][*side]).distance(pointer) < 9.)
                                .map(|(segment, side)| (segment, side, pos(handles[segment][side]) - pointer))
                        })
                    });
                    if let Some((segment, side, _)) = handle_dragging { active_handle = Some((segment, side)); dragging = None; response.request_focus(); continue; }
                    active_handle = None;
                    *selected = points.iter().enumerate().min_by(|(_, first), (_, second)| pos(**first).distance_sq(pointer).total_cmp(&pos(**second).distance_sq(pointer)))
                        .and_then(|(index, point)| (pos(*point).distance(pointer) < 12.).then_some(index));
                    if selected.is_none() && r.contains(pointer) && !options.fixed_x && points.len() < options.max_points {
                        let [input, mut output] = value(pointer);
                        let index = points.partition_point(|point| point[0] < input);
                        if index > 0 && index < points.len() && input - points[index - 1][0] > 0.005 && points[index][0] - input > 0.005 {
                            if options.increasing { output = output.clamp(points[index - 1][1], points[index][1]); }
                            points.insert(index, [input, output]);
                            *selected = Some(index);
                        }
                    }
                    response.request_focus();
                    dragging = selected.map(|index| pos(points[index]) - pointer);
                }
                egui::Event::PointerMoved(position) | egui::Event::PointerButton { pos: position, button: egui::PointerButton::Primary, pressed: false, .. } => {
                    let released = matches!(event, egui::Event::PointerButton { .. });
                    if let (Some((segment, side, offset)), Some(handles)) = (handle_dragging, controls.as_deref_mut()) {
                        let mut target = value(local(position) + offset);
                        let low = if side == 0 { points[segment][0] } else { handles[segment][0][0] };
                        let high = if side == 0 { handles[segment][1][0] } else { points[segment + 1][0] };
                        target[0] = target[0].clamp(low, high);
                        if handles[segment][side].iter().zip(target).any(|(previous, next)| (previous - next).abs() > 0.000001) {
                            controls_edited = true;
                            handles[segment][side] = target;
                        }
                        if released { handle_dragging = None; }
                        continue;
                    }
                    if let (Some(offset), Some(index)) = (dragging, *selected) {
                        let pointer = local(position);
                        outside = options.delete_on_drag_out && !options.fixed_x && index > 0 && index + 1 < points.len() && !r.expand(12.).contains(pointer);
                        move_node(points, index, value(pointer + offset), &options);
                        if released && outside {
                            points.remove(index);
                            *selected = None;
                        }
                    }
                    if released { dragging = None; outside = false; }
                }
                egui::Event::PointerGone => { dragging = None; handle_dragging = None; outside = false; }
                _ => {}
            }
        }
        if let (Some(_), Some(index), Some(pointer)) = (dragging, *selected, ui.input(|input| input.pointer.interact_pos())) {
            outside = options.delete_on_drag_out && !options.fixed_x && index > 0 && index + 1 < points.len() && !r.expand(12.).contains(local(pointer));
        }
        if !ui.is_enabled() || !ui.input(|input| input.pointer.primary_down()) {
            dragging = None;
            handle_dragging = None;
        }
        ui.data_mut(|data| {
            if let Some(offset) = dragging { data.insert_temp(dragging_id, offset); } else { data.remove::<egui::Vec2>(dragging_id); }
            if let Some(handle) = handle_dragging { data.insert_temp(handle_id, handle); } else { data.remove::<(usize, usize, egui::Vec2)>(handle_id); }
        });
        if response.has_focus() && let Some((segment, side)) = active_handle && let Some(handles) = controls.as_deref_mut() {
            let mut target = handles[segment][side];
            ui.input_mut(|input| {
                for (key, axis, direction) in [(egui::Key::ArrowLeft, 0, -1.), (egui::Key::ArrowRight, 0, 1.), (egui::Key::ArrowDown, 1, -1.), (egui::Key::ArrowUp, 1, 1.)] {
                    let step = if input.consume_key(egui::Modifiers::SHIFT, key) { 0.1 } else if input.consume_key(egui::Modifiers::NONE, key) { 0.01 } else { 0. };
                    target[axis] += direction * step;
                }
            });
            let low = if side == 0 { points[segment][0] } else { handles[segment][0][0] };
            let high = if side == 0 { handles[segment][1][0] } else { points[segment + 1][0] };
            target[0] = target[0].clamp(low, high);
            target[1] = target[1].clamp(0., 1.);
            controls_edited |= target != handles[segment][side];
            handles[segment][side] = target;
        } else if response.has_focus() && let Some(mut index) = *selected {
            let mut target = points[index];
            ui.input_mut(|input| {
                let nudge = |input: &mut egui::InputState, key| {
                    if input.consume_key(egui::Modifiers::SHIFT, key) { 0.1 }
                    else if input.consume_key(egui::Modifiers::NONE, key) { 0.01 }
                    else { 0. }
                };
                target[1] += nudge(input, egui::Key::ArrowUp) - nudge(input, egui::Key::ArrowDown);
                for (key, direction) in [(egui::Key::ArrowLeft, -1.), (egui::Key::ArrowRight, 1.)] {
                    let movement = direction * nudge(input, key);
                    if movement != 0. {
                        if options.fixed_x {
                            index = (index as isize + direction as isize).clamp(0, points.len() as isize - 1) as usize;
                            target = points[index];
                        } else { target[0] += movement; }
                    }
                }
            });
            *selected = Some(index);
            move_node(points, index, target, &options);
        }
        let removable = active_handle.is_none() && !options.fixed_x && selected.is_some_and(|i| i > 0 && i + 1 < points.len());
        let delete = response.has_focus()
            && ui
                .input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace));
        if points != &previous_points && let Some(handles) = controls.as_deref_mut() {
            parfait_core::reconcile_curve_controls(points, &previous_points, handles);
        }
        previous_points.clone_from(points);
        let painter = ui.painter();
        painter.rect_filled(r, 0., Color32::from_gray(65));
        if let Some(bins)=histogram {
            let max=bins.iter().copied().max().unwrap_or(1).max(1) as f64;
            for (i,n) in bins.iter().enumerate(){let h=(*n as f64/max).sqrt() as f32*r.height();painter.rect_filled(Rect::from_min_max(egui::pos2(r.left()+i as f32/256.*r.width(),r.bottom()-h),egui::pos2(r.left()+(i+1) as f32/256.*r.width(),r.bottom())),0.,Color32::from_gray(88));}
        }
        for i in 0..=4 {
            let t = i as f32 / 4.;
            let stroke = Stroke::new(1., Color32::from_gray(125));
            painter.line_segment([pos([t, 0.]), pos([t, 1.])], stroke);
            painter.line_segment([pos([0., t]), pos([1., t])], stroke);
        }
        painter.line_segment([pos([0., 0.]), pos([1., 1.])], Stroke::new(0.7, Color32::from_gray(97)));
        if response.hovered() && let Some(pointer) = ui.input(|input| input.pointer.hover_pos()).filter(|pointer| r.contains(*pointer)) {
            let [input, _] = value(pointer);
            painter.line_segment([pos([input, 0.]), pos([input, 1.])], Stroke::new(0.7, Color32::from_gray(160)));
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
        let curve_points = points.as_slice();
        let evaluate = |input| match controls.as_deref() {
            Some(handles) => parfait_core::sample_bezier_response(curve_points, handles, input),
            None => sample(curve_points, input),
        };
        painter.add(egui::Shape::line(
            curve_points
                .windows(2)
                .flat_map(|pair| {
                    let steps = ((pair[1][0] - pair[0][0]) * side).ceil().max(2.) as usize;
                    (0..=steps).map(move |i| {
                        let x = pair[0][0] + (pair[1][0] - pair[0][0]) * i as f32 / steps as f32;
                        pos([x, evaluate(x)])
                    })
                })
                .collect(),
            Stroke::new(1.2, Color32::WHITE),
        ));
        if let (Some(anchor), Some(handles)) = (*selected, controls.as_deref()) {
            for (segment, side) in [(anchor.checked_sub(1), 1), ((anchor + 1 < points.len()).then_some(anchor), 0)] {
                if let Some(segment) = segment {
                    let handle = pos(handles[segment][side]);
                    painter.line_segment([pos(points[anchor]), handle], Stroke::new(1., Color32::from_rgb(190, 180, 235)));
                    painter.circle_filled(handle, 4., Color32::from_gray(65));
                    painter.circle_stroke(handle, 4., Stroke::new(1.5, if active_handle == Some((segment, side)) { Color32::WHITE } else { Color32::from_rgb(200, 190, 245) }));
                }
            }
        }
        for (i, p) in points.iter().enumerate() {
            painter.rect_filled(
                Rect::from_center_size(pos(*p), egui::vec2(if *selected == Some(i) { 7. } else { 5. }, if *selected == Some(i) { 7. } else { 5. })),
                0.,
                if *selected == Some(i) && outside {
                    Color32::LIGHT_RED
                } else if *selected == Some(i) {
                    Color32::WHITE
                } else {
                    Color32::from_rgb(159, 150, 230)
                },
            );
        }
        if outside {
            painter.text(r.center_top() + egui::vec2(0., 5.), egui::Align2::CENTER_TOP, "Release to delete point", egui::FontId::proportional(11.), Color32::LIGHT_RED);
        }
        if let Some(raw) = options.marker.filter(|p| p.is_finite()) {
            let x = raw.clamp(0., 1.);
            painter.circle_stroke(
                pos([x, evaluate(x)]),
                3.,
                Stroke::new(1., Color32::LIGHT_GREEN),
            );
        }
        response.on_hover_text(if options.fixed_x {
            "Drag a square vertically. ↑/↓ adjust output; ←/→ select a sample. Shift changes output by 10%."
        } else if controls.is_some() {
            "Square: move anchor. Round handle: bend the curve without moving its anchor. Click empty space to add an anchor. Delete removes the selected interior anchor."
        } else {
            "Click to add; drag in both axes. Arrow keys move 1%, Shift moves 10%. Delete removes an interior point."
        });
        {
            let index = selected.unwrap_or(0);
            let active = selected.is_some();
            let coordinate = active_handle.and_then(|(segment, side)| controls.as_deref().map(|handles| handles[segment][side])).unwrap_or(points[index]);
            let mut shown = [coordinate[0] * 100., coordinate[1] * 100.];
            let fixed_endpoint = options.fixed_endpoints && (index == 0 || index + 1 == points.len());
            let mut coordinate_changed = false;
            ui.horizontal(|ui| {
                ui.label(if active_handle.is_some() { "Handle X" } else { options.input });
                coordinate_changed |= ui.add_enabled(active && (active_handle.is_some() || (!options.fixed_x && index > 0 && index + 1 < points.len())), egui::DragValue::new(&mut shown[0]).range(0. ..=100.).update_while_editing(false).speed(1.).suffix("%")).changed();
                ui.label(if active_handle.is_some() { "Y" } else { options.output });
                coordinate_changed |= ui.add_enabled(active && (active_handle.is_some() || !fixed_endpoint), egui::DragValue::new(&mut shown[1]).range(0. ..=100.).update_while_editing(false).speed(1.).suffix("%")).changed();
            });
            if let (Some((segment, side)), Some(handles)) = (active_handle, controls.as_deref_mut()) && coordinate_changed {
                let low = if side == 0 { points[segment][0] } else { handles[segment][0][0] };
                let high = if side == 0 { handles[segment][1][0] } else { points[segment + 1][0] };
                let target = [(shown[0] / 100.).clamp(low, high), (shown[1] / 100.).clamp(0., 1.)];
                controls_edited |= target != handles[segment][side];
                handles[segment][side] = target;
            } else if active_handle.is_none() && active && coordinate_changed { move_node(points, index, [shown[0] / 100., shown[1] / 100.], &options); }
        }
        if !options.fixed_x {
            let clicked = ui
                .add_enabled(removable, egui::Button::new(if histogram.is_some(){"선택한 점 삭제"}else{"Delete selected point"}))
                .clicked();
            if removable
                && (delete || clicked)
                && let Some(i) = selected.take()
            {
                points.remove(i);
            }
        }
        if points != &previous_points && let Some(handles) = controls {
            parfait_core::reconcile_curve_controls(points, &previous_points, handles);
        }
        ui.data_mut(|data| {
            if let Some(handle) = active_handle { data.insert_temp(active_handle_id, handle); }
            else { data.remove::<(usize, usize)>(active_handle_id); }
        });
        (r, controls_edited)
    })
    .inner
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn round_handle_drag_changes_curvature_not_anchors_and_keyboard_stays_on_handle()
     {
        let ctx = egui::Context::default();
        let mut curve = parfait_core::ResponseCurve::default();
        let anchors = curve.points.clone();
        let mut selected = Some(0);
        let mut frame = |events| {
            let mut plot = Rect::NOTHING;
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        Pos2::ZERO,
                        egui::vec2(500., 500.),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| {
                    plot = edit_brush(
                        ui,
                        "handles",
                        &mut curve,
                        &mut selected,
                        Options {
                            fixed_endpoints: false,
                            increasing: false,
                            delete_on_drag_out: true,
                            ..Default::default()
                        },
                    );
                },
            );
            output.textures_delta.clear();
            (plot, curve.clone())
        };
        let (plot, initial) = frame(vec![]);
        frame(vec![]);
        assert!(initial.controls.is_none());
        let position = |input: f32, output: f32| {
            egui::pos2(
                plot.left() + input * plot.width(),
                plot.bottom() - output * plot.height(),
            )
        };
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            pressed,
            button: egui::PointerButton::Primary,
            modifiers: Default::default(),
        };
        let handle = position(1. / 3., 1. / 3.);
        let (_, selected_only) = frame(vec![
            egui::Event::PointerMoved(handle),
            button(handle, true),
            button(handle, false),
        ]);
        assert!(selected_only.controls.is_none());
        let target = position(0.15, 0.8);
        let (_, bent) = frame(vec![
            button(handle, true),
            egui::Event::PointerMoved(target),
            button(target, false),
        ]);
        assert_eq!(bent.points, anchors);
        assert!(bent.controls.is_some());
        assert!(bent.sample(0.3) > 0.45);
        let (_, nudged) = frame(vec![egui::Event::Key {
            key: egui::Key::ArrowUp,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        }]);
        assert_eq!(nudged.points, anchors);
        assert!(
            (nudged.controls.as_ref().unwrap()[0][0][1] - 0.81).abs() < 0.0001
        );
        let outside = position(-0.3, 1.2);
        let (_, clamped) = frame(vec![
            button(position(0.15, 0.81), true),
            egui::Event::PointerMoved(outside),
            button(outside, false),
        ]);
        assert_eq!(clamped.points, anchors);
        assert!(clamped.validate());
    }
    #[test]
    fn press_move_release_in_one_frame_adds_moves_and_deletes_without_losing_the_gesture()
     {
        let ctx = egui::Context::default();
        let mut points = vec![[0., 0.], [1., 1.]];
        let mut selected = None;
        let mut frame = |events, enabled| {
            let mut plot = Rect::NOTHING;
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        Pos2::ZERO,
                        egui::vec2(500., 450.),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| {
                    ui.add_enabled_ui(enabled, |ui| {
                        plot = edit(
                            ui,
                            "batched",
                            &mut points,
                            &mut selected,
                            Options {
                                fixed_endpoints: false,
                                increasing: false,
                                delete_on_drag_out: true,
                                ..Default::default()
                            },
                        );
                    });
                },
            );
            output.textures_delta.clear();
            (plot, points.clone())
        };
        let (plot, _) = frame(vec![], true);
        let start = egui::pos2(
            plot.left() + plot.width() * 0.37,
            plot.bottom() - plot.height() * 0.68,
        );
        let end = egui::pos2(
            plot.left() + plot.width() * 0.63,
            plot.bottom() - plot.height() * 0.2,
        );
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            pressed,
            button: egui::PointerButton::Primary,
            modifiers: Default::default(),
        };
        let (_, added) = frame(
            vec![
                egui::Event::PointerMoved(start),
                button(start, true),
                egui::Event::PointerMoved(end),
                button(end, false),
            ],
            true,
        );
        assert_eq!(added.len(), 3);
        assert!((added[1][0] - 0.63).abs() < 0.0001);
        assert!((added[1][1] - 0.2).abs() < 0.0001);
        let (_, disabled) = frame(
            vec![
                egui::Event::PointerMoved(start),
                button(start, true),
                egui::Event::PointerMoved(end),
                button(end, false),
            ],
            false,
        );
        assert_eq!(disabled, added);
        frame(vec![], true);
        let outside = egui::pos2(plot.right() + 40., plot.top());
        let (_, deleted) = frame(
            vec![
                egui::Event::PointerMoved(end),
                button(end, true),
                egui::Event::PointerMoved(outside),
                button(outside, false),
            ],
            true,
        );
        assert_eq!(deleted, vec![[0., 0.], [1., 1.]]);
    }
    #[test]
    fn free_brush_points_add_move_in_both_axes_and_delete_outside_on_release() {
        let ctx = egui::Context::default();
        let mut points = vec![[0., 0.], [1., 1.]];
        let mut selected = None;
        let mut frame = |events| {
            let mut plot = Rect::NOTHING;
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        Pos2::ZERO,
                        egui::vec2(500., 450.),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| {
                    plot = edit(
                        ui,
                        "free",
                        &mut points,
                        &mut selected,
                        Options {
                            fixed_endpoints: false,
                            increasing: false,
                            delete_on_drag_out: true,
                            ..Default::default()
                        },
                    );
                },
            );
            output.textures_delta.clear();
            (plot, points.clone())
        };
        let (plot, _) = frame(vec![]);
        let position = |input: f32, output: f32| {
            egui::pos2(
                plot.left() + plot.width() * input,
                plot.bottom() - plot.height() * output,
            )
        };
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            pressed,
            button: egui::PointerButton::Primary,
            modifiers: Default::default(),
        };
        let start = position(0.37, 0.68);
        let (_, added) =
            frame(vec![egui::Event::PointerMoved(start), button(start, true)]);
        assert_eq!(added.len(), 3);
        let (_, released) = frame(vec![button(start, false)]);
        assert_eq!(released, added);
        frame(vec![button(start, true)]);
        let end = position(0.63, 0.2);
        let (_, moved) =
            frame(vec![egui::Event::PointerMoved(end), button(end, false)]);
        assert!((moved[1][0] - 0.63).abs() < 0.0001);
        assert!((moved[1][1] - 0.2).abs() < 0.0001);
        let peak = position(0.3, 0.9);
        frame(vec![egui::Event::PointerMoved(peak), button(peak, true)]);
        let (_, falling) = frame(vec![button(peak, false)]);
        assert_eq!(falling.len(), 4);
        assert!(falling[1][1] > falling[2][1]);
        assert!(
            parfait_core::ResponseCurve { points: falling, controls: None }
                .validate()
        );
        let key = |key| egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::SHIFT,
        };
        let (_, nudged) =
            frame(vec![key(egui::Key::ArrowRight), key(egui::Key::ArrowDown)]);
        assert!((nudged[1][0] - 0.4).abs() < 0.0001);
        assert!((nudged[1][1] - 0.8).abs() < 0.0001);
        let selected = position(nudged[1][0], nudged[1][1]);
        frame(vec![
            egui::Event::PointerMoved(selected),
            button(selected, true),
        ]);
        let outside = egui::pos2(plot.right() + 40., plot.center().y);
        let (_, before_drop) = frame(vec![egui::Event::PointerMoved(outside)]);
        assert_eq!(before_drop.len(), 4);
        let (_, deleted) = frame(vec![button(outside, false)]);
        assert_eq!(deleted.len(), 3);
        assert!((deleted[1][0] - 0.63).abs() < 0.0001);
        let endpoint = position(0., 0.);
        frame(vec![
            egui::Event::PointerMoved(endpoint),
            button(endpoint, true),
        ]);
        let outside = position(-0.3, 0.6);
        let (_, endpoint_moved) = frame(vec![
            egui::Event::PointerMoved(outside),
            button(outside, false),
        ]);
        assert_eq!(endpoint_moved.len(), 3);
        assert_eq!(endpoint_moved[0][0], 0.);
        assert!((endpoint_moved[0][1] - 0.6).abs() < 0.0001);
        assert!(
            parfait_core::ResponseCurve {
                points: endpoint_moved,
                controls: None,
            }
            .validate()
        );
    }

    #[test]
    fn free_brush_editor_bounds_point_count_and_keeps_endpoints() {
        let ctx = egui::Context::default();
        let mut points: Vec<_> =
            (0..16).map(|index| [index as f32 / 15., 0.5]).collect();
        let mut selected = None;
        let mut frame = |events| {
            let mut plot = Rect::NOTHING;
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        Pos2::ZERO,
                        egui::vec2(500., 450.),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| {
                    plot = edit(
                        ui,
                        "limit",
                        &mut points,
                        &mut selected,
                        Options {
                            fixed_endpoints: false,
                            increasing: false,
                            ..Default::default()
                        },
                    );
                },
            );
            output.textures_delta.clear();
            (plot, points.clone())
        };
        let (plot, _) = frame(vec![]);
        let point = egui::pos2(plot.center().x, plot.top() + 15.);
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            pressed,
            button: egui::PointerButton::Primary,
            modifiers: Default::default(),
        };
        frame(vec![egui::Event::PointerMoved(point), button(point, true)]);
        let (_, bounded) = frame(vec![button(point, false)]);
        assert_eq!(bounded.len(), 16);
        let endpoint = plot.left_center();
        frame(vec![
            egui::Event::PointerMoved(endpoint),
            button(endpoint, true),
        ]);
        frame(vec![button(endpoint, false)]);
        let (_, retained) = frame(vec![egui::Event::Key {
            key: egui::Key::Delete,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        }]);
        assert_eq!(retained.len(), 16);
        assert_eq!(retained[0][0], 0.);
    }

    #[test]
    fn brush_curve_release_frame_keeps_last_drag_position_and_keyboard_constraints()
     {
        let ctx = egui::Context::default();
        let mut points =
            vec![[0., 0.], [0.25, 0.25], [0.5, 0.5], [0.75, 0.75], [1., 1.]];
        let mut selected = Some(2);
        let mut frame = |events| {
            let mut plot = Rect::NOTHING;
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        Pos2::ZERO,
                        egui::vec2(500., 400.),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| {
                    plot = edit(
                        ui,
                        "brush",
                        &mut points,
                        &mut selected,
                        Options {
                            fixed_x: true,
                            fixed_endpoints: false,
                            ..Default::default()
                        },
                    );
                },
            );
            output.textures_delta.clear();
            (plot, points.clone())
        };
        let (plot, _) = frame(vec![]);
        let start = plot.center();
        let end = start - egui::vec2(0., plot.height() * 0.2);
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            pressed,
            button: egui::PointerButton::Primary,
            modifiers: Default::default(),
        };
        frame(vec![egui::Event::PointerMoved(start), button(start, true)]);
        let (_, changed) =
            frame(vec![egui::Event::PointerMoved(end), button(end, false)]);
        assert!((changed[2][1] - 0.7).abs() < 0.001);
        let key = |key| egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        };
        let (_, nudged) = frame(vec![key(egui::Key::ArrowUp)]);
        assert!((nudged[2][1] - 0.71).abs() < 0.001);
        let (_, adjacent) = frame(vec![key(egui::Key::ArrowRight)]);
        assert_eq!(adjacent, nudged);
        let (_, next) = frame(vec![key(egui::Key::ArrowDown)]);
        assert!((next[3][1] - 0.74).abs() < 0.001);
        assert_eq!(next[2], [0.5, nudged[2][1]]);
    }
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
                    screen_rect: Some(Rect::from_min_size(
                        Pos2::ZERO,
                        egui::vec2(700., 400.),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| {
                    ui.horizontal(|ui| {
                        plot = edit(
                            ui,
                            "pressure",
                            &mut a,
                            &mut sa,
                            Options::default(),
                        );
                        edit(
                            ui,
                            "tilt",
                            &mut b,
                            &mut sb,
                            Options { increasing: false, ..Default::default() },
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
            &Options { increasing: false, ..Default::default() },
        );
        assert_eq!(falling[1], [0.8, 0.1]);
        assert!(sample(&falling, 0.4) > 0.1 && sample(&falling, 0.4) < 1.);
        let mut pressure =
            vec![[0., 0.], [0.25, 0.3], [0.5, 0.5], [0.75, 0.7], [1., 1.]];
        move_node(
            &mut pressure,
            2,
            [0.9, 1.],
            &Options { fixed_x: true, ..Default::default() },
        );
        assert_eq!(pressure[2], [0.5, 0.7]);
        move_node(&mut pressure, 0, [0.8, 0.9], &Options::default());
        assert_eq!(pressure[0], [0., 0.]);
    }
}
