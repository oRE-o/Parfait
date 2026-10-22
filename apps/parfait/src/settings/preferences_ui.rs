use super::*;

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Page {
    Tools,
    Tablet,
    Command,
    Interface,
    Performance,
    Cursor,
    Canvas,
    File,
    Fonts,
    Privacy,
}
impl Page {
    const ALL: [(Self, &'static str); 10] = [
        (Self::Tools, "도구 / 수식 키"),
        (Self::Tablet, "태블릿"),
        (Self::Command, "커맨드"),
        (Self::Interface, "인터페이스"),
        (Self::Performance, "퍼포먼스"),
        (Self::Cursor, "커서"),
        (Self::Canvas, "캔버스"),
        (Self::File, "파일 / 복구"),
        (Self::Fonts, "글꼴"),
        (Self::Privacy, "개인정보"),
    ];
}
fn group(ui: &mut egui::Ui, title: &str, contents: impl FnOnce(&mut egui::Ui)) {
    ui.group(|ui| {
        ui.set_min_width(ui.available_width());
        ui.strong(title);
        ui.add_space(8.);
        contents(ui);
    });
    ui.add_space(8.);
}
impl Parfait {
    pub(super) fn preferences_contents(
        &self,
        ui: &mut egui::Ui,
        dialog: &mut SettingsDialog,
    ) {
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(140.);
                for (page, label) in Page::ALL {
                    let response = ui.add_sized([140., 27.], egui::Button::selectable(dialog.page == page, label));
                    if response.clicked() { dialog.page = page; }
                    #[cfg(test)]
                    ui.data_mut(|data|data.insert_temp(egui::Id::new(("preference_page",label)),response.rect));
                }
            });
            ui.separator();
            ui.vertical(|ui| {
                ui.set_width(630.);
                ui.set_min_height(350.);
                match dialog.page {
                    Page::Tools => group(ui,"일시 전환",|ui| {
                        for (value,label) in [
                            (&mut dialog.candidate.space_pan,"Space + 드래그: 이동"),
                            (&mut dialog.candidate.space_rotate,"Shift + Space + 드래그: 보기 회전"),
                            (&mut dialog.candidate.space_zoom,"Ctrl/Cmd + Space + 드래그: 보기 확대"),
                            (&mut dialog.candidate.alt_picker,"Alt + 클릭: 색 추출"),
                            (&mut dialog.candidate.layer_picker,"Ctrl/Cmd + Shift + 클릭: 레이어 선택"),
                            (&mut dialog.candidate.middle_pan,"가운데 버튼: 이동"),
                        ] {ui.checkbox(value,label);}
                    }),
                    Page::Tablet => group(ui,"장치 필압 응답",|ui| {
                        ui.label("모든 브러시의 개별 필압 곡선보다 먼저 적용합니다.");
                        crate::curve_editor::edit(ui,"preferences_pressure",&mut dialog.calibration.points,&mut dialog.pressure_point,crate::curve_editor::Options {
                            input:"장치 입력",output:"보정 필압",side:200.,marker:Some(self.pressure),..Default::default()
                        });
                        if ui.button("직선으로 복원").clicked() {dialog.calibration=Default::default();dialog.pressure_point=None;}
                    }),
                    Page::Command => group(ui,"커맨드 바",|ui| {
                        ui.label("상단에 표시할 명령과 순서");
                        command_list(ui,&mut dialog.candidate.toolbar,&mut dialog.filter,false);
                    }),
                    Page::Interface => {
                        group(ui,"표시",|ui| {
                            crate::widgets::numeric_slider(ui,&mut dialog.candidate.font_size,8. ..=24.,"UI 글꼴 크기",1,"");
                        });
                        group(ui,"분리 팔레트",|ui| {
                            ui.label("마우스 밖 불투명도 · 마우스를 올리면 100%로 복원");
                            for placement in &mut dialog.workspace.panels {
                                ui.horizontal(|ui| {
                                    ui.add_sized([110.,20.],egui::Label::new(placement.palette.title()));
                                    ui.add(egui::Slider::new(&mut placement.inactive_opacity,0.2..=1.).custom_formatter(|value,_|format!("{:.0}%",value*100.)));
                                });
                            }
                        });
                    }
                    Page::Performance => {
                        group(ui,"그리기 / 메모리",|ui| {
                            ui.label("변경된 영역을 갱신하며, 미리보기 작업은 한 번에 하나씩 실행합니다.");
                            ui.label("큰 편집과 미리보기는 메모리 여유를 확인한 뒤 실행합니다.");
                        });
                        group(ui,"복구 저장",|ui| {
                            crate::widgets::numeric_slider(ui,&mut dialog.candidate.autosave_delay_ms,100..=2000,"입력 종료 후 복구 제출 지연",0," ms");
                            ui.label("종료 시 저장과 오류 후 재시도는 유지됩니다.");
                        });
                    }
                    Page::Cursor => group(ui,"브러시 커서",|ui| {
                        ui.checkbox(&mut dialog.display.brush_cursor,"브러시 크기 윤곽 표시");
                    }),
                    Page::Canvas => {
                        group(ui,"표시",|ui| {
                            ui.checkbox(&mut dialog.display.rulers,"눈금자 표시");
                            ui.checkbox(&mut dialog.display.pixel_grid,"확대할 때 픽셀 격자 표시");
                            crate::widgets::numeric_slider(ui,&mut dialog.display.background,20..=96,"캔버스 바깥 밝기",0,"");
                        });
                        group(ui,"표시 각도",|ui| {
                            crate::widgets::numeric_slider(ui,&mut dialog.display.rotation_step,1. ..=90.,"회전 간격",1,"°");
                        });
                    }
                    Page::File => group(ui,"로컬 자동 복구",|ui| {
                        crate::widgets::numeric_slider(ui,&mut dialog.candidate.autosave_delay_ms,100..=2000,"복구 제출 지연",0," ms");
                        ui.label(format!("복구 위치: {}",self.recovery_path.parent().unwrap().display()));
                        ui.label("저장 오류 재시도: 3초");
                        ui.label("복구 기록과 작업 파일 저장은 별도로 관리합니다.");
                    }),
                    Page::Fonts => group(ui,"UI 글꼴",|ui| {
                        ui.label(dialog.candidate.font_path.as_ref().map_or("Pretendard".into(),|p|p.display().to_string()));
                        if ui.button("글꼴 파일 선택…").clicked() && let Some(path)=rfd::FileDialog::new().add_filter("OpenType", &["ttf","otf"]).pick_file() {dialog.candidate.font_path=Some(path);}
                        if ui.button("기본 글꼴로 복원").clicked() {dialog.candidate.font_path=None;}
                        crate::widgets::numeric_slider(ui,&mut dialog.candidate.font_size,8. ..=24.,"UI 글꼴 크기",1,"");
                    }),
                    Page::Privacy => group(ui,"운영체제 클립보드",|ui| {
                        ui.checkbox(&mut dialog.candidate.clipboard_read,"이미지·SVG 읽기 허용");
                        ui.checkbox(&mut dialog.candidate.clipboard_write,"이미지·SVG 쓰기 허용");
                        ui.label("계정이나 원격 전송 서비스는 사용하지 않습니다.");
                    }),
                }
            });
        });
    }
}
