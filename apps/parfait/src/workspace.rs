//! Persistent dock rows, in-app floating palettes and independent native palette windows.
use crate::app::Parfait;
use eframe::egui::{self, Color32, Pos2, Rect};
use serde::{Deserialize, Serialize};
mod resize;

fn default_width_weight() -> f32 {
    1.
}

fn detached_opacity(
    ui: &egui::Ui,
    bounds: Rect,
    inactive: f32,
    dragging: bool,
) -> f32 {
    if dragging {
        0.5
    } else if ui.input(|i| {
        i.pointer.hover_pos().is_some_and(|pos| bounds.contains(pos))
    }) {
        1.
    } else {
        inactive
    }
}

fn insertion_bar(from: Pos2, to: Pos2) -> Rect {
    if (from.x - to.x).abs() < 0.5 {
        Rect::from_min_max(from - egui::vec2(1.5, 0.), to + egui::vec2(1.5, 0.))
    } else {
        Rect::from_min_max(from - egui::vec2(0., 1.5), to + egui::vec2(0., 1.5))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Palette {
    Brushes,
    Properties,
    Sizes,
    Color,
    Navigator,
    Layers,
    History,
    Tools,
}
impl Palette {
    pub const ALL: [Self; 8] = [
        Self::Brushes,
        Self::Properties,
        Self::Sizes,
        Self::Color,
        Self::Navigator,
        Self::Layers,
        Self::History,
        Self::Tools,
    ];
    pub fn title(self) -> &'static str {
        match self {
            Self::Brushes => "보조 도구",
            Self::Properties => "도구 속성",
            Self::Sizes => "브러시 크기",
            Self::Color => "컬러 휠",
            Self::Navigator => "내비게이터",
            Self::Layers => "레이어",
            Self::History => "작업 내역",
            Self::Tools => "도구",
        }
    }
    pub(super) fn minimum_width(self) -> f32 {
        if self == Self::Tools { 64. } else { 180. }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Location {
    Left,
    Right,
    Top,
    Floating,
    Window,
    Rail,
}
#[derive(Clone, Copy, Debug)]
enum DockDirection {
    Left,
    Right,
    Top,
    Bottom,
}
impl DockDirection {
    fn at(rect: Rect, pos: Pos2) -> Self {
        let edge = (rect.width() * 0.12).clamp(10., 24.);
        if pos.y < rect.top() {
            Self::Top
        } else if pos.y > rect.bottom() {
            Self::Bottom
        } else if pos.x < rect.left() + edge {
            Self::Left
        } else if pos.x > rect.right() - edge {
            Self::Right
        } else if pos.y < rect.center().y {
            Self::Top
        } else {
            Self::Bottom
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Placement {
    pub palette: Palette,
    pub location: Location,
    pub visible: bool,
    #[serde(default)]
    pub collapsed: bool,
    #[serde(default)]
    pub row: Option<Palette>,
    #[serde(default)]
    pub window_rect: Option<[f32; 4]>,
    #[serde(default = "default_width_weight")]
    pub inactive_opacity: f32,
    #[serde(skip)]
    scroll_offset: f32,
    pub height: f32,
    #[serde(default = "default_width_weight")]
    width_weight: f32,
    pub rect: [f32; 4],
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub panels: Vec<Placement>,
    pub left_width: f32,
    pub right_width: f32,
    #[serde(skip)]
    pub dragging: Option<Palette>,
    #[serde(skip)]
    pub resizing: bool,
    #[serde(skip)]
    slots: Vec<(Palette, Location, Rect)>,
    #[serde(skip)]
    drag_offset: egui::Vec2,
    #[serde(skip)]
    dock_bounds: Vec<(Location, Rect)>,
    #[serde(skip)]
    dock_clips: Vec<(Location, Rect)>,
    #[serde(skip)]
    drag_origin: Option<(usize, Placement)>,
    #[cfg(test)]
    #[serde(skip)]
    headers: Vec<(Palette, Rect)>,
}
impl Default for Workspace {
    fn default() -> Self {
        Self {
            panels: Palette::ALL
                .into_iter()
                .enumerate()
                .map(|(i, p)| Placement {
                    palette: p,
                    location: if p == Palette::Tools {
                        Location::Rail
                    } else if i < 4 {
                        Location::Left
                    } else {
                        Location::Right
                    },
                    visible: p != Palette::History,
                    collapsed: p == Palette::Sizes,
                    row: None,
                    window_rect: None,
                    inactive_opacity: 1.,
                    scroll_offset: 0.,
                    height: [250., 265., 115., 190., 190., 400., 200., 300.][i],
                    width_weight: 1.,
                    rect: [
                        300. + i as f32 * 24.,
                        120. + i as f32 * 24.,
                        if p == Palette::Tools { 64. } else { 270. },
                        340.,
                    ],
                })
                .collect(),
            left_width: 260.,
            right_width: 250.,
            dragging: None,
            resizing: false,
            slots: Vec::new(),
            drag_offset: egui::Vec2::ZERO,
            dock_bounds: Vec::new(),
            dock_clips: Vec::new(),
            drag_origin: None,
            #[cfg(test)]
            headers: Vec::new(),
        }
    }
}
impl Workspace {
    pub(crate) fn upgrade_tools(&mut self) {
        if self.panels.len() == 7
            && !self.panels.iter().any(|p| p.palette == Palette::Tools)
        {
            self.panels.push(Self::default().panels.remove(7));
        }
    }
    pub(crate) fn upgrade_history(&mut self) {
        if self.panels.len() == 6
            && !self
                .panels
                .iter()
                .any(|placement| placement.palette == Palette::History)
        {
            self.panels.push(Self::default().panels.remove(6));
        }
    }
    pub(crate) fn same_preferences(&self, other: &Self) -> bool {
        self.left_width == other.left_width
            && self.right_width == other.right_width
            && self.panels.len() == other.panels.len()
            && self.panels.iter().zip(&other.panels).all(|(current, saved)| {
                current.palette == saved.palette
                    && current.location == saved.location
                    && current.visible == saved.visible
                    && current.collapsed == saved.collapsed
                    && current.row == saved.row
                    && current.window_rect == saved.window_rect
                    && current.inactive_opacity == saved.inactive_opacity
                    && current.height == saved.height
                    && current.width_weight == saved.width_weight
                    && current.rect == saved.rect
            })
    }
    pub fn validate(&self) -> bool {
        self.panels.len() == Palette::ALL.len()
            && Palette::ALL.iter().all(|p| {
                self.panels.iter().filter(|s| s.palette == *p).count() == 1
            })
            && [self.left_width, self.right_width]
                .iter()
                .all(|v| v.is_finite() && (180. ..=800.).contains(v))
            && self.panels.iter().all(|s| {
                s.window_rect.is_none_or(|r| {
                    r.iter().all(|v| v.is_finite())
                        && r[2] >= s.palette.minimum_width()
                        && r[3] >= 23.
                }) && s.inactive_opacity.is_finite()
                    && (0.2..=1.).contains(&s.inactive_opacity)
                    && s.height.is_finite()
                    && (60. ..=2000.).contains(&s.height)
                    && s.width_weight.is_finite()
                    && (0. ..=1000.).contains(&s.width_weight)
                    && s.rect.iter().all(|v| v.is_finite())
                    && s.rect[2] >= s.palette.minimum_width()
                    && s.rect[3] >= 80.
                    && (s.location != Location::Rail
                        || s.palette == Palette::Tools)
            })
    }
    pub fn relocate(
        &mut self,
        palette: Palette,
        location: Location,
        before: Option<Palette>,
        pos: Pos2,
    ) {
        let i = self.panels.iter().position(|s| s.palette == palette).unwrap();
        let mut item = self.panels.remove(i);
        self.reserve_row_identity(palette);
        item.location = location;
        item.row = before.and_then(|p| {
            self.panels
                .iter()
                .find(|s| s.palette == p)
                .map(|s| s.row.unwrap_or(s.palette))
        });
        if location == Location::Top {
            item.row = Some(Palette::Brushes);
        }
        if location == Location::Floating {
            item.rect[0] = pos.x;
            item.rect[1] = pos.y;
        }
        let index = before
            .and_then(|p| self.panels.iter().position(|s| s.palette == p))
            .unwrap_or(self.panels.len());
        self.panels.insert(index, item);
    }
    fn dock_rows(&self, side: Location) -> Vec<Vec<Palette>> {
        let mut rows: Vec<(Palette, Vec<Palette>)> = Vec::new();
        for p in self.panels.iter().filter(|p| p.visible && p.location == side)
        {
            let key = if side == Location::Top {
                Palette::Brushes
            } else {
                p.row.unwrap_or(p.palette)
            };
            if let Some((_, row)) = rows.iter_mut().find(|(id, _)| *id == key) {
                row.push(p.palette);
            } else {
                rows.push((key, vec![p.palette]));
            }
        }
        rows.into_iter().map(|(_, row)| row).collect()
    }
    fn insert_line(
        &self,
        target: (Palette, Location, Rect),
        direction: DockDirection,
    ) -> Rect {
        let (palette, side, slot) = target;
        let row_key = |palette| {
            if side == Location::Top {
                Palette::Brushes
            } else {
                self.panels
                    .iter()
                    .find(|p| p.palette == palette)
                    .unwrap()
                    .row
                    .unwrap_or(palette)
            }
        };
        let key = row_key(palette);
        let row = self
            .slots
            .iter()
            .filter(|(p, location, _)| *location == side && row_key(*p) == key)
            .map(|(_, _, r)| *r)
            .reduce(|a, b| a.union(b))
            .unwrap_or(slot);
        let (from, to) = match direction {
            DockDirection::Left => (
                egui::pos2(slot.left(), row.top()),
                egui::pos2(slot.left(), row.bottom()),
            ),
            DockDirection::Right => (
                egui::pos2(slot.right(), row.top()),
                egui::pos2(slot.right(), row.bottom()),
            ),
            DockDirection::Top => (row.left_top(), row.right_top()),
            DockDirection::Bottom => (row.left_bottom(), row.right_bottom()),
        };
        insertion_bar(from, to)
    }

    /// Rendered rows also supply the insertion-marker boundaries.
    fn dock_layout(
        &self,
        side: Location,
        bounds: Rect,
    ) -> Vec<(Palette, Rect)> {
        let rows = self.dock_rows(side);
        let width = bounds
            .width()
            .max(rows.iter().map(|r| r.len()).max().unwrap_or(1) as f32 * 180.);
        let preferred: Vec<_> = rows
            .iter()
            .map(|row| {
                row.iter()
                    .filter_map(|palette| {
                        let p = self
                            .panels
                            .iter()
                            .find(|p| p.palette == *palette)
                            .unwrap();
                        (!p.collapsed).then_some(p.height + 29.)
                    })
                    .reduce(f32::max)
            })
            .collect();
        let fixed =
            preferred.iter().filter(|h| h.is_none()).count() as f32 * 23.;
        let expanded = preferred.iter().filter(|h| h.is_some()).count();
        let available = (bounds.height()
            - fixed
            - rows.len().saturating_sub(1) as f32 * 3.)
            .max(expanded as f32 * 89.);
        let mut heights: Vec<f32> =
            preferred.iter().map(|p| p.map_or(23., |_| 0.)).collect();
        let mut remaining = available;
        loop {
            let weight: f32 = preferred
                .iter()
                .enumerate()
                .filter(|(i, p)| p.is_some() && heights[*i] == 0.)
                .map(|(_, p)| p.unwrap())
                .sum();
            let mut clamped = false;
            for (i, p) in preferred.iter().enumerate() {
                if let Some(p) = p
                    && heights[i] == 0.
                    && remaining * p / weight.max(1.) < 89.
                {
                    heights[i] = 89.;
                    remaining -= 89.;
                    clamped = true;
                }
            }
            if !clamped {
                for (i, p) in preferred.iter().enumerate() {
                    if let Some(p) = p
                        && heights[i] == 0.
                    {
                        heights[i] = remaining * p / weight.max(1.);
                    }
                }
                break;
            }
        }
        let mut y = bounds.top();
        let mut result = Vec::new();
        for (row_index, row) in rows.into_iter().enumerate() {
            let row_height = heights[row_index];
            let weight_sum: f32 = row
                .iter()
                .map(|palette| {
                    self.panels
                        .iter()
                        .find(|placement| placement.palette == *palette)
                        .unwrap()
                        .width_weight
                })
                .sum();
            let extra = (width - row.len() as f32 * 180.).max(0.);
            let mut left = bounds.left();
            for palette in &row {
                let placement = self
                    .panels
                    .iter()
                    .find(|placement| placement.palette == *palette)
                    .unwrap();
                let cell_width = 180.
                    + extra
                        * if weight_sum > 0. {
                            placement.width_weight / weight_sum
                        } else {
                            1. / row.len() as f32
                        };
                let height = if placement.collapsed { 23. } else { row_height };
                result.push((
                    *palette,
                    Rect::from_min_size(
                        egui::pos2(left, y),
                        egui::vec2(cell_width, height),
                    ),
                ));
                left += cell_width;
            }
            y += row_height + 3.;
        }
        result
    }
    fn reserve_row_identity(&mut self, palette: Palette) {
        for side in [Location::Left, Location::Right] {
            if let Some(key) = self
                .panels
                .iter()
                .find(|p| {
                    p.palette != palette
                        && p.location == side
                        && p.row == Some(palette)
                })
                .map(|p| p.palette)
            {
                for p in &mut self.panels {
                    if p.location == side && p.row == Some(palette) {
                        p.row = Some(key);
                    }
                }
            }
        }
    }
    fn dock_to(
        &mut self,
        palette: Palette,
        other: Palette,
        direction: DockDirection,
    ) {
        if palette == other {
            return;
        }
        let mut item = self.panels.remove(
            self.panels.iter().position(|p| p.palette == palette).unwrap(),
        );
        self.reserve_row_identity(palette);
        let target =
            self.panels.iter().find(|p| p.palette == other).unwrap().clone();
        let group = target.row.unwrap_or(other);
        item.location = target.location;
        if matches!(direction, DockDirection::Left | DockDirection::Right) {
            // Give all existing members the same explicit row identity.
            for p in &mut self.panels {
                if p.location == target.location
                    && p.row.unwrap_or(p.palette) == group
                {
                    p.row = Some(group);
                }
            }
            item.row = Some(group);
        } else {
            item.row = None;
        }
        let members: Vec<_> = self
            .panels
            .iter()
            .enumerate()
            .filter(|(_, p)| {
                p.location == target.location
                    && p.row.unwrap_or(p.palette) == group
            })
            .map(|(i, _)| i)
            .collect();
        let target_index =
            self.panels.iter().position(|p| p.palette == other).unwrap();
        let index = match direction {
            DockDirection::Left => target_index,
            DockDirection::Right => target_index + 1,
            DockDirection::Top => {
                members.first().copied().unwrap_or(target_index)
            }
            DockDirection::Bottom => {
                members.last().copied().unwrap_or(target_index) + 1
            }
        };
        self.panels.insert(index, item);
    }
    fn header(&mut self, ui: &mut egui::Ui, palette: Palette) {
        let (rect, _) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), 23.),
            egui::Sense::hover(),
        );
        let compact = palette == Palette::Tools && rect.width() < 90.;
        let drag_rect = if compact {
            rect
        } else {
            Rect::from_min_max(rect.min, rect.max - egui::vec2(42., 0.))
        };
        let response = ui.interact(
            drag_rect,
            ui.id().with("move"),
            egui::Sense::click_and_drag(),
        );
        response.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::WidgetType::Button,
                true,
                format!("Move {} palette", palette.title()),
            )
        });
        #[cfg(test)]
        self.headers.push((palette, drag_rect));
        ui.painter().rect_filled(rect, 0., Color32::from_gray(48));
        let painter = ui.painter().with_clip_rect(drag_rect);
        painter.text(
            rect.left_center() + egui::vec2(6., 0.),
            egui::Align2::LEFT_CENTER,
            crate::widgets::Icon::Grip.glyph(),
            egui::FontId::new(10., egui::FontFamily::Name("icons".into())),
            Color32::LIGHT_GRAY,
        );
        if !compact {
            painter.text(
                rect.left_center() + egui::vec2(22., 0.),
                egui::Align2::LEFT_CENTER,
                palette.title(),
                egui::FontId::proportional(11.),
                Color32::LIGHT_GRAY,
            );
        }
        let i = self.panels.iter().position(|p| p.palette == palette).unwrap();
        if !compact {
            let collapse = Rect::from_min_size(
                rect.right_top() - egui::vec2(42., 0.),
                egui::vec2(21., 23.),
            );
            let close = Rect::from_min_size(
                rect.right_top() - egui::vec2(21., 0.),
                egui::vec2(21., 23.),
            );
            if ui
                .put(
                    collapse,
                    egui::Button::new(if self.panels[i].collapsed {
                        "+"
                    } else {
                        "−"
                    })
                    .frame(false),
                )
                .on_hover_text("팔레트 접기 / 펼치기")
                .clicked()
            {
                self.panels[i].collapsed = !self.panels[i].collapsed;
            }
            if ui
                .put(close, egui::Button::new("×").frame(false))
                .on_hover_text("팔레트 숨기기 · 창 메뉴에서 다시 열기")
                .clicked()
            {
                self.panels[i].visible = false;
            }
        }
        response.context_menu(|ui| {
            if palette == Palette::Tools {
                if ui
                    .button(if compact {
                        "막대 도킹"
                    } else {
                        "왼쪽 도구 막대로 도킹"
                    })
                    .clicked()
                {
                    self.reserve_row_identity(palette);
                    self.panels[i].location = Location::Rail;
                    self.panels[i].row = None;
                    self.panels[i].collapsed = false;
                    ui.close();
                }
                if compact && ui.button("숨기기").clicked() {
                    self.panels[i].visible = false;
                    ui.close();
                }
            }
            let spawn = ui.button(if compact {
                "독립 창"
            } else {
                "독립 창으로 열기"
            });
            #[cfg(test)]
            ui.ctx().data_mut(|d| {
                d.insert_temp(
                    egui::Id::new(("native_command", palette)),
                    spawn.rect,
                )
            });
            if spawn.clicked() {
                self.panels[i].location = Location::Window;
                ui.close();
            }
            if compact {
                ui.label(format!(
                    "불투명도 {:.0}%",
                    self.panels[i].inactive_opacity * 100.
                ));
                ui.spacing_mut().slider_width = 40.;
                ui.add(
                    egui::Slider::new(
                        &mut self.panels[i].inactive_opacity,
                        0.2..=1.,
                    )
                    .step_by(0.01)
                    .show_value(false),
                )
                .on_hover_text("마우스가 팔레트 밖에 있을 때의 불투명도 · 마우스를 올리면 100%");
            } else {
                ui.menu_button("마우스 밖 불투명도", |ui| {
                    ui.add(
                        egui::Slider::new(
                            &mut self.panels[i].inactive_opacity,
                            0.2..=1.,
                        )
                        .step_by(0.01)
                        .text("불투명도")
                        .custom_formatter(|v, _| format!("{:.0}%", v * 100.))
                        .custom_parser(|v| {
                            v.trim()
                                .trim_end_matches('%')
                                .trim()
                                .parse::<f64>()
                                .ok()
                                .map(|value| value / 100.)
                        }),
                    );
                });
            }
            if ui
                .button(if compact { "위 도킹" } else { "위에 도킹" })
                .clicked()
            {
                self.reserve_row_identity(palette);
                self.panels[i].location = Location::Top;
                self.panels[i].row = Some(Palette::Brushes);
                ui.close();
            }
            if ui
                .button(if compact {
                    "앱 안"
                } else {
                    "앱 안에 띄우기"
                })
                .clicked()
            {
                self.panels[i].location = Location::Floating;
                ui.close();
            }
            if ui
                .button(if compact {
                    "왼쪽 도킹"
                } else {
                    "왼쪽에 도킹"
                })
                .clicked()
            {
                self.reserve_row_identity(palette);
                self.panels[i].location = Location::Left;
                self.panels[i].row = None;
                ui.close();
            }
            if ui
                .button(if compact {
                    "오른쪽 도킹"
                } else {
                    "오른쪽에 도킹"
                })
                .clicked()
            {
                self.reserve_row_identity(palette);
                self.panels[i].location = Location::Right;
                self.panels[i].row = None;
                ui.close();
            }
        });
        let tool_drag = if palette == Palette::Tools {
            resize::track(
                ui,
                response.id,
                drag_rect,
                egui::CursorIcon::Grab,
                [rect.left(), rect.top(), rect.width(), rect.height()],
            )
            .filter(|drag| drag.delta.length() > 4.)
        } else {
            None
        };
        let drag_started = tool_drag.is_some()
            || (palette != Palette::Tools && response.drag_started());
        if drag_started && self.panels[i].location == Location::Window {
            if ui.input(|input| input.pointer.primary_down()) {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }
        } else if drag_started && self.dragging.is_none() {
            self.drag_origin = Some((i, self.panels[i].clone()));
            self.drag_offset = tool_drag
                .map(|drag| drag.origin)
                .or_else(|| ui.input(|input| input.pointer.press_origin()))
                .unwrap_or(rect.min)
                - rect.min;
            let item = &mut self.panels[i];
            if matches!(
                item.location,
                Location::Left | Location::Right | Location::Top
            ) && !item.collapsed
            {
                item.height = (ui.max_rect().height() - 29.).clamp(60., 2000.);
            }
            item.location = Location::Floating;
            item.rect[0] =
                rect.left() + tool_drag.map_or(0., |drag| drag.delta.x);
            item.rect[1] =
                rect.top() + tool_drag.map_or(0., |drag| drag.delta.y);
            item.rect[2] = rect.width().max(palette.minimum_width());
            self.dragging = Some(palette);
        }
        response.on_hover_cursor(egui::CursorIcon::Grab).on_hover_text(
            "드래그하여 이동 / 도킹 · 우클릭하여 배치 선택 · Esc로 이동 취소",
        );
    }
    /// Identical header, content and resize grip in both docked and floating hosts.
    fn shell(
        &mut self,
        ui: &mut egui::Ui,
        app: &mut Parfait,
        palette: Palette,
    ) {
        ui.spacing_mut().item_spacing.y = 0.;
        if palette == Palette::Tools {
            ui.spacing_mut().scroll = egui::style::ScrollStyle::floating();
        }
        self.header(ui, palette);
        let p =
            self.panels.iter().find(|p| p.palette == palette).unwrap().clone();
        if p.collapsed || !p.visible {
            return;
        }
        let body_height = if matches!(
            p.location,
            Location::Left | Location::Right | Location::Top | Location::Rail
        ) {
            (ui.max_rect().height() - 29.).max(60.)
        } else {
            p.height
        };
        if matches!(palette, Palette::Brushes | Palette::Properties) {
            app.palette(ui, palette, body_height);
        } else {
            let body = egui::ScrollArea::vertical()
                .id_salt(("palette_body", palette))
                .vertical_scroll_offset(p.scroll_offset)
                .max_height(body_height)
                .min_scrolled_height(body_height)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 3.;
                    app.palette(ui, palette, body_height);
                });
            self.panels
                .iter_mut()
                .find(|p| p.palette == palette)
                .unwrap()
                .scroll_offset = body.state.offset.y;
        }
        if matches!(
            p.location,
            Location::Left | Location::Right | Location::Top | Location::Rail
        ) {
            return;
        }
        let (grip, _) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), 6.),
            egui::Sense::hover(),
        );
        ui.painter().line_segment(
            [
                grip.center() - egui::vec2(18., 0.),
                grip.center() + egui::vec2(18., 0.),
            ],
            egui::Stroke::new(1., Color32::GRAY),
        );
    }
    fn dock_minimum(&self, side: Location, margin: f32) -> f32 {
        (self.dock_rows(side).iter().map(|row| row.len()).max().unwrap_or(1)
            as f32
            * 180.
            + margin)
            .min(800.)
    }
    fn dock_width(&self, side: Location, margin: f32) -> f32 {
        let minimum = self.dock_minimum(side, margin);
        let width = if side == Location::Left {
            self.left_width
        } else {
            self.right_width
        };
        width.clamp(minimum, 800.)
    }

    fn side(
        &mut self,
        ui: &mut egui::Ui,
        app: &mut Parfait,
        location: Location,
    ) {
        if !self.panels.iter().any(|p| p.location == location && p.visible) {
            return;
        }
        let left = location == Location::Left;
        let panel = if location == Location::Top {
            egui::Panel::top("dock_top")
        } else if left {
            egui::Panel::left("dock_left")
        } else {
            egui::Panel::right("dock_right")
        };
        let dock_margin =
            egui::Frame::side_top_panel(ui.style()).total_margin().sum().x
                + ui.spacing().scroll.allocated_width()
                + 1.;
        let width = if location == Location::Top {
            self.panels
                .iter()
                .filter(|p| p.visible && p.location == location)
                .map(|p| p.height + 29.)
                .fold(140., f32::max)
                .min(ui.available_height() * 0.4)
        } else {
            self.dock_width(location, dock_margin)
        };
        let width = if location == Location::Top {
            width
        } else {
            let other = if left
                && self
                    .panels
                    .iter()
                    .any(|p| p.visible && p.location == Location::Right)
            {
                180.
            } else {
                0.
            };
            width.min((ui.available_width() - 320. - other).max(180.))
        };
        let result = panel.exact_size(width).resizable(false).show(ui, |ui| {
            let viewport = ui.available_rect_before_wrap();
            let dock_width =
                viewport.width() - ui.spacing().scroll.allocated_width();
            let stack = egui::ScrollArea::both()
                .id_salt(if left { "left_stack" } else { "right_stack" })
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let bounds = Rect::from_min_size(
                        ui.cursor().min,
                        egui::vec2(
                            ui.available_width().min(dock_width),
                            viewport.height(),
                        ),
                    );
                    let layout = self.dock_layout(location, bounds);
                    for (palette, rect) in &layout {
                        ui.scope_builder(
                            egui::UiBuilder::new()
                                .id_salt(*palette)
                                .max_rect(*rect),
                            |ui| {
                                ui.set_width(rect.width());
                                ui.set_clip_rect(
                                    ui.clip_rect().intersect(*rect),
                                );
                                self.shell(ui, app, *palette);
                            },
                        );
                        if self.dragging != Some(*palette) {
                            let visible = rect.intersect(viewport);
                            if visible.is_positive() {
                                self.slots.push((*palette, location, visible));
                            }
                        }
                    }
                    if location != Location::Top {
                        self.dock_resize(ui, location, &layout);
                    }
                    if let Some(total) =
                        layout.iter().map(|(_, r)| *r).reduce(|a, b| a.union(b))
                    {
                        ui.allocate_rect(total, egui::Sense::hover());
                    }
                });
            self.dock_bounds.push((
                location,
                Rect::from_min_size(
                    stack.inner_rect.min - stack.state.offset,
                    egui::vec2(
                        stack.inner_rect.width().min(dock_width),
                        stack.inner_rect.height(),
                    ),
                ),
            ));
            self.dock_clips.push((location, stack.inner_rect));
            #[cfg(test)]
            ui.data_mut(|data| {
                data.insert_temp(
                    egui::Id::new(("dock_scroll_geometry", left)),
                    (stack.inner_rect, stack.content_size),
                );
                data.insert_temp(
                    egui::Id::new(("dock_scroll_id", left)),
                    stack.id,
                )
            });
        });
        if location == Location::Top {
            return;
        }
        let boundary = if left {
            result.response.rect.right()
        } else {
            result.response.rect.left()
        };
        let target = Rect::from_min_max(
            egui::pos2(
                boundary - if left { 10. } else { 0. },
                result.response.rect.top(),
            ),
            egui::pos2(
                boundary + if left { 0. } else { 10. },
                result.response.rect.bottom(),
            ),
        );
        let label = if left { "Resize left dock" } else { "Resize right dock" };
        let next_width = resize::handle(
            ui,
            egui::Id::new(label),
            target,
            egui::CursorIcon::ResizeHorizontal,
            label,
            [width, 0., 0., 0.],
        )
        .map(|drag| {
            self.resizing = true;
            (drag.initial[0] + drag.delta.x * if left { 1. } else { -1. })
                .clamp(self.dock_minimum(location, dock_margin), 800.)
        })
        .unwrap_or(width);
        if left {
            self.left_width = next_width;
        } else {
            self.right_width = next_width;
        }
    }
    pub fn draw(&mut self, ui: &mut egui::Ui, app: &mut Parfait) {
        self.resizing = false;
        self.slots.clear();
        self.dock_bounds.clear();
        self.dock_clips.clear();
        #[cfg(test)]
        self.headers.clear();
        let ctx = ui.ctx().clone();
        if let Some(palette) = self.dragging {
            if ctx.input(|i| i.key_pressed(egui::Key::Escape) || !i.focused) {
                if let Some((index, original)) = self.drag_origin.take() {
                    self.panels.retain(|p| p.palette != palette);
                    self.panels.insert(index.min(self.panels.len()), original);
                }
                self.dragging = None;
            } else if let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) {
                let p = self
                    .panels
                    .iter_mut()
                    .find(|p| p.palette == palette)
                    .unwrap();
                p.rect[0] = pos.x - self.drag_offset.x;
                p.rect[1] = pos.y - self.drag_offset.y;
            }
        }
        if !app.hide_panels
            && self.panels.iter().any(|p| {
                p.palette == Palette::Tools
                    && p.visible
                    && p.location == Location::Rail
            })
        {
            egui::Panel::left("tool_rail")
                .exact_size(34.)
                .resizable(false)
                .frame(
                    egui::Frame::NONE
                        .fill(Color32::from_gray(47))
                        .inner_margin(2.),
                )
                .show(ui, |ui| self.shell(ui, app, Palette::Tools));
        }
        let dock_area = ui.available_rect_before_wrap();
        if !app.hide_panels {
            self.side(ui, app, Location::Top);
            self.side(ui, app, Location::Left);
            self.side(ui, app, Location::Right);
        }
        let floating: Vec<_> = self
            .panels
            .iter()
            .filter(|p| {
                !app.hide_panels
                    && p.visible
                    && p.location == Location::Floating
            })
            .cloned()
            .collect();
        for p in floating {
            let screen = ctx.content_rect();
            let pos = egui::pos2(
                p.rect[0].clamp(
                    screen.left(),
                    (screen.right() - p.rect[2]).max(screen.left()),
                ),
                p.rect[1].clamp(
                    screen.top(),
                    (screen.bottom() - 23.).max(screen.top()),
                ),
            );
            let response =
                egui::Area::new(egui::Id::new(("floating", p.palette)))
                    .order(egui::Order::Foreground)
                    .fixed_pos(pos)
                    .movable(false)
                    .interactable(self.dragging != Some(p.palette))
                    // Only the header is constrained above. Constraining the
                    // entire Area makes tall palettes jump while dragging.
                    .constrain(false)
                    .fade_in(false)
                    .default_size(egui::vec2(
                        p.rect[2],
                        if p.collapsed { 23. } else { p.height + 29. },
                    ))
                    .show(&ctx, |ui| {
                        let bounds = Rect::from_min_size(
                            pos,
                            egui::vec2(
                                p.rect[2],
                                if p.collapsed { 23. } else { p.height + 29. },
                            ),
                        );
                        ui.multiply_opacity(detached_opacity(
                            ui,
                            bounds,
                            p.inactive_opacity,
                            self.dragging == Some(p.palette),
                        ));
                        egui::Frame::NONE.fill(Color32::from_gray(57)).show(
                            ui,
                            |ui| {
                                ui.set_width(p.rect[2]);
                                self.shell(ui, app, p.palette);
                                self.floating_resize(
                                    ui,
                                    p.palette,
                                    Rect::from_min_size(
                                        ui.min_rect().min,
                                        egui::vec2(
                                            p.rect[2],
                                            if p.collapsed {
                                                23.
                                            } else {
                                                p.height + 29.
                                            },
                                        ),
                                    ),
                                );
                            },
                        );
                    });
            let item = self
                .panels
                .iter_mut()
                .find(|s| s.palette == p.palette)
                .unwrap();
            if !self.resizing {
                item.rect[0] = response.response.rect.left();
                item.rect[1] = response.response.rect.top();
            }
            item.rect[3] = (item.height + 29.).max(80.);
        }
        let windows: Vec<_> = self
            .panels
            .iter()
            .filter(|p| p.visible && p.location == Location::Window)
            .cloned()
            .collect();
        for p in windows {
            let mut builder = egui::ViewportBuilder::default()
                .with_visible(
                    app.screen_picker.as_ref().is_none_or(|p| !p.hidden),
                )
                .with_title(format!("Parfait · {}", p.palette.title()))
                .with_transparent(true)
                .with_decorations(false)
                .with_inner_size([
                    p.rect[2],
                    if p.collapsed { 23. } else { p.height + 29. },
                ])
                .with_min_inner_size([
                    p.palette.minimum_width(),
                    if p.collapsed { 23. } else { 89. },
                ])
                .with_max_inner_size([
                    800.,
                    if p.collapsed { 23. } else { 2029. },
                ])
                .with_resizable(true);
            if let Some(r) = p.window_rect {
                builder =
                    builder.with_position([r[0], r[1]]).with_inner_size([
                        r[2],
                        if p.collapsed { 23. } else { r[3] },
                    ]);
            }
            let enabled = ui.is_enabled();
            ctx.show_viewport_immediate(
                egui::ViewportId::from_hash_of(("palette_window", p.palette)),
                builder,
                |ui, class| {
                    if ui.input(|i| i.viewport().close_requested()) {
                        self.panels
                            .iter_mut()
                            .find(|s| s.palette == p.palette)
                            .unwrap()
                            .visible = false;
                        return;
                    }
                    if class != egui::ViewportClass::EmbeddedWindow
                        && let Some(r) = ui.input(|i| i.viewport().inner_rect)
                    {
                        let item = self
                            .panels
                            .iter_mut()
                            .find(|s| s.palette == p.palette)
                            .unwrap();
                        let expanded = if !item.collapsed && r.height() >= 89. {
                            r.height()
                        } else {
                            item.height + 29.
                        };
                        item.window_rect = Some([
                            r.left(),
                            r.top(),
                            r.width().max(p.palette.minimum_width()),
                            expanded,
                        ]);
                        item.rect[2] = r.width().max(p.palette.minimum_width());
                        if !item.collapsed && r.height() >= 89. {
                            item.height = (r.height() - 29.).clamp(60., 2000.);
                        }
                    }
                    ui.multiply_opacity(detached_opacity(
                        ui,
                        ui.ctx().content_rect(),
                        p.inactive_opacity,
                        false,
                    ));
                    egui::CentralPanel::default()
                        .frame(egui::Frame::NONE.fill(Color32::from_gray(57)))
                        .show(ui, |ui| {
                            ui.add_enabled_ui(enabled, |ui| {
                                self.shell(ui, app, p.palette)
                            })
                        });
                    if class != egui::ViewportClass::EmbeddedWindow {
                        crate::chrome::resize_edges(ui);
                    }
                },
            );
        }
        if let Some(palette) = self.dragging {
            if let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) {
                let target = self
                    .slots
                    .iter()
                    .copied()
                    .find(|(_, _, r)| r.contains(pos))
                    .or_else(|| {
                        self.slots
                            .iter()
                            .copied()
                            .filter(|(_, _, r)| r.expand(6.).contains(pos))
                            .min_by(|(_, _, a), (_, _, b)| {
                                a.distance_sq_to_pos(pos)
                                    .total_cmp(&b.distance_sq_to_pos(pos))
                            })
                    });
                let screen = ctx.content_rect();
                let edge = if pos.y < dock_area.top() + 45.
                    && pos.x > dock_area.left() + 100.
                    && pos.x < dock_area.right() - 100.
                {
                    Some(Location::Top)
                } else if pos.x < screen.left() + 100. {
                    Some(Location::Left)
                } else if pos.x > screen.right() - 100. {
                    Some(Location::Right)
                } else {
                    None
                };
                let direction =
                    target.map(|(_, _, rect)| DockDirection::at(rect, pos));
                let marker = target
                    .map(|target| self.insert_line(target, direction.unwrap()))
                    .or_else(|| {
                        edge.map(|side| match side {
                            Location::Top => insertion_bar(
                                dock_area.left_top(),
                                dock_area.right_top(),
                            ),
                            Location::Left => insertion_bar(
                                dock_area.left_top(),
                                dock_area.left_bottom(),
                            ),
                            _ => insertion_bar(
                                dock_area.right_top(),
                                dock_area.right_bottom(),
                            ),
                        })
                    });
                if let Some(marker) = marker {
                    let marker = marker.intersect(screen);
                    let paint = ctx.layer_painter(egui::LayerId::new(
                        egui::Order::Tooltip,
                        egui::Id::new("dock_feedback"),
                    ));
                    paint.rect_filled(
                        marker.expand(1.5).intersect(screen),
                        0.,
                        Color32::from_black_alpha(180),
                    );
                    paint.rect_filled(
                        marker,
                        0.,
                        Color32::from_rgb(230, 65, 75),
                    );
                    #[cfg(test)]
                    ctx.data_mut(|data| {
                        data.insert_temp(
                            egui::Id::new("dock_insert_line"),
                            marker,
                        )
                    });
                }
                if ctx.input(|i| i.pointer.any_released()) {
                    if let Some((other, _, _)) = target {
                        self.dock_to(palette, other, direction.unwrap());
                    } else if let Some(side) = edge {
                        self.relocate(palette, side, None, pos);
                    }
                    self.dragging = None;
                    self.drag_origin = None;
                }
            }
            if !ctx.input(|i| i.pointer.any_down()) {
                self.dragging = None;
                self.drag_origin = None;
            }
        }
    }
}

impl Parfait {
    pub fn workspace_ui(&mut self, ui: &mut egui::Ui) {
        self.update_previews(ui.ctx());
        let mut workspace = std::mem::take(&mut self.workspace);
        workspace.draw(ui, self);
        self.workspace = workspace;
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn detached_surface_alpha_follows_mouse_instead_of_keyboard_focus() {
        for (focused, position, dragging, expected) in [
            (true, Some(egui::pos2(20., 20.)), false, 0.35),
            (false, Some(egui::pos2(100., 100.)), false, 1.),
            (true, Some(egui::pos2(100., 100.)), false, 1.),
            (true, None, false, 0.35),
            (true, Some(egui::pos2(100., 100.)), true, 0.5),
        ] {
            let ctx = egui::Context::default();
            let bounds = Rect::from_min_size(
                egui::pos2(50., 50.),
                egui::vec2(200., 200.),
            );
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        Pos2::ZERO,
                        egui::vec2(800., 600.),
                    )),
                    focused,
                    events: vec![position.map_or(
                        egui::Event::PointerGone,
                        egui::Event::PointerMoved,
                    )],
                    ..Default::default()
                },
                |ui| {
                    ui.multiply_opacity(detached_opacity(
                        ui, bounds, 0.35, dragging,
                    ));
                    egui::Frame::NONE.fill(Color32::from_gray(57)).show(
                        ui,
                        |ui| {
                            ui.set_min_size(egui::vec2(200., 200.));
                        },
                    );
                },
            );
            let alpha = output
                .shapes
                .iter()
                .find_map(|s| match &s.shape {
                    egui::Shape::Rect(r) if r.fill != Color32::TRANSPARENT => {
                        Some(r.fill.a())
                    }
                    _ => None,
                })
                .expect("palette background is painted");
            output.textures_delta.clear();
            assert!(
                (f32::from(alpha) - expected * 255.).abs() <= 1.,
                "focus={focused} pos={position:?} alpha={alpha}"
            );
        }
    }

    #[test]
    fn insertion_markers_span_the_correct_row_boundary() {
        let mut workspace = Workspace::default();
        workspace.dock_to(
            Palette::Properties,
            Palette::Brushes,
            DockDirection::Right,
        );
        let a =
            Rect::from_min_size(egui::pos2(40., 100.), egui::vec2(180., 200.));
        let b = a.translate(egui::vec2(180., 0.));
        workspace.slots = vec![
            (Palette::Brushes, Location::Left, a),
            (Palette::Properties, Location::Left, b),
        ];
        let target = workspace.slots[0];
        let vertical = workspace.insert_line(target, DockDirection::Right);
        assert_eq!(vertical.size(), egui::vec2(3., 200.));
        assert_eq!(vertical.center().x, a.right());
        let horizontal = workspace.insert_line(target, DockDirection::Bottom);
        assert_eq!(horizontal.size(), egui::vec2(360., 3.));
        assert_eq!(horizontal.center().y, a.bottom());
        assert!(matches!(
            DockDirection::at(a, egui::pos2(a.left() + 40., a.top() + 8.)),
            DockDirection::Top
        ));
    }

    #[test]
    fn preference_comparison_covers_geometry_but_ignores_runtime_caches() {
        let original = super::Workspace::default();
        let mut runtime = original.clone();
        runtime.panels[0].scroll_offset = 40.;
        runtime.dragging = Some(super::Palette::Brushes);
        runtime.resizing = true;
        runtime.slots.push((
            super::Palette::Brushes,
            super::Location::Left,
            eframe::egui::Rect::EVERYTHING,
        ));
        runtime.drag_origin = Some((0, runtime.panels[0].clone()));
        assert!(original.same_preferences(&runtime));
        assert_eq!(
            serde_json::to_string(&original).unwrap(),
            serde_json::to_string(&runtime).unwrap()
        );
        let changes: [fn(&mut super::Workspace); 11] = [
            |workspace| workspace.right_width += 10.,
            |workspace| workspace.panels.swap(0, 1),
            |workspace| workspace.panels.pop().map(|_| ()).unwrap(),
            |workspace| workspace.panels[0].palette = super::Palette::Color,
            |workspace| workspace.panels[0].location = super::Location::Window,
            |workspace| workspace.panels[0].visible = false,
            |workspace| workspace.panels[0].row = Some(super::Palette::Color),
            |workspace| {
                workspace.panels[0].window_rect = Some([1., 2., 250., 300.])
            },
            |workspace| workspace.panels[0].height += 1.,
            |workspace| workspace.panels[0].width_weight += 0.5,
            |workspace| workspace.panels[0].rect[0] += 1.,
        ];
        for change in changes {
            let mut changed = original.clone();
            change(&mut changed);
            assert!(!original.same_preferences(&changed));
            assert_ne!(
                serde_json::to_string(&original).unwrap(),
                serde_json::to_string(&changed).unwrap()
            );
        }
    }

    use super::*;
    #[test]
    fn solid_dock_scrollbars_do_not_clip_single_column_palette_actions() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open_legacy(dir.path().to_path_buf()).unwrap();
        let artwork = app.documents.active_session.document.clone();
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        for width in [196., 210., 260., 360.] {
            app.workspace.left_width = width;
            for frame_index in 0..30 {
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(
                            Pos2::ZERO,
                            egui::vec2(960., 640.),
                        )),
                        time: Some(width as f64 + frame_index as f64 / 30.),
                        ..Default::default()
                    },
                    |ui| app.workspace_ui(ui),
                );
                output.textures_delta.clear();
            }
            let (viewport, content) = ctx
                .data(|data| {
                    data.get_temp::<(Rect, egui::Vec2)>(egui::Id::new((
                        "dock_scroll_geometry",
                        true,
                    )))
                })
                .unwrap();
            assert!(
                content.x <= viewport.width() + 0.5,
                "width={width}: viewport={viewport:?}, content={content:?}"
            );
            let details = ctx
                .data(|data| {
                    data.get_temp::<Rect>(egui::Id::new((
                        "flat_icon",
                        "보조 도구 상세",
                    )))
                })
                .unwrap();
            assert!(details.right() <= viewport.right());
            assert_eq!(app.documents.active_session.document, artwork);
        }
    }

    #[test]
    fn batched_tool_rail_drag_keeps_release_position_without_changing_artwork()
    {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open_legacy(dir.path().to_owned()).unwrap();
        let before = app.documents.active_session.document.clone();
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let frame = |app: &mut Parfait, events| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        Pos2::ZERO,
                        egui::vec2(1280., 900.),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| app.workspace_ui(ui),
            );
            output.textures_delta.clear();
        };
        frame(&mut app, vec![]);
        frame(&mut app, vec![]);
        let start = app
            .workspace
            .headers
            .iter()
            .find(|(p, _)| *p == Palette::Tools)
            .unwrap()
            .1
            .center();
        let end = egui::pos2(650., 330.);
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            pressed,
            button: egui::PointerButton::Primary,
            modifiers: Default::default(),
        };
        frame(
            &mut app,
            vec![
                egui::Event::PointerMoved(start),
                button(start, true),
                egui::Event::PointerMoved(end),
                button(end, false),
            ],
        );
        frame(&mut app, vec![]);
        let tools = app
            .workspace
            .panels
            .iter()
            .find(|p| p.palette == Palette::Tools)
            .unwrap();
        assert_eq!(tools.location, Location::Floating);
        assert!(tools.rect[0] > 600. && tools.rect[0] < end.x);
        assert!(tools.rect[1] > 300. && tools.rect[1] < end.y);
        assert!(app.workspace.dragging.is_none());
        assert_eq!(app.documents.active_session.document, before);
    }

    #[test]
    fn header_context_menu_opens_a_separate_window() {
        for palette in [Palette::Brushes, Palette::Tools] {
            let dir = tempfile::tempdir().unwrap();
            let mut app =
                Parfait::open_legacy(dir.path().to_path_buf()).unwrap();
            let ctx = egui::Context::default();
            crate::theme::apply(&ctx);
            let frame = |app: &mut Parfait, events| {
                let mut out = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(
                            Pos2::ZERO,
                            egui::vec2(1280., 900.),
                        )),
                        events,
                        ..Default::default()
                    },
                    |ui| app.workspace_ui(ui),
                );
                out.textures_delta.clear();
            };
            frame(&mut app, vec![]);
            let p = app
                .workspace
                .headers
                .iter()
                .find(|(p, _)| *p == palette)
                .unwrap()
                .1
                .center();
            let button = |pos, pressed, button| egui::Event::PointerButton {
                pos,
                pressed,
                button,
                modifiers: Default::default(),
            };
            frame(
                &mut app,
                vec![
                    egui::Event::PointerMoved(p),
                    button(p, true, egui::PointerButton::Secondary),
                ],
            );
            frame(
                &mut app,
                vec![button(p, false, egui::PointerButton::Secondary)],
            );
            frame(&mut app, vec![]);
            let rect = ctx
                .data(|d| {
                    d.get_temp::<Rect>(egui::Id::new((
                        "native_command",
                        palette,
                    )))
                })
                .expect("right click should open native window menu");
            frame(
                &mut app,
                vec![
                    egui::Event::PointerMoved(rect.center()),
                    button(rect.center(), true, egui::PointerButton::Primary),
                ],
            );
            frame(
                &mut app,
                vec![button(
                    rect.center(),
                    false,
                    egui::PointerButton::Primary,
                )],
            );
            assert_eq!(
                app.workspace
                    .panels
                    .iter()
                    .find(|p| p.palette == palette)
                    .unwrap()
                    .location,
                Location::Window
            );
            assert!(app.workspace.dragging.is_none());
        }
    }
    #[test]
    fn side_docking_preview_matches_rendered_slot_and_window_state_roundtrips()
    {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open_legacy(dir.path().to_path_buf()).unwrap();
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let frame = |app: &mut Parfait, events| {
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        Pos2::ZERO,
                        egui::vec2(1440., 1000.),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| app.workspace_ui(ui),
            );
            out.textures_delta.clear();
        };
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            pressed,
            button: egui::PointerButton::Primary,
            modifiers: Default::default(),
        };
        frame(&mut app, vec![]);
        let from = app
            .workspace
            .headers
            .iter()
            .find(|(p, _)| *p == Palette::Brushes)
            .unwrap()
            .1
            .center();
        frame(
            &mut app,
            vec![egui::Event::PointerMoved(from), button(from, true)],
        );
        frame(
            &mut app,
            vec![egui::Event::PointerMoved(from + egui::vec2(20., 0.))],
        );
        frame(
            &mut app,
            vec![egui::Event::PointerMoved(egui::pos2(650., 300.))],
        );
        let target = app
            .workspace
            .slots
            .iter()
            .find(|(p, _, _)| *p == Palette::Properties)
            .unwrap()
            .2;
        let to = target.right_center() - egui::vec2(8., 0.);
        let bounds = app
            .workspace
            .dock_bounds
            .iter()
            .find(|(s, _)| *s == Location::Left)
            .unwrap()
            .1;
        let mut preview = app.workspace.clone();
        preview.dock_to(
            Palette::Brushes,
            Palette::Properties,
            DockDirection::Right,
        );
        let predicted = preview
            .dock_layout(Location::Left, bounds)
            .into_iter()
            .find(|(p, _)| *p == Palette::Brushes)
            .unwrap()
            .1;
        frame(&mut app, vec![egui::Event::PointerMoved(to)]);
        frame(&mut app, vec![button(to, false)]);
        frame(&mut app, vec![]);
        let actual = app
            .workspace
            .slots
            .iter()
            .find(|(p, _, _)| *p == Palette::Brushes)
            .unwrap()
            .2;
        assert!(
            (actual.min - predicted.min).length() < 3.,
            "{actual:?} != {predicted:?}"
        );
        assert!((actual.size() - predicted.size()).length() < 3.);
        let item = app
            .workspace
            .panels
            .iter_mut()
            .find(|p| p.palette == Palette::Brushes)
            .unwrap();
        item.location = Location::Window;
        item.window_rect = Some([100., 120., 260., 300.]);
        app.persist_preferences(true);
        drop(app);
        let restored = Parfait::open_legacy(dir.path().to_path_buf()).unwrap();
        let item = restored
            .workspace
            .panels
            .iter()
            .find(|p| p.palette == Palette::Brushes)
            .unwrap();
        assert_eq!(item.location, Location::Window);
        assert_eq!(item.window_rect, Some([100., 120., 260., 300.]));
    }
    #[test]
    fn moving_a_row_owner_does_not_merge_unrelated_rows() {
        let mut w = Workspace::default();
        w.dock_to(Palette::Properties, Palette::Brushes, DockDirection::Right);
        w.dock_to(Palette::Brushes, Palette::Color, DockDirection::Top);
        let rows = w.dock_rows(Location::Left);
        assert!(rows.iter().any(|r| r == &vec![Palette::Properties]));
        assert!(rows.iter().any(|r| r == &vec![Palette::Color]));
        assert!(rows.iter().any(|r| r == &vec![Palette::Brushes]));
    }
    #[test]
    fn escape_restores_original_dock_order_and_geometry() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open_legacy(dir.path().to_path_buf()).unwrap();
        let original = app.workspace.panels[0].clone();
        app.workspace.drag_origin = Some((0, original.clone()));
        app.workspace.dragging = Some(original.palette);
        app.workspace.relocate(
            original.palette,
            Location::Floating,
            None,
            egui::pos2(500., 200.),
        );
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let mut out = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(
                    Pos2::ZERO,
                    egui::vec2(1280., 900.),
                )),
                events: vec![egui::Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Default::default(),
                }],
                ..Default::default()
            },
            |ui| app.workspace_ui(ui),
        );
        out.textures_delta.clear();
        assert!(app.workspace.dragging.is_none());
        assert_eq!(app.workspace.panels[0].palette, original.palette);
        assert_eq!(app.workspace.panels[0].location, original.location);
        assert_eq!(app.workspace.panels[0].rect, original.rect);
        assert_eq!(app.workspace.panels[0].height, original.height);
    }
    #[test]
    fn shared_header_collapses_hides_and_persists_without_losing_size() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open_legacy(dir.path().to_path_buf()).unwrap();
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let frame = |app: &mut Parfait, events| {
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        Pos2::ZERO,
                        egui::vec2(1280., 900.),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| app.workspace_ui(ui),
            );
            out.textures_delta.clear();
        };
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            pressed,
            button: egui::PointerButton::Primary,
            modifiers: Default::default(),
        };
        for location in [Location::Left, Location::Floating] {
            {
                let p = &mut app.workspace.panels[0];
                p.location = location;
                p.visible = true;
                p.collapsed = false;
            }
            frame(&mut app, vec![]);
            frame(&mut app, vec![]);
            let r = app
                .workspace
                .headers
                .iter()
                .find(|(p, _)| *p == Palette::Brushes)
                .unwrap()
                .1;
            let pos = r.right_center() + egui::vec2(10.5, 0.);
            frame(
                &mut app,
                vec![egui::Event::PointerMoved(pos), button(pos, true)],
            );
            frame(&mut app, vec![button(pos, false)]);
            assert!(app.workspace.panels[0].collapsed);
            assert_eq!(
                app.workspace.panels[0].height,
                Workspace::default().panels[0].height
            );
            assert!(app.workspace.dragging.is_none());
            let pos = r.right_center() + egui::vec2(31.5, 0.);
            frame(
                &mut app,
                vec![egui::Event::PointerMoved(pos), button(pos, true)],
            );
            frame(&mut app, vec![button(pos, false)]);
            assert!(!app.workspace.panels[0].visible);
        }
        app.persist_preferences(true);
        drop(app);
        let restored = Parfait::open_legacy(dir.path().to_path_buf()).unwrap();
        assert!(restored.workspace.panels[0].collapsed);
        assert!(!restored.workspace.panels[0].visible);
        assert_eq!(
            restored.workspace.panels[0].height,
            Workspace::default().panels[0].height
        );
        // Existing preferences without collapse state still decode as expanded.
        let mut json = serde_json::to_value(&restored.workspace).unwrap();
        for panel in json["panels"].as_array_mut().unwrap() {
            panel.as_object_mut().unwrap().remove("collapsed");
        }
        let old: Workspace = serde_json::from_value(json).unwrap();
        assert!(old.validate());
        assert!(old.panels.iter().all(|p| !p.collapsed));
    }
    #[test]
    fn drag_detaches_and_preferences_restore_workspace_and_brushes() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open_legacy(dir.path().to_path_buf()).unwrap();
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let frame = |app: &mut Parfait, events| {
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        Pos2::ZERO,
                        egui::vec2(1280., 900.),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| app.workspace_ui(ui),
            );
            out.textures_delta.clear();
        };
        frame(&mut app, vec![]);
        let r = app
            .workspace
            .slots
            .iter()
            .find(|s| s.0 == Palette::Brushes)
            .unwrap()
            .2;
        let start = r.min + egui::vec2(45., 11.);
        let end = egui::pos2(600., 300.);
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            pressed,
            button: egui::PointerButton::Primary,
            modifiers: Default::default(),
        };
        frame(&mut app, vec![egui::Event::PointerMoved(start)]);
        frame(&mut app, vec![button(start, true)]);
        frame(
            &mut app,
            vec![egui::Event::PointerMoved(start + egui::vec2(15., 0.))],
        );
        frame(&mut app, vec![egui::Event::PointerMoved(end)]);
        let detached = app
            .workspace
            .panels
            .iter()
            .find(|p| p.palette == Palette::Brushes)
            .unwrap();
        assert_eq!(detached.location, Location::Floating);
        assert_eq!(detached.height, Workspace::default().panels[0].height);
        assert!((detached.rect[2] - r.width()).abs() < 2.);
        assert!(app.workspace.dragging.is_some());
        frame(&mut app, vec![button(end, false)]);
        assert_eq!(
            app.workspace
                .panels
                .iter()
                .find(|p| p.palette == Palette::Brushes)
                .unwrap()
                .location,
            Location::Floating
        );
        frame(&mut app, vec![]);
        let from = app
            .workspace
            .headers
            .iter()
            .find(|v| v.0 == Palette::Brushes)
            .unwrap()
            .1
            .center();
        let to = app
            .workspace
            .headers
            .iter()
            .find(|v| v.0 == Palette::Navigator)
            .unwrap()
            .1
            .center();
        frame(&mut app, vec![egui::Event::PointerMoved(from)]);
        frame(&mut app, vec![button(from, true)]);
        frame(
            &mut app,
            vec![egui::Event::PointerMoved(from + egui::vec2(15., 0.))],
        );
        frame(&mut app, vec![egui::Event::PointerMoved(to)]);
        frame(&mut app, vec![button(to, false)]);
        assert_eq!(
            app.workspace
                .panels
                .iter()
                .find(|p| p.palette == Palette::Brushes)
                .unwrap()
                .location,
            Location::Right
        );
        frame(&mut app, vec![]);
        let grip = app
            .workspace
            .slots
            .iter()
            .find(|v| v.0 == Palette::Brushes)
            .unwrap()
            .2
            .center_bottom()
            - egui::vec2(0., 3.);
        let old_height = app
            .workspace
            .panels
            .iter()
            .find(|p| p.palette == Palette::Brushes)
            .unwrap()
            .height;
        frame(&mut app, vec![egui::Event::PointerMoved(grip)]);
        frame(&mut app, vec![button(grip, true)]);
        frame(
            &mut app,
            vec![egui::Event::PointerMoved(grip + egui::vec2(0., 35.))],
        );
        frame(&mut app, vec![button(grip + egui::vec2(0., 35.), false)]);
        let new_height = app
            .workspace
            .panels
            .iter()
            .find(|p| p.palette == Palette::Brushes)
            .unwrap()
            .height;
        assert!(new_height > old_height);
        app.brush.pressure_curve = [0., 0.1, 0.3, 0.6, 1.].into();
        app.presets.push(crate::brushes::Preset {
            name: "My imported tip".into(),
            favorite: false,
            properties: Default::default(),
            brush: parfait_core::Brush {
                tip: parfait_core::BrushTip::Image {
                    width: 2,
                    height: 2,
                    mask: std::sync::Arc::new(vec![0, 128, 255, 64]),
                },
                ..Default::default()
            },
        });
        app.persist_preferences(true);
        assert!(app.error.is_none(), "{:?}", app.error);
        drop(app);
        let restored = Parfait::open_legacy(dir.path().to_path_buf()).unwrap();
        assert_eq!(
            restored
                .workspace
                .panels
                .iter()
                .find(|p| p.palette == Palette::Brushes)
                .unwrap()
                .location,
            Location::Right
        );
        assert_eq!(
            restored
                .workspace
                .panels
                .iter()
                .find(|p| p.palette == Palette::Brushes)
                .unwrap()
                .height,
            new_height
        );
        assert_eq!(
            restored.brush.pressure_curve,
            parfait_core::ResponseCurve::from([0., 0.1, 0.3, 0.6, 1.])
        );
        assert_eq!(restored.presets.last().unwrap().name, "My imported tip");
        assert!(
            matches!(&restored.presets.last().unwrap().brush.tip,parfait_core::BrushTip::Image{mask,..} if mask.as_slice()==[0,128,255,64])
        );
    }
    #[test]
    fn docking_feedback_tracks_both_sides_and_scrolled_content() {
        for side in [Location::Left, Location::Right] {
            for scrolled in [false, true] {
                let dir = tempfile::tempdir().unwrap();
                let mut app =
                    Parfait::open_legacy(dir.path().to_owned()).unwrap();
                for p in &mut app.workspace.panels {
                    p.location = side;
                    p.height = 280.;
                }
                let ctx = egui::Context::default();
                crate::theme::apply(&ctx);
                let run = |app: &mut Parfait, events| {
                    let mut out = ctx.run_ui(
                        egui::RawInput {
                            screen_rect: Some(Rect::from_min_size(
                                Pos2::ZERO,
                                egui::vec2(1600., 1000.),
                            )),
                            events,
                            ..Default::default()
                        },
                        |ui| app.workspace_ui(ui),
                    );
                    out.textures_delta.clear();
                };
                run(&mut app, vec![]);
                run(&mut app, vec![]);
                let start = app
                    .workspace
                    .headers
                    .iter()
                    .find(|(p, _)| *p == Palette::Brushes)
                    .unwrap()
                    .1
                    .center();
                let button = |pos, pressed| egui::Event::PointerButton {
                    pos,
                    pressed,
                    button: egui::PointerButton::Primary,
                    modifiers: Default::default(),
                };
                run(
                    &mut app,
                    vec![egui::Event::PointerMoved(start), button(start, true)],
                );
                run(
                    &mut app,
                    vec![egui::Event::PointerMoved(
                        start + egui::vec2(20., 0.),
                    )],
                );
                run(
                    &mut app,
                    vec![egui::Event::PointerMoved(egui::pos2(800., 300.))],
                );
                if scrolled {
                    // Read the actual stack ID from the rendered ScrollArea in side().
                    let actual_id = ctx
                        .data(|d| {
                            d.get_temp::<egui::Id>(egui::Id::new((
                                "dock_scroll_id",
                                side == Location::Left,
                            )))
                        })
                        .unwrap();
                    let mut state =
                        egui::scroll_area::State::load(&ctx, actual_id)
                            .unwrap();
                    state.offset.y = 140.;
                    state.store(&ctx, actual_id);
                    run(&mut app, vec![]);
                }
                let target = app
                    .workspace
                    .slots
                    .iter()
                    .find(|(p, _, _)| *p == Palette::Properties)
                    .unwrap()
                    .2;
                let to = target.right_center() - egui::vec2(4., 0.);
                run(&mut app, vec![egui::Event::PointerMoved(to)]);
                let target = app
                    .workspace
                    .slots
                    .iter()
                    .find(|(p, _, _)| *p == Palette::Properties)
                    .unwrap()
                    .2
                    .intersect(ctx.content_rect());
                let preview = ctx
                    .data(|d| {
                        d.get_temp::<Rect>(egui::Id::new("dock_insert_line"))
                    })
                    .unwrap();
                run(&mut app, vec![button(to, false)]);
                run(&mut app, vec![]);
                let actual = app
                    .workspace
                    .slots
                    .iter()
                    .find(|(p, _, _)| *p == Palette::Brushes)
                    .unwrap()
                    .2;
                assert!((preview.width() - 3.).abs() < 0.1);
                assert!((preview.center().x - target.right()).abs() < 0.1);
                assert!((preview.top() - target.top()).abs() < 0.1);
                assert!((preview.bottom() - target.bottom()).abs() < 0.1);
                assert!(actual.is_positive());
                let inserted = app
                    .workspace
                    .panels
                    .iter()
                    .find(|p| p.palette == Palette::Brushes)
                    .unwrap();
                let neighbor = app
                    .workspace
                    .panels
                    .iter()
                    .find(|p| p.palette == Palette::Properties)
                    .unwrap();
                assert_eq!(inserted.location, side);
                assert_eq!(
                    inserted.row,
                    Some(neighbor.row.unwrap_or(neighbor.palette))
                );
            }
        }
    }
    #[test]
    fn tall_floating_palette_keeps_grabbed_header_position() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open_legacy(dir.path().to_owned()).unwrap();
        for p in &mut app.workspace.panels {
            p.visible = p.palette == Palette::Brushes;
            if p.visible {
                p.location = Location::Floating;
                p.height = 1200.;
                p.rect = [150., 200., 260., 1229.];
            }
        }
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let run = |app: &mut Parfait, events| {
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        Pos2::ZERO,
                        egui::vec2(960., 640.),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| app.workspace_ui(ui),
            );
            out.textures_delta.clear();
        };
        run(&mut app, vec![]);
        run(&mut app, vec![]);
        let header = app.workspace.headers[0].1;
        assert!((header.top() - 200.).abs() < 2., "{header:?}");
        let from = header.min + egui::vec2(30., 11.);
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            pressed,
            button: egui::PointerButton::Primary,
            modifiers: Default::default(),
        };
        run(
            &mut app,
            vec![egui::Event::PointerMoved(from), button(from, true)],
        );
        run(
            &mut app,
            vec![egui::Event::PointerMoved(from + egui::vec2(10., 0.))],
        );
        let to = egui::pos2(550., 400.);
        run(&mut app, vec![egui::Event::PointerMoved(to)]);
        let header = app.workspace.headers[0].1;
        assert!(
            (header.min - (to - egui::vec2(30., 11.))).length() < 2.,
            "{header:?}"
        );
        run(&mut app, vec![button(to, false)]);
        let item = app
            .workspace
            .panels
            .iter()
            .find(|p| p.palette == Palette::Brushes)
            .unwrap();
        assert_eq!(item.height, 1200.);
        assert_eq!(item.location, Location::Floating);
    }
}
