use super::*;
use parfait_core::{
    GradientStop, ResponseCurve, ToneAdjustment, ToneLevels,
    automatic_tone_range,
};

pub(super) struct ToneUi {
    pub layers: u64,
    pub channel: usize,
    pub levels: [ToneLevels; 4],
    pub curves: [ResponseCurve; 4],
    pub range: usize,
    pub balance: [[f32; 3]; 3],
    pub preserve: bool,
    pub stops: Vec<GradientStop>,
    pub stop: usize,
    pub reverse: bool,
    pub linear_light: bool,
    pub selected: Option<usize>,
    pub histogram: [[u64; 256]; 4],
}
impl ToneUi {
    pub fn new(app: &Parfait) -> Self {
        let layers = app.selected_layer_mask();
        Self {
            layers,
            channel: 0,
            levels: [ToneLevels::default(); 4],
            curves: std::array::from_fn(|_| ResponseCurve::default()),
            range: 1,
            balance: [[0.; 3]; 3],
            preserve: true,
            stops: vec![
                GradientStop { position: 0., color: [0; 3], midpoint: 0.5 },
                GradientStop { position: 1., color: [255; 3], midpoint: 0.5 },
            ],
            stop: 0,
            reverse: false,
            linear_light: false,
            selected: None,
            // Computed lazily on opening a tonal dialog, never during drawing.
            histogram: [[0; 256]; 4],
        }
    }
}
impl Command {
    pub(super) fn is_tonal(self) -> bool {
        matches!(
            self,
            Self::EditBrightnessContrast
                | Self::EditHsl
                | Self::EditPosterize
                | Self::EditInvert
                | Self::EditLevels
                | Self::EditToneCurve
                | Self::EditColorBalance
                | Self::EditThreshold
                | Self::EditGradientMap
        )
    }
}
impl EditDialog {
    pub(super) fn tone(&self) -> Option<ToneAdjustment> {
        use Command::*;
        Some(match self.command {
            EditLevels => {
                let mut levels = self.tone_ui.levels;
                levels[self.tone_ui.channel] = ToneLevels {
                    black: self.values[0] as u8,
                    white: self.values[1] as u8,
                    gamma: self.values[2],
                    output_black: self.values[3] as u8,
                    output_white: self.values[4] as u8,
                };
                ToneAdjustment::Levels(levels)
            }
            EditToneCurve => {
                ToneAdjustment::Curves(self.tone_ui.curves.clone())
            }
            EditColorBalance => {
                let mut ranges = self.tone_ui.balance;
                ranges[self.tone_ui.range] =
                    std::array::from_fn(|i| self.values[i] / 100.);
                ToneAdjustment::Balance {
                    ranges,
                    preserve_luminance: self.tone_ui.preserve,
                }
            }
            EditGradientMap => ToneAdjustment::Gradient {
                stops: self.tone_ui.stops.clone(),
                reverse: self.tone_ui.reverse,
                linear_light: self.tone_ui.linear_light,
            },
            EditHsl => ToneAdjustment::Hsv {
                hue: self.values[0],
                saturation: self.values[1] / 100.,
                value: self.values[2] / 100.,
            },
            EditThreshold => ToneAdjustment::Threshold {
                value: self.values[0] as u8,
                preserve_alpha: self.tone_ui.preserve,
            },
            _ => return None,
        })
    }
    fn level_values(&mut self) {
        let l = self.tone_ui.levels[self.tone_ui.channel];
        self.values[..5].copy_from_slice(&[
            f32::from(l.black),
            f32::from(l.white),
            l.gamma,
            f32::from(l.output_black),
            f32::from(l.output_white),
        ]);
    }
    pub(super) fn tone_controls(&mut self, ui: &mut egui::Ui) {
        use Command::*;
        match self.command {
            EditLevels => {
                if let Some(ToneAdjustment::Levels(l)) = self.tone() {
                    self.tone_ui.levels = l;
                }
                let previous = self.tone_ui.channel;
                channel_ui(ui, &mut self.tone_ui.channel);
                if previous != self.tone_ui.channel {
                    self.level_values();
                }
                let plot = histogram(
                    ui,
                    &self.tone_ui.histogram[self.tone_ui.channel],
                    self.tone_ui.channel,
                );
                level_handles(ui, plot, &mut self.values);
                ui.horizontal(|ui| {
                    ui.label("입력");
                    ui.add(
                        egui::DragValue::new(&mut self.values[0])
                            .range(0. ..=254.)
                            .prefix("검정 "),
                    );
                    ui.add(
                        egui::DragValue::new(&mut self.values[2])
                            .range(0.1..=10.)
                            .speed(0.01)
                            .prefix("γ "),
                    );
                    ui.add(
                        egui::DragValue::new(&mut self.values[1])
                            .range(1. ..=255.)
                            .prefix("흰색 "),
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("출력");
                    ui.add(
                        egui::DragValue::new(&mut self.values[3])
                            .range(0. ..=255.)
                            .prefix("검정 "),
                    );
                    ui.add(
                        egui::DragValue::new(&mut self.values[4])
                            .range(0. ..=255.)
                            .prefix("흰색 "),
                    );
                });
                if ui.button("자동 조정").clicked() {
                    let (a, b) = automatic_tone_range(
                        &self.tone_ui.histogram[self.tone_ui.channel],
                    );
                    self.values[..5].copy_from_slice(&[
                        f32::from(a),
                        f32::from(b),
                        1.,
                        0.,
                        255.,
                    ]);
                }
            }
            EditToneCurve => {
                let previous = self.tone_ui.channel;
                channel_ui(ui, &mut self.tone_ui.channel);
                if previous != self.tone_ui.channel {
                    self.tone_ui.selected = None;
                }
                crate::curve_editor::edit_tone(
                    ui,
                    ("tone_curve", self.tone_ui.channel),
                    &mut self.tone_ui.curves[self.tone_ui.channel].points,
                    &mut self.tone_ui.selected,
                    &self.tone_ui.histogram[self.tone_ui.channel],
                    crate::curve_editor::Options {
                        input: "입력",
                        output: "출력",
                        side: 230.,
                        fixed_endpoints: false,
                        increasing: false,
                        delete_on_drag_out: true,
                        ..Default::default()
                    },
                );
                ui.label("그래프 클릭: 점 추가 · 드래그: 이동 · 바깥으로 드래그 / Delete: 삭제");
                if ui.button("현재 채널 초기화").clicked() {
                    self.tone_ui.curves[self.tone_ui.channel] =
                        ResponseCurve::default();
                    self.tone_ui.selected = None;
                }
            }
            EditColorBalance => {
                self.tone_ui.balance[self.tone_ui.range] =
                    std::array::from_fn(|i| self.values[i] / 100.);
                let previous = self.tone_ui.range;
                ui.horizontal(|ui| {
                    for (i, name) in
                        ["그림자", "중간조", "하이라이트"].iter().enumerate()
                    {
                        ui.selectable_value(&mut self.tone_ui.range, i, *name);
                    }
                });
                if previous != self.tone_ui.range {
                    for i in 0..3 {
                        self.values[i] =
                            self.tone_ui.balance[self.tone_ui.range][i] * 100.;
                    }
                }
                for (i, name) in ["시안 ↔ 빨강", "마젠타 ↔ 초록", "노랑 ↔ 파랑"]
                    .iter()
                    .enumerate()
                {
                    slider(ui, &mut self.values[i], -100. ..=100., name);
                }
                ui.checkbox(&mut self.tone_ui.preserve, "휘도 유지");
            }
            EditGradientMap => self.gradient_controls(ui),
            EditBrightnessContrast => {
                slider(ui, &mut self.values[0], -100. ..=100., "밝기");
                slider(ui, &mut self.values[1], -100. ..=100., "대비");
                if ui.button("자동 조정").clicked() {
                    let (a, b) =
                        automatic_tone_range(&self.tone_ui.histogram[0]);
                    let span = f32::from(b) - f32::from(a);
                    self.values[1] = (1. - span / 255.) * 100.;
                    self.values[0] = if span > 0. {
                        ((127.5 - (f32::from(a) + f32::from(b)) / 2.) / span
                            * 100.)
                            .clamp(-100., 100.)
                    } else {
                        0.
                    };
                }
            }
            EditHsl => {
                slider(ui, &mut self.values[0], -180. ..=180., "색조");
                slider(ui, &mut self.values[1], -100. ..=100., "채도");
                slider(ui, &mut self.values[2], -100. ..=100., "명도");
            }
            EditPosterize => {
                crate::widgets::numeric_slider(
                    ui,
                    &mut self.values[0],
                    2. ..=20.,
                    "계조 수",
                    0,
                    "",
                );
            }
            EditThreshold => {
                histogram(ui, &self.tone_ui.histogram[0], 0);
                crate::widgets::numeric_slider(
                    ui,
                    &mut self.values[0],
                    0. ..=255.,
                    "경계값",
                    0,
                    "",
                );
                ui.checkbox(&mut self.tone_ui.preserve, "투명도 남기기");
            }
            _ => {}
        }
    }
    fn gradient_controls(&mut self, ui: &mut egui::Ui) {
        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), 54.),
            egui::Sense::click_and_drag(),
        );
        let bar =
            egui::Rect::from_min_max(rect.min, rect.max - egui::vec2(0., 18.));
        let edit = self.tone().unwrap();
        let mut mesh = egui::Mesh::default();
        for i in 0..=256 {
            let t = i as f32 / 256.;
            let color = edit.gradient_color(t);
            let color = egui::Color32::from_rgb(color[0], color[1], color[2]);
            let x = bar.left() + bar.width() * t;
            mesh.colored_vertex(egui::pos2(x, bar.top()), color);
            mesh.colored_vertex(egui::pos2(x, bar.bottom()), color);
            if i < 256 {
                let n = i * 2;
                mesh.add_triangle(n, n + 1, n + 2);
                mesh.add_triangle(n + 1, n + 3, n + 2);
            }
        }
        ui.painter().add(egui::Shape::mesh(mesh));
        let position = |s: &GradientStop| {
            egui::pos2(bar.left() + bar.width() * s.position, bar.bottom() + 9.)
        };
        if (response.clicked() || response.drag_started())
            && let Some(p) = response.interact_pointer_pos()
        {
            let nearest = self
                .tone_ui
                .stops
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| {
                    position(a)
                        .distance_sq(p)
                        .total_cmp(&position(b).distance_sq(p))
                })
                .map(|(i, s)| (i, position(s).distance(p)));
            if let Some((i, d)) = nearest.filter(|(_, d)| *d < 15.) {
                let _ = d;
                self.tone_ui.stop = i;
            } else if self.tone_ui.stops.len() < 32 {
                let t = ((p.x - bar.left()) / bar.width()).clamp(0.002, 0.998);
                let i = self.tone_ui.stops.partition_point(|s| s.position < t);
                if i > 0
                    && i < self.tone_ui.stops.len()
                    && t - self.tone_ui.stops[i - 1].position >= 0.002
                    && self.tone_ui.stops[i].position - t >= 0.002
                {
                    self.tone_ui.stops.insert(
                        i,
                        GradientStop {
                            position: t,
                            color: edit.gradient_color(t),
                            midpoint: 0.5,
                        },
                    );
                    self.tone_ui.stop = i;
                }
            }
        }
        let selected = self.tone_ui.stop.min(self.tone_ui.stops.len() - 1);
        if response.dragged()
            && selected > 0
            && selected + 1 < self.tone_ui.stops.len()
            && let Some(p) = response.interact_pointer_pos()
        {
            self.tone_ui.stops[selected].position =
                ((p.x - bar.left()) / bar.width()).clamp(
                    self.tone_ui.stops[selected - 1].position + 0.001,
                    self.tone_ui.stops[selected + 1].position - 0.001,
                );
        }
        for (i, s) in self.tone_ui.stops.iter().enumerate() {
            ui.painter().circle_filled(
                position(s),
                if i == selected { 6. } else { 4. },
                if i == selected {
                    egui::Color32::WHITE
                } else {
                    egui::Color32::GRAY
                },
            );
        }
        ui.label("바 클릭: 색 추가 · 노드 드래그: 위치 조절");
        ui.horizontal(|ui| {
            if ui.button("◀").clicked() {
                self.tone_ui.stop = selected.saturating_sub(1);
            }
            if ui.button("▶").clicked() {
                self.tone_ui.stop =
                    (selected + 1).min(self.tone_ui.stops.len() - 1);
            }
            ui.label(format!(
                "노드 {} / {}",
                selected + 1,
                self.tone_ui.stops.len()
            ));
        });
        let len = self.tone_ui.stops.len();
        let low = if selected == 0 {
            0.
        } else {
            self.tone_ui.stops[selected - 1].position + 0.001
        };
        let high = if selected + 1 == len {
            1.
        } else {
            self.tone_ui.stops[selected + 1].position - 0.001
        };
        let stop = &mut self.tone_ui.stops[selected];
        ui.horizontal(|ui| {
            ui.label("색");
            ui.color_edit_button_srgb(&mut stop.color);
            let mut percent = stop.position * 100.;
            if ui
                .add_enabled(
                    selected > 0 && selected + 1 < len,
                    egui::DragValue::new(&mut percent)
                        .range(low * 100. ..=high * 100.)
                        .suffix(" %"),
                )
                .changed()
            {
                stop.position = percent / 100.;
            }
        });
        if selected + 1 < len {
            crate::widgets::numeric_slider(
                ui,
                &mut stop.midpoint,
                0.01..=0.99,
                "오른쪽 색과의 혼합 중간점",
                2,
                "",
            );
        }
        if ui
            .add_enabled(
                selected > 0 && selected + 1 < len,
                egui::Button::new("선택 노드 삭제"),
            )
            .clicked()
        {
            self.tone_ui.stops.remove(selected);
            self.tone_ui.stop = selected - 1;
        }
        ui.horizontal(|ui| {
            if ui.button("그라데이션 반전").clicked() {
                let old = self.tone_ui.stops.clone();
                let len = old.len();
                self.tone_ui.stops = old
                    .iter()
                    .enumerate()
                    .rev()
                    .map(|(i, s)| GradientStop {
                        position: 1. - s.position,
                        color: s.color,
                        midpoint: if i > 0 {
                            1. - old[i - 1].midpoint
                        } else {
                            0.5
                        },
                    })
                    .collect();
                self.tone_ui.stop = len - 1 - selected;
            }
            ui.checkbox(&mut self.tone_ui.linear_light, "선형광 혼색");
        });
        ui.horizontal(|ui| {
            for (name, colors) in [
                ("흑백", vec![[0, 0, 0], [255, 255, 255]]),
                ("석양", vec![[28, 20, 63], [183, 59, 98], [255, 219, 151]]),
                ("청록", vec![[10, 33, 57], [39, 157, 161], [243, 245, 215]]),
            ] {
                if ui.button(name).clicked() {
                    let n = colors.len();
                    self.tone_ui.stops = colors
                        .into_iter()
                        .enumerate()
                        .map(|(i, color)| GradientStop {
                            position: i as f32 / (n - 1) as f32,
                            color,
                            midpoint: 0.5,
                        })
                        .collect();
                    self.tone_ui.stop = 0;
                }
            }
        });
    }
}
fn channel_ui(ui: &mut egui::Ui, channel: &mut usize) {
    ui.horizontal(|ui| {
        ui.label("채널");
        for (i, name) in ["RGB", "Red", "Green", "Blue"].iter().enumerate() {
            ui.selectable_value(channel, i, *name);
        }
    });
}
fn histogram(
    ui: &mut egui::Ui,
    bins: &[u64; 256],
    channel: usize,
) -> egui::Rect {
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), 88.),
        egui::Sense::hover(),
    );
    ui.painter().rect_filled(rect, 2., egui::Color32::from_gray(27));
    let max = bins.iter().copied().max().unwrap_or(1).max(1) as f64;
    let color = [
        egui::Color32::GRAY,
        egui::Color32::from_rgb(185, 75, 75),
        egui::Color32::from_rgb(85, 160, 95),
        egui::Color32::from_rgb(80, 125, 200),
    ][channel];
    for (i, n) in bins.iter().enumerate() {
        let h = (*n as f64 / max).sqrt() as f32 * rect.height();
        ui.painter().rect_filled(
            egui::Rect::from_min_max(
                egui::pos2(
                    rect.left() + i as f32 / 256. * rect.width(),
                    rect.bottom() - h,
                ),
                egui::pos2(
                    rect.left() + (i + 1) as f32 / 256. * rect.width(),
                    rect.bottom(),
                ),
            ),
            0.,
            color,
        );
    }
    rect
}
fn level_handles(ui: &mut egui::Ui, plot: egui::Rect, values: &mut [f32; 8]) {
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(plot.width(), 22.),
        egui::Sense::hover(),
    );
    let black = values[0];
    let white = values[1];
    let middle = black + (white - black) * 0.5_f32.powf(values[2]);
    for (i, (value, color)) in [
        (black, egui::Color32::BLACK),
        (middle, egui::Color32::GRAY),
        (white, egui::Color32::WHITE),
    ]
    .into_iter()
    .enumerate()
    {
        let at = egui::pos2(
            rect.left() + value / 255. * rect.width(),
            rect.center().y,
        );
        let response = ui.interact(
            egui::Rect::from_center_size(at, egui::vec2(14., 22.)),
            ui.id().with(("level_handle", i)),
            egui::Sense::drag(),
        );
        ui.painter().circle_filled(at, 5., color);
        ui.painter().circle_stroke(
            at,
            5.,
            egui::Stroke::new(1., egui::Color32::LIGHT_GRAY),
        );
        if response.dragged()
            && let Some(p) = response.interact_pointer_pos()
        {
            let v = ((p.x - rect.left()) / rect.width() * 255.).clamp(0., 255.);
            match i {
                0 => values[0] = v.min(values[1] - 1.).round(),
                2 => values[1] = v.max(values[0] + 1.).round(),
                _ => {
                    let normalized = ((v - black) / (white - black).max(1.))
                        .clamp(0.001, 0.999);
                    values[2] =
                        (normalized.ln() / 0.5_f32.ln()).clamp(0.1, 10.);
                }
            }
        }
    }
}
