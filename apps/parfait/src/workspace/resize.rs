use super::{Location, Palette, Workspace};
use eframe::egui::{self, Color32, CursorIcon, Id, Pos2, Rect, Vec2};

#[derive(Clone, Copy)]
pub(super) struct Drag {
    pub initial: [f32; 4],
    pub delta: Vec2,
    pub origin: Pos2,
}

pub(super) fn handle(
    ui: &mut egui::Ui,
    id: Id,
    rect: Rect,
    cursor: CursorIcon,
    label: &str,
    initial: [f32; 4],
) -> Option<Drag> {
    let response =
        ui.interact(rect, id, egui::Sense::drag()).on_hover_cursor(cursor);
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Other,
            ui.is_enabled(),
            label,
        )
    });
    #[cfg(test)]
    ui.data_mut(|data| {
        data.insert_temp(Id::new(("resize_target", label)), rect)
    });
    let drag = track(ui, id, rect, cursor, initial);
    if response.hovered() || ui.ctx().dragged_id() == Some(id) {
        ui.painter().rect_filled(rect, 0., Color32::from_white_alpha(28));
    }
    drag
}

pub(super) fn track(
    ui: &mut egui::Ui,
    id: Id,
    rect: Rect,
    cursor: CursorIcon,
    initial: [f32; 4],
) -> Option<Drag> {
    let state_id = id.with("resize_origin");
    let mut state = ui.data(|data| data.get_temp::<Drag>(state_id));
    let enabled = ui.is_enabled() && ui.input(|input| input.focused);
    let layer_matches =
        ui.input(|input| input.pointer.press_origin()).is_none_or(|position| {
            ui.ctx()
                .layer_id_at(position)
                .is_none_or(|layer| layer == ui.layer_id())
        });
    let transform = ui.ctx().layer_transform_from_global(ui.layer_id());
    let local =
        |position| transform.map_or(position, |transform| transform * position);
    if !enabled {
        state = None;
    }
    let mut result = state;
    let mut changed = false;
    ui.input(|input| {
        for event in &input.events {
            match *event {
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    ..
                } if enabled
                    && layer_matches
                    && rect.contains(local(pos))
                    && ui.clip_rect().contains(local(pos)) =>
                {
                    state = Some(Drag {
                        initial,
                        delta: Vec2::ZERO,
                        origin: local(pos),
                    });
                    result = state;
                    changed = true;
                }
                egui::Event::PointerMoved(pos)
                | egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    ..
                } => {
                    if let Some(drag) = &mut state {
                        drag.delta = local(pos) - drag.origin;
                        result = Some(*drag);
                        changed = true;
                    }
                    if matches!(event, egui::Event::PointerButton { .. }) {
                        state = None;
                    }
                }
                egui::Event::PointerGone
                | egui::Event::WindowFocused(false)
                | egui::Event::Key {
                    key: egui::Key::Escape,
                    pressed: true,
                    ..
                } => {
                    state = None;
                    result = None;
                }
                _ => {}
            }
        }
        if !input.pointer.primary_down() {
            state = None;
        }
    });
    ui.data_mut(|data| {
        if let Some(drag) = state {
            data.insert_temp(state_id, drag);
        } else {
            data.remove::<Drag>(state_id);
        }
    });
    if state.is_some() {
        ui.ctx().set_dragged_id(id);
        ui.ctx().set_cursor_icon(cursor);
    }
    if changed {
        ui.ctx().request_repaint();
    }
    result
}

impl Workspace {
    pub(super) fn dock_resize(
        &mut self,
        ui: &mut egui::Ui,
        side: Location,
        layout: &[(Palette, Rect)],
    ) {
        let rows = self.dock_rows(side);
        let columns = self.dock_columns(side);
        for pair in columns.windows(2) {
            let bounds = |column: &[Palette]| {
                layout
                    .iter()
                    .filter(|(p, _)| column.contains(p))
                    .map(|(_, r)| *r)
                    .reduce(|a, b| a.union(b))
                    .unwrap()
            };
            let first = bounds(&pair[0]);
            let second = bounds(&pair[1]);
            let target = Rect::from_min_max(
                egui::pos2(first.right() - 4., first.top()),
                egui::pos2(
                    first.right() + 4.,
                    first.bottom().max(second.bottom()),
                ),
            );
            let minimum = |column: &[Palette]| {
                rows.iter()
                    .filter(|row| column.contains(&row[0]))
                    .map(|row| row.len())
                    .max()
                    .unwrap_or(1) as f32
                    * 180.
            };
            let a = minimum(&pair[0]);
            let b = minimum(&pair[1]);
            let weight = |column: &[Palette]| {
                self.panels
                    .iter()
                    .find(|p| p.palette == column[0])
                    .unwrap()
                    .width_weight
            };
            let label = format!(
                "Resize between {} and {}",
                pair[0][0].title(),
                pair[1][0].title()
            );
            if let Some(drag) = handle(
                ui,
                ui.id().with(("dock_column_resize", pair[0][0], pair[1][0])),
                target,
                CursorIcon::ResizeHorizontal,
                &label,
                [
                    first.width(),
                    second.width(),
                    weight(&pair[0]),
                    weight(&pair[1]),
                ],
            ) {
                self.resizing = true;
                let extra = drag.initial[0] + drag.initial[1] - a - b;
                if extra > 0.001 {
                    let fraction = (drag.initial[0] - a + drag.delta.x)
                        .clamp(0., extra)
                        / extra;
                    let weight = (drag.initial[2] + drag.initial[3]).max(1.);
                    for p in &mut self.panels {
                        if pair[0].contains(&p.palette) {
                            p.width_weight = weight * fraction;
                        }
                        if pair[1].contains(&p.palette) {
                            p.width_weight = weight * (1. - fraction);
                        }
                    }
                }
            }
        }
        for (row_index, row) in rows.iter().enumerate() {
            let column =
                columns.iter().find(|column| column.contains(&row[0])).unwrap();
            let bounds = row
                .iter()
                .filter_map(|palette| {
                    layout
                        .iter()
                        .find(|(item, _)| item == palette)
                        .map(|(_, rect)| *rect)
                })
                .reduce(|first, second| first.union(second))
                .unwrap();
            let height = row
                .iter()
                .filter_map(|palette| {
                    self.panels
                        .iter()
                        .find(|placement| {
                            placement.palette == *palette
                                && !placement.collapsed
                        })
                        .map(|placement| placement.height)
                })
                .reduce(f32::max);
            if height.is_some() {
                let next = rows.iter().skip(row_index + 1).find(|row| {
                    column.contains(&row[0])
                        && row.iter().any(|palette| {
                            self.panels.iter().any(|placement| {
                                placement.palette == *palette
                                    && !placement.collapsed
                            })
                        })
                });
                let height = (bounds.height() - 29.).max(60.);
                let next_height = next.map(|row| {
                    row.iter()
                        .filter_map(|p| {
                            layout
                                .iter()
                                .find(|(palette, _)| palette == p)
                                .map(|(_, rect)| rect.height() - 29.)
                        })
                        .fold(60., f32::max)
                });
                let target = Rect::from_min_max(
                    egui::pos2(bounds.left(), bounds.bottom() - 7.),
                    egui::pos2(bounds.right(), bounds.bottom() + 1.),
                );
                let label = format!("Resize row below {}", row[0].title());
                if let Some(drag) = next.and_then(|_| {
                    handle(
                        ui,
                        ui.id().with(("row_resize", row[0])),
                        target,
                        CursorIcon::ResizeVertical,
                        &label,
                        [height, next_height.unwrap_or(0.), 0., 0.],
                    )
                }) {
                    self.resizing = true;
                    let delta = drag.delta.y.clamp(
                        if next.is_some() {
                            (60. - drag.initial[0]).max(drag.initial[1] - 2000.)
                        } else {
                            60. - drag.initial[0]
                        },
                        if next.is_some() {
                            (drag.initial[1] - 60.).min(2000. - drag.initial[0])
                        } else {
                            2000. - drag.initial[0]
                        },
                    );
                    for placement in &mut self.panels {
                        // Heights are proportional weights. Normalize the
                        // untouched rows to their displayed heights as well,
                        // so moving one splitter cannot resize a third row.
                        if column.contains(&placement.palette)
                            && !placement.collapsed
                            && let Some((_, rect)) = layout
                                .iter()
                                .find(|(p, _)| *p == placement.palette)
                        {
                            placement.height =
                                (rect.height() - 29.).clamp(60., 2000.);
                        }
                        if !placement.collapsed
                            && row.contains(&placement.palette)
                        {
                            placement.height = drag.initial[0] + delta;
                        } else if !placement.collapsed
                            && next.is_some_and(|row| {
                                row.contains(&placement.palette)
                            })
                        {
                            placement.height = drag.initial[1] - delta;
                        }
                    }
                }
            }
            for pair in row.windows(2) {
                let first_rect = layout
                    .iter()
                    .find(|(palette, _)| *palette == pair[0])
                    .unwrap()
                    .1;
                let second_rect = layout
                    .iter()
                    .find(|(palette, _)| *palette == pair[1])
                    .unwrap()
                    .1;
                let target = Rect::from_min_max(
                    egui::pos2(first_rect.right() - 4., bounds.top() + 23.),
                    egui::pos2(first_rect.right() + 4., bounds.bottom() - 7.),
                );
                if !target.is_positive() {
                    continue;
                }
                ui.painter().vline(
                    first_rect.right(),
                    target.y_range(),
                    egui::Stroke::new(1., Color32::from_gray(46)),
                );
                let first_weight = self
                    .panels
                    .iter()
                    .find(|placement| placement.palette == pair[0])
                    .unwrap()
                    .width_weight;
                let second_weight = self
                    .panels
                    .iter()
                    .find(|placement| placement.palette == pair[1])
                    .unwrap()
                    .width_weight;
                let label = format!(
                    "Resize between {} and {}",
                    pair[0].title(),
                    pair[1].title()
                );
                if let Some(drag) = handle(
                    ui,
                    ui.id().with(("column_resize", pair[0], pair[1])),
                    target,
                    CursorIcon::ResizeHorizontal,
                    &label,
                    [
                        first_rect.width(),
                        second_rect.width(),
                        first_weight,
                        second_weight,
                    ],
                ) {
                    self.resizing = true;
                    let extra = drag.initial[0] + drag.initial[1] - 360.;
                    if extra > 0.001 {
                        let fraction = (drag.initial[0] - 180. + drag.delta.x)
                            .clamp(0., extra)
                            / extra;
                        let weight = drag.initial[2] + drag.initial[3];
                        for placement in &mut self.panels {
                            if placement.palette == pair[0] {
                                placement.width_weight = weight * fraction;
                            }
                            if placement.palette == pair[1] {
                                placement.width_weight =
                                    weight * (1. - fraction);
                            }
                        }
                    }
                }
            }
        }
    }

    pub(super) fn floating_resize(
        &mut self,
        ui: &mut egui::Ui,
        palette: Palette,
        rect: Rect,
    ) {
        let collapsed = self
            .panels
            .iter()
            .find(|placement| placement.palette == palette)
            .unwrap()
            .collapsed;
        for (name, horizontal, vertical) in [
            ("top left", -1, -1),
            ("top right", 1, -1),
            ("bottom left", -1, 1),
            ("bottom right", 1, 1),
            ("left", -1, 0),
            ("right", 1, 0),
            ("top", 0, -1),
            ("bottom", 0, 1),
        ] {
            if collapsed && vertical != 0 {
                continue;
            }
            let corner = horizontal != 0 && vertical != 0;
            let target = Rect::from_min_max(
                egui::pos2(
                    if horizontal < 0 {
                        rect.left()
                    } else if horizontal > 0 {
                        rect.right() - if corner { 10. } else { 6. }
                    } else {
                        rect.left() + 10.
                    },
                    if vertical < 0 {
                        rect.top()
                    } else if vertical > 0 {
                        rect.bottom() - if corner { 10. } else { 6. }
                    } else {
                        rect.top() + if collapsed { 0. } else { 10. }
                    },
                ),
                egui::pos2(
                    if horizontal < 0 {
                        rect.left() + if corner { 10. } else { 6. }
                    } else if horizontal > 0 {
                        rect.right()
                    } else {
                        rect.right() - 10.
                    },
                    if vertical < 0 {
                        rect.top() + if corner { 10. } else { 6. }
                    } else if vertical > 0 {
                        rect.bottom()
                    } else {
                        rect.bottom() - if collapsed { 0. } else { 10. }
                    },
                ),
            );
            let cursor = if corner {
                if horizontal == vertical {
                    CursorIcon::ResizeNwSe
                } else {
                    CursorIcon::ResizeNeSw
                }
            } else if horizontal != 0 {
                CursorIcon::ResizeHorizontal
            } else {
                CursorIcon::ResizeVertical
            };
            let label = format!("Resize {} {name}", palette.title());
            if let Some(drag) = handle(
                ui,
                ui.id().with(("floating_resize", palette, name)),
                target,
                cursor,
                &label,
                [rect.left(), rect.top(), rect.width(), rect.height()],
            ) {
                self.resizing = true;
                let width = (drag.initial[2]
                    + drag.delta.x * horizontal as f32)
                    .clamp(palette.minimum_width(), 800.);
                let height = if collapsed {
                    23.
                } else {
                    (drag.initial[3] + drag.delta.y * vertical as f32)
                        .clamp(89., 2029.)
                };
                let placement = self
                    .panels
                    .iter_mut()
                    .find(|placement| placement.palette == palette)
                    .unwrap();
                placement.rect[0] = drag.initial[0]
                    + if horizontal < 0 { drag.initial[2] - width } else { 0. };
                placement.rect[1] = drag.initial[1]
                    + if vertical < 0 { drag.initial[3] - height } else { 0. };
                placement.rect[2] = width;
                if !collapsed {
                    placement.height = height - 29.;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Parfait;

    struct Fixture {
        app: Parfait,
        ctx: egui::Context,
        directory: tempfile::TempDir,
    }

    impl Fixture {
        fn new() -> Self {
            let directory = tempfile::tempdir().unwrap();
            let mut app =
                Parfait::open_legacy(directory.path().to_owned()).unwrap();
            app.documents.active_session.document =
                parfait_core::Document::new(32, 24);
            let ctx = egui::Context::default();
            crate::theme::apply(&ctx);
            Self { app, ctx, directory }
        }

        fn frame(&mut self, events: Vec<egui::Event>) {
            let mut output = self.ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        Pos2::ZERO,
                        egui::vec2(1440., 1000.),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| {
                    self.app.workspace_ui(ui);
                    egui::CentralPanel::default()
                        .show(ui, |ui| self.app.canvas(ui));
                },
            );
            output.textures_delta.clear();
        }

        fn target(&self, label: &str) -> Rect {
            self.ctx
                .data(|data| data.get_temp(Id::new(("resize_target", label))))
                .unwrap()
        }

        fn placement(&self, palette: Palette) -> &super::super::Placement {
            self.app
                .workspace
                .panels
                .iter()
                .find(|placement| placement.palette == palette)
                .unwrap()
        }
        fn rect(&self, palette: Palette) -> Rect {
            self.app
                .workspace
                .slots
                .iter()
                .find(|(p, _, _)| *p == palette)
                .unwrap()
                .2
        }

        fn drag(&mut self, label: &str, delta: Vec2, batched: bool) {
            self.frame(vec![]);
            self.frame(vec![]);
            let start = self.target(label).center();
            if batched {
                self.frame(vec![
                    egui::Event::PointerMoved(start),
                    button(start, true),
                    egui::Event::PointerMoved(start + delta),
                    button(start + delta, false),
                ]);
            } else {
                self.frame(vec![
                    egui::Event::PointerMoved(start),
                    button(start, true),
                ]);
                self.frame(vec![egui::Event::PointerMoved(
                    start + delta * 0.5,
                )]);
                self.frame(vec![button(start + delta, false)]);
            }
            self.frame(vec![]);
        }
    }

    fn button(pos: Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            pressed,
            button: egui::PointerButton::Primary,
            modifiers: Default::default(),
        }
    }

    #[test]
    fn row_splitters_share_height_and_retain_final_release_for_fast_and_slow_drags()
     {
        for batched in [false, true] {
            let mut fixture = Fixture::new();
            let artwork = fixture.app.documents.active_session.document.clone();
            let collapsed = fixture.placement(Palette::Sizes).height;
            fixture.frame(vec![]);
            fixture.frame(vec![]);
            let brushes = fixture.rect(Palette::Brushes).height();
            let properties = fixture.rect(Palette::Properties).height();
            let color = fixture.rect(Palette::Color).height();
            fixture.drag(
                "Resize row below 보조 도구",
                egui::vec2(0., 70.),
                batched,
            );
            assert!(
                (fixture.rect(Palette::Brushes).height() - brushes - 70.).abs()
                    < 0.1
            );
            assert!(
                (fixture.rect(Palette::Properties).height() - properties + 70.)
                    .abs()
                    < 0.1
            );
            assert!(
                (fixture.rect(Palette::Color).height() - color).abs() < 0.1
            );
            fixture.drag(
                "Resize row below 도구 속성",
                egui::vec2(0., -40.),
                batched,
            );
            assert!(
                (fixture.rect(Palette::Properties).height() - properties
                    + 110.)
                    .abs()
                    < 0.1
            );
            assert!(
                (fixture.rect(Palette::Color).height() - color - 40.).abs()
                    < 0.1
            );
            assert_eq!(fixture.placement(Palette::Sizes).height, collapsed);
            assert!(fixture.placement(Palette::Sizes).collapsed);
            assert_eq!(fixture.app.documents.active_session.document, artwork);
            assert!(fixture.app.workspace.dragging.is_none());
        }
    }

    #[test]
    fn adjacent_columns_keep_total_width_and_persist_weights_with_legacy_defaults()
     {
        let mut fixture = Fixture::new();
        fixture.app.workspace.left_width = 600.;
        fixture.app.workspace.dock_to(
            Palette::Properties,
            Palette::Brushes,
            super::super::DockDirection::Right,
        );
        fixture.frame(vec![]);
        fixture.frame(vec![]);
        let layout = fixture.app.workspace.dock_layout(
            Location::Left,
            Rect::from_min_size(Pos2::ZERO, egui::vec2(572., 1000.)),
        );
        let before = layout
            .iter()
            .find(|(palette, _)| *palette == Palette::Brushes)
            .unwrap()
            .1;
        let artwork = fixture.app.documents.active_session.document.clone();
        fixture.drag(
            "Resize between 보조 도구 and 도구 속성",
            egui::vec2(40., 0.),
            true,
        );
        let actual = fixture.app.workspace.dock_layout(
            Location::Left,
            Rect::from_min_size(Pos2::ZERO, egui::vec2(572., 1000.)),
        );
        let first = actual
            .iter()
            .find(|(palette, _)| *palette == Palette::Brushes)
            .unwrap()
            .1;
        let second = actual
            .iter()
            .find(|(palette, _)| *palette == Palette::Properties)
            .unwrap()
            .1;
        assert!((first.width() - before.width() - 40.).abs() < 1.);
        assert!((first.width() + second.width() - 572.).abs() < 0.1);
        assert!(first.height() < second.height());
        assert_eq!(second.height(), 1000.);
        assert_eq!(fixture.app.documents.active_session.document, artwork);
        fixture.drag(
            "Resize between 보조 도구 and 도구 속성",
            egui::vec2(1000., 0.),
            false,
        );
        let limited = fixture.app.workspace.dock_layout(
            Location::Left,
            Rect::from_min_size(Pos2::ZERO, egui::vec2(572., 1000.)),
        );
        assert_eq!(
            limited
                .iter()
                .find(|(palette, _)| *palette == Palette::Properties)
                .unwrap()
                .1
                .width(),
            180.
        );
        fixture.app.persist_preferences(true);
        let saved = parfait_storage::load_preferences(
            &fixture.directory.path().join("preferences.sqlite"),
        )
        .unwrap()
        .unwrap();
        let saved: crate::preferences::Preferences =
            serde_json::from_str(&saved).unwrap();
        assert!(saved.workspace.same_preferences(&fixture.app.workspace));
        let encoded = serde_json::to_value(&fixture.app.workspace).unwrap();
        let restored: Workspace =
            serde_json::from_value(encoded.clone()).unwrap();
        assert!(restored.same_preferences(&fixture.app.workspace));
        let mut legacy = encoded;
        for placement in legacy["panels"].as_array_mut().unwrap() {
            placement.as_object_mut().unwrap().remove("width_weight");
        }
        let legacy: Workspace = serde_json::from_value(legacy).unwrap();
        assert!(
            legacy.panels.iter().all(|placement| placement.width_weight == 1.)
        );
        assert!(legacy.validate());
        let original = fixture.app.workspace.clone();
        drop(fixture.app);
        let restored =
            Parfait::open_legacy(fixture.directory.path().to_owned()).unwrap();
        assert!(restored.workspace.same_preferences(&original));
    }

    #[test]
    fn dock_edges_and_floating_axes_resize_without_paint_or_unwanted_axis_changes()
     {
        let mut fixture = Fixture::new();
        let artwork = fixture.app.documents.active_session.document.clone();
        fixture.drag("Resize left dock", egui::vec2(65., 0.), true);
        assert_eq!(fixture.app.workspace.left_width, 325.);
        fixture.drag("Resize right dock", egui::vec2(-50., 0.), false);
        assert_eq!(fixture.app.workspace.right_width, 300.);
        let placement = fixture
            .app
            .workspace
            .panels
            .iter_mut()
            .find(|placement| placement.palette == Palette::Navigator)
            .unwrap();
        placement.location = Location::Floating;
        placement.rect = [500., 150., 270., 219.];
        fixture.drag("Resize 내비게이터 right", egui::vec2(90., 0.), false);
        assert_eq!(fixture.placement(Palette::Navigator).rect[2], 360.);
        assert_eq!(fixture.placement(Palette::Navigator).height, 190.);
        fixture.drag("Resize 내비게이터 left", egui::vec2(50., 0.), true);
        assert_eq!(fixture.placement(Palette::Navigator).rect[0], 550.);
        assert_eq!(fixture.placement(Palette::Navigator).rect[2], 310.);
        fixture.drag("Resize 내비게이터 bottom", egui::vec2(0., 40.), true);
        assert_eq!(fixture.placement(Palette::Navigator).height, 230.);
        fixture.drag("Resize 내비게이터 top", egui::vec2(0., -25.), false);
        assert_eq!(fixture.placement(Palette::Navigator).rect[1], 125.);
        assert_eq!(fixture.placement(Palette::Navigator).height, 255.);
        fixture.drag(
            "Resize 내비게이터 bottom right",
            egui::vec2(-1000., -1000.),
            true,
        );
        assert_eq!(fixture.placement(Palette::Navigator).rect[2], 180.);
        assert_eq!(fixture.placement(Palette::Navigator).height, 60.);
        assert_eq!(fixture.app.documents.active_session.document, artwork);
        assert!(fixture.app.workspace.dragging.is_none());
    }

    #[test]
    fn pointer_loss_stops_resize_and_limits_do_not_accumulate_drift() {
        let mut fixture = Fixture::new();
        fixture.frame(vec![]);
        fixture.frame(vec![]);
        let original = fixture.rect(Palette::Brushes).height() - 29.;
        let start = fixture.target("Resize row below 보조 도구").center();
        fixture
            .frame(vec![egui::Event::PointerMoved(start), button(start, true)]);
        fixture.frame(vec![egui::Event::PointerMoved(
            start + egui::vec2(0., 1000.),
        )]);
        assert_eq!(fixture.placement(Palette::Properties).height, 60.);
        fixture.frame(vec![egui::Event::PointerMoved(
            start + egui::vec2(0., 30.),
        )]);
        assert!(
            (fixture.placement(Palette::Brushes).height - original - 30.).abs()
                < 0.1
        );
        fixture.frame(vec![egui::Event::WindowFocused(false)]);
        fixture.frame(vec![
            egui::Event::PointerMoved(start + egui::vec2(0., 100.)),
            button(start, false),
        ]);
        assert!(
            (fixture.placement(Palette::Brushes).height - original - 30.).abs()
                < 0.1
        );
        assert!(!fixture.app.workspace.resizing);
        assert!(fixture.app.workspace.validate());
    }
}
