use crate::{app::Parfait, shortcuts::Command};
use parfait_core::SelectionMode;
mod boundary;
pub(crate) use boundary::BoundaryCache;

impl Parfait {
    pub(crate) fn retain_selection_render(&mut self, previous: u64) {
        let revision = self.documents.active_session.document.revision;
        if self.render_cache.rendered == previous {
            self.render_cache.rendered = revision;
        }
        if self.render_cache.preview_revision == previous {
            self.render_cache.preview_revision = revision;
        }
    }
    pub(crate) fn pixel_selection_reason(
        &self,
        command: Command,
    ) -> Option<&'static str> {
        if self.documents.active_session.editing.stroke.is_some()
            || self.documents.active_session.layer_ui.list.owns_pointer()
            || self.dialog_open()
            || self.workspace.dragging.is_some()
            || self.workspace.resizing
        {
            return Some("그리기와 창 조작을 마친 뒤 선택 범위를 바꿔 주세요.");
        }
        if matches!(
            command,
            Command::Deselect
                | Command::InvertSelection
                | Command::LayerSelectionAdd
                | Command::LayerSelectionSubtract
                | Command::LayerSelectionIntersect
        ) && self.documents.active_session.document.selection.is_none()
        {
            return Some("먼저 선택 범위를 만들어 주세요.");
        }
        if matches!(
            command,
            Command::LayerMaskOutsideSelection | Command::LayerMaskSelection
        ) {
            if self.selected_layer_mask().count_ones() != 1 {
                return Some("마스크를 만들 레이어 하나를 선택해 주세요.");
            }
            return parfait_storage::layer_mask_creation_error(
                &self.documents.active_session.document,
                self.documents.active_session.document.active,
            );
        }
        if command == Command::Deselect {
            return None;
        }
        parfait_storage::selection_update_error(
            &self.documents.active_session.document,
            command != Command::SelectAll,
        )
    }

    pub(crate) fn execute_pixel_selection(&mut self, command: Command) {
        if let Some(reason) = self.pixel_selection_reason(command) {
            self.notice = Some(reason.into());
            return;
        }
        let selected = self.selected_layer_mask();
        let revision = self.documents.active_session.document.revision;
        self.edit_named(command.label(), |document| {
            if let Some(mode) = command.selection_mode() {
                document.selection_from_layers(selected, mode);
            } else {
                match command {
                    Command::SelectAll => {
                        document.select_all_pixels();
                    }
                    Command::Deselect => {
                        document.deselect_pixels();
                    }
                    Command::InvertSelection => {
                        document.invert_pixel_selection();
                    }
                    Command::LayerMaskOutsideSelection
                    | Command::LayerMaskSelection => {
                        document.create_mask_from_selection(
                            document.active,
                            command == Command::LayerMaskOutsideSelection,
                        );
                    }
                    _ => unreachable!(),
                }
            }
        });
        if !matches!(
            command,
            Command::LayerMaskOutsideSelection | Command::LayerMaskSelection
        ) {
            self.retain_selection_render(revision);
        }
        if matches!(
            command,
            Command::LayerMaskOutsideSelection | Command::LayerMaskSelection
        ) {
            self.documents.active_session.editing.mask_target = Some(
                self.documents.active_session.document.layers
                    [self.documents.active_session.document.active]
                    .id,
            );
            self.documents.active_session.editing.mask_overlay = None;
        } else if self.documents.active_session.document.selection.is_none()
            && command != Command::Deselect
        {
            self.notice = Some("선택 범위가 작성되지 않았습니다.".into());
        } else if command.selection_mode() == Some(SelectionMode::Replace) {
            self.notice = None;
        }
    }
}

#[cfg(test)]
mod tests;
