use crate::{app::Parfait, document_session::SessionKey, shortcuts::Command};
use eframe::egui;
use parfait_core::{CanvasEdit, PixelClipboard, PixelEdit};
use std::borrow::Cow;

mod assist;
mod geometry_canvas;
mod geometry_ui;
mod local_preview;
#[cfg(test)]
mod tests;
mod tone_ui;
mod transform_canvas;
mod transform_ui;

#[derive(Default)]
pub(crate) struct ImageEditState {
    pub dialog: Option<EditDialog>,
    pub clipboard: Option<PixelClipboard>,
    // Keep ownership alive on clipboard backends such as X11.
    system_clipboard: Option<arboard::Clipboard>,
    pub canvas_rect: Option<egui::Rect>,
    preview_job: Option<transform_canvas::PreviewJob>,
    local_job: Option<local_preview::Job>,
}
pub(crate) struct EditDialog {
    owner: SessionKey,
    revision: u64,
    command: Command,
    values: [f32; 8],
    width: u32,
    height: u32,
    offset: [i32; 2],
    low: [u8; 3],
    high: [u8; 3],
    preview: Option<egui::TextureHandle>,
    transform: parfait_core::RasterTransform,
    mask_target: bool,
    transform_canvas: transform_canvas::TransformCanvas,
    reference: Option<std::sync::Arc<PixelClipboard>>,
    tone_ui: tone_ui::ToneUi,
    geometry: geometry_ui::GeometryUi,
    local_preview: local_preview::LocalPreview,
    shading: parfait_core::LocalShading,
    name: String,
    materials:
        Vec<(String, crate::local_materials::MaterialKind, std::path::PathBuf)>,
    error: Option<String>,
}
impl EditDialog {
    fn new(app: &Parfait, command: Command) -> Self {
        let document = &app.documents.active_session.document;
        let mut values = [0.; 8];
        let mut width = document.width;
        let mut height = document.height;
        let mut offset = [0, 0];
        match command {
            Command::EditSuperResolution => {
                width = (document.width * 2).min(4096);
                height = (document.height * 2).min(4096);
                values[0] = 4.;
                values[1] = 0.5;
            }
            Command::EditAutoColor
            | Command::EditAutoColorHints
            | Command::EditAutoColorOptions => {
                values[0] = 220.;
                values[1] = 4.;
            }
            Command::EditColorReference => {
                values[0] = 100.;
                values[1] = 1.;
            }
            Command::EditAutoShade => {
                values[0] = -45.;
                values[1] = 50.;
            }
            Command::EditRefGuides => {
                if let Some(rect) =
                    app.documents.active_session.editing.align.guides
                {
                    width = rect.x1 - rect.x0;
                    height = rect.y1 - rect.y0;
                    offset = [rect.x0 as i32, rect.y0 as i32];
                }
            }
            Command::EditAdvancedFill => values[0] = 100.,
            Command::EditBorder => values[0] = 3.,
            Command::EditPosterize => values[0] = 8.,
            Command::EditThreshold => values[0] = 128.,
            Command::EditLevels => {
                values[1] = 255.;
                values[2] = 1.;
                values[4] = 255.;
            }
            Command::EditToneCurve => {
                values[..5].copy_from_slice(&[0., 0.25, 0.5, 0.75, 1.])
            }
            _ => {}
        }
        let mut transform = parfait_core::RasterTransform::default();
        match command {
            Command::EditTransformFlipX => transform.scale[0] = -1.,
            Command::EditTransformFlipY => transform.scale[1] = -1.,
            Command::EditTransformMesh => {
                transform.kind = parfait_core::WarpKind::Mesh
            }
            Command::EditTransformPins => {
                transform.kind = parfait_core::WarpKind::Pins
            }
            _ => {}
        }
        Self {
            owner: app.documents.active_session.key,
            revision: document.revision,
            command,
            values,
            width,
            height,
            offset,
            low: app.brush.color.map(|v| (f32::from(v) * 0.65).round() as u8),
            high: app.brush.color,
            preview: None,
            transform,
            mask_target: app.editing_layer_mask(),
            transform_canvas: transform_canvas::TransformCanvas::default(),
            reference: None,
            tone_ui: tone_ui::ToneUi::new(app),
            geometry: geometry_ui::GeometryUi::new([
                document.width,
                document.height,
            ]),
            local_preview: local_preview::LocalPreview::default(),
            shading: parfait_core::LocalShading {
                radius: (document.width.min(document.height) as f32 / 4.)
                    .clamp(1., 512.),
                ..Default::default()
            },
            name: document.name.clone(),
            materials: Vec::new(),
            error: None,
        }
    }
    fn pixel(&self, color: [u8; 3]) -> Option<PixelEdit> {
        use Command::*;
        let color = [color[0], color[1], color[2], 255];
        Some(match self.command {
            EditFill => PixelEdit::Fill(color),
            EditAdvancedFill => PixelEdit::Fill([
                color[0],
                color[1],
                color[2],
                (self.values[0] * 2.55).round() as u8,
            ]),
            EditClearOutside => PixelEdit::ClearOutside,
            EditBorder => {
                PixelEdit::Border { color, radius: self.values[0] as u32 }
            }
            EditBrightnessContrast => PixelEdit::BrightnessContrast {
                brightness: self.values[0] / 100.,
                contrast: self.values[1] / 100.,
            },
            EditHsl => PixelEdit::Hsl {
                hue: self.values[0],
                saturation: self.values[1] / 100.,
                lightness: self.values[2] / 100.,
            },
            EditPosterize => PixelEdit::Posterize(self.values[0] as u8),
            EditInvert => PixelEdit::Invert,
            EditLevels => PixelEdit::Levels {
                black: self.values[0] as u8,
                white: self.values[1] as u8,
                gamma: self.values[2],
            },
            EditToneCurve => {
                PixelEdit::Curve(self.values[..5].try_into().unwrap())
            }
            EditColorBalance => PixelEdit::Balance([
                self.values[0] / 100.,
                self.values[1] / 100.,
                self.values[2] / 100.,
            ]),
            EditThreshold => PixelEdit::Threshold(self.values[0] as u8),
            EditGradientMap => {
                PixelEdit::Gradient { low: self.low, high: self.high }
            }
            _ => return None,
        })
    }
    fn canvas(&self, app: &Parfait) -> Option<CanvasEdit> {
        use Command::*;
        Some(match self.command {
            EditSuperResolution => CanvasEdit::SuperResolve {
                width: self.width,
                height: self.height,
                iterations: self.values[0] as u8,
                strength: self.values[1],
            },
            EditResolution => {
                if self.geometry.nearest {
                    CanvasEdit::ResizeNearest {
                        width: self.width,
                        height: self.height,
                    }
                } else {
                    CanvasEdit::Resize {
                        width: self.width,
                        height: self.height,
                    }
                }
            }
            EditCanvasSize | EditCanvasBasic => CanvasEdit::Extent {
                width: self.width,
                height: self.height,
                offset: self.offset,
            },
            EditCrop => CanvasEdit::Crop(
                app.documents.active_session.document.selection_bounds()?,
            ),
            EditCanvasRotateRight => CanvasEdit::RotateRight,
            EditCanvasRotateHalf => CanvasEdit::RotateHalf,
            EditCanvasRotateLeft => CanvasEdit::RotateLeft,
            EditCanvasFlipX => CanvasEdit::FlipHorizontal,
            EditCanvasFlipY => CanvasEdit::FlipVertical,
            _ => return None,
        })
    }
}
impl Parfait {
    pub(crate) fn configure_hsv_preview(&mut self, values: [f32; 3]) {
        if let Some(draft) = &mut self.image_edit.dialog
            && draft.command == Command::EditHsl
        {
            draft.values[..3].copy_from_slice(&[
                values[0].clamp(-180., 180.),
                values[1].clamp(-100., 100.),
                values[2].clamp(-100., 100.),
            ]);
        }
    }
    pub(crate) fn image_edit_preview_pending(&self) -> bool {
        self.image_edit.local_job.is_some()
            || self.image_edit.preview_job.is_some()
    }
    pub(crate) fn image_edit_reason(
        &self,
        command: Command,
    ) -> Option<&'static str> {
        if self.documents.is_empty() {
            return Some("그림을 먼저 열어 주세요.");
        }
        if matches!(
            command,
            Command::EditTransformApply | Command::EditTransformCancel
        ) {
            return self
                .image_edit
                .dialog
                .as_ref()
                .is_none_or(|d| !d.command.is_raster_transform())
                .then_some("진행 중인 변형이 없습니다.");
        }
        if self.dialog_open()
            || self.workspace.dragging.is_some()
            || self.workspace.resizing
            || self.documents.active_session.layer_ui.list.owns_pointer()
            || self.documents.active_session.editing.stroke.is_some()
        {
            return Some("현재 조작이나 대화상자를 마쳐 주세요.");
        }
        let document = &self.documents.active_session.document;
        if command.is_settings_command()
            || matches!(
                command,
                Command::EditScreenColor | Command::EditScreenColorHide
            )
        {
            return None;
        }
        if matches!(
            command,
            Command::EditCut | Command::EditCopy | Command::EditCopySvg
        ) && !self.options.clipboard_write
        {
            return Some("개인정보 설정에서 클립보드 쓰기를 허용해 주세요.");
        }
        if matches!(command, Command::EditPaste | Command::EditPasteAtView)
            && !self.options.clipboard_read
        {
            return Some("개인정보 설정에서 클립보드 읽기를 허용해 주세요.");
        }

        if command.is_native_command() {
            return self.native_command_reason(command);
        }
        if matches!(
            command,
            Command::EditTextInkBounds | Command::EditVectorCenters
        ) {
            return None;
        }
        if command.is_material_registration() {
            return None;
        }
        if command.is_tonal() {
            if self.editing_layer_mask() {
                return Some("레이어 내용 썸네일을 선택해 주세요.");
            }
            return EditDialog::new(self, command)
                .local_operation(self)
                .and_then(|op| op.reason(document));
        }
        if command.is_color_assist() {
            let draft = EditDialog::new(self, command);
            if self.editing_layer_mask() {
                return Some("레이어 내용 썸네일을 선택해 주세요.");
            }
            return draft.assist_reason(document);
        }
        if command.alignment().is_some() {
            return self.alignment_plan(command).err();
        }
        if command.align_reference().is_some() {
            return None;
        }
        if matches!(command, Command::EditPaste | Command::EditPasteAtView) {
            if let Some(reason) = document.layer_insertion_reason() {
                return Some(reason);
            }
            return parfait_storage::layer_growth_error(
                document,
                "붙여넣은 레이어".len(),
            );
        }
        if command == Command::EditPressure {
            return None;
        }
        if command == Command::EditClearHistory {
            return (!document.validate())
                .then_some("이미지 데이터가 유효하지 않습니다.");
        }
        let draft = EditDialog::new(self, command);
        if let Some(canvas) = draft.canvas(self) {
            return document.canvas_edit_reason(canvas);
        }
        if command == Command::EditCrop {
            return Some("빈 범위가 아닌 선택 범위를 만들어 주세요.");
        }
        if command.is_raster_transform() {
            return if self.editing_layer_mask() {
                document
                    .mask_transform_reason(document.active, &draft.transform)
            } else {
                document.transform_reason(document.active, &draft.transform)
            };
        }
        if self.editing_layer_mask() {
            return Some("레이어의 내용 썸네일을 선택해 주세요.");
        }
        if matches!(command, Command::EditCopy | Command::EditCopySvg) {
            return (document.layers[document.active].kind
                != parfait_core::LayerKind::Raster)
                .then_some("래스터 레이어를 선택해 주세요.");
        }
        if command == Command::EditCut
            && document.layers[document.active].alpha_locked
        {
            return Some("투명 픽셀 잠금을 해제한 후 잘라내 주세요.");
        }
        document.pixel_edit_reason(
            document.active,
            draft.pixel(self.brush.color).unwrap_or(PixelEdit::Fill([0; 4])),
        )
    }

    pub(crate) fn execute_image_edit(
        &mut self,
        command: Command,
        ctx: &egui::Context,
    ) {
        if let Some(reason) = self.image_edit_reason(command) {
            self.notice = Some(reason.into());
            return;
        }
        if matches!(
            command,
            Command::EditScreenColor | Command::EditScreenColorHide
        ) {
            self.start_screen_picker(
                command == Command::EditScreenColorHide,
                None,
                ctx,
            );
            return;
        }
        if command.is_settings_command() {
            self.execute_settings(command, ctx);
            return;
        }
        if command.is_native_command() {
            self.execute_native_command(command);
            return;
        }
        if matches!(
            command,
            Command::EditTextInkBounds | Command::EditVectorCenters
        ) {
            let settings = &mut self.documents.active_session.editing.align;
            if command == Command::EditTextInkBounds {
                settings.text_ink = !settings.text_ink;
            } else {
                settings.vector_center = !settings.vector_center;
            }
            return;
        }
        if command == Command::EditCopySvg {
            self.copy_svg();
            return;
        }
        if matches!(
            command,
            Command::EditTransformApply | Command::EditTransformCancel
        ) {
            self.cancel_transform_preview();
            let draft = self.image_edit.dialog.take().unwrap();
            if command == Command::EditTransformApply
                && !self.apply_image_edit(&draft)
            {
                self.image_edit.dialog = Some(draft);
            }
            return;
        }
        if command.alignment().is_some() {
            self.execute_alignment(command);
            return;
        }
        if let Some(reference) = command.align_reference()
            && command != Command::EditRefGuides
        {
            self.documents.active_session.editing.align.reference = reference;
            return;
        }
        if command == Command::EditPressure {
            self.calibration_dialog =
                Some(crate::calibration::CalibrationDialog::new(
                    self.calibration.clone(),
                ));
            return;
        }
        if matches!(command, Command::EditCut | Command::EditCopy) {
            self.copy_image_with(command, |app, clip| {
                app.system_clipboard()?
                    .set_image(arboard::ImageData {
                        width: clip.width as usize,
                        height: clip.height as usize,
                        bytes: Cow::Borrowed(&clip.pixels),
                    })
                    .map_err(|e| e.to_string())
            });
            return;
        }
        if matches!(command, Command::EditPaste | Command::EditPasteAtView) {
            self.paste_image(command);
            return;
        }
        let mut draft = EditDialog::new(self, command);
        if command.is_tonal() {
            draft.tone_ui.histogram = self
                .documents
                .active_session
                .document
                .tone_histogram(draft.tone_ui.layers);
        }
        if matches!(
            command,
            Command::EditAutoColorHints | Command::EditColorReference
        ) {
            let Some(path) = rfd::FileDialog::new()
                .add_filter("PNG/JPEG", &["png", "jpg", "jpeg"])
                .pick_file()
            else {
                return;
            };
            if !self.reserve_pixel_headroom(128 * 1024 * 1024) {
                self.notice =
                    Some("참조 이미지를 위한 메모리가 부족합니다.".into());
                return;
            }
            match crate::local_materials::read_local_image(&path) {
                Ok(image) => draft.reference = Some(std::sync::Arc::new(image)),
                Err(error) => {
                    self.error = Some(error.to_string());
                    return;
                }
            }
        }
        if command.is_material_registration() {
            draft.materials = self.local_materials();
        }
        if matches!(
            command,
            Command::EditFill
                | Command::EditAutoColor
                | Command::EditClearOutside
                | Command::EditInvert
                | Command::EditCrop
                | Command::EditCanvasRotateRight
                | Command::EditCanvasRotateHalf
                | Command::EditCanvasRotateLeft
                | Command::EditCanvasFlipX
                | Command::EditCanvasFlipY
        ) {
            self.apply_image_edit(&draft);
        } else {
            self.finish_layer_edit();
            self.image_edit.dialog = Some(draft);
        }
        ctx.request_repaint();
    }

    fn system_clipboard(&mut self) -> Result<&mut arboard::Clipboard, String> {
        if self.image_edit.system_clipboard.is_none() {
            self.image_edit.system_clipboard =
                Some(arboard::Clipboard::new().map_err(|e| e.to_string())?);
        }
        Ok(self.image_edit.system_clipboard.as_mut().unwrap())
    }
    fn copy_svg(&mut self) {
        let doc = &self.documents.active_session.document;
        let needed = u64::from(doc.width) * u64::from(doc.height) * 8;
        if !self.reserve_pixel_headroom(needed) {
            self.notice = Some("SVG를 만들 메모리가 부족합니다.".into());
            return;
        }
        let doc = &self.documents.active_session.document;
        let result = if let Some(node) =
            doc.native_content(doc.active).filter(|_| {
                doc.selection.is_none() && doc.layers[doc.active].mask.is_none()
            }) {
            node.svg(doc.width, doc.height)
        } else {
            doc.copy_layer_pixels(doc.active)
                .ok_or("복사할 내용이 없습니다.")
                .and_then(|clip| clip.to_svg())
        };
        let svg = match result {
            Ok(svg) => svg,
            Err(reason) => {
                self.notice = Some(reason.into());
                return;
            }
        };
        match self
            .system_clipboard()
            .and_then(|c| c.set_text(svg).map_err(|e| e.to_string()))
        {
            Ok(()) => {
                self.image_edit.clipboard = None;
                self.notice =
                    Some("활성 레이어 내용을 SVG로 복사했습니다.".into());
            }
            Err(error) => self.error = Some(error),
        }
    }
    fn copy_image_with(
        &mut self,
        command: Command,
        write: impl FnOnce(&mut Self, &PixelClipboard) -> Result<(), String>,
    ) {
        let needed = u64::from(self.documents.active_session.document.width)
            * u64::from(self.documents.active_session.document.height)
            * 8;
        if !self.reserve_pixel_headroom(needed) {
            self.notice = Some("클립보드를 위한 메모리가 부족합니다.".into());
            return;
        }
        let doc = &self.documents.active_session.document;
        let Some(clip) = doc.copy_layer_pixels(doc.active) else {
            self.notice = Some("복사할 픽셀이 없습니다.".into());
            return;
        };
        if let Err(error) = write(self, &clip) {
            self.error =
                Some(format!("클립보드에 복사하지 못했습니다: {error}"));
            return;
        }
        self.image_edit.clipboard = Some(clip);
        if command == Command::EditCut {
            self.edit_named(command.label(), |doc| {
                doc.edit_layer_pixels(doc.active, PixelEdit::Fill([0; 4]));
            });
        }
    }
    fn paste_image(&mut self, command: Command) {
        // Clipboard access happens only on explicit Paste, never while drawing.
        if !self.reserve_pixel_headroom(128 * 1024 * 1024) {
            self.notice = Some("붙여넣기를 위한 메모리가 부족합니다.".into());
            return;
        }
        let image = match self
            .system_clipboard()
            .and_then(|c| c.get_image().map_err(|e| e.to_string()))
        {
            Ok(image) => image,
            Err(error) => {
                self.notice =
                    Some(format!("클립보드 이미지를 읽지 못했습니다: {error}"));
                return;
            }
        };
        if image.width == 0
            || image.height == 0
            || image.width > 4096
            || image.height > 4096
            || image.bytes.len() != image.width * image.height * 4
        {
            self.notice = Some(
                "1~4096픽셀 크기의 RGBA 이미지만 붙여넣을 수 있습니다.".into(),
            );
            return;
        }
        let doc = &self.documents.active_session.document;
        let mut origin = self
            .image_edit
            .clipboard
            .as_ref()
            .filter(|c| {
                c.width as usize == image.width
                    && c.height as usize == image.height
                    && c.pixels.as_slice() == image.bytes.as_ref()
            })
            .map_or(
                [
                    (i64::from(doc.width) - image.width as i64).div_euclid(2)
                        as i32,
                    (i64::from(doc.height) - image.height as i64).div_euclid(2)
                        as i32,
                ],
                |c| c.origin,
            );
        if command == Command::EditPasteAtView
            && let Some(rect) = self.image_edit.canvas_rect
        {
            let center = self.documents.active_session.view.document(
                rect.center(),
                rect,
                egui::vec2(doc.width as f32, doc.height as f32),
            );
            origin = [
                (center.x - image.width as f32 / 2.).round() as i32,
                (center.y - image.height as f32 / 2.).round() as i32,
            ];
        }
        if origin[0] as i64 + image.width as i64 <= 0
            || origin[1] as i64 + image.height as i64 <= 0
            || origin[0] >= doc.width as i32
            || origin[1] >= doc.height as i32
        {
            self.notice =
                Some("붙여넣을 이미지가 캔버스 밖에 있습니다.".into());
            return;
        }
        let clip = PixelClipboard {
            width: image.width as u32,
            height: image.height as u32,
            origin,
            pixels: image.bytes.into_owned(),
        };
        self.edit_named(command.label(), |doc| {
            doc.paste_pixels(&clip, origin);
        });
        let active = self.documents.active_session.document.active;
        self.set_layer_selection(1 << active, active);
        self.reveal_layer(active);
    }

    fn image_edit_budget(&mut self, draft: &EditDialog) -> bool {
        let doc = &self.documents.active_session.document;
        let bytes = if let Some(edit) = draft.canvas(self) {
            if let Some(reason) = doc.canvas_edit_reason(edit) {
                self.notice = Some(reason.into());
                return false;
            }
            doc.canvas_edit_bytes(edit).unwrap()
        } else {
            doc.logical_pixel_bytes().saturating_add(
                if matches!(
                    draft.command,
                    Command::EditAutoShade
                        | Command::EditAutoColor
                        | Command::EditAutoColorHints
                        | Command::EditAutoColorOptions
                ) {
                    u64::from(doc.width)
                        * u64::from(doc.height)
                        * if draft.command == Command::EditAutoShade {
                            8
                        } else {
                            4
                        }
                } else {
                    0
                },
            )
        };
        let actual = doc
            .layers
            .iter()
            .map(|l| {
                l.pixels.len() as u64
                    + l.mask.as_ref().map_or(0, |m| match &m.coverage {
                        parfait_core::MaskCoverage::Dense(v) => v.len() as u64,
                        _ => 0,
                    })
            })
            .sum::<u64>()
            + doc.selection.as_ref().map_or(0, |v| match v {
                parfait_core::MaskCoverage::Dense(v) => v.len() as u64,
                _ => 0,
            });
        let actual = actual
            + if doc.contents.is_empty() {
                0
            } else {
                8 + doc
                    .contents
                    .iter()
                    .filter_map(|n| n.encoded_bytes())
                    .sum::<u64>()
            };
        let overhead = parfait_storage::snapshot_size(doc)
            .unwrap_or(u64::MAX)
            .saturating_sub(actual)
            .saturating_add(
                if matches!(
                    draft.command,
                    Command::EditAutoShade
                        | Command::EditAutoColor
                        | Command::EditAutoColorHints
                        | Command::EditAutoColorOptions
                ) {
                    4096
                } else {
                    0
                },
            );
        if bytes.saturating_add(overhead) > 512 * 1024 * 1024
            || self
                .documents
                .logical_bytes()
                .saturating_sub(doc.logical_pixel_bytes())
                .saturating_add(bytes)
                > self.documents.logical_limit
        {
            self.notice = Some(
                "이 편집은 이미지 또는 열린 문서의 메모리 예산을 초과합니다."
                    .into(),
            );
            return false;
        }
        if !self.reserve_pixel_headroom(
            bytes
                .saturating_mul(2)
                .saturating_add(
                    draft
                        .reference
                        .as_ref()
                        .map_or(0, |c| c.pixels.len() as u64),
                )
                .saturating_add(
                    u64::from(doc.width)
                        * u64::from(doc.height)
                        * if draft.command == Command::EditSuperResolution {
                            16
                        } else {
                            12
                        },
                ),
        ) {
            self.notice =
                Some("실행 취소를 유지할 메모리가 부족합니다.".into());
            return false;
        }
        true
    }
    fn apply_image_edit(&mut self, draft: &EditDialog) -> bool {
        if self.documents.active_key() != Some(draft.owner)
            || self.documents.active_session.document.revision != draft.revision
        {
            self.cancel_transform_preview();
            self.notice =
                Some("대상 그림이 변경되어 편집을 취소했습니다.".into());
            return false;
        }
        if draft.command == Command::EditClearHistory {
            let session = &mut self.documents.active_session;
            session.history =
                parfait_core::History::restored(session.history.state_id());
            return true;
        }
        if draft.command == Command::EditRefGuides {
            let doc = &self.documents.active_session.document;
            if draft.offset[0] < 0
                || draft.offset[1] < 0
                || draft.width == 0
                || draft.height == 0
                || draft.offset[0] as u64 + u64::from(draft.width)
                    > u64::from(doc.width)
                || draft.offset[1] as u64 + u64::from(draft.height)
                    > u64::from(doc.height)
            {
                self.notice = Some(
                    "가이드는 캔버스 안의 비어 있지 않은 범위여야 합니다."
                        .into(),
                );
                return false;
            }
            let settings = &mut self.documents.active_session.editing.align;
            settings.guides = Some(parfait_core::PixelRect {
                x0: draft.offset[0] as u32,
                y0: draft.offset[1] as u32,
                x1: draft.offset[0] as u32 + draft.width,
                y1: draft.offset[1] as u32 + draft.height,
            });
            settings.reference = crate::alignment::AlignReference::Guides;
            return true;
        }
        if draft.command.is_raster_transform()
            && let Some(reason) =
                draft.transform_reason(&self.documents.active_session.document)
        {
            self.notice = Some(reason.into());
            return false;
        }
        if draft.command.is_raster_transform()
            && let Some(next) = draft.cached_transform()
        {
            let current = &self.documents.active_session.document;
            if let Some(reason) = draft.transform_reason(current) {
                self.notice = Some(reason.into());
                return false;
            }
            if !next.validate()
                || parfait_storage::snapshot_size(next)
                    .is_none_or(|n| n > 512 * 1024 * 1024)
                || self
                    .documents
                    .logical_bytes()
                    .saturating_sub(current.logical_pixel_bytes())
                    .saturating_add(next.logical_pixel_bytes())
                    > self.documents.logical_limit
            {
                self.notice = Some(
                    "변형 결과가 그림 또는 저장 예산을 초과합니다.".into(),
                );
                return false;
            }
            if !self.can_reserve_pixel_headroom(transform_canvas::new_buffers(
                current, next,
            )) {
                self.notice =
                    Some("변형 결과를 유지할 메모리가 부족합니다.".into());
                return false;
            }
            let next = next.clone();
            self.edit_named(draft.command.label(), |doc| {
                let name = std::mem::take(&mut doc.name);
                *doc = next;
                doc.name = name;
            });
            return true;
        }
        if let Some(operation) = draft.local_operation(self) {
            if let Some(reason) =
                operation.reason(&self.documents.active_session.document)
            {
                self.notice = Some(reason.into());
                return false;
            }
            let cached = draft
                .local_preview
                .cached
                .as_ref()
                .filter(|(op, _)| *op == operation)
                .map(|(_, doc)| doc);
            if let Some(next) = cached {
                let current = &self.documents.active_session.document;
                if !next.validate()
                    || parfait_storage::snapshot_size(next)
                        .is_none_or(|n| n > 512 * 1024 * 1024)
                    || self
                        .documents
                        .logical_bytes()
                        .saturating_sub(current.logical_pixel_bytes())
                        .saturating_add(next.logical_pixel_bytes())
                        > self.documents.logical_limit
                    || !self.can_reserve_pixel_headroom(
                        local_preview::buffer_growth(current, next),
                    )
                {
                    self.notice =
                        Some("보정 결과를 유지할 메모리가 부족합니다.".into());
                    return false;
                }
                let next = next.clone();
                self.edit_named(draft.command.label(), |doc| {
                    let name = std::mem::take(&mut doc.name);
                    *doc = next;
                    doc.name = name;
                });
            } else {
                if !self.image_edit_budget(draft) {
                    return false;
                }
                self.edit_named(draft.command.label(), |doc| {
                    operation.apply(doc, || false);
                });
            }
            if matches!(
                draft.command,
                Command::EditAutoColor
                    | Command::EditAutoColorHints
                    | Command::EditAutoColorOptions
            ) {
                let active = self.documents.active_session.document.active;
                self.set_layer_selection(1 << active, active);
                self.reveal_layer(active);
            }
            if draft.command == Command::EditAutoShade {
                let active = self.documents.active_session.document.active;
                self.set_layer_selection(1 << active, active);
                self.reveal_layer(active);
            }
            if matches!(operation, local_preview::Operation::Canvas(_)) {
                self.documents
                    .active_session
                    .view
                    .request_zoom(crate::canvas::ViewZoom::Fit);
            }
            return true;
        }
        if !self.image_edit_budget(draft) {
            return false;
        }
        if draft.command.is_material_registration() {
            let kind = if draft.command == Command::EditMaterialImage {
                crate::local_materials::MaterialKind::Image
            } else {
                crate::local_materials::MaterialKind::Template
            };
            return match self.register_local_material(&draft.name, kind) {
                Ok(path) => {
                    self.notice = Some(format!(
                        "로컬 소재를 등록했습니다: {}",
                        path.display()
                    ));
                    true
                }
                Err(error) => {
                    self.notice = Some(error.to_string());
                    false
                }
            };
        }
        if draft.command.is_color_assist() {
            if let Some(reason) =
                draft.assist_reason(&self.documents.active_session.document)
            {
                self.notice = Some(reason.into());
                return false;
            }
            let revision = self.documents.active_session.document.revision;
            self.edit_named(draft.command.label(), |doc| {
                draft.assist_apply(doc);
            });
            if self.documents.active_session.document.revision == revision {
                self.notice=Some("적용할 변경이 없습니다. 열린 영역, 경계값 또는 힌트를 확인해 주세요.".into());
            }
            if matches!(
                draft.command,
                Command::EditAutoColor
                    | Command::EditAutoColorHints
                    | Command::EditAutoColorOptions
            ) {
                let active = self.documents.active_session.document.active;
                self.set_layer_selection(1 << active, active);
                self.reveal_layer(active);
            }
            return true;
        }
        if let Some(edit) = draft.pixel(self.brush.color) {
            if let Some(reason) =
                self.documents.active_session.document.pixel_edit_reason(
                    self.documents.active_session.document.active,
                    edit,
                )
            {
                self.notice = Some(reason.into());
                return false;
            }
            self.edit_named(draft.command.label(), |doc| {
                doc.edit_layer_pixels(doc.active, edit);
            });
        } else if draft.command.is_raster_transform() {
            let doc = &self.documents.active_session.document;
            if let Some(reason) = draft.transform_reason(doc) {
                self.notice = Some(reason.into());
                return false;
            }
            self.edit_named(draft.command.label(), |doc| {
                draft.apply_transform(doc);
            });
        } else if let Some(edit) = draft.canvas(self) {
            self.edit_named(draft.command.label(), |doc| {
                doc.edit_canvas(edit);
            });
            self.documents
                .active_session
                .view
                .request_zoom(crate::canvas::ViewZoom::Fit);
        }
        true
    }

    pub(crate) fn image_edit_dialog_ui(&mut self, ctx: &egui::Context) {
        let Some(mut draft) = self.image_edit.dialog.take() else {
            return;
        };
        if self.documents.active_key() != Some(draft.owner)
            || self.documents.active_session.document.revision != draft.revision
        {
            self.notice =
                Some("대상 그림이 변경되어 편집을 취소했습니다.".into());
            return;
        }
        if draft.command.is_raster_transform() {
            self.transform_window(ctx, draft);
            return;
        }
        if draft.local_operation(self).is_some() {
            self.local_edit_window(ctx, draft);
            return;
        }
        let mut confirm = false;
        let mut cancel = false;
        let mut preview = false;
        let mut material_use = None;
        let response=egui::Modal::new(egui::Id::new("image_edit_dialog")).show(ctx,|ui|{
            ui.set_width(390.);
            ui.heading(draft.command.label());ui.separator();
            egui::ScrollArea::vertical().max_height((ctx.content_rect().height()-200.).max(120.)).show(ui,|ui|{
                use Command::*;
                match draft.command {
                    command if command.is_raster_transform()=>draft.transform_controls(ui),
                    command if command.is_color_assist()=>draft.assist_controls(ui),
                    command if command.is_material_registration()=>{
                        ui.label("로컬 소재 이름");ui.add(egui::TextEdit::singleline(&mut draft.name).char_limit(128));
                        ui.label("이미지는 선택한 범위의 표시 결과를 PNG로, 템플릿은 모든 레이어와 설정을 Parfait 파일로 등록합니다.");
                        ui.collapsing("등록된 소재",|ui|{for (name,kind,path) in &draft.materials{ui.horizontal(|ui|{ui.label(name);if ui.button(if *kind==crate::local_materials::MaterialKind::Template{"새 그림으로 사용"}else{"새 레이어로 사용"}).clicked(){material_use=Some((*kind,path.clone()));}});}});
                    }
                    EditSuperResolution=>{
                        ui.label("학습 모델 없이 반복 역투영으로 저해상도 원본과 일치하도록 색을 보정하는 확대입니다. 새 세부를 생성하는 모델 방식과 결과가 다릅니다.");
                        ui.horizontal(|ui|{ui.label("너비");ui.add(egui::DragValue::new(&mut draft.width).range(1..=4096));ui.label("높이");ui.add(egui::DragValue::new(&mut draft.height).range(1..=4096));});
                        slider(ui,&mut draft.values[0],1. ..=8.,"반복 횟수");slider(ui,&mut draft.values[1],0. ..=1.,"보정 강도");
                    }
                    EditRefGuides=>{
                        ui.label("캔버스 안의 정렬 기준 사각형을 지정합니다. 가이드는 저장되는 그림에 포함되지 않습니다.");
                        ui.horizontal(|ui|{ui.label("X");ui.add(egui::DragValue::new(&mut draft.offset[0]).range(0..=4095));ui.label("Y");ui.add(egui::DragValue::new(&mut draft.offset[1]).range(0..=4095));});
                        ui.horizontal(|ui|{ui.label("너비");ui.add(egui::DragValue::new(&mut draft.width).range(1..=4096));ui.label("높이");ui.add(egui::DragValue::new(&mut draft.height).range(1..=4096));});
                    }
                    EditClearHistory=>{ui.label("이 그림의 실행 취소·다시 실행 기록을 비웁니다. 현재 그림과 파일 저장 상태는 유지됩니다.");}
                    EditResolution|EditCanvasSize|EditCanvasBasic=>{
                        ui.horizontal(|ui|{ui.label("너비");ui.add(egui::DragValue::new(&mut draft.width).range(1..=4096));ui.label("높이");ui.add(egui::DragValue::new(&mut draft.height).range(1..=4096));});
                        if matches!(draft.command,EditCanvasSize|EditCanvasBasic) {
                            ui.label("기존 이미지의 왼쪽 위 위치 (새 캔버스 안의 픽셀 좌표)");
                            ui.horizontal(|ui|{ui.label("X");ui.add(egui::DragValue::new(&mut draft.offset[0]).range(-4096..=4096));ui.label("Y");ui.add(egui::DragValue::new(&mut draft.offset[1]).range(-4096..=4096));});
                            if ui.button("가운데 배치").clicked(){draft.offset=[(draft.width as i32-self.documents.active_session.document.width as i32)/2,(draft.height as i32-self.documents.active_session.document.height as i32)/2];}
                        } else {ui.label("색은 투명도를 고려한 쌍선형 보간, 마스크와 선택 범위는 최근접 보간으로 크기를 변경합니다.");}
                    }
                    EditBrightnessContrast=>{slider(ui,&mut draft.values[0],-100. ..=100.,"밝기");slider(ui,&mut draft.values[1],-100. ..=100.,"대비");}
                    EditHsl=>{slider(ui,&mut draft.values[0],-180. ..=180.,"색조");slider(ui,&mut draft.values[1],-100. ..=100.,"채도");slider(ui,&mut draft.values[2],-100. ..=100.,"명도");}
                    EditPosterize=>slider(ui,&mut draft.values[0],2. ..=255.,"계조 수"),
                    EditThreshold=>slider(ui,&mut draft.values[0],0. ..=255.,"경계값"),
                    EditLevels=>{slider(ui,&mut draft.values[0],0. ..=254.,"입력 검정");slider(ui,&mut draft.values[1],1. ..=255.,"입력 흰색");slider(ui,&mut draft.values[2],0.1..=10.,"감마");}
                    EditToneCurve=>{ui.label("입력 0 / 25 / 50 / 75 / 100%에 대응하는 출력값 (구간 사이 선형 보간)");for (i,value) in draft.values[..5].iter_mut().enumerate(){slider(ui,value,0. ..=1.,&format!("{}%",i*25));}}
                    EditColorBalance=>{for (i,name) in ["시안 ↔ 빨강","마젠타 ↔ 초록","노랑 ↔ 파랑"].iter().enumerate(){slider(ui,&mut draft.values[i],-100. ..=100.,name);}}
                    EditGradientMap=>{ui.horizontal(|ui|{ui.label("어두운 색");ui.color_edit_button_srgb(&mut draft.low);ui.label("밝은 색");ui.color_edit_button_srgb(&mut draft.high);});}
                    EditAdvancedFill=>{slider(ui,&mut draft.values[0],0. ..=100.,"색 불투명도");ui.label("현재 그리기색으로 선택 범위 전체를 채웁니다. 선택이 없으면 레이어 전체에 적용합니다.");}
                    EditBorder=>{slider(ui,&mut draft.values[0],1. ..=128.,"안쪽 테두리 두께 (px)");ui.label("선택 경계 안쪽에 그리기색을 적용합니다. 모서리는 사각형입니다.");}
                    _=>{}
                }
                if !matches!(draft.command,EditClearHistory|EditRefGuides) && !draft.command.is_material_registration(){
                    preview|=ui.button("미리보기 갱신").clicked();
                    if let Some(texture)=&draft.preview {ui.add(egui::Image::new(texture).max_size(egui::vec2(320.,220.)));}
                    ui.label("확인할 때 한 번 적용합니다. 취소하면 그림은 바뀌지 않습니다.");
                }
                if let Some(error)=&draft.error{ui.colored_label(egui::Color32::LIGHT_RED,error);}
            });
            ui.separator();ui.horizontal(|ui|{
                let ok=ui.button("확인");let no=ui.button("취소");
                #[cfg(test)] ui.data_mut(|d|{d.insert_temp(egui::Id::new("image_edit_accept"),ok.rect);d.insert_temp(egui::Id::new("image_edit_cancel"),no.rect);});
                confirm|=ok.clicked();cancel|=no.clicked();
            });
        });
        if response.is_top_modal && !response.any_popup_open {
            cancel |= ctx.input_mut(|i| {
                i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)
            });
            // Enter is left to numeric editors, avoiding accidental dialog confirmation.
        }
        if cancel {
            return;
        }
        if let Some((kind, path)) = material_use {
            if let Err(error) = self.use_local_material(kind, &path) {
                self.error = Some(error.to_string());
            }
            return;
        }
        if preview {
            if self.image_edit_budget(&draft) {
                let mut doc = self.documents.active_session.document.clone();
                let valid = if draft.command.is_color_assist() {
                    draft.assist_reason(&doc).is_none() && {
                        draft.assist_apply(&mut doc);
                        true
                    }
                } else if let Some(edit) = draft.pixel(self.brush.color) {
                    doc.pixel_edit_reason(doc.active, edit).is_none() && {
                        doc.edit_layer_pixels(doc.active, edit);
                        true
                    }
                } else if draft.command.is_raster_transform() {
                    doc.transform_reason(doc.active, &draft.transform).is_none()
                        && {
                            doc.transform_layer_pixels(
                                doc.active,
                                &draft.transform,
                            );
                            true
                        }
                } else if let Some(edit) = draft.canvas(self) {
                    doc.canvas_edit_reason(edit).is_none() && {
                        doc.edit_canvas(edit);
                        true
                    }
                } else {
                    false
                };
                if valid {
                    let pixels = doc.composite();
                    let image = image::RgbaImage::from_raw(
                        doc.width, doc.height, pixels,
                    )
                    .unwrap();
                    let small = image::imageops::thumbnail(&image, 320, 220);
                    draft.preview = Some(ctx.load_texture(
                        "image_edit_preview",
                        egui::ColorImage::from_rgba_unmultiplied(
                            [small.width() as usize, small.height() as usize],
                            small.as_raw(),
                        ),
                        egui::TextureOptions::LINEAR,
                    ));
                    draft.error = None;
                } else {
                    draft.error = Some("입력 값을 확인해 주세요.".into());
                }
            } else {
                draft.error = self.notice.clone();
            }
        }
        if confirm {
            if self.apply_image_edit(&draft) {
                return;
            }
            draft.error = self.notice.clone();
        }
        self.image_edit.dialog = Some(draft);
    }
}
fn slider(
    ui: &mut egui::Ui,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    label: &str,
) {
    crate::widgets::numeric_slider(ui, value, range, label, 2, "");
    #[cfg(test)]
    ui.data_mut(|data| {
        if let Some((track, _)) = data.get_temp::<(egui::Rect, egui::Rect)>(
            egui::Id::new(("numeric_slider", label)),
        ) {
            data.insert_temp(
                egui::Id::new(("image_edit_slider", label)),
                track,
            );
        }
    });
}
