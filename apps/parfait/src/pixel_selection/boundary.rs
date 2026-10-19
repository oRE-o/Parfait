use crate::canvas::View;
use eframe::egui::{self, Color32, Pos2, Rect, Vec2};
use parfait_core::{Document, MaskCoverage};
use std::{sync::Arc, time::Duration};

const MAX_CELLS: f32 = 262144.;
const MAX_QUADS: usize = 16384;
const DASH: f32 = 4.;

#[derive(Clone, Copy, PartialEq)]
struct GeometryKey {
    rect: Rect,
    dimensions: [u32; 2],
    zoom: f32,
    angle: f32,
    pan: Vec2,
    flips: [bool; 2],
    pixels_per_point: f32,
}

#[derive(Default)]
pub(crate) struct BoundaryCache {
    source: Option<MaskCoverage>,
    key: Option<GeometryKey>,
    meshes: [Arc<egui::Mesh>; 2],
    step: u32,
    #[cfg(test)]
    pub(super) rebuilds: usize,
}

struct Segment {
    start: Pos2,
    end: Pos2,
    phase: f32,
}

struct Contours<'a> {
    document: &'a Document,
    view: &'a View,
    viewport: Rect,
    size: Vec2,
    first: [i32; 2],
    last: [i32; 2],
    step: i32,
    segments: Vec<Segment>,
    quads: usize,
}

fn same_source(first: &Option<MaskCoverage>, second: &Option<MaskCoverage>) -> bool {
    match (first, second) {
        (None, None) => true,
        (Some(MaskCoverage::Uniform(first)), Some(MaskCoverage::Uniform(second))) => {
            first == second
        }
        (Some(MaskCoverage::Dense(first)), Some(MaskCoverage::Dense(second))) => {
            Arc::ptr_eq(first, second)
        }
        _ => false,
    }
}

fn clipped(start: Pos2, end: Pos2, rect: Rect) -> Option<(Pos2, Pos2, f32)> {
    let direction = end - start;
    let mut low = 0_f32;
    let mut high = 1_f32;
    for (origin, delta, min, max) in [
        (start.x, direction.x, rect.left(), rect.right()),
        (start.y, direction.y, rect.top(), rect.bottom()),
    ] {
        if delta.abs() < f32::EPSILON {
            if origin < min || origin > max {
                return None;
            }
        } else {
            let first = (min - origin) / delta;
            let last = (max - origin) / delta;
            low = low.max(first.min(last));
            high = high.min(first.max(last));
            if low >= high {
                return None;
            }
        }
    }
    Some((
        start + direction * low,
        start + direction * high,
        direction.length() * low,
    ))
}

impl Contours<'_> {
    fn selected(&self, column: i32, row: i32) -> bool {
        if column < 0
            || row < 0
            || column >= self.document.width as i32
            || row >= self.document.height as i32
        {
            return false;
        }
        let column = (column + self.step / 2).min(self.document.width as i32 - 1) as usize;
        let row = (row + self.step / 2).min(self.document.height as i32 - 1) as usize;
        self.document
            .selection_value(row * self.document.width as usize + column)
            >= 128
    }

    fn segment(&mut self, horizontal: bool, major: i32, first: i32, last: i32) -> bool {
        let limit = if horizontal {
            self.document.width
        } else {
            self.document.height
        } as i32;
        let cross_limit = if horizontal {
            self.document.height
        } else {
            self.document.width
        } as i32;
        let first = first.clamp(0, limit) as f32;
        let last = last.clamp(0, limit) as f32;
        let major = major.clamp(0, cross_limit) as f32;
        if first >= last {
            return true;
        }
        let points = if horizontal {
            [egui::vec2(first, major), egui::vec2(last, major)]
        } else {
            [egui::vec2(major, first), egui::vec2(major, last)]
        };
        let start = self.view.screen(points[0], self.viewport, self.size);
        let end = self.view.screen(points[1], self.viewport, self.size);
        let Some((start, end, offset)) = clipped(start, end, self.viewport) else {
            return true;
        };
        let length = start.distance(end);
        let phase = first * self.view.zoom + offset;
        let count = ((length + phase.rem_euclid(DASH)) / DASH).ceil() as usize + 1;
        if self.quads.saturating_add(count) > MAX_QUADS {
            return false;
        }
        self.quads += count;
        self.segments.push(Segment { start, end, phase });
        true
    }

    fn trace(&mut self, horizontal: bool) -> bool {
        let (major_axis, minor_axis) = if horizontal { (1, 0) } else { (0, 1) };
        for major in (self.first[major_axis]..=self.last[major_axis]).step_by(self.step as usize) {
            let mut run = None;
            for minor in (self.first[minor_axis]..self.last[minor_axis]).step_by(self.step as usize)
            {
                let (column, row) = if horizontal {
                    (minor, major)
                } else {
                    (major, minor)
                };
                let previous = if horizontal {
                    self.selected(column, row - self.step)
                } else {
                    self.selected(column - self.step, row)
                };
                let edge = self.selected(column, row) != previous;
                if edge {
                    run.get_or_insert(minor);
                } else if let Some(first) = run.take()
                    && !self.segment(horizontal, major, first, minor)
                {
                    return false;
                }
            }
            if let Some(first) = run
                && !self.segment(horizontal, major, first, self.last[minor_axis])
            {
                return false;
            }
        }
        true
    }

    fn meshes(self, pixels_per_point: f32) -> [Arc<egui::Mesh>; 2] {
        let mut meshes: [egui::Mesh; 2] = std::array::from_fn(|_| egui::Mesh {
            vertices: Vec::with_capacity(self.quads * 4),
            indices: Vec::with_capacity(self.quads * 6),
            ..Default::default()
        });
        for segment in self.segments {
            let length = segment.start.distance(segment.end);
            if length <= 0. {
                continue;
            }
            let direction = (segment.end - segment.start) / length;
            let normal = egui::vec2(-direction.y, direction.x) * 0.5 / pixels_per_point;
            let initial = DASH - segment.phase.rem_euclid(DASH);
            let count = ((length + segment.phase.rem_euclid(DASH)) / DASH).ceil() as usize;
            let initial_parity = (segment.phase / DASH).floor() as usize;
            for dash in 0..count {
                let offset = if dash == 0 {
                    0.
                } else {
                    initial + (dash - 1) as f32 * DASH
                };
                let next = (initial + dash as f32 * DASH).min(length);
                if next <= offset {
                    continue;
                }
                let start = segment.start + direction * offset;
                let end = segment.start + direction * next;
                let parity = (initial_parity + dash) & 1;
                for (phase, mesh) in meshes.iter_mut().enumerate() {
                    let color = if phase == parity {
                        Color32::BLACK
                    } else {
                        Color32::WHITE
                    };
                    let index = mesh.vertices.len() as u32;
                    for pos in [start - normal, start + normal, end + normal, end - normal] {
                        mesh.vertices.push(egui::epaint::Vertex {
                            pos,
                            uv: egui::epaint::WHITE_UV,
                            color,
                        });
                    }
                    mesh.indices
                        .extend([index, index + 1, index + 2, index, index + 2, index + 3]);
                }
            }
        }
        meshes.map(Arc::new)
    }
}

impl BoundaryCache {
    fn rebuild(&mut self, document: &Document, view: &View, key: GeometryKey) {
        self.meshes = Default::default();
        self.step = 1;
        let rect = key.rect;
        if document.selection.is_none()
            || !view.zoom.is_finite()
            || !(0.05..=12.).contains(&view.zoom)
            || !view.angle.is_finite()
            || !view.pan.is_finite()
            || !rect.is_finite()
            || !rect.is_positive()
            || !key.pixels_per_point.is_finite()
            || key.pixels_per_point <= 0.
            || !(1..=4096).contains(&document.width)
            || !(1..=4096).contains(&document.height)
        {
            return;
        }
        if matches!(document.selection, Some(MaskCoverage::Uniform(0..=127))) {
            return;
        }
        let size = egui::vec2(document.width as f32, document.height as f32);
        let mut bounds = Rect::NOTHING;
        for point in [
            rect.left_top(),
            rect.right_top(),
            rect.right_bottom(),
            rect.left_bottom(),
        ] {
            let point = view.document(point, rect, size).to_pos2();
            bounds.extend_with(point);
        }
        if !bounds.is_finite() {
            return;
        }
        let extent = bounds.size().min(size) + Vec2::splat(4.);
        let mut step = 1_u32;
        while (step as f32) * view.zoom < 1.
            || (extent.x / step as f32 + 4.) * (extent.y / step as f32 + 4.) > MAX_CELLS
        {
            step *= 2;
        }
        let overlap = bounds.intersect(Rect::from_min_size(Pos2::ZERO, size));
        if !overlap.is_positive() {
            return;
        }
        loop {
            let first = [overlap.left(), overlap.top()]
                .map(|value| ((value / step as f32).floor() as i32 - 1) * step as i32);
            let last = [overlap.right(), overlap.bottom()]
                .map(|value| ((value / step as f32).ceil() as i32 + 1) * step as i32);
            let mut contours = Contours {
                document,
                view,
                viewport: rect,
                size,
                first,
                last,
                step: step as i32,
                segments: Vec::new(),
                quads: 0,
            };
            if contours.trace(true) && contours.trace(false) {
                self.meshes = contours.meshes(key.pixels_per_point);
                self.step = step;
                break;
            }
            step *= 2;
            if step > 8192 {
                self.step = step;
                break;
            }
        }
    }

    pub(crate) fn paint(&mut self, ui: &egui::Ui, document: &Document, view: &View, rect: Rect) {
        let key = GeometryKey {
            rect,
            dimensions: [document.width, document.height],
            zoom: view.zoom,
            angle: view.angle,
            pan: view.pan,
            flips: [view.flip_horizontal, view.flip_vertical],
            pixels_per_point: ui.ctx().pixels_per_point(),
        };
        if self.key != Some(key) || !same_source(&self.source, &document.selection) {
            self.rebuild(document, view, key);
            self.source = document.selection.clone();
            self.key = Some(key);
            #[cfg(test)]
            {
                self.rebuilds += 1;
            }
        }
        if document.selection.is_none() {
            return;
        }
        let phase = ui.input(|input| (input.time * 5.).floor() as usize % 2);
        let painter = ui.painter().with_clip_rect(rect);
        if !self.meshes[phase].vertices.is_empty() {
            painter.add(egui::Shape::Mesh(self.meshes[phase].clone()));
            ui.ctx().request_repaint_after(Duration::from_millis(200));
        }
        let text = if matches!(document.selection, Some(MaskCoverage::Uniform(0))) {
            "빈 선택 영역 — 선택 해제로 다시 그릴 수 있습니다"
        } else if self.step > 1 {
            "선택 영역 — 경계선 간략 표시"
        } else {
            "선택 영역"
        };
        let position = rect.left_top() + egui::vec2(8., 8.);
        let galley =
            painter.layout_no_wrap(text.into(), egui::FontId::proportional(11.), Color32::WHITE);
        painter.rect_filled(
            Rect::from_min_size(
                position - egui::vec2(4., 2.),
                galley.size() + egui::vec2(8., 4.),
            ),
            2.,
            Color32::from_black_alpha(170),
        );
        painter.galley(position, galley, Color32::WHITE);
    }
}

#[cfg(test)]
mod tests;
