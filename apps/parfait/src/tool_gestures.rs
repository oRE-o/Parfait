//! Canvas gestures dispatch to the same document/history operations as menus.
use crate::{
    app::Parfait,
    tools::{ToolDrag, ToolKind as K},
};
use eframe::egui::{self, Rect, Vec2};
use parfait_core::{
    BrushOperation, MaskCoverage, Point, SampleSource, SelectionMode, Stroke,
};
use std::sync::Arc;
impl Parfait {
    pub(crate) fn tool_pointer_down(
        &mut self,
        p: Vec2,
        modifiers: egui::Modifiers,
    ) -> bool {
        let Some(kind) = self.active_tool() else {
            return false;
        };
        if matches!(
            kind,
            K::Hand | K::Rotate | K::ZoomIn | K::ZoomOut | K::Picker
        ) {
            return false;
        }
        let doc = &self.documents.active_session.document;
        if kind == K::PickerLayer {
            let i = (p.y as u32 * doc.width + p.x as u32) as usize;
            self.brush
                .color
                .copy_from_slice(&doc.layers[doc.active].source_pixel(i)[..3]);
            return true;
        }
        if kind == K::Text {
            self.execute_native_command(
                crate::shortcuts::Command::NewTextLayer,
            );
            if let Some(d) = &mut self.native_dialog {
                let mut body = d.body.as_ref().clone();
                if let parfait_core::LayerContent::Text(t) = &mut body {
                    t.matrix = parfait_core::Affine::translation(p.x, p.y);
                    t.size = self.tools.size;
                }
                d.body = Arc::new(body);
            }
            return true;
        }
        if kind == K::Object
            && let Some(i) = doc.layer_at(p.x as u32, p.y as u32)
        {
            self.select_layer(i);
        }
        let mode = if modifiers.shift && modifiers.alt {
            SelectionMode::Intersect
        } else if modifiers.shift {
            SelectionMode::Add
        } else if modifiers.alt {
            SelectionMode::Subtract
        } else {
            match self.tools.selection_mode {
                1 => SelectionMode::Add,
                2 => SelectionMode::Subtract,
                3 => SelectionMode::Intersect,
                _ => SelectionMode::Replace,
            }
        };
        self.tools.drag = Some(ToolDrag {
            owner: self.documents.active_session.key,
            revision: self.documents.active_session.document.revision,
            kind,
            start: p,
            end: p,
            points: vec![[p.x, p.y]],
            mode,
        });
        true
    }
    pub(crate) fn tool_pointer_move(&mut self, p: Vec2, shift: bool) {
        if let Some(d) = &mut self.tools.drag {
            d.end = p;
            if shift && !matches!(d.kind, K::Lasso | K::Object | K::MoveLayer) {
                let delta = p - d.start;
                if d.kind == K::Line {
                    let angle = (delta.angle() / std::f32::consts::FRAC_PI_4)
                        .round()
                        * std::f32::consts::FRAC_PI_4;
                    d.end = d.start + Vec2::angled(angle) * delta.length();
                } else {
                    let side = delta.x.abs().max(delta.y.abs());
                    d.end = d.start
                        + egui::vec2(
                            delta.x.signum() * side,
                            delta.y.signum() * side,
                        );
                }
            }
            if d.kind == K::Lasso
                && d.points.len() < 4096
                && (p - Vec2::from(*d.points.last().unwrap())).length() >= 1.
            {
                d.points.push([p.x, p.y]);
            }
        }
    }
    pub(crate) fn tool_pointer_up(&mut self, p: Vec2, shift: bool) {
        self.tool_pointer_move(p, shift);
        let Some(d) = self.tools.drag.take() else {
            return;
        };
        if d.owner != self.documents.active_session.key
            || d.revision != self.documents.active_session.document.revision
        {
            return;
        }
        let doc = &self.documents.active_session.document;
        let count = u64::from(doc.width) * u64::from(doc.height);
        // Bound temporary coverage, composited reference, output and undo headroom.
        if !self.reserve_pixel_headroom(count * 14 + 1024 * 1024) {
            self.notice = Some("보조 도구 처리 메모리가 부족합니다.".into());
            return;
        }
        let old_revision = self.documents.active_session.document.revision;
        if matches!(
            d.kind,
            K::AutoAll
                | K::AutoLayer
                | K::AutoReference
                | K::FillAll
                | K::FillLayer
                | K::FillReference
        ) {
            let source = match d.kind {
                K::AutoLayer | K::FillLayer => SampleSource::Active,
                K::AutoReference | K::FillReference => SampleSource::Reference,
                _ => SampleSource::Composite,
            };
            let doc = &self.documents.active_session.document;
            let Some(coverage) = doc.region_at(
                d.start.x as u32,
                d.start.y as u32,
                self.tools.tolerance,
                self.tools.contiguous,
                source,
            ) else {
                self.notice = Some("참조할 레이어가 없습니다.".into());
                return;
            };
            if d.kind.is_selection() {
                self.edit_named("자동 선택", |doc| {
                    doc.combine_selection(coverage, d.mode);
                });
                self.retain_selection_render(old_revision);
            } else {
                self.paint_tool_result(Some(coverage), None);
            }
        } else if d.kind.is_selection() {
            let doc = &self.documents.active_session.document;
            let (width, height) = (doc.width, doc.height);
            let bounds = if d.kind == K::Lasso {
                d.points.iter().fold(Rect::NOTHING, |r, p| {
                    r.union(Rect::from_min_size(
                        egui::pos2(p[0], p[1]),
                        Vec2::ZERO,
                    ))
                })
            } else {
                Rect::from_two_pos(d.start.to_pos2(), d.end.to_pos2())
            };
            let center = bounds.center();
            let radius = bounds.size() * 0.5;
            let mut values = vec![0; (width * height) as usize];
            if d.kind != K::Lasso || d.points.len() >= 3 {
                for y in bounds.top().floor().max(0.) as u32
                    ..bounds.bottom().ceil().min(height as f32) as u32
                {
                    for x in bounds.left().floor().max(0.) as u32
                        ..bounds.right().ceil().min(width as f32) as u32
                    {
                        let p = egui::vec2(x as f32 + 0.5, y as f32 + 0.5);
                        let inside = match d.kind {
                            K::EllipseSelect => {
                                radius.x > 0.
                                    && radius.y > 0.
                                    && ((p - center.to_vec2()) / radius)
                                        .length_sq()
                                        <= 1.
                            }
                            K::Lasso => polygon_contains(&d.points, p),
                            _ => true,
                        };
                        if inside {
                            values[(y * width + x) as usize] = 255;
                        }
                    }
                }
                self.edit_named("선택 범위", |doc| {
                    doc.combine_selection(
                        MaskCoverage::Dense(Arc::new(values)),
                        d.mode,
                    );
                });
                self.retain_selection_render(old_revision);
            }
        } else if matches!(d.kind, K::Object | K::MoveLayer) {
            let delta = d.end - d.start;
            let delta = [delta.x.round() as i32, delta.y.round() as i32];
            let doc = &self.documents.active_session.document;
            let active = doc.active;
            let id = doc.layers[active].id;
            if !doc.layer_ancestors_unlocked(active) {
                self.notice = Some("레이어 잠금을 해제해 주세요.".into());
                return;
            }
            if let Some(node) = doc.native_content(active) {
                if let Ok(node) =
                    node.transformed(parfait_core::Affine::translation(
                        delta[0] as f32,
                        delta[1] as f32,
                    ))
                {
                    let body = node.body().as_ref().clone();
                    self.edit_named("오브젝트 이동", |doc| {
                        let _ = doc.replace_content(active, body);
                    });
                }
            } else {
                self.edit_named("레이어 이동", |doc| {
                    doc.translate_layer_pixels(&[(id, delta)]);
                });
            }
        } else if matches!(d.kind, K::LinearGradient | K::RadialGradient) {
            self.paint_tool_result(
                None,
                Some((
                    [d.start.x, d.start.y],
                    [d.end.x, d.end.y],
                    d.kind == K::RadialGradient,
                )),
            );
        } else if matches!(d.kind, K::Line | K::Rectangle | K::Ellipse) {
            if !self.tool_raster_allowed() {
                return;
            }
            let mut points = vec![[d.start.x, d.start.y]];
            let b = Rect::from_two_pos(d.start.to_pos2(), d.end.to_pos2());
            if d.kind == K::Line {
                points.push([d.end.x, d.end.y]);
            } else if d.kind == K::Rectangle {
                points = vec![
                    [b.left(), b.top()],
                    [b.right(), b.top()],
                    [b.right(), b.bottom()],
                    [b.left(), b.bottom()],
                    [b.left(), b.top()],
                ];
            } else {
                points = (0..=128)
                    .map(|i| {
                        let a = i as f32 * std::f32::consts::TAU / 128.;
                        [
                            b.center().x + a.cos() * b.width() * 0.5,
                            b.center().y + a.sin() * b.height() * 0.5,
                        ]
                    })
                    .collect();
            }
            if self.tools.shape_fill && d.kind != K::Line {
                let doc = &self.documents.active_session.document;
                let mut values = vec![0; (doc.width * doc.height) as usize];
                for y in b.top().max(0.) as u32
                    ..b.bottom().ceil().min(doc.height as f32) as u32
                {
                    for x in b.left().max(0.) as u32
                        ..b.right().ceil().min(doc.width as f32) as u32
                    {
                        if polygon_contains(
                            &points,
                            egui::vec2(x as f32 + 0.5, y as f32 + 0.5),
                        ) {
                            values[(y * doc.width + x) as usize] = 255;
                        }
                    }
                }
                self.paint_tool_result(
                    Some(MaskCoverage::Dense(Arc::new(values))),
                    None,
                );
            } else {
                let mut brush = self.brush.clone();
                brush.size = self.tools.size;
                brush.opacity = self.tools.opacity;
                brush.pressure = false;
                brush.pressure_opacity = false;
                brush.smoothing = 0.;
                brush.spacing = 0.05;
                brush.operation = BrushOperation::Paint;
                brush.erase = self.tools.color_slot == 2;
                self.edit_named("도형 그리기", |doc| {
                    let first = points[0];
                    let mut stroke = Stroke::begin(
                        doc,
                        brush,
                        Point { x: first[0], y: first[1], pressure: 1. },
                    );
                    for p in &points[1..] {
                        stroke.extend(
                            doc,
                            Point { x: p[0], y: p[1], pressure: 1. },
                        );
                    }
                });
            }
        }
    }
    fn tool_raster_allowed(&mut self) -> bool {
        let doc = &self.documents.active_session.document;
        if self.editing_layer_mask() {
            self.notice =
                Some("마스크에는 브러시와 지우개를 사용해 주세요.".into());
            return false;
        }
        if doc.native_content(doc.active).is_some()
            || doc.layers[doc.active].kind != parfait_core::LayerKind::Raster
            || !doc.layer_ancestors_unlocked(doc.active)
            || !doc.layers[doc.active].visible
        {
            self.notice =
                Some("보이는 잠금 해제 래스터 레이어를 선택해 주세요.".into());
            false
        } else {
            true
        }
    }
    fn paint_tool_result(
        &mut self,
        coverage: Option<MaskCoverage>,
        gradient: Option<([f32; 2], [f32; 2], bool)>,
    ) {
        if !self.tool_raster_allowed() {
            return;
        }
        let color = self.brush.color;
        let end = Some(self.tools.secondary);
        let opacity = self.tools.opacity;
        let erase = self.tools.color_slot == 2;
        self.edit_named(
            if gradient.is_some() { "그라데이션" } else { "채우기" },
            |doc| {
                doc.paint_tool(
                    coverage.as_ref(),
                    color,
                    end,
                    gradient,
                    opacity,
                    erase,
                );
            },
        );
    }
    pub(crate) fn paint_tool_gesture(
        &self,
        ui: &egui::Ui,
        rect: Rect,
        size: Vec2,
    ) {
        let Some(d) = &self.tools.drag else {
            return;
        };
        if d.owner != self.documents.active_session.key {
            return;
        }
        let view = &self.documents.active_session.view;
        let mut points = if d.kind == K::Lasso {
            d.points.clone()
        } else if matches!(
            d.kind,
            K::Line
                | K::LinearGradient
                | K::RadialGradient
                | K::MoveLayer
                | K::Object
        ) {
            vec![[d.start.x, d.start.y], [d.end.x, d.end.y]]
        } else {
            let b = Rect::from_two_pos(d.start.to_pos2(), d.end.to_pos2());
            if matches!(d.kind, K::Ellipse | K::EllipseSelect) {
                (0..=96)
                    .map(|i| {
                        let a = i as f32 * std::f32::consts::TAU / 96.;
                        [
                            b.center().x + a.cos() * b.width() * 0.5,
                            b.center().y + a.sin() * b.height() * 0.5,
                        ]
                    })
                    .collect()
            } else {
                vec![
                    [b.left(), b.top()],
                    [b.right(), b.top()],
                    [b.right(), b.bottom()],
                    [b.left(), b.bottom()],
                    [b.left(), b.top()],
                ]
            }
        };
        if d.kind == K::Lasso {
            points.push([d.end.x, d.end.y]);
        }
        let points: Vec<_> = points
            .into_iter()
            .map(|p| view.screen(Vec2::from(p), rect, size))
            .collect();
        ui.painter().with_clip_rect(rect).add(egui::Shape::line(
            points,
            egui::Stroke::new(1.5, egui::Color32::from_rgb(180, 200, 235)),
        ));
    }
}
fn polygon_contains(points: &[[f32; 2]], p: Vec2) -> bool {
    if points.len() < 3 {
        return false;
    }
    let mut inside = false;
    let mut previous = points[points.len() - 1];
    for current in points {
        if (current[1] > p.y) != (previous[1] > p.y)
            && p.x
                < (previous[0] - current[0]) * (p.y - current[1])
                    / (previous[1] - current[1])
                    + current[0]
        {
            inside = !inside;
        }
        previous = *current;
    }
    inside
}
