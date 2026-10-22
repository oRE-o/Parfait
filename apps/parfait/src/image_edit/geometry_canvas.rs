//! Geometric preview uses existing textures; dragging never edits document pixels.
use super::*;
use egui::{Color32, Pos2, Rect, Vec2};

fn exterior_color(document: &parfait_core::Document) -> Color32 {
    // Outside the original raster, only procedural fills and paper contribute.
    // Composite a single pixel through the existing folder/blend logic.
    let mut sample = parfait_core::Document::new(1, 1);
    sample.layers = document.layers.clone();
    let clear = std::sync::Arc::new(vec![0; 4]);
    for layer in &mut sample.layers {
        layer.pixels = clear.clone();
        if let Some(mask) = &mut layer.mask {
            mask.coverage = parfait_core::MaskCoverage::Uniform(255);
        }
    }
    let pixel = sample.composite();
    Color32::from_rgba_unmultiplied(pixel[0], pixel[1], pixel[2], pixel[3])
}

#[derive(Clone, Copy)]
pub(super) struct GeometryDrag {
    handle: [i32; 2],
    origin: Vec2,
    bounds: Rect,
}
impl EditDialog {
    fn geometry_bounds(&self) -> Rect {
        let size = egui::vec2(self.width as f32, self.height as f32);
        if self.command == Command::EditResolution {
            Rect::from_center_size(
                egui::pos2(
                    self.geometry.original[0] as f32 / 2.,
                    self.geometry.original[1] as f32 / 2.,
                ),
                size,
            )
        } else {
            Rect::from_min_size(
                egui::pos2(-self.offset[0] as f32, -self.offset[1] as f32),
                size,
            )
        }
    }
    fn drag_geometry(&mut self, drag: GeometryDrag, point: Vec2) {
        let delta = point - drag.origin;
        if self.command == Command::EditResolution {
            let mut size = drag.bounds.size();
            for axis in 0..2 {
                if drag.handle[axis] != 0 {
                    size[axis] = (size[axis]
                        + delta[axis] * drag.handle[axis] as f32 * 2.)
                        .clamp(1., 4096.);
                }
            }
            if self.geometry.linked {
                let ratio = self.geometry.original[1] as f32
                    / self.geometry.original[0] as f32;
                if drag.handle[0] != 0 {
                    size.y = (size.x * ratio).clamp(1., 4096.);
                    size.x = size.y / ratio;
                } else {
                    size.x = (size.y / ratio).clamp(1., 4096.);
                    size.y = size.x * ratio;
                }
            }
            self.width = size.x.round().clamp(1., 4096.) as u32;
            self.height = size.y.round().clamp(1., 4096.) as u32;
        } else {
            let mut bounds = drag.bounds;
            if drag.handle == [0, 0] {
                bounds = bounds.translate(delta);
            } else {
                for axis in 0..2 {
                    if drag.handle[axis] < 0 {
                        bounds.min[axis] =
                            (drag.bounds.min[axis] + delta[axis]).clamp(
                                bounds.max[axis] - 4096.,
                                bounds.max[axis] - 1.,
                            );
                    }
                    if drag.handle[axis] > 0 {
                        bounds.max[axis] =
                            (drag.bounds.max[axis] + delta[axis]).clamp(
                                bounds.min[axis] + 1.,
                                bounds.min[axis] + 4096.,
                            );
                    }
                }
            }
            self.width = bounds.width().round().clamp(1., 4096.) as u32;
            self.height = bounds.height().round().clamp(1., 4096.) as u32;
            self.offset = [
                (-bounds.left()).round().clamp(-4096., 4096.) as i32,
                (-bounds.top()).round().clamp(-4096., 4096.) as i32,
            ];
        }
    }
}
impl Parfait {
    pub(super) fn geometry_canvas_overlay(
        &self,
        ctx: &egui::Context,
        draft: &mut EditDialog,
    ) {
        if !matches!(
            draft.command,
            Command::EditResolution
                | Command::EditCanvasSize
                | Command::EditCanvasBasic
        ) || !draft.local_preview.enabled
            || draft.local_preview.compare
        {
            return;
        }
        let Some(viewport) = self.image_edit.canvas_rect else {
            return;
        };
        let Some(source) = self.render_cache.texture.as_ref() else {
            return;
        };
        let size = egui::vec2(
            draft.geometry.original[0] as f32,
            draft.geometry.original[1] as f32,
        );
        let view = &self.documents.active_session.view;
        let exterior = *draft.geometry.exterior.get_or_insert_with(|| {
            exterior_color(&self.documents.active_session.document)
        });
        let screen = |p: Pos2| view.screen(p.to_vec2(), viewport, size);
        if draft.geometry.checker.is_none() {
            let options = egui::TextureOptions {
                wrap_mode: egui::TextureWrapMode::Repeat,
                ..egui::TextureOptions::NEAREST
            };
            draft.geometry.checker = Some(ctx.load_texture(
                "extent_checker",
                egui::ColorImage::new(
                    [2, 2],
                    vec![
                        Color32::from_gray(235),
                        Color32::from_gray(218),
                        Color32::from_gray(218),
                        Color32::from_gray(235),
                    ],
                ),
                options,
            ));
        }
        egui::Area::new(egui::Id::new("canvas_geometry_overlay"))
            .order(egui::Order::Middle)
            .fixed_pos(viewport.min)
            .movable(false)
            .constrain(false)
            .fade_in(false)
            .show(ctx, |ui| {
                ui.set_min_size(viewport.size());
                ui.set_clip_rect(viewport);
                let bounds = draft.geometry_bounds();
                let points = [
                    bounds.left_top(),
                    bounds.right_top(),
                    bounds.right_bottom(),
                    bounds.left_bottom(),
                ]
                .map(screen);
                let painter = ui.painter();
                painter.rect_filled(
                    viewport,
                    0.,
                    Color32::from_black_alpha(145),
                );
                let mesh = |texture, rect: Rect, uv: Rect| {
                    let vertices = [
                        rect.left_top(),
                        rect.right_top(),
                        rect.right_bottom(),
                        rect.left_bottom(),
                    ];
                    let coords = [
                        uv.left_top(),
                        uv.right_top(),
                        uv.right_bottom(),
                        uv.left_bottom(),
                    ];
                    let mut mesh = egui::Mesh::with_texture(texture);
                    for i in 0..4 {
                        mesh.vertices.push(egui::epaint::Vertex {
                            pos: screen(vertices[i]),
                            uv: coords[i],
                            color: Color32::WHITE,
                        });
                    }
                    mesh.indices = vec![0, 1, 2, 0, 2, 3];
                    egui::Shape::mesh(mesh)
                };
                painter.add(mesh(
                    draft.geometry.checker.as_ref().unwrap().id(),
                    bounds,
                    Rect::from_min_size(Pos2::ZERO, bounds.size() / 32.),
                ));
                ui.painter().add(egui::Shape::convex_polygon(
                    points.to_vec(),
                    exterior,
                    egui::Stroke::NONE,
                ));
                let exact_texture =
                    draft.local_preview.texture.as_ref().filter(|_| {
                        draft.local_preview.cached.as_ref().is_some_and(
                            |(op, _)| {
                                Some(op) == draft.local_operation(self).as_ref()
                            },
                        )
                    });
                if draft.command == Command::EditResolution
                    || exact_texture.is_some()
                {
                    let texture = exact_texture.unwrap_or(source);
                    painter.add(mesh(
                        texture.id(),
                        bounds,
                        Rect::from_min_max(Pos2::ZERO, egui::pos2(1., 1.)),
                    ));
                } else {
                    let visible =
                        bounds.intersect(Rect::from_min_size(Pos2::ZERO, size));
                    if visible.is_positive() {
                        painter.add(mesh(
                            source.id(),
                            visible,
                            Rect::from_min_max(
                                egui::pos2(
                                    visible.left() / size.x,
                                    visible.top() / size.y,
                                ),
                                egui::pos2(
                                    visible.right() / size.x,
                                    visible.bottom() / size.y,
                                ),
                            ),
                        ));
                    }
                }
                painter.add(egui::Shape::closed_line(
                    points.to_vec(),
                    egui::Stroke::new(1., Color32::from_gray(180)),
                ));
                let mut handles = Vec::new();
                if draft.command != Command::EditResolution {
                    handles.push(([0, 0], bounds.center()));
                }
                for y in -1..=1 {
                    for x in -1..=1 {
                        if x != 0 || y != 0 {
                            handles.push((
                                [x, y],
                                Pos2::new(
                                    bounds.center().x
                                        + bounds.width() * x as f32 / 2.,
                                    bounds.center().y
                                        + bounds.height() * y as f32 / 2.,
                                ),
                            ));
                        }
                    }
                }
                for (handle, point) in handles {
                    let center = screen(point);
                    let hit = Rect::from_center_size(center, Vec2::splat(14.));
                    let response = ui.interact(
                        hit,
                        ui.id().with(handle),
                        egui::Sense::drag(),
                    );
                    let cursor = match handle {
                        [0, 0] => egui::CursorIcon::Move,
                        [0, _] => egui::CursorIcon::ResizeVertical,
                        [_, 0] => egui::CursorIcon::ResizeHorizontal,
                        [x, y] if x == y => egui::CursorIcon::ResizeNwSe,
                        _ => egui::CursorIcon::ResizeNeSw,
                    };
                    response.clone().on_hover_cursor(cursor);
                    response.widget_info(|| {
                        egui::WidgetInfo::labeled(
                            egui::WidgetType::Other,
                            true,
                            if handle == [0, 0] {
                                "캔버스 범위 이동"
                            } else {
                                "캔버스 크기 조절"
                            },
                        )
                    });
                    if handle == [0, 0] {
                        ui.painter().line_segment(
                            [
                                center - egui::vec2(5., 0.),
                                center + egui::vec2(5., 0.),
                            ],
                            egui::Stroke::new(1., Color32::WHITE),
                        );
                        ui.painter().line_segment(
                            [
                                center - egui::vec2(0., 5.),
                                center + egui::vec2(0., 5.),
                            ],
                            egui::Stroke::new(1., Color32::WHITE),
                        );
                    } else {
                        ui.painter().rect_filled(
                            Rect::from_center_size(center, Vec2::splat(8.)),
                            0.,
                            Color32::WHITE,
                        );
                        ui.painter().rect_stroke(
                            Rect::from_center_size(center, Vec2::splat(8.)),
                            0.,
                            egui::Stroke::new(1., Color32::GRAY),
                            egui::StrokeKind::Outside,
                        );
                    }
                    if response.drag_started()
                        && let Some(start) =
                            ui.input(|i| i.pointer.press_origin())
                    {
                        draft.geometry.drag = Some(GeometryDrag {
                            handle,
                            origin: view.document(start, viewport, size),
                            bounds,
                        });
                    }
                    #[cfg(test)]
                    ui.data_mut(|data| {
                        data.insert_temp(
                            egui::Id::new(("geometry_handle", handle)),
                            hit,
                        )
                    });
                }
                if let Some(drag) = draft.geometry.drag {
                    if ctx.input(|i| {
                        !i.focused
                            || i.key_pressed(egui::Key::Escape)
                            || i.pointer.hover_pos().is_none()
                    }) {
                        draft.geometry.drag = None;
                    } else {
                        if let Some(point) =
                            ctx.input(|i| i.pointer.interact_pos())
                        {
                            let before =
                                (draft.width, draft.height, draft.offset);
                            draft.drag_geometry(
                                drag,
                                view.document(point, viewport, size),
                            );
                            if before
                                != (draft.width, draft.height, draft.offset)
                            {
                                ctx.request_repaint();
                            }
                        }
                        if ctx.input(|i| !i.pointer.primary_down()) {
                            draft.geometry.drag = None;
                        }
                    }
                }
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn canvas_handles_update_geometry_and_cancel_preserves_pixels_and_history()
    {
        for paper in [false, true] {
            let mut document = if paper {
                parfait_core::Document::new_with_paper(4, 4)
            } else {
                parfait_core::Document::new(4, 4)
            };
            document
                .create_solid_layer([70, 150, 210], false, "Fill".into())
                .unwrap();
            document
                .layers
                .last_mut()
                .unwrap()
                .mask
                .as_mut()
                .unwrap()
                .coverage = parfait_core::MaskCoverage::Uniform(0);
            let outside = exterior_color(&document).to_srgba_unmultiplied();
            document.edit_canvas(CanvasEdit::Extent {
                width: 8,
                height: 8,
                offset: [2, 2],
            });
            assert_eq!(&document.composite()[..4], &outside);
        }
        for command in [Command::EditCanvasSize, Command::EditResolution] {
            let directory = tempfile::tempdir().unwrap();
            let mut app =
                Parfait::open_legacy(directory.path().to_owned()).unwrap();
            app.documents.active_session.document =
                parfait_core::Document::new(32, 24);
            let before = app.documents.active_session.document.clone();
            let ctx = egui::Context::default();
            crate::theme::apply(&ctx);
            let frame = |app: &mut Parfait, events| {
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(
                            Pos2::ZERO,
                            egui::vec2(1440., 1000.),
                        )),
                        events,
                        ..Default::default()
                    },
                    |ui| {
                        egui::CentralPanel::default()
                            .show(ui, |ui| app.canvas(ui));
                        app.image_edit_dialog_ui(ui.ctx());
                    },
                );
                output.textures_delta.clear();
            };
            frame(&mut app, vec![]);
            app.documents.active_session.view.zoom = 10.;
            app.documents.active_session.view.pan = egui::vec2(-300., -150.);
            app.image_edit.dialog = Some(EditDialog::new(&app, command));
            frame(&mut app, vec![]);
            frame(&mut app, vec![]);
            let accept = ctx
                .data(|data| {
                    data.get_temp::<Rect>(egui::Id::new("image_edit_accept"))
                })
                .unwrap();
            assert_eq!(
                ctx.layer_id_at(accept.center()),
                Some(egui::LayerId::new(
                    egui::Order::Middle,
                    egui::Id::new(("canvas_edit_window", command))
                ))
            );
            let start = ctx
                .data(|data| {
                    data.get_temp::<Rect>(egui::Id::new((
                        "geometry_handle",
                        [-1, -1],
                    )))
                })
                .unwrap()
                .center();
            let button = |position, pressed| egui::Event::PointerButton {
                pos: position,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Default::default(),
            };
            frame(
                &mut app,
                vec![egui::Event::PointerMoved(start), button(start, true)],
            );
            frame(
                &mut app,
                vec![egui::Event::PointerMoved(start - egui::vec2(40., 30.))],
            );
            frame(&mut app, vec![button(start - egui::vec2(40., 30.), false)]);
            let draft = app.image_edit.dialog.as_ref().unwrap();
            if command == Command::EditCanvasSize {
                assert_eq!([draft.width, draft.height], [36, 27]);
                assert_eq!(draft.offset, [4, 3]);
            } else {
                assert_eq!([draft.width, draft.height], [40, 30]);
            }
            assert!(
                app.documents.active_session.document.same_edit_state(&before)
            );
            assert!(!app.documents.active_session.history.can_undo());
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
            assert!(app.image_edit.dialog.is_none());
            assert!(
                app.documents.active_session.document.same_edit_state(&before)
            );
        }
    }
}
