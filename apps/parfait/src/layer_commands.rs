use crate::{
    app::Parfait,
    shortcuts::{Category, Command},
    widgets::{self, Icon},
};
use eframe::egui;

#[cfg(test)]
mod tests;

impl Parfait {
    fn tree_command_disabled_reason(&self, command: Command) -> Option<&'static str> {
        self.tree_flag_reason(command)
    }

    pub(crate) fn command_disabled_reason(&self, command: Command) -> Option<&'static str> {
        if self.stroke.is_some() {
            return Some("Finish the current stroke first.");
        }
        if let Some(reason) = self.tree_command_disabled_reason(command) {
            return Some(reason);
        }
        if let Some(conversion) = command.raster_conversion(self.brush.color) {
            return self.raster_conversion_disabled_reason(conversion);
        }
        if command == Command::ClearEditingTarget {
            return self.command_disabled_reason(if self.editing_layer_mask() {
                Command::ClearLayerMask
            } else {
                Command::ClearLayer
            });
        }
        if command.category() == Category::Layer {
            let mask = self.selected_layer_mask();
            if !self.doc.has_folders()
                && command == Command::LayerClipping
                && !self.doc.layers[self.doc.active].clipped
                && self.doc.layers.iter().enumerate().any(|(index, _)| {
                    mask & (1 << index) != 0
                        && !self.doc.layers[..index]
                            .iter()
                            .enumerate()
                            .any(|(below, layer)| mask & (1 << below) == 0 && !layer.clipped)
                })
            {
                return Some("Keep an unclipped base layer below every selected clipping layer.");
            }
            if mask.count_ones() > 1
                && matches!(command, Command::MoveLayerUp | Command::MoveLayerDown)
            {
                if self.stroke.is_some()
                    || self.dialog_open()
                    || self.layer_list.owns_pointer()
                    || self.workspace.dragging.is_some()
                    || self.workspace.resizing
                {
                    return Some("Finish the current gesture or dialog first.");
                }
                let can_move = (0..self.doc.layers.len()).any(|index| {
                    if mask & (1 << index) == 0 {
                        return false;
                    }
                    let next = if command == Command::MoveLayerUp {
                        index
                            .checked_add(1)
                            .filter(|index| *index < self.doc.layers.len())
                    } else {
                        index.checked_sub(1)
                    };
                    next.is_some_and(|index| mask & (1 << index) == 0)
                });
                return if can_move {
                    None
                } else {
                    Some("The selected layers are already at this boundary.")
                };
            }
            if let Some(reason) = self.layer_command_disabled_reason(command, self.doc.active) {
                return Some(reason);
            }
            match command {
                Command::DeleteLayer if mask.count_ones() as usize >= self.doc.layers.len() => {
                    return Some("Keep at least one layer in the drawing.");
                }
                Command::DuplicateLayer => {
                    return parfait_storage::layer_duplication_error(&self.doc, mask);
                }
                _ => {}
            }
            return None;
        }
        match command {
            Command::Undo if !self.history.can_undo() => Some("There is no earlier edit to undo."),
            Command::Redo if !self.history.can_redo() => Some("There is no later edit to redo."),
            _ => None,
        }
    }

    pub(crate) fn layer_command_disabled_reason(
        &self,
        command: Command,
        layer: usize,
    ) -> Option<&'static str> {
        if self.stroke.is_some() {
            return Some("Finish the current stroke first.");
        }
        if self.layer_list.owns_pointer() {
            return Some("Finish the layer drag first.");
        }
        if self.dialog_open() {
            return Some("Close the current dialog before editing layers.");
        }
        if self.workspace.dragging.is_some() || self.workspace.resizing {
            return Some("Finish moving or resizing the palette first.");
        }
        if let Some(action) = command.layer_combine() {
            if matches!(
                action,
                parfait_core::LayerCombine::TransferDown | parfait_core::LayerCombine::MergeDown
            ) && self.selected_layer_mask().count_ones() != 1
            {
                return Some("Select one active layer, or use Merge selected layers.");
            }
            if action == parfait_core::LayerCombine::CopyVisible
                && let Some(reason) =
                    parfait_storage::layer_growth_error(&self.doc, "Visible copy".len())
            {
                return Some(reason);
            }
            return self
                .doc
                .layer_combine_reason(action, self.selected_layer_mask());
        }
        let Some(target) = self.doc.layers.get(layer) else {
            return Some("The target layer no longer exists.");
        };
        if self.doc.has_folders() {
            if matches!(
                command,
                Command::DeleteLayer
                    | Command::MoveLayerUp
                    | Command::MoveLayerDown
                    | Command::MoveActiveLayerUp
                    | Command::MoveActiveLayerDown
                    | Command::MoveActiveLayerToTop
                    | Command::MoveActiveLayerToBottom
            ) {
                return self.tree_collection_reason(command, 1 << layer);
            }
            if command == Command::LayerAlphaLock && target.kind == parfait_core::LayerKind::Folder
            {
                return Some("투명 픽셀 잠금은 픽셀 레이어에서 사용할 수 있습니다.");
            }
            if command == Command::LayerClipping
                && !target.clipped
                && self.doc.layer_clipping_base_below(layer, 0).is_none()
            {
                return Some("같은 폴더 안에 통과 모드가 아닌 클리핑 기반을 남겨 주세요.");
            }
        }
        if command.is_layer_mask() {
            return self.mask_command_disabled_reason(command, layer);
        }
        match command {
            Command::NewLayerFolder
            | Command::GroupLayersInFolder
            | Command::UngroupLayerFolder => self.folder_command_reason(command),
            Command::NewLayer => {
                let parent = if target.kind == parfait_core::LayerKind::Folder {
                    Some(layer)
                } else {
                    target.parent.and_then(|id| self.doc.layer_index(id))
                };
                if parent.is_some_and(|index| !self.doc.layer_ancestors_unlocked(index)) {
                    return Some("Unlock the parent folder before inserting a layer.");
                }
                parfait_storage::layer_growth_error(
                    &self.doc,
                    "Layer ".len() + self.doc.layers.len().ilog10() as usize + 1,
                )
            }
            Command::DuplicateLayer => {
                parfait_storage::layer_duplication_error(&self.doc, 1 << layer)
            }
            Command::DeleteLayer if self.doc.layers.len() == 1 => {
                Some("Keep at least one layer in the drawing.")
            }
            Command::MoveLayerUp | Command::MoveActiveLayerUp | Command::MoveActiveLayerToTop
                if layer + 1 == self.doc.layers.len() =>
            {
                Some("This layer is already at the top.")
            }
            Command::MoveLayerDown
            | Command::MoveActiveLayerDown
            | Command::MoveActiveLayerToBottom
                if layer == 0 =>
            {
                Some("This layer is already at the bottom.")
            }
            Command::SelectLayerAbove | Command::SelectLayerBelow if self.doc.layers.len() == 1 => {
                Some("There is only one layer in the drawing.")
            }
            Command::LayerClipping
                if !target.clipped
                    && !self.doc.layers[..layer].iter().any(|layer| !layer.clipped) =>
            {
                Some("There is no unclipped base layer below this layer.")
            }
            _ => None,
        }
    }

    pub(crate) fn execute_layer_command(&mut self, command: Command, layer: usize) {
        if self.doc.has_folders()
            && matches!(
                command,
                Command::DeleteLayer | Command::MoveLayerUp | Command::MoveLayerDown
            )
        {
            self.execute_tree_collection(command, 1 << layer);
            return;
        }
        if let Some(reason) = self.layer_command_disabled_reason(command, layer) {
            self.notice = Some(reason.into());
            return;
        }
        if !matches!(
            command,
            Command::DeleteLayer
                | Command::MoveLayerUp
                | Command::MoveLayerDown
                | Command::LayerVisibility
                | Command::LayerLock
                | Command::LayerAlphaLock
                | Command::LayerReference
                | Command::LayerDraft
                | Command::LayerClipping
        ) {
            return;
        }
        let active_before = self.doc.active;
        let revision = self.doc.revision;
        self.edit_named(command.label(), |document| match command {
            Command::DeleteLayer => {
                document.layers.remove(layer);
                if document.active > layer {
                    document.active -= 1;
                } else if document.active == layer {
                    document.active = layer.min(document.layers.len() - 1);
                }
            }
            Command::MoveLayerUp | Command::MoveLayerDown => {
                let destination = if command == Command::MoveLayerUp {
                    layer + 1
                } else {
                    layer - 1
                };
                document.layers.swap(layer, destination);
                if document.active == layer {
                    document.active = destination;
                } else if document.active == destination {
                    document.active = layer;
                }
            }
            Command::LayerVisibility => {
                document.layers[layer].visible = !document.layers[layer].visible
            }
            Command::LayerLock => document.layers[layer].locked = !document.layers[layer].locked,
            Command::LayerAlphaLock => {
                document.layers[layer].alpha_locked = !document.layers[layer].alpha_locked
            }
            Command::LayerClipping => {
                document.layers[layer].clipped = !document.layers[layer].clipped
            }
            Command::LayerReference => {
                document.layers[layer].reference = !document.layers[layer].reference
            }
            Command::LayerDraft => document.layers[layer].draft = !document.layers[layer].draft,
            _ => unreachable!(),
        });
        if self.doc.active != active_before && self.doc.revision == revision {
            self.doc.touch();
        }
    }

    pub(crate) fn combine_layer_command(&mut self, command: Command) {
        if let Some(reason) = self.command_disabled_reason(command) {
            self.notice = Some(reason.into());
            return;
        }
        let Some(action) = command.layer_combine() else {
            return;
        };
        let mask = self.selected_layer_mask();
        let revision = self.doc.revision;
        self.edit_named(command.label(), |document| {
            document.combine_layers(action, mask);
        });
        if self.doc.revision != revision {
            self.layer_list = Default::default();
        }
    }

    pub(crate) fn layer_command_icon(&mut self, ui: &mut egui::Ui, icon: Icon, command: Command) {
        if ui.available_size_before_wrap().x < 22. {
            ui.end_row();
        }
        let reason = self.command_disabled_reason(command);
        let response = ui
            .add_enabled_ui(reason.is_none(), |ui| {
                widgets::flat_icon(ui, icon, false, self.keymap.tooltip(command), 22.)
            })
            .inner;
        #[cfg(test)]
        ui.data_mut(|data| {
            data.insert_temp(egui::Id::new(("layer_command", command)), response.rect)
        });
        if response.clicked() {
            self.execute_command(command, ui.ctx());
        }
        if let Some(reason) = reason {
            response.on_disabled_hover_text(reason);
        }
    }
    pub(crate) fn layer_operations_menu(&mut self, ui: &mut egui::Ui) {
        for command in [
            Command::TransferLayerDown,
            Command::MergeLayerDown,
            Command::MergeSelectedLayers,
            Command::MergeVisibleLayers,
            Command::CopyVisibleLayers,
            Command::FlattenImage,
            Command::ClearLayer,
        ] {
            self.command_menu_item(ui, command);
        }
    }
}
