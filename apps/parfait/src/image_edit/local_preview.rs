//! One local worker, bounded reservations and exact, session-checked preview reuse.
use super::*;
use parfait_core::{Document, ToneAdjustment};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::time::Duration;
#[cfg(test)]
mod tests;

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Operation {
    Tone(u64, ToneAdjustment),
    Pixel(u64, PixelEdit),
    Canvas(CanvasEdit),
    Shade(usize, parfait_core::LocalShading),
    Assist {
        command: Command,
        values: [f32; 8],
        low: [u8; 3],
        high: [u8; 3],
        reference: Option<Arc<PixelClipboard>>,
    },
}
impl Operation {
    pub fn reason(&self, doc: &Document) -> Option<&'static str> {
        match self {
            Self::Tone(layers, edit) => doc.tone_edit_reason(*layers, edit),
            Self::Pixel(layers, edit) => doc
                .layers
                .iter()
                .enumerate()
                .filter(|(i, _)| layers & (1_u64 << i) != 0)
                .find_map(|(i, _)| doc.pixel_edit_reason(i, *edit)),
            Self::Canvas(edit) => doc.canvas_edit_reason(*edit),
            Self::Shade(index, options) => {
                doc.local_shading_reason(*index, *options)
            }
            Self::Assist { command, values, low, high, reference } => {
                if matches!(
                    command,
                    Command::EditAutoColor
                        | Command::EditAutoColorHints
                        | Command::EditAutoColorOptions
                ) {
                    doc.region_color_reason(
                        &parfait_core::RegionColors {
                            threshold: values[0] as u8,
                            minimum_area: values[1] as u32,
                            include_open_regions: values[2] > 0.5,
                            palette: vec![*low, *high],
                        },
                        reference.as_deref(),
                    )
                } else {
                    doc.pixel_edit_reason(doc.active, PixelEdit::Invert)
                }
            }
        }
    }
    pub fn apply(
        &self,
        doc: &mut Document,
        cancelled: impl Fn() -> bool,
    ) -> bool {
        match self {
            Self::Tone(layers, edit) => {
                doc.adjust_layer_tones_cancellable(*layers, edit, cancelled)
            }
            Self::Pixel(layers, edit) => {
                if self.reason(doc).is_some() {
                    return false;
                }
                let mut next = doc.clone();
                let mut changed = false;
                for i in 0..next.layers.len() {
                    if layers & (1_u64 << i) != 0 {
                        changed |= next.edit_layer_pixels_cancellable(
                            i, *edit, &cancelled,
                        );
                    }
                }
                if cancelled() || !changed {
                    return false;
                }
                *doc = next;
                true
            }
            Self::Canvas(edit) => !cancelled() && doc.edit_canvas(*edit),
            Self::Shade(index, options) => {
                doc.add_local_shading_cancellable(*index, *options, cancelled)
            }
            Self::Assist { command, values, low, high, reference } => {
                if cancelled() {
                    return false;
                }
                match command {
                    Command::EditAutoShade => {
                        doc.shade_layer(doc.active, values[0], values[1] / 100.)
                    }
                    Command::EditColorReference => {
                        reference.as_deref().is_some_and(|r| {
                            doc.match_layer_colors_with(
                                doc.active,
                                r,
                                values[0] / 100.,
                                values[1] > 0.5,
                            )
                        })
                    }
                    _ => doc.color_regions(
                        &parfait_core::RegionColors {
                            threshold: values[0] as u8,
                            minimum_area: values[1] as u32,
                            include_open_regions: values[2] > 0.5,
                            palette: vec![*low, *high],
                        },
                        reference.as_deref(),
                    ),
                }
            }
        }
    }
}
pub(super) struct LocalPreview {
    pub enabled: bool,
    pub compare: bool,
    pub texture: Option<egui::TextureHandle>,
    pub cached: Option<(Operation, Document)>,
    requested: Option<Operation>,
}
impl Default for LocalPreview {
    fn default() -> Self {
        Self {
            enabled: true,
            compare: false,
            texture: None,
            cached: None,
            requested: None,
        }
    }
}
pub(super) struct Job {
    owner: SessionKey,
    revision: u64,
    operation: Operation,
    pub source: Document,
    receiver: mpsc::Receiver<(Document, Vec<u8>)>,
    cancel: Arc<AtomicBool>,
    pub reservation: u64,
}
impl Drop for Job {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
    }
}
impl EditDialog {
    pub(crate) fn reference_bytes(&self) -> Option<u64> {
        self.reference.as_ref().map(|r| r.pixels.len() as u64)
    }
    pub(super) fn local_operation(&self, app: &Parfait) -> Option<Operation> {
        if self.command == Command::EditAutoShade {
            return Some(Operation::Shade(
                app.documents.active_session.document.active,
                self.shading,
            ));
        }
        if let Some(tone) = self.tone() {
            return Some(Operation::Tone(self.tone_ui.layers, tone));
        }
        if let Some(pixel) = self.pixel(app.brush.color) {
            return Some(Operation::Pixel(
                if self.command.is_tonal() {
                    self.tone_ui.layers
                } else {
                    1_u64 << app.documents.active_session.document.active
                },
                pixel,
            ));
        }
        if let Some(canvas) = self.canvas(app) {
            return Some(Operation::Canvas(canvas));
        }
        if self.command.is_color_assist() {
            return Some(Operation::Assist {
                command: self.command,
                values: self.values,
                low: self.low,
                high: self.high,
                reference: self.reference.clone(),
            });
        }
        None
    }
}
impl Parfait {
    pub(crate) fn local_preview_source(&self) -> Option<&Document> {
        self.image_edit.local_job.as_ref().map(|j| &j.source)
    }
    pub(crate) fn local_preview_cached(&self) -> Option<&Document> {
        self.image_edit
            .dialog
            .as_ref()?
            .local_preview
            .cached
            .as_ref()
            .map(|(_, d)| d)
    }
    pub(crate) fn local_preview_reservation(&self) -> u64 {
        self.image_edit.local_job.as_ref().map_or(0, |j| j.reservation)
    }
    pub(crate) fn local_canvas_texture(&self) -> Option<&egui::TextureHandle> {
        let draft = self.image_edit.dialog.as_ref()?;
        if !draft.local_preview.enabled
            || draft.local_preview.compare
            || self.documents.active_key() != Some(draft.owner)
            || self.documents.active_session.document.revision != draft.revision
        {
            return None;
        }
        if matches!(draft.local_operation(self), Some(Operation::Canvas(_))) {
            return None;
        }
        draft.local_preview.texture.as_ref()
    }
    pub(crate) fn cancel_local_preview(&self) {
        if let Some(job) = &self.image_edit.local_job {
            job.cancel.store(true, Ordering::Release);
        }
    }
    pub(crate) fn poll_local_preview(&mut self, ctx: &egui::Context) {
        let Some(job) = &self.image_edit.local_job else {
            return;
        };
        let result = match job.receiver.try_recv() {
            Ok(value) => Some(value),
            Err(mpsc::TryRecvError::Empty) => {
                ctx.request_repaint_after(Duration::from_millis(16));
                return;
            }
            Err(mpsc::TryRecvError::Disconnected) => None,
        };
        let job = self.image_edit.local_job.take().unwrap();
        let current = self
            .image_edit
            .dialog
            .as_ref()
            .and_then(|d| d.local_operation(self));
        let Some(draft) = &mut self.image_edit.dialog else {
            return;
        };
        if job.cancel.load(Ordering::Acquire)
            || draft.owner != job.owner
            || draft.revision != job.revision
            || current.as_ref() != Some(&job.operation)
        {
            draft.local_preview.requested = None;
            return;
        }
        if let Some((doc, mut pixels)) = result {
            let w = doc.width as usize;
            for (i, p) in pixels.as_chunks_mut::<4>().0.iter_mut().enumerate() {
                let bg = if (i % w / 16 + i / w / 16).is_multiple_of(2) {
                    235.
                } else {
                    218.
                };
                let alpha = f32::from(p[3]) / 255.;
                for c in &mut p[..3] {
                    *c = (f32::from(*c) * alpha + bg * (1. - alpha)).round()
                        as u8;
                }
                p[3] = 255;
            }
            draft.local_preview.texture = Some(ctx.load_texture(
                "local_edit_preview",
                egui::ColorImage::from_rgba_unmultiplied(
                    [doc.width as usize, doc.height as usize],
                    &pixels,
                ),
                egui::TextureOptions::LINEAR,
            ));
            draft.local_preview.cached = Some((job.operation.clone(), doc));
            draft.error = None;
        } else {
            draft.error = Some(
                "미리보기 계산이 종료되었습니다. 다시 계산할 수 있습니다."
                    .into(),
            );
        }
    }
    fn request_local_preview(
        &mut self,
        draft: &mut EditDialog,
        operation: &Operation,
        ctx: &egui::Context,
    ) {
        if !draft.local_preview.enabled {
            return;
        }
        if let Some(job) = &self.image_edit.local_job {
            if job.operation != *operation {
                job.cancel.store(true, Ordering::Release);
            }
            return;
        }
        if draft.local_preview.requested.as_ref() == Some(operation) {
            return;
        }
        if let Some(reason) =
            operation.reason(&self.documents.active_session.document)
        {
            draft.error = Some(reason.into());
            return;
        }
        let doc = &self.documents.active_session.document;
        let output = if let Operation::Canvas(edit) = operation {
            doc.canvas_edit_bytes(*edit).unwrap_or(u64::MAX)
        } else {
            doc.logical_pixel_bytes()
                + u64::from(doc.width)
                    * u64::from(doc.height)
                    * if matches!(operation, Operation::Shade(..)) {
                        8
                    } else {
                        4
                    }
        };
        let reservation = output
            .saturating_mul(2)
            .saturating_add(u64::from(doc.width) * u64::from(doc.height) * 24)
            .saturating_add(32 * 1024 * 1024)
            .saturating_add(draft.reference_bytes().unwrap_or(0));
        let carried =
            draft.local_preview.cached.as_ref().map_or(0, |(_, cache)| {
                transform_canvas::new_buffers(doc, cache)
            });
        draft.local_preview.requested = Some(operation.clone());
        if !self.can_reserve_pixel_headroom(reservation.saturating_add(carried))
        {
            draft.error=Some("미리보기를 위한 메모리가 부족합니다. 크기를 줄이거나 미리보기를 꺼 주세요.".into());
            return;
        }
        let source = doc.clone();
        let mut work = source.clone();
        let op = operation.clone();
        let cancel = Arc::new(AtomicBool::new(false));
        let stop = cancel.clone();
        let context = ctx.clone();
        let (sender, receiver) = mpsc::sync_channel(1);
        match std::thread::Builder::new()
            .name("parfait-local-edit-preview".into())
            .spawn(move || {
                op.apply(&mut work, || stop.load(Ordering::Acquire));
                if !stop.load(Ordering::Acquire) {
                    let pixels = work.composite();
                    if !stop.load(Ordering::Acquire) {
                        let _ = sender.send((work, pixels));
                    }
                }
                context.request_repaint();
            }) {
            Ok(_) => {
                self.image_edit.local_job = Some(Job {
                    owner: draft.owner,
                    revision: draft.revision,
                    operation: operation.clone(),
                    source,
                    receiver,
                    cancel,
                    reservation,
                })
            }
            Err(_) => {
                draft.error =
                    Some("미리보기 작업을 시작하지 못했습니다.".into())
            }
        }
    }
    pub(super) fn local_edit_window(
        &mut self,
        ctx: &egui::Context,
        mut draft: EditDialog,
    ) {
        if draft.command == Command::EditAutoShade {
            self.shading_light_handle(ctx, &mut draft);
        }
        self.geometry_canvas_overlay(ctx, &mut draft);
        let mut open = true;
        let mut confirm = false;
        let mut cancel = false;
        let mut reset = false;
        let operation = draft.local_operation(self).unwrap();
        let is_canvas = matches!(operation, Operation::Canvas(_));
        let window_output=crate::editor_window::window(ctx,draft.command,480.).open(&mut open).show(ctx,|ui|{
            ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
            ui.set_width(350.);
            ui.spacing_mut().item_spacing.y = 8.;
            egui::ScrollArea::vertical().max_height((ctx.content_rect().height()-180.).clamp(110.,620.)).show(ui,|ui|{
                if draft.command.is_tonal(){draft.tone_controls(ui);}
                else if draft.command.is_color_assist(){draft.assist_controls(ui);}
                else if matches!(draft.command,Command::EditResolution|Command::EditCanvasSize|Command::EditCanvasBasic|Command::EditSuperResolution){
                    draft.geometry_controls(ui);
                    if draft.command==Command::EditSuperResolution {ui.label("학습 모델 없이 원본과의 차이를 반복 보정합니다.");slider(ui,&mut draft.values[0],1. ..=8.,"반복 횟수");slider(ui,&mut draft.values[1],0. ..=1.,"보정 강도");}
                } else if draft.command==Command::EditAdvancedFill {slider(ui,&mut draft.values[0],0. ..=100.,"색 불투명도");}
                else if draft.command==Command::EditBorder {slider(ui,&mut draft.values[0],1. ..=128.,"안쪽 테두리 (px)");}
                if is_canvas && draft.command==Command::EditSuperResolution && draft.local_preview.enabled && !draft.local_preview.compare && let Some(texture)=&draft.local_preview.texture {ui.add(egui::Image::new(texture).max_size(egui::vec2(330.,175.)));}
                if let Some(error)=&draft.error{ui.colored_label(egui::Color32::LIGHT_RED,error);}
            });
            });
            ui.add_space(8.);
            ui.vertical(|ui| {
                ui.set_width(100.);
                let ok=crate::widgets::action_button(ui,"확인",100.);
                let no=crate::widgets::action_button(ui,"취소",100.);
                #[cfg(test)] ui.data_mut(|d|{d.insert_temp(egui::Id::new("image_edit_accept"),ok.rect);d.insert_temp(egui::Id::new("image_edit_cancel"),no.rect);});
                confirm=ok.clicked(); cancel=no.clicked();
                ui.add_space(8.);
                reset=crate::widgets::action_button(ui,"초기화",100.).clicked();
                ui.add_space(8.);
                ui.checkbox(&mut draft.local_preview.enabled,"미리보기");
                ui.checkbox(&mut draft.local_preview.compare,"원본 비교");
                if draft.local_preview.cached.as_ref().is_none_or(|(op,_)|*op!=operation) && draft.local_preview.enabled { ui.weak("계산 중…"); }
            });
            });
        });
        if matches!(
            draft.command,
            Command::EditResolution
                | Command::EditCanvasSize
                | Command::EditCanvasBasic
        ) && let Some(output) = &window_output
        {
            // The canvas overlay is an interactive Area; its large hit surface
            // must stay below the controls even after a handle is clicked.
            ctx.move_to_top(output.response.layer_id);
        }
        #[cfg(test)]
        if let Some(output) = &window_output {
            ctx.data_mut(|d| {
                d.insert_temp(
                    egui::Id::new(("image_edit_window", draft.command)),
                    output.response.rect,
                )
            });
        }
        let _ = window_output;
        cancel |= !open
            || ctx.input_mut(|i| {
                i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)
            });
        if cancel {
            self.cancel_local_preview();
            return;
        }
        if reset {
            let histogram = draft.tone_ui.histogram;
            let reference = draft.reference.clone();
            draft = EditDialog::new(self, draft.command);
            draft.tone_ui.histogram = histogram;
            draft.reference = reference;
            self.cancel_local_preview();
        }
        let operation = draft.local_operation(self).unwrap();
        if !draft.local_preview.enabled {
            self.cancel_local_preview();
        }
        if confirm {
            self.cancel_local_preview();
            if self.apply_image_edit(&draft) {
                return;
            }
            draft.error = self.notice.clone();
        }
        // Geometry follows immediately using the source texture. Exact native
        // content/extent and sampling previews wait until a size gesture ends.
        if !matches!(
            draft.command,
            Command::EditResolution
                | Command::EditCanvasSize
                | Command::EditCanvasBasic
        ) || !ctx.input(|i| i.pointer.primary_down())
        {
            self.request_local_preview(&mut draft, &operation, ctx);
        }
        self.image_edit.dialog = Some(draft);
    }
    fn shading_light_handle(
        &self,
        ctx: &egui::Context,
        draft: &mut EditDialog,
    ) {
        let Some(rect) = self.image_edit.canvas_rect else {
            return;
        };
        let doc = &self.documents.active_session.document;
        let size = egui::vec2(doc.width as f32, doc.height as f32);
        let view = &self.documents.active_session.view;
        let center = view.screen(size * 0.5, rect, size);
        let s = &mut draft.shading;
        let point = if s.point_light {
            view.screen(
                egui::vec2(s.position[0] * size.x, s.position[1] * size.y),
                rect,
                size,
            )
        } else {
            let (sin, cos) = s.angle.to_radians().sin_cos();
            center + egui::vec2(cos, sin) * 75.
        };
        let point = egui::pos2(
            point.x.clamp(rect.left() + 16., rect.right() - 16.),
            point.y.clamp(rect.top() + 16., rect.bottom() - 16.),
        );
        egui::Area::new(egui::Id::new("local_shading_light"))
            .fixed_pos(point - egui::vec2(16., 16.))
            .movable(false)
            .constrain(false)
            .show(ctx, |ui| {
                let (r, response) = ui.allocate_exact_size(
                    egui::vec2(32., 32.),
                    egui::Sense::drag(),
                );
                ui.painter().circle_filled(
                    r.center(),
                    10.,
                    egui::Color32::from_rgb(255, 220, 110),
                );
                ui.painter().circle_stroke(
                    r.center(),
                    14.,
                    egui::Stroke::new(1.5, egui::Color32::WHITE),
                );
                let response = response
                    .on_hover_text(if s.point_light {
                        "광원 위치 드래그"
                    } else {
                        "광원 방향 드래그"
                    })
                    .on_hover_cursor(egui::CursorIcon::Grab);
                if response.dragged()
                    && let Some(p) = response.interact_pointer_pos()
                {
                    if s.point_light {
                        let p = view.document(p, rect, size);
                        s.position = [
                            (p.x / size.x).clamp(-1., 2.),
                            (p.y / size.y).clamp(-1., 2.),
                        ];
                    } else {
                        let d = p - center;
                        s.angle = d.y.atan2(d.x).to_degrees();
                    }
                }
            });
    }
}

pub(super) fn buffer_growth(source: &Document, next: &Document) -> u64 {
    super::transform_canvas::new_buffers(source, next)
}
