//! Persistent dock rows, in-app floating palettes and independent native palette windows.
use crate::app::Parfait;
use eframe::egui::{self, Color32, Pos2, Rect};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Palette {
    Brushes,
    Properties,
    Sizes,
    Color,
    Navigator,
    Layers,
}
impl Palette {
    pub const ALL: [Self; 6] = [
        Self::Brushes,
        Self::Properties,
        Self::Sizes,
        Self::Color,
        Self::Navigator,
        Self::Layers,
    ];
    pub fn title(self) -> &'static str {
        match self {
            Self::Brushes => "Sub tools",
            Self::Properties => "Tool property / Pressure",
            Self::Sizes => "Brush size",
            Self::Color => "Color wheel",
            Self::Navigator => "Navigator",
            Self::Layers => "Layers",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Location {
    Left,
    Right,
    Floating,
    Window,
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
        let x = (pos.x - rect.left()) / rect.width();
        if x < 0.25 {
            Self::Left
        } else if x > 0.75 {
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
    #[serde(skip)]
    scroll_offset: f32,
    pub height: f32,
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
    slots: Vec<(Palette, Location, Rect)>,
    #[serde(skip)]
    drag_offset: egui::Vec2,
    #[serde(skip)]
    dock_bounds: Vec<(Location, Rect)>,
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
                    location: if i < 4 {
                        Location::Left
                    } else {
                        Location::Right
                    },
                    visible: true,
                    collapsed: false,
                    row: None,
                    window_rect: None,
                    scroll_offset: 0.,
                    height: [185., 145., 115., 205., 240., 400.][i],
                    rect: [300. + i as f32 * 24., 120. + i as f32 * 24., 270., 340.],
                })
                .collect(),
            left_width: 230.,
            right_width: 250.,
            dragging: None,
            slots: Vec::new(),
            drag_offset: egui::Vec2::ZERO,
            dock_bounds: Vec::new(),
            drag_origin: None,
            #[cfg(test)]
            headers: Vec::new(),
        }
    }
}
impl Workspace {
    pub fn validate(&self) -> bool {
        self.panels.len() == 6
            && Palette::ALL
                .iter()
                .all(|p| self.panels.iter().filter(|s| s.palette == *p).count() == 1)
            && [self.left_width, self.right_width]
                .iter()
                .all(|v| v.is_finite() && (180. ..=800.).contains(v))
            && self.panels.iter().all(|s| {
                s.window_rect
                    .is_none_or(|r| r.iter().all(|v| v.is_finite()) && r[2] >= 180. && r[3] >= 23.)
                    && s.height.is_finite()
                    && (60. ..=2000.).contains(&s.height)
                    && s.rect.iter().all(|v| v.is_finite())
                    && s.rect[2] >= 180.
                    && s.rect[3] >= 80.
            })
    }
    pub fn relocate(
        &mut self,
        palette: Palette,
        location: Location,
        before: Option<Palette>,
        pos: Pos2,
    ) {
        let i = self
            .panels
            .iter()
            .position(|s| s.palette == palette)
            .unwrap();
        let mut item = self.panels.remove(i);
        self.reserve_row_identity(palette);
        item.location = location;
        item.row = before.and_then(|p| {
            self.panels
                .iter()
                .find(|s| s.palette == p)
                .map(|s| s.row.unwrap_or(s.palette))
        });
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
        for p in self
            .panels
            .iter()
            .filter(|p| p.visible && p.location == side)
        {
            let key = p.row.unwrap_or(p.palette);
            if let Some((_, row)) = rows.iter_mut().find(|(id, _)| *id == key) {
                row.push(p.palette);
            } else {
                rows.push((key, vec![p.palette]));
            }
        }
        rows.into_iter().map(|(_, row)| row).collect()
    }
    /// The same layout calculation drives both the rendered slots and drop preview.
    fn dock_layout(&self, side: Location, bounds: Rect) -> Vec<(Palette, Rect)> {
        let rows = self.dock_rows(side);
        let width = bounds
            .width()
            .max(rows.iter().map(|r| r.len()).max().unwrap_or(1) as f32 * 180.);
        let mut y = bounds.top();
        let mut result = Vec::new();
        for row in rows {
            let cell_width = width / row.len() as f32;
            let mut row_height: f32 = 23.;
            for (column, palette) in row.iter().enumerate() {
                let p = self.panels.iter().find(|p| p.palette == *palette).unwrap();
                let h = if p.collapsed { 23. } else { p.height + 29. };
                row_height = row_height.max(h);
                result.push((
                    *palette,
                    Rect::from_min_size(
                        egui::pos2(bounds.left() + column as f32 * cell_width, y),
                        egui::vec2(cell_width, h),
                    ),
                ));
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
                .find(|p| p.palette != palette && p.location == side && p.row == Some(palette))
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
    fn dock_to(&mut self, palette: Palette, other: Palette, direction: DockDirection) {
        if palette == other {
            return;
        }
        let mut item = self.panels.remove(
            self.panels
                .iter()
                .position(|p| p.palette == palette)
                .unwrap(),
        );
        self.reserve_row_identity(palette);
        let target = self
            .panels
            .iter()
            .find(|p| p.palette == other)
            .unwrap()
            .clone();
        let group = target.row.unwrap_or(other);
        item.location = target.location;
        if matches!(direction, DockDirection::Left | DockDirection::Right) {
            // Give all existing members the same explicit row identity.
            for p in &mut self.panels {
                if p.location == target.location && p.row.unwrap_or(p.palette) == group {
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
            .filter(|(_, p)| p.location == target.location && p.row.unwrap_or(p.palette) == group)
            .map(|(i, _)| i)
            .collect();
        let target_index = self.panels.iter().position(|p| p.palette == other).unwrap();
        let index = match direction {
            DockDirection::Left => target_index,
            DockDirection::Right => target_index + 1,
            DockDirection::Top => members.first().copied().unwrap_or(target_index),
            DockDirection::Bottom => members.last().copied().unwrap_or(target_index) + 1,
        };
        self.panels.insert(index, item);
    }
    fn header(&mut self, ui: &mut egui::Ui, palette: Palette) {
        let (rect, _) =
            ui.allocate_exact_size(egui::vec2(ui.available_width(), 23.), egui::Sense::hover());
        let drag_rect = Rect::from_min_max(rect.min, rect.max - egui::vec2(42., 0.));
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
        ui.painter().with_clip_rect(drag_rect).text(
            rect.left_center() + egui::vec2(6., 0.),
            egui::Align2::LEFT_CENTER,
            format!("≡  {}", palette.title()),
            egui::FontId::proportional(11.),
            Color32::LIGHT_GRAY,
        );
        let i = self
            .panels
            .iter()
            .position(|p| p.palette == palette)
            .unwrap();
        let collapse =
            Rect::from_min_size(rect.right_top() - egui::vec2(42., 0.), egui::vec2(21., 23.));
        let close =
            Rect::from_min_size(rect.right_top() - egui::vec2(21., 0.), egui::vec2(21., 23.));
        if ui
            .put(
                collapse,
                egui::Button::new(if self.panels[i].collapsed { "+" } else { "−" }).frame(false),
            )
            .on_hover_text("Collapse / expand palette")
            .clicked()
        {
            self.panels[i].collapsed = !self.panels[i].collapsed;
        }
        if ui
            .put(close, egui::Button::new("×").frame(false))
            .on_hover_text("Hide palette · reopen from View → Palettes")
            .clicked()
        {
            self.panels[i].visible = false;
        }
        response.context_menu(|ui| {
            let spawn = ui.button("Open in separate window");
            #[cfg(test)]
            ui.ctx().data_mut(|d| {
                d.insert_temp(egui::Id::new(("native_command", palette)), spawn.rect)
            });
            if spawn.clicked() {
                self.panels[i].location = Location::Window;
                ui.close();
            }
            if ui.button("Float inside main window").clicked() {
                self.panels[i].location = Location::Floating;
                ui.close();
            }
            if ui.button("Dock on left").clicked() {
                self.reserve_row_identity(palette);
                self.panels[i].location = Location::Left;
                self.panels[i].row = None;
                ui.close();
            }
            if ui.button("Dock on right").clicked() {
                self.reserve_row_identity(palette);
                self.panels[i].location = Location::Right;
                self.panels[i].row = None;
                ui.close();
            }
        });
        if response.drag_started() && self.panels[i].location == Location::Window {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
        } else if response.drag_started() && self.dragging.is_none() {
            self.drag_origin = Some((i, self.panels[i].clone()));
            self.drag_offset = ui
                .input(|input| input.pointer.press_origin())
                .unwrap_or(rect.min)
                - rect.min;
            let item = &mut self.panels[i];
            item.location = Location::Floating;
            item.rect[0] = rect.left();
            item.rect[1] = rect.top();
            item.rect[2] = rect.width().max(180.);
            self.dragging = Some(palette);
        }
        response
            .on_hover_cursor(egui::CursorIcon::Grab)
            .on_hover_text("Drag to move or dock. Right-click for separate window / dock commands. Escape cancels a move.");
    }
    /// Identical header, content and resize grip in both docked and floating hosts.
    fn shell(&mut self, ui: &mut egui::Ui, app: &mut Parfait, palette: Palette) {
        ui.spacing_mut().item_spacing.y = 0.;
        self.header(ui, palette);
        let p = self
            .panels
            .iter()
            .find(|p| p.palette == palette)
            .unwrap()
            .clone();
        if p.collapsed || !p.visible {
            return;
        }
        let body = egui::ScrollArea::vertical()
            .id_salt(("palette_body", palette))
            .vertical_scroll_offset(p.scroll_offset)
            .max_height(p.height)
            .min_scrolled_height(p.height)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 3.;
                app.palette(ui, palette);
            });
        self.panels
            .iter_mut()
            .find(|p| p.palette == palette)
            .unwrap()
            .scroll_offset = body.state.offset.y;
        let (grip, response) =
            ui.allocate_exact_size(egui::vec2(ui.available_width(), 6.), egui::Sense::drag());
        ui.painter().line_segment(
            [
                grip.center() - egui::vec2(18., 0.),
                grip.center() + egui::vec2(18., 0.),
            ],
            egui::Stroke::new(1., Color32::GRAY),
        );
        if response.dragged() {
            let delta = ui.input(|i| i.pointer.delta());
            let item = self
                .panels
                .iter_mut()
                .find(|p| p.palette == palette)
                .unwrap();
            item.height = (item.height + delta.y).clamp(60., 2000.);
            if matches!(p.location, Location::Floating | Location::Window) {
                item.rect[2] = (item.rect[2] + delta.x).clamp(180., 800.);
                if p.location == Location::Window {
                    if let Some(r) = &mut item.window_rect {
                        r[2] = item.rect[2];
                        r[3] = item.height + 29.;
                    }
                    ui.ctx()
                        .send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
                            item.rect[2],
                            item.height + 29.,
                        )));
                }
            }
        }
        response.on_hover_cursor(if p.location == Location::Floating {
            egui::CursorIcon::ResizeNwSe
        } else {
            egui::CursorIcon::ResizeVertical
        });
    }
    fn side(&mut self, ui: &mut egui::Ui, app: &mut Parfait, location: Location) {
        if !self
            .panels
            .iter()
            .any(|p| p.location == location && p.visible)
        {
            return;
        }
        let left = location == Location::Left;
        let panel = if left {
            egui::Panel::left("dock_left")
        } else {
            egui::Panel::right("dock_right")
        };
        let result = panel
            .default_size(if left {
                self.left_width
            } else {
                self.right_width
            })
            .size_range(
                ((self
                    .dock_rows(location)
                    .iter()
                    .map(|r| r.len())
                    .max()
                    .unwrap_or(1) as f32
                    * 180.
                    + 16.)
                    .min(800.))..=800.,
            )
            .resizable(true)
            .show(ui, |ui| {
                let viewport = ui.available_rect_before_wrap();
                egui::ScrollArea::both()
                    .id_salt(if left { "left_stack" } else { "right_stack" })
                    .show(ui, |ui| {
                        let bounds = Rect::from_min_size(ui.cursor().min, viewport.size());
                        self.dock_bounds.push((location, bounds));
                        let layout = self.dock_layout(location, bounds);
                        for (palette, rect) in &layout {
                            ui.scope_builder(
                                egui::UiBuilder::new().id_salt(*palette).max_rect(*rect),
                                |ui| {
                                    ui.set_width(rect.width());
                                    ui.set_clip_rect(ui.clip_rect().intersect(*rect));
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
                        if let Some(total) =
                            layout.iter().map(|(_, r)| *r).reduce(|a, b| a.union(b))
                        {
                            ui.allocate_rect(total, egui::Sense::hover());
                        }
                    });
            });
        if left {
            self.left_width = result.response.rect.width();
        } else {
            self.right_width = result.response.rect.width();
        }
    }
    pub fn draw(&mut self, ui: &mut egui::Ui, app: &mut Parfait) {
        self.slots.clear();
        self.dock_bounds.clear();
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
        let dock_area = ui.available_rect_before_wrap();
        if !app.hide_panels {
            self.side(ui, app, Location::Left);
            self.side(ui, app, Location::Right);
        }
        let floating: Vec<_> = self
            .panels
            .iter()
            .filter(|p| !app.hide_panels && p.visible && p.location == Location::Floating)
            .cloned()
            .collect();
        for p in floating {
            let screen = ctx.content_rect();
            let pos = egui::pos2(
                p.rect[0].clamp(
                    screen.left(),
                    (screen.right() - p.rect[2]).max(screen.left()),
                ),
                p.rect[1].clamp(screen.top(), (screen.bottom() - 23.).max(screen.top())),
            );
            let response = egui::Area::new(egui::Id::new(("floating", p.palette)))
                .order(egui::Order::Foreground)
                .fixed_pos(pos)
                .movable(false)
                .show(&ctx, |ui| {
                    egui::Frame::NONE
                        .fill(Color32::from_gray(57))
                        .show(ui, |ui| {
                            ui.set_width(p.rect[2]);
                            self.shell(ui, app, p.palette);
                        });
                });
            let item = self
                .panels
                .iter_mut()
                .find(|s| s.palette == p.palette)
                .unwrap();
            item.rect[0] = response.response.rect.left();
            item.rect[1] = response.response.rect.top();
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
                .with_title(format!("Parfait · {}", p.palette.title()))
                .with_decorations(false)
                .with_inner_size([p.rect[2], if p.collapsed { 23. } else { p.height + 29. }])
                .with_min_inner_size([180., 23.])
                .with_resizable(true);
            if let Some(r) = p.window_rect {
                builder = builder
                    .with_position([r[0], r[1]])
                    .with_inner_size([r[2], if p.collapsed { 23. } else { r[3] }]);
            }
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
                        item.window_rect = Some([r.left(), r.top(), r.width().max(180.), expanded]);
                        item.rect[2] = r.width().max(180.);
                        if !item.collapsed && r.height() >= 89. {
                            item.height = (r.height() - 29.).clamp(60., 2000.);
                        }
                    }
                    egui::CentralPanel::default()
                        .frame(egui::Frame::NONE.fill(Color32::from_gray(57)))
                        .show(ui, |ui| self.shell(ui, app, p.palette));
                    if class != egui::ViewportClass::EmbeddedWindow {
                        crate::chrome::resize_edges(ui);
                    }
                },
            );
        }
        if let Some(palette) = self.dragging {
            if let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) {
                let target = self.slots.iter().find(|(_, _, r)| r.contains(pos)).copied();
                let screen = ctx.content_rect();
                let edge = if pos.x < screen.left() + 100. {
                    Some(Location::Left)
                } else if pos.x > screen.right() - 100. {
                    Some(Location::Right)
                } else {
                    None
                };
                let paint = ctx.layer_painter(egui::LayerId::new(
                    egui::Order::Tooltip,
                    egui::Id::new("dock_feedback"),
                ));
                let direction = target.map(|(_, _, r)| DockDirection::at(r, pos));
                let mut planned = self.clone();
                if let Some((other, side, _)) = target {
                    planned.dock_to(palette, other, direction.unwrap());
                    if let Some((_, bounds)) = self.dock_bounds.iter().find(|(s, _)| *s == side)
                        && let Some((_, preview)) = planned
                            .dock_layout(side, {
                                let mut bounds = *bounds;
                                let desired = planned
                                    .dock_rows(side)
                                    .iter()
                                    .map(|r| r.len())
                                    .max()
                                    .unwrap_or(1)
                                    as f32
                                    * 180.;
                                let desired = desired.min(784.);
                                if side == Location::Right && desired > bounds.width() {
                                    bounds.min.x = bounds.right() - desired;
                                }
                                bounds
                            })
                            .into_iter()
                            .find(|(p, _)| *p == palette)
                    {
                        paint.rect_filled(
                            preview,
                            0.,
                            Color32::from_rgba_unmultiplied(90, 160, 240, 75),
                        );
                        paint.rect_stroke(
                            preview,
                            0.,
                            egui::Stroke::new(2., Color32::LIGHT_BLUE),
                            egui::StrokeKind::Inside,
                        );
                    }
                } else if let Some(side) = edge {
                    planned.relocate(palette, side, None, pos);
                    let bounds = self
                        .dock_bounds
                        .iter()
                        .find(|(s, _)| *s == side)
                        .map(|(_, r)| *r)
                        .unwrap_or_else(|| {
                            let width = if side == Location::Left {
                                self.left_width
                            } else {
                                self.right_width
                            };
                            Rect::from_min_size(
                                egui::pos2(
                                    if side == Location::Left {
                                        dock_area.left() + 8.
                                    } else {
                                        dock_area.right() - width + 8.
                                    },
                                    dock_area.top() + 8.,
                                ),
                                egui::vec2(width - 16., dock_area.height() - 16.),
                            )
                        });
                    if let Some((_, preview)) = planned
                        .dock_layout(side, bounds)
                        .into_iter()
                        .find(|(p, _)| *p == palette)
                    {
                        paint.rect_filled(
                            preview,
                            0.,
                            Color32::from_rgba_unmultiplied(90, 160, 240, 75),
                        );
                        paint.rect_stroke(
                            preview,
                            0.,
                            egui::Stroke::new(2., Color32::LIGHT_BLUE),
                            egui::StrokeKind::Inside,
                        );
                    }
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
        if !self.hide_panels {
            self.tool_rail(ui);
        }
        self.update_previews(ui.ctx());
        let mut workspace = std::mem::take(&mut self.workspace);
        workspace.draw(ui, self);
        self.workspace = workspace;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn header_context_menu_opens_a_separate_window() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open(dir.path().to_path_buf()).unwrap();
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let frame = |app: &mut Parfait, events| {
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1280., 900.))),
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
            .find(|(p, _)| *p == Palette::Brushes)
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
            .data(|d| d.get_temp::<Rect>(egui::Id::new(("native_command", Palette::Brushes))))
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
            vec![button(rect.center(), false, egui::PointerButton::Primary)],
        );
        assert_eq!(
            app.workspace
                .panels
                .iter()
                .find(|p| p.palette == Palette::Brushes)
                .unwrap()
                .location,
            Location::Window
        );
        assert!(app.workspace.dragging.is_none());
    }
    #[test]
    fn side_docking_preview_matches_rendered_slot_and_window_state_roundtrips() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open(dir.path().to_path_buf()).unwrap();
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let frame = |app: &mut Parfait, events| {
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1440., 1000.))),
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
        preview.dock_to(Palette::Brushes, Palette::Properties, DockDirection::Right);
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
        let restored = Parfait::open(dir.path().to_path_buf()).unwrap();
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
        let mut app = Parfait::open(dir.path().to_path_buf()).unwrap();
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
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1280., 900.))),
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
        let mut app = Parfait::open(dir.path().to_path_buf()).unwrap();
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let frame = |app: &mut Parfait, events| {
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1280., 900.))),
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
            assert_eq!(app.workspace.panels[0].height, 185.);
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
        let restored = Parfait::open(dir.path().to_path_buf()).unwrap();
        assert!(restored.workspace.panels[0].collapsed);
        assert!(!restored.workspace.panels[0].visible);
        assert_eq!(restored.workspace.panels[0].height, 185.);
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
        let mut app = Parfait::open(dir.path().to_path_buf()).unwrap();
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let frame = |app: &mut Parfait, events| {
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1280., 900.))),
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
        assert_eq!(detached.height, 185.);
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
        app.brush.pressure_curve = [0., 0.1, 0.3, 0.6, 1.];
        app.presets.push(crate::brushes::Preset {
            name: "My imported tip".into(),
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
        let restored = Parfait::open(dir.path().to_path_buf()).unwrap();
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
        assert_eq!(restored.brush.pressure_curve, [0., 0.1, 0.3, 0.6, 1.]);
        assert_eq!(restored.presets.last().unwrap().name, "My imported tip");
        assert!(
            matches!(&restored.presets.last().unwrap().brush.tip,parfait_core::BrushTip::Image{mask,..} if mask.as_slice()==[0,128,255,64])
        );
    }
}
