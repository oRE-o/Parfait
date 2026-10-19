use crate::{app::Parfait, shortcuts::Command, widgets};
use eframe::egui;
use parfait_core::Layer;
use std::sync::Arc;

pub(crate) struct LayerDetails {
    draft: Layer,
    revision: u64,
    focus: bool,
    composing: bool,
    needs_edit: bool,
    max_name_bytes: usize,
}

impl LayerDetails {
    fn error(&self) -> Option<&'static str> {
        if self.draft.name.trim().is_empty() {
            Some("이름을 입력해 주세요.")
        } else if self.draft.name.chars().any(char::is_control) {
            Some("이름에 제어 문자를 사용할 수 없습니다.")
        } else if self.needs_edit || self.draft.name.chars().count() > 128 {
            Some("기존 이름이 128자를 넘습니다. 이름을 수정해 주세요.")
        } else if !self.draft.opacity.is_finite() || !(0.0..=1.0).contains(&self.draft.opacity) {
            Some("불투명도는 0부터 100% 사이의 숫자로 입력해 주세요.")
        } else if self.draft.name.len() > self.max_name_bytes {
            Some("이 이름은 작업 파일의 저장 크기 한도를 넘습니다.")
        } else if self.draft.kind != parfait_core::LayerKind::Folder
            && self.draft.blend_mode == parfait_core::BlendMode::Through
        {
            Some("통과 합성은 폴더에서만 사용할 수 있습니다.")
        } else {
            None
        }
    }
}

impl Parfait {
    pub(crate) fn open_layer_details(&mut self) {
        if let Some(reason) = self.command_disabled_reason(Command::LayerDetails) {
            self.notice = Some(reason.into());
            return;
        }
        self.finish_layer_edit();
        let mut draft = self.doc.layers[self.doc.active].clone();
        let metadata_bytes = parfait_storage::snapshot_size(&self.doc)
            .and_then(|bytes| bytes.checked_sub(draft.name.len() as u64))
            .unwrap_or(u64::MAX);
        let max_name_bytes = (512 * 1024 * 1024u64).saturating_sub(metadata_bytes) as usize;
        let needs_edit = draft.name.chars().nth(128).is_some();
        draft.name = draft.name.chars().take(128).collect();
        self.layer_details = Some(LayerDetails {
            draft,
            revision: self.doc.revision,
            focus: true,
            composing: false,
            needs_edit,
            max_name_bytes,
        });
    }

    pub(crate) fn layer_details_ui(&mut self, ctx: &egui::Context) {
        if self.error.is_some() {
            return;
        }
        let Some(dialog) = &self.layer_details else {
            return;
        };
        let target = self.doc.layer_index(dialog.draft.id);
        if self.doc.revision != dialog.revision
            || target.is_none_or(|index| {
                !Arc::ptr_eq(&dialog.draft.pixels, &self.doc.layers[index].pixels)
            })
        {
            self.layer_details = None;
            self.notice = Some("대상 레이어가 변경되어 설정을 취소했습니다.".into());
            return;
        }
        let target = target.unwrap();
        let clipping_allowed = self.doc.layers[target].clipped
            || self.doc.layer_clipping_base_below(target, 0).is_some();
        let dialog = self.layer_details.as_mut().unwrap();
        let first_frame = dialog.focus;
        let was_composing = dialog.composing;
        let mut ime_event = false;
        ctx.input(|input| {
            for event in &input.events {
                match event {
                    egui::Event::Ime(egui::ImeEvent::Preedit { text, .. }) => {
                        dialog.composing = !text.is_empty();
                        ime_event = true;
                    }
                    egui::Event::Ime(egui::ImeEvent::Commit(_)) => {
                        dialog.composing = false;
                        ime_event = true;
                    }
                    _ => {}
                }
            }
        });
        let mut confirm = false;
        let mut cancel = false;
        let modal = egui::Modal::new(egui::Id::new("layer_details_modal")).show(ctx, |ui| {
            ui.set_width(380.);
            ui.horizontal(|ui| {
                ui.strong("레이어 기본 설정");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    cancel |= widgets::action_button(ui, "×", 24.).clicked();
                });
            });
            ui.separator();
            egui::ScrollArea::vertical().max_height(350.).show(ui, |ui| {
                egui::Grid::new("layer_details_fields").num_columns(2).spacing([12., 9.]).show(ui, |ui| {
                    ui.label("이름");
                    let field = ui.add(egui::TextEdit::singleline(&mut dialog.draft.name).char_limit(128).desired_width(260.));
                    #[cfg(test)]
                    ui.data_mut(|data| data.insert_temp(egui::Id::new("layer_details_name"), field.rect));
                    if dialog.focus {
                        field.request_focus();
                        if let Some(mut state) = egui::TextEdit::load_state(ctx, field.id) {
                            state.cursor.set_char_range(Some(egui::text::CCursorRange::two(
                                egui::text::CCursor::new(0), egui::text::CCursor::new(dialog.draft.name.chars().count()),
                            )));
                            state.store(ctx, field.id);
                        }
                        dialog.focus = false;
                    }
                    if field.changed() { dialog.needs_edit = false; }
                    ui.end_row();
                    ui.label("합성 모드");
                    let response = crate::layer_controls::blend_select(ui, &mut dialog.draft.blend_mode, dialog.draft.kind == parfait_core::LayerKind::Folder, dialog.draft.clipped, 260.);
                    #[cfg(test)]
                    ui.data_mut(|data| data.insert_temp(egui::Id::new("layer_details_blend"), response.rect));
                    #[cfg(not(test))]
                    let _ = response;
                    ui.end_row();
                    ui.label("불투명도");
                    ui.scope_builder(egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(ui.cursor().min, egui::vec2(260., 20.))), |ui| {
                        crate::layer_controls::opacity(ui, &mut dialog.draft.opacity);
                    });
                    ui.end_row();
                });
                ui.separator();
                ui.checkbox(&mut dialog.draft.visible, "레이어 표시");
                ui.checkbox(&mut dialog.draft.locked, "레이어 잠금");
                ui.add_enabled(dialog.draft.kind == parfait_core::LayerKind::Raster, egui::Checkbox::new(&mut dialog.draft.alpha_locked, "투명 픽셀 잠금"));
                ui.add_enabled_ui(clipping_allowed || dialog.draft.clipped, |ui| {
                    ui.checkbox(&mut dialog.draft.clipped, "아래 레이어에서 클리핑");
                });
                ui.checkbox(&mut dialog.draft.reference, "참조 레이어").on_hover_text("참조 대상으로 표시합니다. 참조 채우기와 경계 넘침 방지는 아직 지원하지 않습니다.");
                ui.checkbox(&mut dialog.draft.draft, "밑그림 레이어").on_hover_text("캔버스와 작업 파일에는 남고 PNG 출력에서는 제외됩니다.");
                ui.small("활성 레이어 하나에 적용합니다. 픽셀과 마스크는 바꾸지 않습니다.");
                if let Some(error) = dialog.error() { ui.colored_label(egui::Color32::LIGHT_RED, error); }
            });
            ui.separator();
            widgets::action_row(ui, |ui| {
                let accept = ui.add_enabled_ui(dialog.error().is_none() && !dialog.composing, |ui| widgets::action_button(ui, "확인", 80.)).inner;
                let decline = widgets::action_button(ui, "취소", 80.);
                confirm |= accept.clicked();
                cancel |= decline.clicked();
                #[cfg(test)]
                ui.data_mut(|data| {
                    data.insert_temp(egui::Id::new("layer_details_accept"), accept.rect);
                    data.insert_temp(egui::Id::new("layer_details_cancel"), decline.rect);
                });
            });
        });
        #[cfg(test)]
        ctx.data_mut(|data| {
            data.insert_temp(egui::Id::new("layer_details_bounds"), modal.response.rect)
        });
        if modal.is_top_modal && !modal.any_popup_open && !was_composing && !dialog.composing {
            cancel |=
                ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
            if !first_frame && !ime_event {
                confirm |= ctx
                    .input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
            }
        }
        if cancel {
            self.layer_details = None;
        } else if confirm && dialog.error().is_none() && (!dialog.draft.clipped || clipping_allowed)
        {
            let dialog = self.layer_details.take().unwrap();
            self.edit_named("Layer details", |document| {
                let target = &mut document.layers[target];
                target.name = dialog.draft.name;
                target.opacity = dialog.draft.opacity;
                target.blend_mode = dialog.draft.blend_mode;
                target.visible = dialog.draft.visible;
                target.locked = dialog.draft.locked;
                target.alpha_locked = dialog.draft.alpha_locked;
                target.clipped = dialog.draft.clipped;
                target.reference = dialog.draft.reference;
                target.draft = dialog.draft.draft;
            });
        }
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod through_tests;
