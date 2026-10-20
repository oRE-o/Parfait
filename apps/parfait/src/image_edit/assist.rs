use super::*;
impl Command {
    pub(crate) fn is_color_assist(self) -> bool {
        matches!(
            self,
            Self::EditAutoColor
                | Self::EditAutoColorHints
                | Self::EditAutoColorOptions
                | Self::EditAutoShade
                | Self::EditColorReference
        )
    }
    pub(crate) fn is_material_registration(self) -> bool {
        matches!(self, Self::EditMaterialImage | Self::EditMaterialTemplate)
    }
}
impl EditDialog {
    pub(super) fn region_options(&self) -> parfait_core::RegionColors {
        parfait_core::RegionColors {
            threshold: self.values[0] as u8,
            minimum_area: self.values[1] as u32,
            include_open_regions: self.values[2] > 0.5,
            palette: vec![self.low, self.high],
        }
    }
    pub(super) fn assist_reason(
        &self,
        doc: &parfait_core::Document,
    ) -> Option<&'static str> {
        if self.command == Command::EditAutoShade {
            return doc.local_shading_reason(doc.active, self.shading);
        }
        if matches!(
            self.command,
            Command::EditAutoColor
                | Command::EditAutoColorHints
                | Command::EditAutoColorOptions
        ) {
            doc.region_color_reason(
                &self.region_options(),
                self.reference.as_deref(),
            )
        } else {
            doc.pixel_edit_reason(doc.active, PixelEdit::Invert)
        }
    }
    pub(super) fn assist_apply(
        &self,
        doc: &mut parfait_core::Document,
    ) -> bool {
        match self.command {
            Command::EditAutoColor
            | Command::EditAutoColorHints
            | Command::EditAutoColorOptions => doc.color_regions(
                &self.region_options(),
                self.reference.as_deref(),
            ),
            Command::EditAutoShade => {
                doc.add_local_shading(doc.active, self.shading)
            }
            Command::EditColorReference => {
                self.reference.as_deref().is_some_and(|image| {
                    doc.match_layer_colors_with(
                        doc.active,
                        image,
                        self.values[0] / 100.,
                        self.values[1] > 0.5,
                    )
                })
            }
            _ => false,
        }
    }
    pub(super) fn assist_controls(&mut self, ui: &mut egui::Ui) {
        match self.command {
            Command::EditAutoShade => {
                ui.label("실루엣을 둥근 표면으로 근사합니다. 암부·명부를 별도 클리핑 레이어로 만들며 밑색은 유지합니다.");
                ui.label("입체 구조 추론이나 투영 그림자는 지원하지 않습니다.");
                ui.horizontal(|ui| {
                    if ui.button("부드럽게").clicked() {
                        self.shading.bands = 0;
                    }
                    if ui.button("셀 음영").clicked() {
                        self.shading.bands = 3;
                    }
                    if ui.button("석양").clicked() {
                        self.shading.shadow = [72, 56, 112];
                        self.shading.highlight = [255, 192, 112];
                        self.shading.elevation = 20.;
                    }
                });
                slider(
                    ui,
                    &mut self.shading.angle,
                    -180. ..=180.,
                    "빛 방향 (도)",
                );
                slider(
                    ui,
                    &mut self.shading.elevation,
                    5. ..=85.,
                    "빛 높이 (도)",
                );
                slider(
                    ui,
                    &mut self.shading.radius,
                    1. ..=512.,
                    "둥근 표면 반경 (px)",
                );
                slider(ui, &mut self.shading.strength, 0. ..=1., "농도");
                ui.horizontal(|ui| {
                    ui.label("계조 (0: 부드럽게)");
                    ui.add(
                        egui::DragValue::new(&mut self.shading.bands)
                            .range(0..=4),
                    );
                    if self.shading.bands == 1 {
                        self.shading.bands = 2;
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("암부");
                    ui.color_edit_button_srgb(&mut self.shading.shadow);
                    ui.label("명부");
                    ui.color_edit_button_srgb(&mut self.shading.highlight);
                });
                ui.checkbox(&mut self.shading.point_light, "점 광원");
                if self.shading.point_light {
                    slider(
                        ui,
                        &mut self.shading.position[0],
                        -1. ..=2.,
                        "광원 X / 캔버스 폭",
                    );
                    slider(
                        ui,
                        &mut self.shading.position[1],
                        -1. ..=2.,
                        "광원 Y / 캔버스 높이",
                    );
                }
                slider(
                    ui,
                    &mut self.shading.line_influence,
                    0. ..=1.,
                    "참조 레이어 선화 영향",
                );
            }
            Command::EditColorReference => {
                ui.label("로컬 PNG/JPEG의 색 분포를 활성 레이어에 맞춥니다.");
                slider(ui, &mut self.values[0], 0. ..=100., "적용도");
                let mut preserve = self.values[1] > 0.5;
                if ui.checkbox(&mut preserve, "밝기 유지").changed() {
                    self.values[1] = f32::from(preserve);
                }
            }
            _ => {
                ui.label("밝은 연결 영역을 찾아 새 레이어에 색을 배치합니다. 기본값은 화면 가장자리로 열린 영역을 제외합니다. 힌트가 있으면 각 영역 안의 힌트 색을 평균합니다.");
                slider(ui, &mut self.values[0], 1. ..=255., "밝은 영역 경계값");
                slider(ui, &mut self.values[1], 1. ..=1000., "최소 영역 (px)");
                let mut edges = self.values[2] > 0.5;
                if ui
                    .checkbox(&mut edges, "바깥으로 열린 영역도 채우기")
                    .changed()
                {
                    self.values[2] = f32::from(edges);
                }
                ui.horizontal(|ui| {
                    ui.label("배색");
                    ui.color_edit_button_srgb(&mut self.low);
                    ui.color_edit_button_srgb(&mut self.high);
                });
            }
        }
    }
}
