//! Persistent palette placement. Floating palettes remain inside the application window.
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
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Placement {
    pub palette: Palette,
    pub location: Location,
    pub visible: bool,
    #[serde(default)]
    pub collapsed: bool,
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
                s.height.is_finite()
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
        item.location = location;
        if location == Location::Floating {
            item.rect[0] = pos.x;
            item.rect[1] = pos.y;
        }
        let index = before
            .and_then(|p| self.panels.iter().position(|s| s.palette == p))
            .unwrap_or(self.panels.len());
        self.panels.insert(index, item);
    }
    fn header(&mut self, ui: &mut egui::Ui, palette: Palette) {
        let (rect, _) =
            ui.allocate_exact_size(egui::vec2(ui.available_width(), 23.), egui::Sense::hover());
        let drag_rect = Rect::from_min_max(rect.min, rect.max - egui::vec2(42., 0.));
        let response = ui.interact(drag_rect, ui.id().with("move"), egui::Sense::drag());
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
        if response.drag_started() && self.dragging.is_none() {
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
            .on_hover_text("Drag to move or dock. Escape cancels the move.");
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
            if p.location == Location::Floating {
                item.rect[2] = (item.rect[2] + delta.x).clamp(180., 800.);
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
            .size_range(180. ..=600.)
            .resizable(true)
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt(if left { "left_stack" } else { "right_stack" })
                    .show(ui, |ui| {
                        let ids: Vec<_> = self
                            .panels
                            .iter()
                            .filter(|p| p.visible && p.location == location)
                            .map(|p| p.palette)
                            .collect();
                        for palette in ids {
                            ui.push_id(palette, |ui| {
                                let start = ui.cursor().min;
                                self.shell(ui, app, palette);
                                if self.dragging != Some(palette) {
                                    self.slots.push((
                                        palette,
                                        location,
                                        Rect::from_min_max(start, ui.min_rect().max),
                                    ));
                                }
                            });
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
        self.side(ui, app, Location::Left);
        self.side(ui, app, Location::Right);
        let floating: Vec<_> = self
            .panels
            .iter()
            .filter(|p| p.visible && p.location == Location::Floating)
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
                if let Some((_, _, r)) = target {
                    paint.line_segment(
                        [r.left_top(), r.right_top()],
                        egui::Stroke::new(3., Color32::LIGHT_BLUE),
                    );
                } else if let Some(side) = edge {
                    let x = if side == Location::Left {
                        screen.left()
                    } else {
                        screen.right() - 6.
                    };
                    paint.rect_filled(
                        Rect::from_min_size(
                            egui::pos2(x, screen.top()),
                            egui::vec2(6., screen.height()),
                        ),
                        0.,
                        Color32::LIGHT_BLUE,
                    );
                }
                if ctx.input(|i| i.pointer.any_released()) {
                    if let Some((other, side, _)) = target {
                        self.relocate(palette, side, Some(other), pos);
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
        self.tool_rail(ui);
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
