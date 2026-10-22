use crate::{app::Parfait, shortcuts::Command, theme::ACCENT};
use eframe::egui::{self, Color32, RichText};

mod catalog;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum TopMenu {
    File,
    Edit,
    Layer,
    Select,
    View,
    Filter,
    Window,
    Help,
}

impl TopMenu {
    const ALL: [Self; 8] = [
        Self::File,
        Self::Edit,
        Self::Layer,
        Self::Select,
        Self::View,
        Self::Filter,
        Self::Window,
        Self::Help,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::File => "파일",
            Self::Edit => "편집",
            Self::Layer => "레이어",
            Self::Select => "선택",
            Self::View => "표시",
            Self::Filter => "필터",
            Self::Window => "창",
            Self::Help => "도움말",
        }
    }
}

impl Parfait {
    pub(crate) fn menu_bar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("menu").show(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                self.application_menu(ui);
                for menu in TopMenu::ALL {
                    let response = ui.menu_button(menu.label(), |ui| {
                        ui.set_min_width(188.);
                        self.top_menu_contents(ui, menu);
                    });
                    #[cfg(test)]
                    ui.data_mut(|data| {
                        data.insert_temp(egui::Id::new(("top_menu", menu)), response.response.rect);
                        if response.inner.is_some() {
                            data.insert_temp(egui::Id::new("open_top_menu"), menu);
                        }
                    });
                    let _ = response;
                }
                let status_width = 310.;
                self.window_drag(ui, (ui.available_width() - status_width).max(1.), false);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    self.window_controls(ui);
                    let color = if self.documents.is_empty() || self.tabs.enabled { Color32::from_gray(184) } else if self.documents.active_session.recovery.failed {
                        Color32::LIGHT_RED
                    } else if self.documents.active_session.recovery.saved >= self.documents.active_session.document.revision {
                        Color32::from_gray(184)
                    } else {
                        ACCENT
                    };
                    let response = ui.label(RichText::new(self.recovery_status()).color(color).size(11.));
                    response.clone().on_hover_text("앱을 다시 열 때 쓰는 복구 기록입니다. 작업 파일 저장과는 별도입니다.");
                    #[cfg(test)]
                    ui.data_mut(|data| {
                        data.insert_temp(egui::Id::new("menu_save_status"), response.rect)
                    });
                    let _ = response;
                    ui.separator();
                    let response = ui.label(RichText::new(self.project_status()).color(
                        if self.project_is_modified() { ACCENT } else { Color32::from_gray(184) }
                    ).size(11.));
                    response.clone().on_hover_text("작업 파일에 마지막으로 저장한 상태와 비교합니다. 복구 저장이 끝나도 파일의 수정 표시는 남습니다.");
                    #[cfg(test)]
                    ui.data_mut(|data| data.insert_temp(egui::Id::new("project_save_status"), response.rect));
                });
            });
        });
    }

    fn application_menu(&mut self, ui: &mut egui::Ui) {
        let response = ui.menu_button("   ", |ui| {
            ui.set_min_width(250.);
            let enabled = !self.dialog_open();
            ui.add_enabled_ui(enabled, |ui| {
                if ui.button("Parfait에 대하여…").clicked() {
                    self.settings.about_open = true;
                    ui.close();
                }
                ui.separator();
                for command in [
                    Command::EditPreferences,
                    Command::EditPrivacySettings,
                    Command::EditCommandBarSettings,
                    Command::ShortcutSettings,
                    Command::EditModifierSettings,
                ] {
                    self.top_menu_command(ui, command);
                }
                ui.separator();
                self.top_menu_command(ui, Command::EditFontSettings);
                ui.separator();
                ui.menu_button("외부 컨트롤러", |ui| {
                    for command in [
                        Command::EditControllerSettings,
                        Command::EditQuickSettings,
                        Command::EditToolCycleSettings,
                        Command::EditExternalDevice,
                        Command::EditControllerAdd,
                    ] {
                        self.top_menu_command(ui, command);
                    }
                });
                self.top_menu_command(ui, Command::EditPressure);
                self.top_menu_command(ui, Command::EditSystemServices);
                ui.separator();
                self.top_menu_command(ui, Command::Notices);
                ui.separator();
                self.top_menu_command(ui, Command::Quit);
            });
        });
        let center = response.response.rect.center();
        ui.painter().circle_filled(
            center,
            9.,
            Color32::from_rgb(153, 170, 194),
        );
        ui.painter().text(
            center,
            egui::Align2::CENTER_CENTER,
            "P",
            egui::FontId::proportional(13.),
            Color32::from_gray(34),
        );
        response.response.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::WidgetType::Button,
                true,
                "Parfait 설정 메뉴",
            )
        });
        #[cfg(test)]
        ui.data_mut(|data| {
            data.insert_temp(
                egui::Id::new("application_menu"),
                response.response.rect,
            )
        });
    }

    fn top_menu_contents(&mut self, ui: &mut egui::Ui, menu: TopMenu) {
        let caption = ui.label(
            RichText::new("미구현 표시 항목은 준비 중입니다.").small().weak(),
        );
        #[cfg(test)]
        ui.data_mut(|data| {
            data.insert_temp(
                egui::Id::new(("catalog_caption", menu)),
                caption.rect,
            );
        });
        let _ = caption;
        ui.separator();
        if menu == TopMenu::View {
            self.display_menu(ui);
        } else {
            self.catalog_menu(
                ui,
                catalog::entries(menu),
                egui::Id::new(("catalog_root", menu)),
            );
        }
    }

    pub(crate) fn expanded_view_menu(&mut self, ui: &mut egui::Ui) {
        self.catalog_menu(
            ui,
            catalog::VIEW,
            egui::Id::new(("catalog_root", TopMenu::View)),
        );
    }

    fn catalog_menu(
        &mut self,
        ui: &mut egui::Ui,
        items: &'static [catalog::MenuItem],
        path: egui::Id,
    ) {
        ui.set_min_width(252.);
        let height =
            (ui.ctx().content_rect().height() - 112.).clamp(100., 680.);
        let output = ui.scope(|ui| {
            ui.set_max_height(height);
            egui::ScrollArea::vertical()
            .id_salt(path)
            .max_height(height)
            .show(ui, |ui| {
                for (index, item) in items.iter().enumerate() {
                    let id = path.with(index);
                    ui.push_id(id, |ui| match item {
                        catalog::MenuItem::Action(command) => self.top_menu_command(ui, *command),
                        catalog::MenuItem::Toggle(command) => {
                            let selected = self.menu_selected(*command);
                            self.top_menu_toggle(ui, *command, selected);
                        }
                        catalog::MenuItem::Planned(label) => {
                            let response = ui.add_enabled(
                                false,
                                egui::Button::new(*label).shortcut_text("미구현"),
                            );
                            #[cfg(test)]
                            ui.data_mut(|data| {
                                data.insert_temp(
                                    egui::Id::new(("unsupported_menu", *label)),
                                    (response.rect, response.enabled()),
                                );
                                data.insert_temp(
                                    egui::Id::new(("catalog_planned", id)),
                                    (response.rect, response.enabled()),
                                );
                            });
                            response.on_disabled_hover_text(
                                "아직 구현되거나 이 메뉴에 연결되지 않은 항목입니다. 실행 및 단축키 지정은 지원하지 않습니다.",
                            );
                        }
                        catalog::MenuItem::Group(label, children) => {
                            let response = ui.menu_button(*label, |ui| {
                                self.catalog_menu(ui, children, id);
                                if *label=="캔버스" {ui.separator();self.canvas_menu_entries(ui);}
                            });
                            #[cfg(test)]
                            ui.data_mut(|data| {
                                data.insert_temp(
                                    egui::Id::new(("catalog_group", id)),
                                    (response.response.rect, response.inner.is_some()),
                                );
                            });
                            let _ = response;
                        }
                        catalog::MenuItem::Separator => {
                            ui.separator();
                        }
                    });
                }
            })
        }).inner;
        #[cfg(test)]
        ui.data_mut(|data| {
            data.insert_temp(
                egui::Id::new(("catalog_viewport", path)),
                output.inner_rect,
            );
        });
        let _ = output;
    }

    fn menu_selected(&self, command: Command) -> bool {
        if self.documents.is_empty() {
            return false;
        }
        if let Some(reference) = command.align_reference() {
            return self.documents.active_session.editing.align.reference
                == reference;
        }
        match command {
            Command::EditTextInkBounds => {
                self.documents.active_session.editing.align.text_ink
            }
            Command::EditVectorCenters => {
                self.documents.active_session.editing.align.vector_center
            }
            Command::LayerVisibility => {
                self.documents.active_session.document.layers
                    [self.documents.active_session.document.active]
                    .visible
            }
            Command::LayerLock => {
                self.documents.active_session.document.layers
                    [self.documents.active_session.document.active]
                    .locked
            }
            Command::LayerAlphaLock => {
                self.documents.active_session.document.layers
                    [self.documents.active_session.document.active]
                    .alpha_locked
            }
            Command::LayerClipping => {
                self.documents.active_session.document.layers
                    [self.documents.active_session.document.active]
                    .clipped
            }
            Command::LayerReference => {
                self.documents.active_session.document.layers
                    [self.documents.active_session.document.active]
                    .reference
            }
            Command::LayerDraft => {
                self.documents.active_session.document.layers
                    [self.documents.active_session.document.active]
                    .draft
            }
            Command::LayerMaskEnabled => {
                self.documents.active_session.document.layers
                    [self.documents.active_session.document.active]
                    .mask
                    .as_ref()
                    .is_some_and(|mask| mask.enabled)
            }
            Command::ShowMaskArea => self.active_mask_overlay().is_some(),
            Command::RulerBar => self.display.rulers,
            Command::PixelGrid => self.display.pixel_grid,
            Command::FlipHorizontal => {
                self.documents.active_session.view.flip_horizontal
            }
            Command::FlipVertical => {
                self.documents.active_session.view.flip_vertical
            }
            Command::HidePalettes => self.hide_panels,
            _ => self.workspace.panels.iter().any(|placement| {
                Some(placement.palette) == command.palette()
                    && placement.visible
            }),
        }
    }

    pub(crate) fn top_menu_command(
        &mut self,
        ui: &mut egui::Ui,
        command: Command,
    ) {
        self.command_menu_item_named(ui, command, menu_label(command));
    }

    pub(crate) fn top_menu_toggle(
        &mut self,
        ui: &mut egui::Ui,
        command: Command,
        selected: bool,
    ) {
        self.command_toggle_named(ui, command, selected, menu_label(command));
    }
}

fn menu_label(command: Command) -> &'static str {
    match command {
        Command::NewDrawing => "새 그림…",
        Command::OpenProject => "열기…",
        Command::SaveProject => "저장",
        Command::SaveAs => "다른 이름으로 저장…",
        Command::ExportPng => "PNG로 내보내기…",
        Command::Undo => "실행 취소",
        Command::Redo => "다시 실행",
        Command::ConvertToDrawingColor => "선 색을 그리기색으로 변경",
        Command::BrightnessToOpacity => "휘도를 투명도로 변환",
        Command::NewLayer => "새 래스터 레이어",
        Command::DuplicateLayer => "레이어 복제",
        Command::DeleteLayer => "레이어 삭제",
        Command::TransferLayerDown => "아래 레이어로 전송",
        Command::MergeLayerDown => "아래 레이어와 결합",
        Command::MergeSelectedLayers => "선택 레이어 결합",
        Command::MergeVisibleLayers => "표시 레이어 결합",
        Command::CopyVisibleLayers => "표시 레이어 복사본 결합",
        Command::FlattenImage => "이미지 통합",
        Command::ClearLayer => "선택 레이어 비우기",
        Command::MoveLayerUp => "위로 이동",
        Command::MoveLayerDown => "아래로 이동",
        Command::LayerVisibility => "레이어 표시",
        Command::LayerLock => "레이어 잠금",
        Command::LayerAlphaLock => "투명 픽셀 잠금",
        Command::LayerClipping => "아래 레이어에 클리핑",
        Command::RenameLayer => "레이어 이름 변경…",
        Command::LayerReference => "참조 레이어로 설정",
        Command::LayerDraft => "밑그림 레이어로 설정",
        Command::LayerDetails => "레이어 기본 설정…",
        Command::NewLayerFolder => "새 레이어 폴더",
        Command::GroupLayersInFolder => "폴더를 만들어 레이어 넣기",
        Command::UngroupLayerFolder => "레이어 폴더 해제",
        Command::LayerSelectionCreate => "선택 범위 만들기",
        Command::LayerSelectionAdd => "선택 범위에 추가",
        Command::LayerSelectionSubtract => "선택 범위에서 빼기",
        Command::LayerSelectionIntersect => "선택 범위와 교차",
        Command::LayerMaskOutsideSelection => "선택 범위 이외를 마스크",
        Command::LayerMaskSelection => "선택 범위를 마스크",
        Command::SelectAll => "전체 선택",
        Command::Deselect => "선택 해제",
        Command::InvertSelection => "선택 범위 반전",
        Command::MoveActiveLayerUp => "앞으로",
        Command::MoveActiveLayerDown => "뒤로",
        Command::MoveActiveLayerToTop => "맨 위로 이동",
        Command::MoveActiveLayerToBottom => "맨 아래로 이동",
        Command::SelectLayerAbove => "위 레이어",
        Command::SelectLayerBelow => "아래 레이어",
        Command::LayerMaskRevealAll => "마스크 만들기 — 모두 표시",
        Command::LayerMaskHideAll => "마스크 만들기 — 모두 숨기기",
        Command::DeleteLayerMask => "마스크 삭제",
        Command::ApplyLayerMask => "마스크를 레이어에 적용",
        Command::LayerMaskEnabled => "마스크 활성화",
        Command::ClearLayerMask => "마스크 비우기",
        Command::ClearEditingTarget => "편집 대상 비우기",
        Command::ShowMaskArea => "마스크 범위 표시",
        Command::ShortcutSettings => "단축키 설정…",
        Command::ZoomIn => "확대",
        Command::ZoomOut => "축소",
        Command::ActualSize => "100% 표시",
        Command::DoubleSize => "200% 표시",
        Command::FitCanvas => "화면에 맞추기",
        Command::ResetDisplay => "표시 초기화",
        Command::RotateLeft => "왼쪽으로 회전",
        Command::RotateRight => "오른쪽으로 회전",
        Command::ResetRotation => "회전 초기화",
        Command::ResetOrientation => "회전 / 반전 초기화",
        Command::FlipHorizontal => "좌우 반전",
        Command::FlipVertical => "상하 반전",
        Command::RulerBar => "눈금자",
        Command::PixelGrid => "픽셀 격자 (800% 이상)",
        Command::DisplaySettings => "표시 설정…",
        Command::PaletteTools => "도구 팔레트",
        Command::PaletteBrushes => "보조 도구",
        Command::PaletteProperties => "도구 속성",
        Command::PaletteSizes => "브러시 크기",
        Command::PaletteColor => "컬러 휠",
        Command::PaletteNavigator => "내비게이터",
        Command::PaletteLayers => "레이어",
        Command::PaletteHistory => "작업 내역",
        Command::HidePalettes => "모든 팔레트 숨기기",
        Command::ResetPaletteLayout => "팔레트 배치 초기화",
        Command::SubToolDetail => "보조 도구 상세…",
        Command::ShortcutGuide => "키보드 단축키 안내",
        Command::Notices => "오픈소스 라이선스",
        _ => command.label(),
    }
}
