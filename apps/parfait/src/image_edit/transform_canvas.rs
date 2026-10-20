use super::*;
use parfait_core::{Document, PixelRect, RasterTransform, WarpKind, WarpPin};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Duration,
};
#[cfg(test)]
mod tests;
#[derive(Default)]
pub(super) struct TransformCanvas {
    bounds: Option<PixelRect>,
    drag: Option<Drag>,
    pub(super) selected: usize,
    pub(super) selected_nodes: Vec<usize>,
    texture: Option<egui::TextureHandle>,
    cached: Option<(RasterTransform, Document)>,
    requested: Option<RasterTransform>,
    shown: Option<RasterTransform>,
}
#[derive(Clone, Copy)]
enum Handle {
    Move,
    Scale([i8; 2]),
    Rotate,
    Corner(usize),
    Shear(usize, f32),
    Node(usize),
    Pin(usize),
    PinRotate(usize),
}
struct Drag {
    handle: Handle,
    start: egui::Vec2,
    base: RasterTransform,
}
pub(super) struct PreviewJob {
    owner: SessionKey,
    revision: u64,
    transform: RasterTransform,
    receiver: mpsc::Receiver<Result<PreviewOutput, String>>,
    cancel: Arc<AtomicBool>,
    reservation: u64,
    pub source: Document,
}
struct PreviewOutput {
    document: Document,
    pixels: Vec<u8>,
}
impl EditDialog {
    pub(super) fn cached_transform(&self) -> Option<&Document> {
        self.transform_canvas
            .cached
            .as_ref()
            .filter(|(t, _)| *t == self.transform)
            .map(|(_, doc)| doc)
    }
    pub(super) fn transform_reason(
        &self,
        doc: &Document,
    ) -> Option<&'static str> {
        if self.mask_target {
            doc.mask_transform_reason(doc.active, &self.transform)
        } else {
            doc.transform_reason(doc.active, &self.transform)
        }
    }
    pub(super) fn apply_transform(&self, doc: &mut Document) -> bool {
        if self.mask_target {
            doc.transform_layer_mask(doc.active, &self.transform)
        } else {
            doc.transform_layer_pixels(doc.active, &self.transform)
        }
    }
}
impl Parfait {
    pub(crate) fn transform_cached_result(&self) -> Option<&Document> {
        self.image_edit
            .dialog
            .as_ref()?
            .transform_canvas
            .cached
            .as_ref()
            .map(|(_, doc)| doc)
    }
    pub(crate) fn transform_preview_source(&self) -> Option<&Document> {
        self.image_edit.preview_job.as_ref().map(|j| &j.source)
    }
    pub(crate) fn transform_reserved_bytes(&self) -> u64 {
        self.image_edit.preview_job.as_ref().map_or(0, |j| j.reservation)
    }
    pub(crate) fn cancel_transform_preview(&self) {
        if let Some(job) = &self.image_edit.preview_job {
            job.cancel.store(true, Ordering::Release);
        }
    }
    pub(crate) fn transform_canvas_texture(
        &self,
    ) -> Option<&egui::TextureHandle> {
        let draft = self.image_edit.dialog.as_ref()?;
        if !draft.command.is_raster_transform()
            || self.documents.active_key() != Some(draft.owner)
            || self.documents.active_session.document.revision != draft.revision
        {
            return None;
        }
        draft.transform_canvas.texture.as_ref()
    }
    pub(crate) fn poll_transform_preview(&mut self, ctx: &egui::Context) {
        let Some(job) = &self.image_edit.preview_job else {
            return;
        };
        let result = match job.receiver.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => {
                ctx.request_repaint_after(Duration::from_millis(16));
                return;
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                Err("변형 미리보기 작업이 종료되었습니다.".into())
            }
        };
        let job = self.image_edit.preview_job.take().unwrap();
        if job.cancel.load(Ordering::Acquire) {
            if let Some(draft) = &mut self.image_edit.dialog {
                draft.transform_canvas.requested = None;
            }
            return;
        }
        let Some(draft) = &mut self.image_edit.dialog else {
            return;
        };
        if draft.owner != job.owner
            || draft.revision != job.revision
            || draft.transform != job.transform
        {
            return;
        }
        match result {
            Ok(PreviewOutput { document, mut pixels }) => {
                let w = job.source.width as usize;
                for (i, p) in
                    pixels.as_chunks_mut::<4>().0.iter_mut().enumerate()
                {
                    let bg = if (i % w / 16 + i / w / 16).is_multiple_of(2) {
                        235.
                    } else {
                        218.
                    };
                    let a = f32::from(p[3]) / 255.;
                    for c in &mut p[..3] {
                        *c = (f32::from(*c) * a + bg * (1. - a)).round() as u8;
                    }
                    p[3] = 255;
                }
                draft.transform_canvas.texture = Some(ctx.load_texture(
                    "transform_canvas_preview",
                    egui::ColorImage::from_rgba_unmultiplied(
                        [job.source.width as usize, job.source.height as usize],
                        &pixels,
                    ),
                    egui::TextureOptions::LINEAR,
                ));
                draft.transform_canvas.cached =
                    Some((job.transform.clone(), document));
                draft.transform_canvas.shown = Some(job.transform);
                draft.error = None;
            }
            Err(error) => {
                draft.error = Some(error);
                draft.transform_canvas.requested = Some(job.transform);
            }
        }
    }
    fn request_transform_preview(
        &mut self,
        draft: &mut EditDialog,
        ctx: &egui::Context,
    ) {
        if self.image_edit.preview_job.is_some()
            || draft.transform_canvas.requested.as_ref()
                == Some(&draft.transform)
        {
            return;
        }
        let doc = &self.documents.active_session.document;
        if let Some(reason) = draft.transform_reason(doc) {
            draft.error = Some(reason.into());
            return;
        }
        let reservation = u64::from(doc.width) * u64::from(doc.height) * 20
            + 32 * 1024 * 1024;
        let carried = draft
            .transform_canvas
            .cached
            .as_ref()
            .map_or(0, |(_, cached)| new_buffers(doc, cached));
        if !self.can_reserve_pixel_headroom(reservation.saturating_add(carried))
        {
            draft.error=Some("미리보기를 위한 메모리가 부족합니다. 확인으로 적용하거나 크기를 줄여 주세요.".into());
            draft.transform_canvas.requested = Some(draft.transform.clone());
            return;
        }
        let source = self.documents.active_session.document.clone();
        let mut work = source.clone();
        let transform = draft.transform.clone();
        let t = transform.clone();
        let mask = draft.mask_target;
        let cancel = Arc::new(AtomicBool::new(false));
        let stop = cancel.clone();
        let context = ctx.clone();
        let (sender, receiver) = mpsc::sync_channel(1);
        let spawned = std::thread::Builder::new()
            .name("parfait-transform-preview".into())
            .spawn(move || {
                if mask {
                    work.transform_layer_mask_cancellable(
                        work.active,
                        &t,
                        || stop.load(Ordering::Acquire),
                    );
                } else {
                    work.transform_layer_pixels_cancellable(
                        work.active,
                        &t,
                        || stop.load(Ordering::Acquire),
                    );
                }
                if !stop.load(Ordering::Acquire) {
                    let pixels = work.composite();
                    if !stop.load(Ordering::Acquire) {
                        let _ = sender
                            .send(Ok(PreviewOutput { document: work, pixels }));
                    }
                }
                context.request_repaint();
            });
        match spawned {
            Ok(_) => {
                draft.transform_canvas.requested = Some(transform.clone());
                self.image_edit.preview_job = Some(PreviewJob {
                    owner: draft.owner,
                    revision: draft.revision,
                    transform,
                    receiver,
                    cancel,
                    reservation,
                    source,
                });
            }
            Err(_) => {
                draft.error =
                    Some("미리보기 작업을 시작하지 못했습니다.".into())
            }
        }
    }
    pub(super) fn transform_window(
        &mut self,
        ctx: &egui::Context,
        mut draft: EditDialog,
    ) {
        let doc = &self.documents.active_session.document;
        if draft.transform_canvas.bounds.is_none() {
            draft.transform_canvas.bounds = if draft.mask_target {
                doc.mask_transform_bounds(doc.active)
            } else {
                doc.transform_bounds(doc.active)
            };
        }
        self.transform_handles(ctx, &mut draft);
        let mut confirm = false;
        let mut cancel = false;
        let mut open = true;
        let before = draft.transform.clone();
        crate::editor_window::window(ctx,draft.command,315.).open(&mut open)
            .show(ctx,|ui|{
                egui::ScrollArea::vertical().max_height((ctx.content_rect().height()-210.).clamp(140.,520.)).show(ui,|ui|{
                    draft.transform_controls(ui);
                    if ui.button("미리보기 다시 계산").clicked(){draft.transform_canvas.requested=None;}
                    ui.label("캔버스의 점과 테두리를 직접 드래그합니다. Enter 적용 · Esc 취소");
                    if draft.transform.kind==WarpKind::Pins{ui.label("빈 곳 클릭: 핀 추가 · Alt+핀 클릭: 삭제 · 선택 핀의 원: 회전");}
                    if draft.transform_canvas.shown.as_ref()!=Some(&draft.transform){ui.label("미리보기 계산 중…");}
                    if let Some(error)=&draft.error{ui.colored_label(egui::Color32::LIGHT_RED,error);}
                });
                ui.separator();ui.horizontal(|ui|{
                    let apply=ui.button("확인");let discard=ui.button("취소");
                    #[cfg(test)]ui.data_mut(|d|{d.insert_temp(egui::Id::new("image_edit_accept"),apply.rect);d.insert_temp(egui::Id::new("image_edit_cancel"),discard.rect);});
                    confirm|=apply.clicked();cancel|=discard.clicked();
                });
            });
        if before != draft.transform {
            draft.transform_canvas.requested = None;
        }
        if before.grid != draft.transform.grid {
            draft.transform_canvas.drag = None;
            draft.transform_canvas.selected_nodes.clear();
            draft.transform_canvas.selected = 0;
        }
        if !ctx.egui_wants_keyboard_input() {
            confirm |= ctx.input_mut(|i| {
                i.consume_key(egui::Modifiers::NONE, egui::Key::Enter)
            });
        }
        cancel |= !open
            || ctx.input_mut(|i| {
                i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)
            });
        if cancel {
            self.cancel_transform_preview();
            return;
        }
        if confirm {
            self.cancel_transform_preview();
            if self.apply_image_edit(&draft) {
                return;
            }
            draft.error = self.notice.clone();
            draft.transform_canvas.requested = None;
        }
        // One bounded job, with changes coalesced while it runs. Releasing a
        // drag also schedules the final exact preview, independent of event rate.
        self.request_transform_preview(&mut draft, ctx);
        self.image_edit.dialog = Some(draft);
    }
    fn transform_handles(
        &mut self,
        ctx: &egui::Context,
        draft: &mut EditDialog,
    ) {
        let (Some(rect), Some(bounds)) =
            (self.image_edit.canvas_rect, draft.transform_canvas.bounds)
        else {
            return;
        };
        let session = &self.documents.active_session;
        let size = egui::vec2(
            session.document.width as f32,
            session.document.height as f32,
        );
        let view = &session.view;
        let to_screen =
            |p: [f32; 2]| view.screen(egui::vec2(p[0], p[1]), rect, size);
        let to_doc = |p: egui::Pos2| view.document(p, rect, size);
        let extent = egui::vec2(
            (bounds.x1 - bounds.x0) as f32,
            (bounds.y1 - bounds.y0) as f32,
        );
        let origin = egui::vec2(bounds.x0 as f32, bounds.y0 as f32);
        let center = origin + extent * 0.5;
        let normalized = |p: [f32; 2]| {
            [origin.x + p[0] * extent.x, origin.y + p[1] * extent.y]
        };
        let mut handles = Vec::new();
        if draft.transform.kind == WarpKind::Projective {
            let corners = [
                [bounds.x0 as f32, bounds.y0 as f32],
                [bounds.x1 as f32, bounds.y0 as f32],
                [bounds.x1 as f32, bounds.y1 as f32],
                [bounds.x0 as f32, bounds.y1 as f32],
            ];
            for (i, p) in corners.iter().enumerate() {
                if let Some(mapped) = draft.transform.map_point(*p, bounds) {
                    let mode = if matches!(
                        draft.command,
                        Command::EditTransformFree
                            | Command::EditTransformDistort
                            | Command::EditTransformPerspective
                    ) {
                        Handle::Corner(i)
                    } else if draft.command == Command::EditTransformShear {
                        Handle::Shear(0, if i < 2 { -1. } else { 1. })
                    } else if draft.command == Command::EditTransformRotate {
                        Handle::Rotate
                    } else {
                        Handle::Scale([
                            if i == 0 || i == 3 { -1 } else { 1 },
                            if i < 2 { -1 } else { 1 },
                        ])
                    };
                    handles.push((mode, to_screen(mapped)));
                }
            }
            let mids = [[0.5, 0.], [1., 0.5], [0.5, 1.], [0., 0.5]];
            for (i, p) in mids.iter().enumerate() {
                if let Some(mapped) =
                    draft.transform.map_point(normalized(*p), bounds)
                {
                    let mode = if draft.command == Command::EditTransformShear {
                        Handle::Shear(
                            usize::from(i % 2 == 1),
                            [-1., 1., 1., -1.][i],
                        )
                    } else {
                        Handle::Scale([[0, -1], [1, 0], [0, 1], [-1, 0]][i])
                    };
                    handles.push((mode, to_screen(mapped)));
                }
            }
            if let Some(mapped) = draft.transform.map_point(
                [center.x, bounds.y0 as f32 - 30. / view.zoom],
                bounds,
            ) {
                handles.push((Handle::Rotate, to_screen(mapped)));
            }
        } else if draft.transform.kind == WarpKind::Mesh {
            for (i, offset) in draft.transform.nodes.iter().enumerate() {
                let s = draft.transform.mesh_source(i);
                handles.push((
                    Handle::Node(i),
                    to_screen(normalized([s[0] + offset[0], s[1] + offset[1]])),
                ));
            }
        } else {
            for (i, pin) in draft.transform.pins.iter().enumerate() {
                let point = to_screen(normalized(pin.target));
                handles.push((Handle::Pin(i), point));
            }
            if let Some(pin) =
                draft.transform.pins.get(draft.transform_canvas.selected)
            {
                let point = to_screen(normalized(pin.target));
                handles.push((
                    Handle::PinRotate(draft.transform_canvas.selected),
                    point + egui::vec2(0., -24.),
                ));
            }
        }
        let overlay = |ui: &mut egui::Ui| {
            ui.set_clip_rect(rect);
            let (_, response) = ui.allocate_exact_size(
                rect.size(),
                egui::Sense::click_and_drag(),
            );
            let painter = ui.painter().with_clip_rect(rect);
            let color = egui::Color32::from_rgb(100, 190, 240);
            if draft.transform.kind == WarpKind::Projective {
                for i in 0..4 {
                    if handles.len() >= 4 {
                        painter.line_segment(
                            [handles[i].1, handles[(i + 1) % 4].1],
                            egui::Stroke::new(1., color),
                        );
                    }
                }
            } else if let Some(triangles) = draft.transform.mesh_triangles() {
                for t in triangles {
                    let t = t.map(|p| to_screen(normalized(p)));
                    for i in 0..3 {
                        painter.line_segment(
                            [t[i], t[(i + 1) % 3]],
                            egui::Stroke::new(0.5, color.gamma_multiply(0.55)),
                        );
                    }
                }
            }
            for (index, (handle, point)) in handles.iter().enumerate() {
                let radius = if matches!(handle, Handle::PinRotate(_)) {
                    5.
                } else {
                    4.
                };
                painter.circle_filled(
                    *point,
                    radius,
                    if matches!(handle,Handle::Node(i) if draft.transform_canvas.selected_nodes.contains(i)){egui::Color32::from_rgb(240,190,80)}else{egui::Color32::from_gray(35)},
                );
                painter.circle_stroke(
                    *point,
                    radius,
                    egui::Stroke::new(1.5, color),
                );
                if let Handle::Pin(i) = handle
                    && *i == draft.transform_canvas.selected
                {
                    painter.circle_stroke(
                        *point,
                        24.,
                        egui::Stroke::new(0.8, color),
                    );
                }
                #[cfg(test)]
                ui.data_mut(|d| {
                    d.insert_temp(
                        egui::Id::new(("transform_handle", index)),
                        *point,
                    )
                });
                let _ = index;
            }
            let nearest = |point: egui::Pos2| {
                handles
                    .iter()
                    .filter(|(_, p)| p.distance(point) < 10.)
                    .min_by(|a, b| {
                        a.1.distance_sq(point)
                            .total_cmp(&b.1.distance_sq(point))
                    })
                    .map(|(h, _)| *h)
            };
            if draft.transform_canvas.drag.is_none()
                && (response.drag_started()
                    || (response.is_pointer_button_down_on()
                        && ctx.input(|i| i.pointer.primary_pressed())
                        && ctx
                            .input(|i| i.pointer.press_origin())
                            .and_then(nearest)
                            .is_some()))
            {
                let point = ctx
                    .input(|i| i.pointer.press_origin())
                    .unwrap_or(response.rect.center());
                let start = to_doc(point);
                let mut handle = nearest(point);
                if handle.is_none() && draft.transform.kind == WarpKind::Pins {
                    handle = add_pin(draft, bounds, start).map(Handle::Pin);
                }
                if handle.is_none()
                    && draft.transform.kind == WarpKind::Projective
                {
                    handle = Some(Handle::Move);
                }
                if let Some(handle) = handle {
                    if let Handle::Node(index) = handle {
                        draft.values[0] = index as f32;
                        if !draft
                            .transform_canvas
                            .selected_nodes
                            .contains(&index)
                        {
                            if !ctx.input(|i| i.modifiers.shift) {
                                draft.transform_canvas.selected_nodes.clear();
                            }
                            draft.transform_canvas.selected_nodes.push(index);
                        }
                    }
                    if let Handle::Pin(index) | Handle::PinRotate(index) =
                        handle
                    {
                        draft.transform_canvas.selected = index;
                    }
                    draft.transform_canvas.drag = Some(Drag {
                        handle,
                        start,
                        base: draft.transform.clone(),
                    });
                }
            }
            let moved =
                draft.transform_canvas.drag.as_ref().is_some_and(|drag| {
                    ctx.input(|i| i.pointer.interact_pos()).is_some_and(|p| {
                        (to_doc(p) - drag.start).length_sq() > 0.0625
                    })
                });
            if response.clicked()
                && !moved
                && let Some(point) = response.interact_pointer_pos()
            {
                match nearest(point) {
                    Some(Handle::Pin(index)) => {
                        if ctx.input(|i| i.modifiers.alt) {
                            draft.transform_canvas.drag = None;
                            draft.transform.pins.remove(index);
                            draft.transform_canvas.selected =
                                draft.transform_canvas.selected.min(
                                    draft
                                        .transform
                                        .pins
                                        .len()
                                        .saturating_sub(1),
                                );
                        } else {
                            draft.transform_canvas.selected = index;
                        }
                    }
                    Some(Handle::Node(index)) => {
                        draft.values[0] = index as f32;
                        if ctx.input(|i| i.modifiers.shift) {
                            if !draft
                                .transform_canvas
                                .selected_nodes
                                .contains(&index)
                            {
                                draft
                                    .transform_canvas
                                    .selected_nodes
                                    .push(index);
                            }
                        } else {
                            draft.transform_canvas.selected_nodes = vec![index];
                        }
                    }
                    None if draft.transform.kind == WarpKind::Pins => {
                        add_pin(draft, bounds, to_doc(point));
                    }
                    _ => {}
                }
            }
            if let Some(drag) = &draft.transform_canvas.drag
                && let Some(point) = ctx.input(|i| i.pointer.interact_pos())
            {
                let end = to_doc(point);
                let delta = end - drag.start;
                let mut next = drag.base.clone();
                let shift = ctx.input(|i| i.modifiers.shift);
                match drag.handle {
                    Handle::Move => {
                        next.translation = [
                            drag.base.translation[0] + delta.x,
                            drag.base.translation[1] + delta.y,
                        ];
                    }
                    Handle::Node(index) => {
                        let selected = if draft
                            .transform_canvas
                            .selected_nodes
                            .contains(&index)
                        {
                            draft.transform_canvas.selected_nodes.clone()
                        } else {
                            vec![index]
                        };
                        for i in selected {
                            next.nodes[i] = [
                                drag.base.nodes[i][0] + delta.x / extent.x,
                                drag.base.nodes[i][1] + delta.y / extent.y,
                            ];
                        }
                    }
                    Handle::Pin(i) => {
                        next.pins[i].target = [
                            drag.base.pins[i].target[0] + delta.x / extent.x,
                            drag.base.pins[i].target[1] + delta.y / extent.y,
                        ];
                    }
                    Handle::PinRotate(i) => {
                        let pivot = normalized(next.pins[i].target);
                        let pivot = egui::vec2(pivot[0], pivot[1]);
                        next.pins[i].rotation += ((end - pivot).angle()
                            - (drag.start - pivot).angle())
                        .to_degrees();
                    }
                    Handle::Rotate => {
                        let pivot = center
                            + egui::vec2(
                                next.translation[0],
                                next.translation[1],
                            );
                        next.angle += ((end - pivot).angle()
                            - (drag.start - pivot).angle())
                        .to_degrees();
                        if shift {
                            next.angle = (next.angle / 45.).round() * 45.;
                        }
                    }
                    Handle::Scale(sign) => {
                        let (sin, cos) = (-next.angle).to_radians().sin_cos();
                        let local = egui::vec2(
                            delta.x * cos - delta.y * sin,
                            delta.x * sin + delta.y * cos,
                        );
                        for c in 0..2 {
                            if sign[c] != 0 {
                                next.scale[c] += 2. * local[c] / extent[c]
                                    * f32::from(sign[c]);
                            }
                        }
                        if shift {
                            let ratio = if sign[0] != 0 {
                                next.scale[0] / drag.base.scale[0]
                            } else {
                                next.scale[1] / drag.base.scale[1]
                            };
                            next.scale = [
                                drag.base.scale[0] * ratio,
                                drag.base.scale[1] * ratio,
                            ];
                        }
                    }
                    Handle::Shear(axis, sign) => {
                        next.shear[axis] +=
                            2. * delta[axis] / extent[1 - axis] * sign;
                    }
                    Handle::Corner(i) => {
                        let corners = [
                            [bounds.x0 as f32, bounds.y0 as f32],
                            [bounds.x1 as f32, bounds.y0 as f32],
                            [bounds.x1 as f32, bounds.y1 as f32],
                            [bounds.x0 as f32, bounds.y1 as f32],
                        ];
                        let mut quad = corners
                            .map(|p| drag.base.map_point(p, bounds).unwrap());
                        quad[i] = [quad[i][0] + delta.x, quad[i][1] + delta.y];
                        if let Some(fit) =
                            RasterTransform::fit_quad(bounds, quad)
                        {
                            next = fit;
                        } else {
                            draft.error=Some("이 위치에서는 사각형 변형을 유지할 수 없습니다.".into());
                            return;
                        }
                    }
                }
                if next.validate() {
                    if next != draft.transform {
                        draft.transform_canvas.requested = None;
                    }
                    draft.transform = next;
                    draft.error = None;
                } else {
                    draft.error = Some(
                        "접히거나 범위를 넘는 변형은 적용하지 않습니다.".into(),
                    );
                }
            }
            if !ctx.input(|i| i.focused) {
                if let Some(drag) = draft.transform_canvas.drag.take() {
                    draft.transform = drag.base;
                    draft.transform_canvas.requested = None;
                }
            } else if response.drag_stopped()
                || !ctx.input(|i| i.pointer.primary_down())
            {
                draft.transform_canvas.drag = None;
            }
        };
        egui::Area::new(egui::Id::new("transform_canvas_handles"))
            .fixed_pos(rect.min)
            .movable(false)
            .constrain(false)
            .show(ctx, overlay);
    }
}
fn add_pin(
    draft: &mut EditDialog,
    bounds: PixelRect,
    point: egui::Vec2,
) -> Option<usize> {
    if draft.transform.pins.len() >= 32 {
        return None;
    }
    let source = draft.transform.unmap_point([point.x, point.y], bounds)?;
    let size = [(bounds.x1 - bounds.x0) as f32, (bounds.y1 - bounds.y0) as f32];
    let source = [
        (source[0] - bounds.x0 as f32) / size[0],
        (source[1] - bounds.y0 as f32) / size[1],
    ];
    if source.iter().any(|v| !(0. ..=1.).contains(v))
        || draft.transform.pins.iter().any(|p| {
            (p.source[0] - source[0]).powi(2)
                + (p.source[1] - source[1]).powi(2)
                < 1e-6
        })
    {
        return None;
    }
    let target = [
        (point.x - bounds.x0 as f32) / size[0],
        (point.y - bounds.y0 as f32) / size[1],
    ];
    draft.transform.pins.push(WarpPin { source, target, rotation: 0. });
    let index = draft.transform.pins.len() - 1;
    draft.transform_canvas.selected = index;
    draft.transform_canvas.requested = None;
    Some(index)
}

pub(super) fn new_buffers(before: &Document, after: &Document) -> u64 {
    let mut census = parfait_core::BufferCensus::default();
    census.add_document(before);
    let bytes = census.bytes();
    census.add_document(after);
    census.bytes().saturating_sub(bytes)
}
