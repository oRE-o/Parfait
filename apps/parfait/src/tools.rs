//! Tool containers are independent of their input operation and brush tip.
use crate::{
    app::Parfait,
    brushes::{BrushGroup, Preset},
    document_session::SessionKey,
    widgets::{self, Icon},
};
use eframe::egui::{self, Color32, Rect, Vec2};
use serde::{Deserialize, Serialize};

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize,
)]
pub enum ToolCategory {
    Operation,
    Move,
    Selection,
    AutoSelect,
    Picker,
    #[default]
    Pen,
    Pencil,
    Brush,
    Airbrush,
    Decoration,
    Eraser,
    Blend,
    Fill,
    Gradient,
    Figure,
    Text,
}
impl ToolCategory {
    pub const ALL: [Self; 16] = [
        Self::Operation,
        Self::Move,
        Self::Selection,
        Self::AutoSelect,
        Self::Picker,
        Self::Pen,
        Self::Pencil,
        Self::Brush,
        Self::Airbrush,
        Self::Decoration,
        Self::Eraser,
        Self::Blend,
        Self::Fill,
        Self::Gradient,
        Self::Figure,
        Self::Text,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Operation => "조작",
            Self::Move => "이동",
            Self::Selection => "선택 범위",
            Self::AutoSelect => "자동 선택",
            Self::Picker => "스포이드",
            Self::Pen => "펜",
            Self::Pencil => "연필",
            Self::Brush => "붓",
            Self::Airbrush => "에어브러시",
            Self::Decoration => "데코레이션",
            Self::Eraser => "지우개",
            Self::Blend => "색 혼합",
            Self::Fill => "채우기",
            Self::Gradient => "그라데이션",
            Self::Figure => "도형",
            Self::Text => "텍스트",
        }
    }
    pub fn icon(self) -> Icon {
        match self {
            Self::Operation => Icon::Move,
            Self::Move => Icon::Hand,
            Self::Selection => Icon::Select,
            Self::AutoSelect => Icon::Magic,
            Self::Picker => Icon::Picker,
            Self::Pen => Icon::Pen,
            Self::Pencil => Icon::Pencil,
            Self::Brush => Icon::Brush,
            Self::Airbrush => Icon::Airbrush,
            Self::Decoration => Icon::Material,
            Self::Eraser => Icon::Eraser,
            Self::Blend => Icon::Blend,
            Self::Fill => Icon::Fill,
            Self::Gradient => Icon::Gradient,
            Self::Figure => Icon::Shape,
            Self::Text => Icon::Text,
        }
    }
    pub fn from_brush(group: BrushGroup) -> Self {
        match group {
            BrushGroup::Ink => Self::Pen,
            BrushGroup::Sketch => Self::Pencil,
            BrushGroup::Paint => Self::Brush,
            BrushGroup::Airbrush => Self::Airbrush,
            BrushGroup::Eraser => Self::Eraser,
            BrushGroup::Blend => Self::Blend,
            BrushGroup::Materials => Self::Decoration,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum ToolKind {
    Hand = 1,
    Picker = 2,
    Rotate = 3,
    ZoomIn = 4,
    ZoomOut = 5,
    Object = 6,
    MoveLayer = 7,
    RectangleSelect = 8,
    EllipseSelect = 9,
    Lasso = 10,
    AutoAll = 11,
    AutoLayer = 12,
    AutoReference = 13,
    FillAll = 14,
    FillLayer = 15,
    LinearGradient = 16,
    RadialGradient = 17,
    Line = 18,
    Rectangle = 19,
    Ellipse = 20,
    Text = 21,
    PickerLayer = 22,
    FillReference = 23,
}
impl ToolKind {
    pub fn icon(self) -> Icon {
        match self {
            Self::Hand => Icon::Hand,
            Self::Picker | Self::PickerLayer => Icon::Picker,
            Self::Rotate => Icon::Rotate,
            Self::ZoomIn | Self::ZoomOut => Icon::Search,
            Self::Object | Self::MoveLayer => Icon::Move,
            Self::RectangleSelect | Self::EllipseSelect | Self::Lasso => {
                Icon::Select
            }
            Self::AutoAll | Self::AutoLayer | Self::AutoReference => {
                Icon::Magic
            }
            Self::FillAll | Self::FillLayer | Self::FillReference => Icon::Fill,
            Self::LinearGradient | Self::RadialGradient => Icon::Gradient,
            Self::Line | Self::Rectangle | Self::Ellipse => Icon::Shape,
            Self::Text => Icon::Text,
        }
    }
    pub fn is_selection(self) -> bool {
        matches!(
            self,
            Self::RectangleSelect
                | Self::EllipseSelect
                | Self::Lasso
                | Self::AutoAll
                | Self::AutoLayer
                | Self::AutoReference
        )
    }
}
#[derive(Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct NativeOptions {
    size: f32,
    opacity: f32,
    tolerance: f32,
    contiguous: bool,
    selection_mode: u8,
    shape_fill: bool,
    zoom_drag: bool,
}
impl Default for NativeOptions {
    fn default() -> Self {
        Self {
            size: 4.,
            opacity: 1.,
            tolerance: 10.,
            contiguous: true,
            selection_mode: 0,
            shape_fill: false,
            zoom_drag: true,
        }
    }
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct SubTool {
    pub id: u64,
    pub name: String,
    pub kind: ToolKind,
    pub category: ToolCategory,
    #[serde(default)]
    pub options: NativeOptions,
}
#[derive(Clone, PartialEq)]
pub struct ToolDrag {
    pub owner: SessionKey,
    pub revision: u64,
    pub kind: ToolKind,
    pub start: Vec2,
    pub end: Vec2,
    pub points: Vec<[f32; 2]>,
    pub mode: parfait_core::SelectionMode,
}
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ToolSettings {
    pub category: ToolCategory,
    pub subtools: Vec<SubTool>,
    pub assignments: Vec<(String, ToolCategory)>,
    pub remembered: Vec<(ToolCategory, String)>,
    pub active_id: Option<u64>,
    pub size: f32,
    pub opacity: f32,
    pub tolerance: f32,
    pub contiguous: bool,
    pub zoom_drag: bool,
    pub selection_mode: u8,
    pub shape_fill: bool,
    pub primary: [u8; 3],
    pub secondary: [u8; 3],
    pub color_slot: u8,
    #[serde(default)]
    pub front_color: u8,
    #[serde(skip)]
    pub drag: Option<ToolDrag>,
}
impl Default for ToolSettings {
    fn default() -> Self {
        use ToolCategory as C;
        use ToolKind as K;
        let definitions = [
            ("오브젝트", K::Object, C::Operation),
            ("레이어 이동", K::MoveLayer, C::Operation),
            ("손바닥", K::Hand, C::Move),
            ("회전", K::Rotate, C::Move),
            ("줌 인", K::ZoomIn, C::Move),
            ("줌 아웃", K::ZoomOut, C::Move),
            ("직사각형 선택", K::RectangleSelect, C::Selection),
            ("타원 선택", K::EllipseSelect, C::Selection),
            ("올가미", K::Lasso, C::Selection),
            ("다른 레이어 참조 선택", K::AutoAll, C::AutoSelect),
            ("편집 레이어만 참조 선택", K::AutoLayer, C::AutoSelect),
            ("참조 레이어용 선택", K::AutoReference, C::AutoSelect),
            ("표시 색 취득", K::Picker, C::Picker),
            ("편집 레이어에서 취득", K::PickerLayer, C::Picker),
            ("다른 레이어 참조", K::FillAll, C::Fill),
            ("편집 레이어만 참조", K::FillLayer, C::Fill),
            ("참조 레이어 채우기", K::FillReference, C::Fill),
            ("선형 그라데이션", K::LinearGradient, C::Gradient),
            ("원형 그라데이션", K::RadialGradient, C::Gradient),
            ("직선", K::Line, C::Figure),
            ("직사각형", K::Rectangle, C::Figure),
            ("타원", K::Ellipse, C::Figure),
            ("텍스트", K::Text, C::Text),
        ];
        Self {
            category: C::Pen,
            subtools: definitions
                .into_iter()
                .enumerate()
                .map(|(i, (name, kind, category))| SubTool {
                    id: i as u64 + 1,
                    name: name.into(),
                    kind,
                    category,
                    options: {
                        let mut options = NativeOptions::default();
                        if kind == K::Text {
                            options.size = 32.;
                        }
                        options
                    },
                })
                .collect(),
            assignments: vec![],
            remembered: vec![],
            active_id: None,
            size: 4.,
            opacity: 1.,
            tolerance: 10.,
            contiguous: true,
            zoom_drag: true,
            selection_mode: 0,
            shape_fill: false,
            primary: [46, 43, 54],
            secondary: [255; 3],
            color_slot: 0,
            front_color: 0,
            drag: None,
        }
    }
}
// Runtime gestures do not participate in preference comparisons.

impl ToolSettings {
    pub fn validate(&self) -> bool {
        self.subtools.len() <= 128
            && self.assignments.len() <= 64
            && self.remembered.len() <= 16
            && self.size.is_finite()
            && (1. ..=512.).contains(&self.size)
            && self.opacity.is_finite()
            && (0. ..=1.).contains(&self.opacity)
            && self.tolerance.is_finite()
            && (0. ..=100.).contains(&self.tolerance)
            && self
                .active_id
                .is_none_or(|id| self.subtools.iter().any(|t| t.id == id))
            && self.color_slot <= 2
            && self.front_color <= 1
            && self.selection_mode <= 3
            && self.subtools.iter().enumerate().all(|(i, t)| {
                t.id > 0
                    && t.id <= 1_000_000_000
                    && !t.name.trim().is_empty()
                    && t.name.len() <= 128
                    && t.options.size.is_finite()
                    && (1. ..=512.).contains(&t.options.size)
                    && t.options.opacity.is_finite()
                    && (0. ..=1.).contains(&t.options.opacity)
                    && t.options.tolerance.is_finite()
                    && (0. ..=100.).contains(&t.options.tolerance)
                    && t.options.selection_mode <= 3
                    && !self.subtools[..i].iter().any(|p| p.id == t.id)
            })
            && self.assignments.iter().all(|(s, _)| s.len() <= 128)
            && self
                .remembered
                .iter()
                .all(|(s, n)| ToolCategory::ALL.contains(s) && n.len() <= 128)
    }
    pub fn preset_category(&self, preset: &Preset) -> ToolCategory {
        self.assignments
            .iter()
            .find(|(name, _)| *name == preset.name)
            .map_or_else(
                || ToolCategory::from_brush(BrushGroup::of(&preset.brush)),
                |(_, c)| *c,
            )
    }
    pub fn assign(&mut self, name: String, category: ToolCategory) {
        if let Some(entry) =
            self.assignments.iter_mut().find(|(n, _)| *n == name)
        {
            entry.1 = category;
        } else if self.assignments.len() < 64 {
            self.assignments.push((name, category));
        }
    }
}
impl Parfait {
    pub(crate) fn active_tool(&self) -> Option<ToolKind> {
        if self.navigation_tool == 0 {
            return None;
        }
        self.tools
            .subtools
            .iter()
            .find(|t| {
                t.id == self.tools.active_id.unwrap_or(0)
                    && t.kind as u8 == self.navigation_tool
            })
            .map(|t| t.kind)
            .or_else(|| {
                use ToolKind::*;
                Some(match self.navigation_tool {
                    1 => Hand,
                    2 => Picker,
                    3 => Rotate,
                    4 => ZoomIn,
                    5 => ZoomOut,
                    6 => Object,
                    7 => MoveLayer,
                    8 => RectangleSelect,
                    9 => EllipseSelect,
                    10 => Lasso,
                    11 => AutoAll,
                    12 => AutoLayer,
                    13 => AutoReference,
                    14 => FillAll,
                    15 => FillLayer,
                    16 => LinearGradient,
                    17 => RadialGradient,
                    18 => Line,
                    19 => Rectangle,
                    20 => Ellipse,
                    21 => Text,
                    22 => PickerLayer,
                    23 => FillReference,
                    _ => return None,
                })
            })
    }
    pub(crate) fn remember_native_options(&mut self) {
        if self.navigation_tool == 0 {
            return;
        }
        let options = NativeOptions {
            size: self.tools.size,
            opacity: self.tools.opacity,
            tolerance: self.tools.tolerance,
            contiguous: self.tools.contiguous,
            selection_mode: self.tools.selection_mode,
            shape_fill: self.tools.shape_fill,
            zoom_drag: self.tools.zoom_drag,
        };
        if let Some(t) = self
            .tools
            .subtools
            .iter_mut()
            .find(|t| Some(t.id) == self.tools.active_id)
        {
            t.options = options;
        }
    }
    pub(crate) fn choose_subtool(&mut self, id: u64) {
        self.remember_native_options();
        if let Some(tool) =
            self.tools.subtools.iter().find(|t| t.id == id).cloned()
        {
            self.finish_stroke();
            self.tools.drag = None;
            self.tools.size = tool.options.size;
            self.tools.opacity = tool.options.opacity;
            self.tools.tolerance = tool.options.tolerance;
            self.tools.contiguous = tool.options.contiguous;
            self.tools.selection_mode = tool.options.selection_mode;
            self.tools.shape_fill = tool.options.shape_fill;
            self.tools.zoom_drag = tool.options.zoom_drag;
            self.navigation_tool = tool.kind as u8;
            self.tools.active_id = Some(id);
            self.remember_tool(tool.name);
        }
    }
    pub(crate) fn remember_tool(&mut self, name: String) {
        if let Some(entry) = self
            .tools
            .remembered
            .iter_mut()
            .find(|(c, _)| *c == self.tools.category)
        {
            entry.1 = name;
        } else {
            self.tools.remembered.push((self.tools.category, name));
        }
    }
    pub(crate) fn choose_tool_category(&mut self, category: ToolCategory) {
        self.finish_stroke();
        self.tools.drag = None;
        self.tools.category = category;
        let name = self
            .tools
            .remembered
            .iter()
            .find(|(c, _)| *c == category)
            .map(|(_, n)| n.clone());
        if let Some(i) = self.presets.iter().position(|p| {
            self.tools.preset_category(p) == category
                && name.as_ref() == Some(&p.name)
        }) {
            self.select_preset(i);
            return;
        }
        if let Some(id) = self
            .tools
            .subtools
            .iter()
            .find(|t| t.category == category && name.as_ref() == Some(&t.name))
            .map(|t| t.id)
        {
            self.choose_subtool(id);
            return;
        }
        if let Some(id) = self
            .tools
            .subtools
            .iter()
            .find(|t| t.category == category)
            .map(|t| t.id)
        {
            self.choose_subtool(id);
        } else if let Some(i) = self
            .presets
            .iter()
            .position(|p| self.tools.preset_category(p) == category)
        {
            self.select_preset(i);
        }
    }
    pub(crate) fn tool_properties(&mut self, ui: &mut egui::Ui) {
        use ToolKind as K;
        let Some(kind) = self.active_tool() else {
            return;
        };
        let name = self
            .tools
            .subtools
            .iter()
            .find(|t| Some(t.id) == self.tools.active_id)
            .map_or("보조 도구", |t| t.name.as_str());
        ui.add_sized(
            [ui.available_width(), 42.],
            egui::Label::new(egui::RichText::new(name).strong()).truncate(),
        );
        ui.separator();
        match kind {
            K::Hand | K::Object | K::MoveLayer => {
                ui.vertical_centered(|ui| {
                    ui.add_space(10.);
                    ui.weak("설정이 없습니다.");
                });
            }
            K::Rotate => {
                let mut degrees =
                    self.documents.active_session.view.angle.to_degrees();
                tool_parameter(ui, "회전각", &mut degrees, -180. ..=180.);
                self.documents.active_session.view.angle = degrees.to_radians();
                if ui.small_button("회전 초기화").clicked() {
                    self.documents.active_session.view.angle = 0.;
                }
            }
            K::ZoomIn | K::ZoomOut => {
                ui.checkbox(&mut self.tools.zoom_drag, "드래그로 확대/축소");
            }
            K::Picker | K::PickerLayer => {
                ui.weak(if kind == K::Picker {
                    "표시된 모든 레이어 참조"
                } else {
                    "편집 레이어만 참조"
                });
            }
            K::Text => {
                tool_parameter(
                    ui,
                    "글자 크기",
                    &mut self.tools.size,
                    1. ..=512.,
                );
            }
            _ => {
                if kind.is_selection() {
                    ui.horizontal_wrapped(|ui| {
                        ui.label("작성 방법");
                        for (i, label) in ["새로", "추가", "빼기", "교차"]
                            .into_iter()
                            .enumerate()
                        {
                            ui.selectable_value(
                                &mut self.tools.selection_mode,
                                i as u8,
                                label,
                            );
                        }
                    });
                }
                if matches!(
                    kind,
                    K::AutoAll
                        | K::AutoLayer
                        | K::AutoReference
                        | K::FillAll
                        | K::FillLayer
                        | K::FillReference
                ) {
                    tool_parameter(
                        ui,
                        "색의 오차",
                        &mut self.tools.tolerance,
                        0. ..=100.,
                    );
                    ui.checkbox(&mut self.tools.contiguous, "인접 픽셀 선택");
                }
                if !kind.is_selection() {
                    let mut opacity = self.tools.opacity * 100.;
                    tool_parameter(ui, "불투명도", &mut opacity, 0. ..=100.);
                    self.tools.opacity = opacity / 100.;
                }
                if matches!(kind, K::Line | K::Rectangle | K::Ellipse) {
                    tool_parameter(
                        ui,
                        "선 두께",
                        &mut self.tools.size,
                        1. ..=512.,
                    );
                    if kind != K::Line {
                        ui.checkbox(&mut self.tools.shape_fill, "내부 채우기");
                    }
                }
            }
        }
    }
    pub(crate) fn color_slots(&mut self, ui: &mut egui::Ui, vertical: bool) {
        if self.tools.color_slot == 0 {
            self.tools.primary = self.brush.color;
        } else if self.tools.color_slot == 1 {
            self.tools.secondary = self.brush.color;
        }
        let compact = vertical || ui.available_width() < 320.;
        if self.tools.color_slot < 2 {
            self.tools.front_color = self.tools.color_slot;
        }
        let square = if vertical { 20. } else { 25. };
        let offset = if vertical { 8. } else { 12. };
        let size = if compact {
            egui::vec2(square + offset, square + offset + 15.)
        } else {
            egui::vec2(112., 24.)
        };
        let (bounds, _) = ui.allocate_exact_size(size, egui::Sense::hover());
        let rects = if compact {
            [
                Rect::from_min_size(bounds.min, egui::vec2(square, square)),
                Rect::from_min_size(
                    bounds.min + egui::vec2(offset, offset),
                    egui::vec2(square, square),
                ),
                Rect::from_min_size(
                    bounds.min + egui::vec2(0., square + offset + 5.),
                    egui::vec2(size.x, 10.),
                ),
            ]
        } else {
            std::array::from_fn(|i| {
                Rect::from_min_size(
                    bounds.min + egui::vec2(i as f32 * 38., 0.),
                    egui::vec2(36., 24.),
                )
            })
        };
        let front = self.tools.front_color;
        let labels = ["메인 컬러 (색상 1)", "서브 컬러 (색상 2)", "투명색"];
        // Register the exposed part of the rear swatch only. Overlapping ink
        // buttons never both receive the same click, even in a batched frame.
        for slot in [1 - front, front, 2] {
            let r = rects[slot as usize];
            let rear = compact && slot == 1 - front;
            let targets = if rear {
                let f = rects[front as usize];
                if slot == 1 {
                    [
                        Rect::from_min_max(
                            egui::pos2(f.right(), r.top()),
                            r.max,
                        ),
                        Rect::from_min_max(
                            egui::pos2(r.left(), f.bottom()),
                            egui::pos2(f.right(), r.bottom()),
                        ),
                    ]
                } else {
                    [
                        Rect::from_min_max(
                            r.min,
                            egui::pos2(r.right(), f.top()),
                        ),
                        Rect::from_min_max(
                            egui::pos2(r.left(), f.top()),
                            egui::pos2(f.left(), r.bottom()),
                        ),
                    ]
                }
            } else {
                [r, Rect::NOTHING]
            };
            let mut clicked = false;
            for (part, target) in targets.into_iter().enumerate() {
                if !target.is_positive() {
                    continue;
                }
                let response = ui.interact(
                    target,
                    ui.id().with(("ink", slot, part)),
                    egui::Sense::click(),
                );
                if part == 0 {
                    response.widget_info(|| {
                        egui::WidgetInfo::selected(
                            egui::WidgetType::SelectableLabel,
                            ui.is_enabled(),
                            self.tools.color_slot == slot,
                            labels[slot as usize],
                        )
                    });
                }
                clicked |= response.clicked();
                response.on_hover_text(labels[slot as usize]);
            }
            ui.painter().rect_filled(r, 2., Color32::from_gray(58));
            let color = if slot == 0 {
                self.tools.primary
            } else {
                self.tools.secondary
            };
            if slot == 2 {
                for y in 0..(r.height() / 5.).ceil() as usize {
                    for x in 0..(r.width() / 5.).ceil() as usize {
                        let cell = Rect::from_min_size(
                            r.min + egui::vec2(x as f32 * 5., y as f32 * 5.),
                            egui::vec2(5., 5.),
                        )
                        .intersect(r.shrink(2.));
                        if cell.is_positive() {
                            ui.painter().rect_filled(
                                cell,
                                0.,
                                Color32::from_gray(if (x + y) % 2 == 0 {
                                    220
                                } else {
                                    140
                                }),
                            );
                        }
                    }
                }
            } else {
                ui.painter().rect_filled(
                    r.shrink(2.),
                    1.,
                    Color32::from_rgb(color[0], color[1], color[2]),
                );
            }
            ui.painter().rect_stroke(
                r.shrink(0.5),
                2.,
                egui::Stroke::new(
                    if self.tools.color_slot == slot { 2. } else { 1. },
                    if self.tools.color_slot == slot {
                        crate::theme::ACCENT
                    } else {
                        Color32::from_gray(110)
                    },
                ),
                egui::StrokeKind::Inside,
            );
            if clicked {
                self.tools.color_slot = slot;
                if slot < 2 {
                    self.tools.front_color = slot;
                    self.brush.color = color;
                }
                ui.ctx().request_repaint();
            }
        }
        if compact && !vertical {
            let [h, s, v] = crate::widgets::rgb_to_hsv(self.brush.color);
            ui.painter().text(
                egui::pos2(ui.max_rect().right(), bounds.bottom() - 1.),
                egui::Align2::RIGHT_BOTTOM,
                format!(
                    "H {:.0}  S {:.0}  V {:.0}",
                    h * 360.,
                    s * 100.,
                    v * 100.
                ),
                egui::FontId::proportional(11.),
                Color32::from_gray(196),
            );
        }
    }

    pub(crate) fn nonbrush_rows(&mut self, ui: &mut egui::Ui) {
        let ids: Vec<_> = self
            .tools
            .subtools
            .iter()
            .filter(|t| t.category == self.tools.category)
            .map(|t| t.id)
            .collect();
        for id in ids {
            let t = self
                .tools
                .subtools
                .iter()
                .find(|t| t.id == id)
                .unwrap()
                .clone();
            ui.push_id(("subtool", id), |ui| {
                let (rect, _) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), 42.),
                    egui::Sense::hover(),
                );
                let response = widgets::brush_row(
                    ui,
                    rect,
                    &t.name,
                    self.navigation_tool == t.kind as u8
                        && self.tools.active_id == Some(id),
                    t.kind.icon(),
                    None,
                );
                if response.clicked() {
                    self.choose_subtool(id);
                }
                response.context_menu(|ui| {
                    if let Some(tool) =
                        self.tools.subtools.iter_mut().find(|s| s.id == id)
                    {
                        ui.add(
                            egui::TextEdit::singleline(&mut tool.name)
                                .char_limit(40),
                        );
                    }
                    ui.menu_button("다른 분류로 이동", |ui| {
                        for c in ToolCategory::ALL {
                            if ui.button(c.label()).clicked() {
                                self.tools
                                    .subtools
                                    .iter_mut()
                                    .find(|s| s.id == id)
                                    .unwrap()
                                    .category = c;
                                ui.close();
                            }
                        }
                    });
                    if ui.button("보조 도구 복제").clicked()
                        && self.tools.subtools.len() < 128
                    {
                        let mut t = t.clone();
                        t.id = self
                            .tools
                            .subtools
                            .iter()
                            .map(|t| t.id)
                            .max()
                            .unwrap_or(0)
                            + 1;
                        t.name.push_str(" 복사");
                        self.tools.subtools.push(t);
                        ui.close();
                    }
                    if ui.button("보조 도구 삭제").clicked() {
                        self.tools.subtools.retain(|s| s.id != id);
                        if self.tools.active_id == Some(id) {
                            self.tools.active_id = None;
                            self.navigation_tool = 0;
                            self.choose_tool_category(self.tools.category);
                        }
                        ui.close();
                    }
                });
            });
        }
    }
    pub(crate) fn subtool_add_menu(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("＋ 보조 도구 추가", |ui| {
            ui.set_max_height(360.);
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.strong("브러시 / 소재");
                for i in 0..self.presets.len() {
                    let name = self.presets[i].name.clone();
                    if ui.button(&name).clicked() {
                        self.tools.assign(name, self.tools.category);
                        self.select_preset(i);
                        ui.close();
                    }
                }
                ui.separator();
                ui.strong("기본 보조 도구");
                for mut t in ToolSettings::default().subtools {
                    if ui.button(&t.name).clicked()
                        && self.tools.subtools.len() < 128
                    {
                        t.id = self
                            .tools
                            .subtools
                            .iter()
                            .map(|t| t.id)
                            .max()
                            .unwrap_or(0)
                            + 1;
                        t.category = self.tools.category;
                        let id = t.id;
                        self.tools.subtools.push(t);
                        self.choose_subtool(id);
                        ui.close();
                    }
                }
            });
        });
    }
}
fn tool_parameter(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
) {
    ui.push_id(label, |ui| {
        crate::brush_widgets::parameter(
            ui,
            value,
            crate::brush_widgets::Parameter {
                label,
                range,
                decimals: 1,
                unit: "",
                logarithmic: false,
                dynamics: None,
                layout: crate::brush_widgets::ParameterLayout::Palette,
            },
        );
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    #[test]
    fn brush_in_move_category_survives_restart_and_native_tools_restore() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open_legacy(dir.path().to_owned()).unwrap();
        let name = app.presets[0].name.clone();
        app.tools.assign(name.clone(), ToolCategory::Move);
        app.tools.category = ToolCategory::Move;
        app.select_preset(0);
        assert_eq!(app.navigation_tool, 0);
        app.choose_tool_category(ToolCategory::Pen);
        app.choose_tool_category(ToolCategory::Move);
        assert_eq!(app.tip_name, name);
        assert_eq!(app.navigation_tool, 0);
        let hand = app
            .tools
            .subtools
            .iter()
            .find(|t| t.kind == ToolKind::Hand)
            .unwrap()
            .id;
        let brush = app.brush.clone();
        app.choose_subtool(hand);
        app.tools.size = 81.;
        app.tools.opacity = 0.31;
        let picker = app
            .tools
            .subtools
            .iter()
            .find(|t| t.kind == ToolKind::Picker)
            .unwrap()
            .id;
        app.choose_subtool(picker);
        assert_eq!(app.tools.size, 4.);
        app.choose_subtool(hand);
        assert_eq!(app.tools.size, 81.);
        assert_eq!(app.tools.opacity, 0.31);
        assert!(app.brush == brush);
        app.persist_preferences(true);
        drop(app);
        let app = Parfait::open_legacy(dir.path().to_owned()).unwrap();
        assert!(!app.preferences_read_failed);
        assert_eq!(app.tools.category, ToolCategory::Move);
        assert_eq!(
            app.tools.preset_category(&app.presets[0]),
            ToolCategory::Move
        );
        assert_eq!(app.active_tool(), Some(ToolKind::Hand));
    }
    #[test]
    fn selection_and_shape_gestures_cancel_and_keep_single_history_entry() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open_legacy(dir.path().to_owned()).unwrap();
        let select = app
            .tools
            .subtools
            .iter()
            .find(|t| t.kind == ToolKind::RectangleSelect)
            .unwrap()
            .id;
        app.choose_subtool(select);
        let before = app.documents.active_session.document.clone();
        assert!(app.tool_pointer_down(egui::vec2(2., 2.), Default::default()));
        app.tool_pointer_up(egui::vec2(12., 10.), false);
        assert_eq!(
            app.documents
                .active_session
                .document
                .selection_value(2 * before.width as usize + 2),
            255
        );
        assert_eq!(app.documents.active_session.document.selection_value(0), 0);
        assert!(Arc::ptr_eq(
            &app.documents.active_session.document.layers[1].pixels,
            &before.layers[1].pixels
        ));
        let mut invalid = app.tools.clone();
        invalid.subtools[1].id = invalid.subtools[0].id;
        assert!(!invalid.validate());
    }
}
