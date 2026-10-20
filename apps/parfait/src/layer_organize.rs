use crate::{app::Parfait, shortcuts::Command, widgets};
use eframe::egui;
use parfait_core::LayerId;
use std::sync::Arc;

#[cfg(test)]
mod tests;

pub(crate) struct RenameLayer {
    target: LayerId,
    revision: u64,
    pixels: Arc<Vec<u8>>,
    draft: String,
    needs_edit: bool,
    focus: bool,
    composing: bool,
}

impl RenameLayer {
    fn error(&self) -> Option<&'static str> {
        if self.draft.trim().is_empty() {
            Some("이름을 입력해 주세요.")
        } else if self.draft.chars().any(char::is_control) {
            Some("이름에 줄바꿈이나 제어 문자를 사용할 수 없습니다.")
        } else if self.needs_edit {
            Some(
                "기존 이름이 128자를 넘습니다. 입력을 수정한 뒤 확인해 주세요.",
            )
        } else {
            None
        }
    }
}

impl Parfait {
    pub(crate) fn execute_layer_organization(&mut self, command: Command) {
        if let Some(reason) = self.command_disabled_reason(command) {
            self.notice = Some(reason.into());
            return;
        }
        if command == Command::RenameLayer {
            self.finish_layer_edit();
            let target = &self.doc.layers[self.doc.active];
            self.rename_layer = Some(RenameLayer {
                target: target.id,
                revision: self.doc.revision,
                pixels: target.pixels.clone(),
                draft: target.name.chars().take(128).collect(),
                needs_edit: target.name.chars().nth(128).is_some(),
                focus: true,
                composing: false,
            });
            return;
        }
        let count = self.doc.layers.len();
        let active = self.doc.active;
        if matches!(
            command,
            Command::SelectLayerAbove | Command::SelectLayerBelow
        ) {
            let destination = if command == Command::SelectLayerAbove {
                (active + 1) % count
            } else {
                (active + count - 1) % count
            };
            self.select_layer(destination);
            return;
        }
        if self.doc.has_non_raster_layers() {
            self.execute_tree_collection(command, 1 << active);
            return;
        }
        let destination = match command {
            Command::MoveActiveLayerUp => active + 1,
            Command::MoveActiveLayerDown => active - 1,
            Command::MoveActiveLayerToTop => count - 1,
            Command::MoveActiveLayerToBottom => 0,
            _ => return,
        };
        let mask = self.selected_layer_mask();
        let mut order: Vec<_> = (0..count).collect();
        order.remove(active);
        order.insert(destination, active);
        self.apply_layer_order_named(order, mask, command.label());
    }

    pub(crate) fn layer_rename_ui(&mut self, ctx: &egui::Context) {
        let Some(dialog) = &self.rename_layer else {
            return;
        };
        if self.error.is_some() {
            return;
        }
        let target = self.doc.layer_index(dialog.target);
        if self.doc.revision != dialog.revision
            || target.is_none_or(|index| {
                !Arc::ptr_eq(&dialog.pixels, &self.doc.layers[index].pixels)
            })
        {
            self.rename_layer = None;
            self.notice =
                Some("대상 레이어가 변경되어 이름 변경을 취소했습니다.".into());
            return;
        }
        let dialog = self.rename_layer.as_mut().unwrap();
        let first_frame = dialog.focus;
        let was_composing = dialog.composing;
        let mut ime_event = false;
        ctx.input(|input| {
            for event in &input.events {
                match event {
                    egui::Event::Ime(egui::ImeEvent::Preedit {
                        text, ..
                    }) => {
                        ime_event = true;
                        dialog.composing = !text.is_empty();
                    }
                    egui::Event::Ime(egui::ImeEvent::Commit(_)) => {
                        ime_event = true;
                        dialog.composing = false;
                    }
                    _ => {}
                }
            }
        });
        let mut confirm = false;
        let mut cancel = false;
        let modal = egui::Modal::new(egui::Id::new("rename_layer_dialog"))
            .show(ctx, |ui| {
                ui.set_width(362.);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("레이어 이름 변경").strong());
                    ui.with_layout(
                        egui::Layout::right_to_left(egui::Align::Center),
                        |ui| {
                            let close = widgets::flat_icon(
                                ui,
                                widgets::Icon::Close,
                                false,
                                "이름 변경 취소",
                                18.,
                            );
                            #[cfg(test)]
                            ui.data_mut(|data| {
                                data.insert_temp(
                                    egui::Id::new("rename_close"),
                                    close.rect,
                                )
                            });
                            cancel |= close.clicked();
                        },
                    );
                });
                ui.separator();
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        ui.set_width(270.);
                        ui.horizontal(|ui| {
                            ui.label("이름:");
                            let field = ui.add(
                                egui::TextEdit::singleline(&mut dialog.draft)
                                    .id(egui::Id::new("rename_layer_name"))
                                    .desired_width(220.)
                                    .char_limit(128)
                                    .return_key(None),
                            );
                            #[cfg(test)]
                            ui.data_mut(|data| {
                                data.insert_temp(
                                    egui::Id::new("rename_field"),
                                    field.rect,
                                )
                            });
                            if dialog.focus {
                                field.request_focus();
                                if let Some(mut state) =
                                    egui::TextEdit::load_state(ctx, field.id)
                                {
                                    state.cursor.set_char_range(Some(
                                        egui::text::CCursorRange::two(
                                            egui::text::CCursor::new(0),
                                            egui::text::CCursor::new(
                                                dialog.draft.chars().count(),
                                            ),
                                        ),
                                    ));
                                    state.store(ctx, field.id);
                                }
                                dialog.focus = false;
                            }
                            if field.changed() {
                                dialog.needs_edit = false;
                            }
                        });
                        if let Some(error) = dialog.error() {
                            ui.colored_label(
                                egui::Color32::from_rgb(235, 160, 151),
                                error,
                            );
                        }
                    });
                    ui.vertical(|ui| {
                        let accept = ui
                            .add_enabled_ui(
                                dialog.error().is_none() && !dialog.composing,
                                |ui| widgets::action_button(ui, "확인", 80.),
                            )
                            .inner;
                        let decline = widgets::action_button(ui, "취소", 80.);
                        #[cfg(test)]
                        ui.data_mut(|data| {
                            data.insert_temp(
                                egui::Id::new("rename_accept"),
                                accept.rect,
                            );
                            data.insert_temp(
                                egui::Id::new("rename_cancel"),
                                decline.rect,
                            );
                        });
                        confirm |= accept.clicked();
                        cancel |= decline.clicked();
                    });
                });
            });
        #[cfg(test)]
        ctx.data_mut(|data| {
            data.insert_temp(egui::Id::new("rename_modal"), modal.response.rect)
        });
        if modal.is_top_modal
            && !modal.any_popup_open
            && !was_composing
            && !dialog.composing
        {
            cancel |= ctx.input_mut(|input| {
                input.consume_key(egui::Modifiers::NONE, egui::Key::Escape)
            });
            if !first_frame && !ime_event {
                confirm |= ctx.input_mut(|input| {
                    input.consume_key(egui::Modifiers::NONE, egui::Key::Enter)
                });
            }
        }
        if cancel {
            self.rename_layer = None;
        } else if confirm && dialog.error().is_none() {
            let dialog = self.rename_layer.take().unwrap();
            let target = target.unwrap();
            if dialog.draft != self.doc.layers[target].name {
                self.edit_named("Rename layer", |document| {
                    document.layers[target].name = dialog.draft
                });
            }
        }
    }
}
