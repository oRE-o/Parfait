use crate::{app::Parfait, shortcuts::Command};
use parfait_core::{LayerId, LayerKind};

#[cfg(test)]
mod tests;

impl Parfait {
    pub(crate) fn folder_command_reason(
        &self,
        command: Command,
    ) -> Option<&'static str> {
        match command {
            Command::NewLayerFolder | Command::GroupLayersInFolder => {
                parfait_storage::layer_folder_growth_error(
                    &self.documents.active_session.document,
                    (command == Command::GroupLayersInFolder)
                        .then(|| self.selected_layer_mask()),
                    "폴더 ".len()
                        + self
                            .documents
                            .active_session
                            .document
                            .next_layer_id
                            .max(1)
                            .ilog10() as usize
                        + 1,
                )
            }
            Command::UngroupLayerFolder => {
                let target = &self.documents.active_session.document.layers
                    [self.documents.active_session.document.active];
                if target.kind != LayerKind::Folder {
                    Some("폴더를 선택해 주세요.")
                } else if !self
                    .documents
                    .active_session
                    .document
                    .layer_ancestors_unlocked(
                        self.documents.active_session.document.active,
                    )
                {
                    Some("폴더와 부모의 잠금을 해제해 주세요.")
                } else if self.documents.active_session.document.layers.len()
                    == 1
                {
                    Some("레이어를 하나 이상 남겨 주세요.")
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn folder_name(&self) -> String {
        format!("폴더 {}", self.documents.active_session.document.next_layer_id)
    }

    pub(crate) fn execute_folder_command(&mut self, command: Command) {
        if let Some(reason) = self.command_disabled_reason(command) {
            self.notice = Some(reason.into());
            return;
        }
        let name = self.folder_name();
        let selection = self.selected_layer_mask();
        self.edit_named(command.label(), |document| match command {
            Command::NewLayerFolder => {
                document.create_layer_folder(name);
            }
            Command::GroupLayersInFolder => {
                document.group_layers_in_folder(selection, name);
            }
            Command::UngroupLayerFolder => {
                document.ungroup_layer_folder(document.active);
            }
            _ => unreachable!(),
        });
        self.set_layer_selection(
            1 << self.documents.active_session.document.active,
            self.documents.active_session.document.active,
        );
        self.reveal_layer(self.documents.active_session.document.active);
    }

    pub(crate) fn tree_collection_reason(
        &self,
        command: Command,
        selection: u64,
    ) -> Option<&'static str> {
        match command {
            Command::DeleteLayer => self
                .documents
                .active_session
                .document
                .layer_tree_delete_reason(selection),
            Command::DuplicateLayer => {
                parfait_storage::layer_duplication_error(
                    &self.documents.active_session.document,
                    selection,
                )
            }
            Command::MoveLayerUp
            | Command::MoveLayerDown
            | Command::MoveActiveLayerUp
            | Command::MoveActiveLayerDown
            | Command::MoveActiveLayerToTop
            | Command::MoveActiveLayerToBottom => {
                let upward = matches!(
                    command,
                    Command::MoveLayerUp
                        | Command::MoveActiveLayerUp
                        | Command::MoveActiveLayerToTop
                );
                let edge = matches!(
                    command,
                    Command::MoveActiveLayerToTop
                        | Command::MoveActiveLayerToBottom
                );
                self.documents
                    .active_session
                    .document
                    .layer_sibling_order(selection, upward, edge)
                    .is_none()
                    .then_some(
                        "움직일 수 있는 같은 폴더의 레이어를 선택해 주세요.",
                    )
            }
            _ => None,
        }
    }

    pub(crate) fn execute_tree_collection(
        &mut self,
        command: Command,
        selection: u64,
    ) {
        if self.documents.active_session.editing.stroke.is_some()
            || self.dialog_open()
            || self.documents.active_session.layer_ui.list.owns_pointer()
            || self.workspace.dragging.is_some()
            || self.workspace.resizing
        {
            return;
        }
        if let Some(reason) = self.tree_collection_reason(command, selection) {
            self.notice = Some(reason.into());
            return;
        }
        if matches!(
            command,
            Command::MoveLayerUp
                | Command::MoveLayerDown
                | Command::MoveActiveLayerUp
                | Command::MoveActiveLayerDown
                | Command::MoveActiveLayerToTop
                | Command::MoveActiveLayerToBottom
        ) {
            let upward = matches!(
                command,
                Command::MoveLayerUp
                    | Command::MoveActiveLayerUp
                    | Command::MoveActiveLayerToTop
            );
            let edge = matches!(
                command,
                Command::MoveActiveLayerToTop
                    | Command::MoveActiveLayerToBottom
            );
            let order = self
                .documents
                .active_session
                .document
                .layer_sibling_order(selection, upward, edge)
                .unwrap();
            self.apply_layer_order_named(
                order,
                self.selected_layer_mask(),
                command.label(),
            );
        } else {
            let mut copied = None;
            self.edit_named(command.label(), |document| {
                if command == Command::DeleteLayer {
                    document.delete_layer_subtrees(selection);
                } else {
                    copied = document.duplicate_layer_subtrees(selection);
                }
            });
            self.set_layer_selection(
                copied.unwrap_or(
                    1 << self.documents.active_session.document.active,
                ),
                self.documents.active_session.document.active,
            );
        }
        self.reveal_layer(self.documents.active_session.document.active);
    }

    pub(crate) fn tree_flag_reason(
        &self,
        command: Command,
    ) -> Option<&'static str> {
        if !self.documents.active_session.document.has_non_raster_layers() {
            return None;
        }
        let selection = self.selected_layer_mask();
        if matches!(
            command,
            Command::LayerClipping
                | Command::LayerReference
                | Command::LayerDraft
        ) && self
            .documents
            .active_session
            .document
            .layers
            .iter()
            .enumerate()
            .any(|(index, layer)| {
                selection & (1 << index) != 0 && layer.kind.is_paper()
            })
        {
            return Some(
                "용지는 클리핑이나 참조, 밑그림으로 설정할 수 없습니다.",
            );
        }
        match command {
            Command::DeleteLayer
            | Command::DuplicateLayer
            | Command::MoveLayerUp
            | Command::MoveLayerDown => self.tree_collection_reason(command, selection),
            Command::MoveActiveLayerUp
            | Command::MoveActiveLayerDown
            | Command::MoveActiveLayerToTop
            | Command::MoveActiveLayerToBottom => {
                self.tree_collection_reason(command, 1 << self.documents.active_session.document.active)
            }
            Command::LayerAlphaLock
                if self.documents.active_session.document.layers.iter().enumerate().any(|(index, layer)| {
                    selection & (1 << index) != 0 && layer.kind != LayerKind::Raster
                }) =>
            {
                Some("투명 픽셀 잠금은 픽셀 레이어에서 사용할 수 있습니다.")
            }
            Command::LayerClipping
                if !self.documents.active_session.document.layers
                    [self.documents.active_session.document.active]
                    .clipped =>
            {
                self.documents
                    .active_session
                    .document
                    .layers
                    .iter()
                    .enumerate()
                    .any(|(index, _)| {
                        selection & (1 << index) != 0
                            && self
                                .documents
                                .active_session
                                .document
                                .layer_clipping_base_below(index, selection)
                                .is_none()
                    })
                    .then_some("같은 폴더 안에 통과 모드가 아닌 클리핑 기반을 남겨 주세요.")
            }
            _ => None,
        }
    }

    pub(crate) fn move_tree_selection(
        &mut self,
        selection: u64,
        boundary: usize,
        parent: Option<LayerId>,
    ) {
        if let Some(reason) = parfait_storage::layer_reparent_error(
            &self.documents.active_session.document,
            selection,
            boundary,
            parent,
        ) {
            self.notice = Some(reason.into());
            return;
        }
        let selected = self.selected_layer_mask();
        let identities: Vec<_> = self
            .documents
            .active_session
            .document
            .layers
            .iter()
            .enumerate()
            .filter(|(index, _)| selected & (1 << index) != 0)
            .map(|(_, layer)| layer.id)
            .collect();
        self.edit_named("Move layer subtrees", |document| {
            document.move_layer_subtrees(selection, boundary, parent);
        });
        let selected = self
            .documents
            .active_session
            .document
            .layers
            .iter()
            .enumerate()
            .filter(|(_, layer)| identities.contains(&layer.id))
            .fold(0, |mask, (index, _)| mask | (1 << index));
        self.set_layer_selection(
            selected,
            self.documents.active_session.document.active,
        );
        self.reveal_layer(self.documents.active_session.document.active);
    }

    pub(crate) fn group_layer_drop_reason(
        &self,
        selection: u64,
        target: usize,
    ) -> Option<&'static str> {
        let document = &self.documents.active_session.document;
        let Some(layer) = document.layers.get(target) else {
            return Some("대상 레이어가 없습니다.");
        };
        if layer.kind == LayerKind::Folder || layer.kind.is_paper() {
            return Some("일반 레이어 위에 놓아 새 폴더로 묶어 주세요.");
        }
        if document
            .layer_selection_subtrees(selection)
            .is_none_or(|mask| mask & (1 << target) != 0)
        {
            return Some("선택하지 않은 레이어 위에 놓아 주세요.");
        }
        parfait_storage::layer_folder_growth_error(
            document,
            Some(selection | (1 << target)),
            self.folder_name().len(),
        )
    }

    pub(crate) fn group_layer_drop(&mut self, selection: u64, target: usize) {
        if let Some(reason) = self.group_layer_drop_reason(selection, target) {
            self.notice = Some(reason.into());
            return;
        }
        let name = self.folder_name();
        self.edit_named("레이어를 새 폴더로 묶기", |document| {
            document.group_layers_in_folder(selection | (1 << target), name);
        });
        let active = self.documents.active_session.document.active;
        self.set_layer_selection(1 << active, active);
        self.reveal_layer(active);
    }
}
