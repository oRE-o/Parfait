use super::*;
pub(super) struct GeometryUi {
    pub original: [u32; 2],
    pub linked: bool,
    pub nearest: bool,
    pub anchor: [usize; 2],
    pub(super) drag: Option<super::geometry_canvas::GeometryDrag>,
    pub(super) checker: Option<egui::TextureHandle>,
    pub(super) exterior: Option<egui::Color32>,
}
impl GeometryUi {
    pub(super) fn new(original: [u32; 2]) -> Self {
        Self {
            original,
            linked: true,
            nearest: false,
            anchor: [1, 1],
            drag: None,
            checker: None,
            exterior: None,
        }
    }
}
impl EditDialog {
    pub(super) fn geometry_controls(&mut self, ui: &mut egui::Ui) {
        let resize = matches!(
            self.command,
            Command::EditResolution | Command::EditSuperResolution
        );
        ui.label(format!(
            "원본: {} × {} px",
            self.geometry.original[0], self.geometry.original[1]
        ));
        if resize {
            ui.checkbox(&mut self.geometry.linked, "종횡비 고정");
        }
        let before = [self.width, self.height];
        let mut width_changed = false;
        let mut height_changed = false;
        ui.horizontal(|ui| {
            ui.label("너비");
            width_changed = ui
                .add(
                    egui::DragValue::new(&mut self.width)
                        .range(1..=4096)
                        .suffix(" px"),
                )
                .changed();
            ui.label("높이");
            height_changed = ui
                .add(
                    egui::DragValue::new(&mut self.height)
                        .range(1..=4096)
                        .suffix(" px"),
                )
                .changed();
        });
        if resize && self.geometry.linked {
            if width_changed {
                self.height = ((self.width as f64
                    * self.geometry.original[1] as f64
                    / self.geometry.original[0] as f64)
                    .round() as u32)
                    .clamp(1, 4096);
            } else if height_changed {
                self.width = ((self.height as f64
                    * self.geometry.original[0] as f64
                    / self.geometry.original[1] as f64)
                    .round() as u32)
                    .clamp(1, 4096);
            }
        }
        if resize {
            let mut percent =
                self.width as f32 / self.geometry.original[0] as f32 * 100.;
            ui.horizontal(|ui| {
                ui.label("배율");
                if ui
                    .add(
                        egui::DragValue::new(&mut percent)
                            .range(0.1..=409600.)
                            .speed(1.)
                            .suffix(" %"),
                    )
                    .changed()
                {
                    self.width =
                        (self.geometry.original[0] as f32 * percent / 100.)
                            .round()
                            .clamp(1., 4096.) as u32;
                    self.height =
                        (self.geometry.original[1] as f32 * percent / 100.)
                            .round()
                            .clamp(1., 4096.) as u32;
                }
            });
            if self.command == Command::EditResolution {
                egui::ComboBox::from_id_salt("resize_sampling")
                    .selected_text(if self.geometry.nearest {
                        "최근접 (픽셀 아트)"
                    } else {
                        "쌍선형 (투명도 고려)"
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.geometry.nearest,
                            false,
                            "쌍선형 (투명도 고려)",
                        );
                        ui.selectable_value(
                            &mut self.geometry.nearest,
                            true,
                            "최근접 (픽셀 아트)",
                        );
                    });
            }
        } else {
            ui.label("기준점");
            let previous = self.geometry.anchor;
            egui::Grid::new("canvas_anchor").spacing(egui::vec2(3., 3.)).show(
                ui,
                |ui| {
                    for y in 0..3 {
                        for x in 0..3 {
                            if ui
                                .selectable_label(
                                    self.geometry.anchor == [x, y],
                                    [
                                        ["↖", "↑", "↗"],
                                        ["←", "●", "→"],
                                        ["↙", "↓", "↘"],
                                    ][y][x],
                                )
                                .clicked()
                            {
                                self.geometry.anchor = [x, y];
                            }
                        }
                        ui.end_row();
                    }
                },
            );
            if before != [self.width, self.height]
                || previous != self.geometry.anchor
            {
                self.place_anchor();
            }
            ui.horizontal(|ui| {
                ui.label("이미지 위치 X");
                ui.add(
                    egui::DragValue::new(&mut self.offset[0])
                        .range(-4096..=4096),
                );
                ui.label("Y");
                ui.add(
                    egui::DragValue::new(&mut self.offset[1])
                        .range(-4096..=4096),
                );
            });
            let cropped = self.offset[0] < 0
                || self.offset[1] < 0
                || self.offset[0] as i64 + i64::from(self.geometry.original[0])
                    > i64::from(self.width)
                || self.offset[1] as i64 + i64::from(self.geometry.original[1])
                    > i64::from(self.height);
            if cropped {
                ui.label("새 캔버스 밖의 픽셀은 잘립니다. 실행 취소로 복원할 수 있습니다.");
            }
        }
        ui.label(format!("출력: {} × {} px", self.width, self.height));
        ui.weak("그림의 테두리 핸들을 드래그해 크기를 조절할 수 있습니다.");
    }
    fn place_anchor(&mut self) {
        self.offset = [
            ((self.width as i32 - self.geometry.original[0] as i32)
                * self.geometry.anchor[0] as i32)
                / 2,
            ((self.height as i32 - self.geometry.original[1] as i32)
                * self.geometry.anchor[1] as i32)
                / 2,
        ];
    }
}
