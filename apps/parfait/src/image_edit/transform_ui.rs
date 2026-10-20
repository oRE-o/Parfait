use super::*;
impl EditDialog {
    pub(super) fn transform_controls(&mut self, ui: &mut egui::Ui) {
        use Command::*;
        if self.command == EditTransformMesh {
            let mut grid = self.transform.grid;
            ui.horizontal(|ui| {
                ui.label("격자 가로 / 세로");
                ui.add(egui::DragValue::new(&mut grid[0]).range(2..=16));
                ui.add(egui::DragValue::new(&mut grid[1]).range(2..=16));
            });
            if grid != self.transform.grid {
                self.transform.resize_grid(grid[0], grid[1]);
                self.values[0] = 0.;
            }
            ui.label("격자 수를 바꾸면 점의 변형을 초기화합니다. Shift+클릭으로 여러 점을 선택할 수 있습니다.");
            let mut selected =
                (self.values[0] as usize).min(self.transform.nodes.len() - 1);
            egui::ComboBox::from_id_salt("mesh_selected_node")
                .selected_text(format!("점 {}", selected + 1))
                .show_ui(ui, |ui| {
                    for i in 0..self.transform.nodes.len() {
                        ui.selectable_value(
                            &mut selected,
                            i,
                            format!("점 {}", i + 1),
                        );
                    }
                });
            self.values[0] = selected as f32;
            slider(
                ui,
                &mut self.transform.nodes[selected][0],
                -1. ..=1.,
                "가로 이동 / 내용 폭",
            );
            slider(
                ui,
                &mut self.transform.nodes[selected][1],
                -1. ..=1.,
                "세로 이동 / 내용 높이",
            );
            if ui.button("모든 점 초기화").clicked() {
                self.transform.nodes.fill([0.; 2]);
            }
            return;
        }
        if self.command == EditTransformPins {
            ui.label(format!(
                "핀 {} / 32개 · 캔버스를 클릭해서 추가",
                self.transform.pins.len()
            ));
            if !self.transform.pins.is_empty() {
                let mut selected = self
                    .transform_canvas
                    .selected
                    .min(self.transform.pins.len() - 1);
                egui::ComboBox::from_id_salt("puppet_selected_pin")
                    .selected_text(format!("핀 {}", selected + 1))
                    .show_ui(ui, |ui| {
                        for i in 0..self.transform.pins.len() {
                            ui.selectable_value(
                                &mut selected,
                                i,
                                format!("핀 {}", i + 1),
                            );
                        }
                    });
                self.transform_canvas.selected = selected;
                let pin = &mut self.transform.pins[selected];
                slider(ui, &mut pin.target[0], -1. ..=2., "핀 X / 내용 폭");
                slider(ui, &mut pin.target[1], -1. ..=2., "핀 Y / 내용 높이");
                slider(ui, &mut pin.rotation, -180. ..=180., "핀 회전 (도)");
            }
            if ui.button("모든 핀 삭제").clicked() {
                self.transform.pins.clear();
            }
            return;
        }
        let full =
            matches!(self.command, EditTransformFree | EditTransformDistort);
        if full
            || matches!(
                self.command,
                EditTransformScale | EditTransformScaleRotate
            )
        {
            slider(ui, &mut self.transform.scale[0], 0.05..=8., "가로 배율");
            slider(ui, &mut self.transform.scale[1], 0.05..=8., "세로 배율");
        }
        if full
            || matches!(
                self.command,
                EditTransformRotate | EditTransformScaleRotate
            )
        {
            slider(ui, &mut self.transform.angle, -180. ..=180., "회전 (도)");
        }
        if full || self.command == EditTransformShear {
            slider(
                ui,
                &mut self.transform.shear[0],
                -2. ..=2.,
                "가로 평행 왜곡",
            );
            slider(
                ui,
                &mut self.transform.shear[1],
                -2. ..=2.,
                "세로 평행 왜곡",
            );
        }
        if full || self.command == EditTransformPerspective {
            slider(
                ui,
                &mut self.transform.perspective[0],
                -0.9..=0.9,
                "가로 원근",
            );
            slider(
                ui,
                &mut self.transform.perspective[1],
                -0.9..=0.9,
                "세로 원근",
            );
        }
        if matches!(self.command, EditTransformFlipX | EditTransformFlipY) {
            ui.label("내용의 경계 중심을 기준으로 반전합니다. 캔버스 방향은 바꾸지 않습니다.");
        }
        ui.horizontal(|ui| {
            ui.label("이동 X");
            ui.add(
                egui::DragValue::new(&mut self.transform.translation[0])
                    .range(-8192. ..=8192.),
            );
            ui.label("Y");
            ui.add(
                egui::DragValue::new(&mut self.transform.translation[1])
                    .range(-8192. ..=8192.),
            );
        });
        ui.label("선택이 있으면 선택한 픽셀, 없으면 활성 레이어 전체를 변형합니다. 캔버스 밖으로 나간 픽셀은 잘립니다.");
    }
}
