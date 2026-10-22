use crate::{app::Parfait, shortcuts::Command};
use eframe::egui;
use parfait_core::{BrushOperation, Point, Stroke};

impl Parfait {
    pub(crate) fn active_mask_overlay(&self) -> Option<parfait_core::LayerId> {
        self.documents
            .active_session
            .document
            .layers
            .get(self.documents.active_session.document.active)
            .filter(|layer| {
                self.documents.active_session.editing.mask_overlay
                    == Some(layer.id)
                    && layer.mask.is_some()
            })
            .map(|layer| layer.id)
    }

    pub(crate) fn layer_target_thumbnail(
        &mut self,
        ui: &mut egui::Ui,
        layer: usize,
        rect: egui::Rect,
        mask: bool,
    ) {
        let selected = layer == self.documents.active_session.document.active
            && self.editing_layer_mask() == mask;
        if mask {
            if let Some(Some(texture)) =
                self.render_cache.mask_previews.get(layer)
            {
                ui.painter().image(
                    texture.id(),
                    rect,
                    egui::Rect::from_min_max(
                        egui::Pos2::ZERO,
                        egui::pos2(1., 1.),
                    ),
                    egui::Color32::WHITE,
                );
            } else {
                ui.painter().rect_filled(
                    rect,
                    0.,
                    egui::Color32::from_gray(115),
                );
            }
            if self.documents.active_session.document.layers[layer]
                .mask
                .as_ref()
                .is_some_and(|mask| !mask.enabled)
            {
                let inset = rect.shrink(3.);
                let stroke =
                    egui::Stroke::new(2., egui::Color32::from_rgb(235, 96, 96));
                ui.painter().line_segment(
                    [inset.left_top(), inset.right_bottom()],
                    stroke,
                );
                ui.painter().line_segment(
                    [inset.right_top(), inset.left_bottom()],
                    stroke,
                );
            }
        }
        ui.painter().rect_stroke(
            rect.expand(1.),
            0.,
            egui::Stroke::new(
                if selected { 2. } else { 1. },
                if selected {
                    egui::Color32::WHITE
                } else {
                    egui::Color32::from_gray(38)
                },
            ),
            egui::StrokeKind::Inside,
        );
        let response = ui.interact(
            rect,
            ui.id().with((
                "layer_target",
                self.documents.active_session.document.layers[layer].id,
                mask,
            )),
            egui::Sense::click(),
        );
        response.widget_info(|| {
            egui::WidgetInfo::selected(
                egui::WidgetType::SelectableLabel,
                ui.is_enabled(),
                selected,
                if mask { "Edit layer mask" } else { "Edit layer content" },
            )
        });
        if response.clicked() {
            self.select_layer_target(layer, mask);
            ui.ctx().request_repaint();
        }
        #[cfg(test)]
        ui.data_mut(|data| {
            data.insert_temp(egui::Id::new(("layer_target", layer, mask)), rect)
        });
        response.on_hover_text(if mask {
            "Edit layer mask: Paint restores coverage; Erase hides it. Drawing color is ignored. Enable a disabled mask before drawing. Blend, blur and smudge are not supported on masks."
        } else {
            "Edit original layer content. Drag the layer name to reorder."
        });
    }

    pub(crate) fn editing_layer_mask(&self) -> bool {
        self.documents
            .active_session
            .document
            .layers
            .get(self.documents.active_session.document.active)
            .is_some_and(|layer| {
                self.documents.active_session.editing.mask_target
                    == Some(layer.id)
                    && layer.mask.is_some()
            })
    }

    pub(crate) fn select_layer_target(&mut self, layer: usize, mask: bool) {
        if self.documents.active_session.editing.stroke.is_some()
            || self.dialog_open()
            || self.workspace.dragging.is_some()
            || self.workspace.resizing
            || self.documents.active_session.layer_ui.list.owns_pointer()
            || layer >= self.documents.active_session.document.layers.len()
            || (mask
                && self.documents.active_session.document.layers[layer]
                    .mask
                    .is_none())
        {
            return;
        }
        self.select_layer(layer);
        self.set_layer_selection(1 << layer, layer);
        self.documents.active_session.editing.mask_target = mask
            .then_some(self.documents.active_session.document.layers[layer].id);
    }

    pub(crate) fn mask_command_disabled_reason(
        &self,
        command: Command,
        layer: usize,
    ) -> Option<&'static str> {
        let target =
            self.documents.active_session.document.layers.get(layer)?;
        if matches!(
            command,
            Command::LayerMaskRevealAll | Command::LayerMaskHideAll
        ) {
            return parfait_storage::layer_mask_creation_error(
                &self.documents.active_session.document,
                layer,
            );
        }
        if target.mask.is_none() {
            return Some("This layer has no mask.");
        }
        if command == Command::ClearLayerMask
            && self.documents.active_session.document.selection.is_some()
            && let Some(reason) = parfait_storage::mask_edit_error(
                &self.documents.active_session.document,
                layer,
            )
        {
            return Some(reason);
        }
        if !matches!(command, Command::LayerMaskEnabled | Command::ShowMaskArea)
            && !self
                .documents
                .active_session
                .document
                .layer_ancestors_unlocked(layer)
        {
            return Some("Unlock the layer before changing its mask.");
        }
        if command == Command::ApplyLayerMask
            && self
                .documents
                .active_session
                .document
                .native_content(layer)
                .is_some()
        {
            return Some("텍스트·벡터를 먼저 래스터화해 주세요 (레이어 메뉴).");
        }
        if command == Command::ApplyLayerMask
            && target.kind == parfait_core::LayerKind::Folder
        {
            return Some(
                "폴더 마스크 적용은 래스터화 기능을 연결한 뒤 지원합니다.",
            );
        }
        if command == Command::ApplyLayerMask && target.alpha_locked {
            return Some("Unlock transparent pixels before applying the mask.");
        }
        if command == Command::ClearLayerMask
            && target.mask.as_ref().is_some_and(|mask| {
                matches!(
                    mask.coverage,
                    parfait_core::MaskCoverage::Uniform(255)
                )
            })
        {
            return Some("This mask already reveals the entire layer.");
        }
        None
    }

    pub(crate) fn execute_mask_command(&mut self, command: Command) {
        if !command.is_layer_mask() {
            return;
        }
        if let Some(reason) = self.command_disabled_reason(command) {
            self.notice = Some(reason.into());
            return;
        }
        let layer = self.documents.active_session.document.active;
        if command == Command::ShowMaskArea {
            self.documents.active_session.editing.mask_overlay = if self
                .active_mask_overlay()
                .is_some()
            {
                None
            } else {
                Some(self.documents.active_session.document.layers[layer].id)
            };
            return;
        }
        self.edit_named(command.label(), |document| match command {
            Command::LayerMaskRevealAll | Command::LayerMaskHideAll => {
                document.create_layer_mask(
                    layer,
                    if command == Command::LayerMaskRevealAll {
                        255
                    } else {
                        0
                    },
                );
            }
            Command::DeleteLayerMask => {
                document.remove_layer_mask(layer);
            }
            Command::ApplyLayerMask => {
                document.apply_layer_mask(layer);
            }
            Command::LayerMaskEnabled => {
                let enabled =
                    !document.layers[layer].mask.as_ref().unwrap().enabled;
                document.set_layer_mask_enabled(layer, enabled);
            }
            Command::ClearLayerMask => {
                document.clear_layer_mask(layer);
            }
            _ => unreachable!(),
        });
        if matches!(
            command,
            Command::LayerMaskRevealAll | Command::LayerMaskHideAll
        ) {
            self.documents.active_session.editing.mask_target =
                Some(self.documents.active_session.document.layers[layer].id);
        } else if matches!(
            command,
            Command::DeleteLayerMask | Command::ApplyLayerMask
        ) {
            self.documents.active_session.editing.mask_target = None;
            self.documents.active_session.editing.mask_overlay = None;
        }
    }

    pub(crate) fn mask_brush_disabled_reason(&self) -> Option<&'static str> {
        if let Some(reason) = parfait_storage::mask_edit_error(
            &self.documents.active_session.document,
            self.documents.active_session.document.active,
        ) {
            return Some(reason);
        }
        let target = &self.documents.active_session.document.layers
            [self.documents.active_session.document.active];
        if target.mask.as_ref().is_none_or(|mask| !mask.enabled) {
            return Some("Enable the layer mask before drawing on it.");
        }
        if !matches!(
            self.brush.operation(),
            BrushOperation::Paint | BrushOperation::Erase
        ) {
            return Some(
                "Mask editing supports Paint to restore and Erase to hide. Select the content thumbnail to blend, blur or smudge.",
            );
        }
        None
    }

    pub(crate) fn begin_canvas_stroke(&mut self, point: Point) -> bool {
        let Some(target) = self.documents.active_key() else {
            return false;
        };
        if !self.editing_layer_mask()
            && self
                .documents
                .active_session
                .document
                .native_content(self.documents.active_session.document.active)
                .is_some()
        {
            self.notice = Some(
                "텍스트·벡터에 그리려면 먼저 래스터화해 주세요 (레이어 메뉴)."
                    .into(),
            );
            return false;
        }
        let session = self.documents.get_by_key(target).unwrap();
        if session.editing.stroke.is_some()
            || self.dialog_open()
            || (session.document.layers[session.document.active].kind
                != parfait_core::LayerKind::Raster
                && !self.editing_layer_mask())
            || !session
                .document
                .layer_ancestors_allow_edit(session.document.active)
        {
            return false;
        }
        let mask = self.editing_layer_mask();
        if mask && let Some(reason) = self.mask_brush_disabled_reason() {
            self.notice = Some(reason.into());
            return false;
        }
        self.finish_layer_edit_for(target);
        if !self.begin_history_for(
            target,
            &format!(
                "{}: {}",
                if mask { "Mask brush" } else { "Brush" },
                self.tip_name
            ),
        ) {
            return false;
        }
        let mut brush = self.brush.clone();
        if self.tools.color_slot == 2 {
            brush.erase = true;
        }
        let session = self.documents.get_by_key_mut(target).unwrap();
        session.editing.stroke = if mask {
            Stroke::begin_mask(&mut session.document, brush, point)
        } else {
            Some(Stroke::begin(&mut session.document, brush, point))
        };
        session.editing.stroke_owner =
            session.editing.stroke.as_ref().map(|_| target);
        if session.editing.stroke.is_none() {
            session.commit_edit();
        }
        session.editing.stroke.is_some()
    }

    pub(crate) fn layer_mask_menu(&mut self, ui: &mut egui::Ui) {
        for command in [Command::LayerMaskRevealAll, Command::LayerMaskHideAll]
        {
            self.command_menu_item(ui, command);
        }
        ui.separator();
        for command in [
            Command::DeleteLayerMask,
            Command::ApplyLayerMask,
            Command::ClearLayerMask,
        ] {
            self.command_menu_item(ui, command);
        }
        let enabled = self.documents.active_session.document.layers
            [self.documents.active_session.document.active]
            .mask
            .as_ref()
            .is_some_and(|mask| mask.enabled);
        self.command_toggle(ui, Command::LayerMaskEnabled, enabled);
        self.command_toggle(
            ui,
            Command::ShowMaskArea,
            self.active_mask_overlay().is_some(),
        );
    }
}

#[cfg(test)]
mod tests;
