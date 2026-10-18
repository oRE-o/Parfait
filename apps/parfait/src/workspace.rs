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
                    height: [185., 145., 115., 205., 240., 400.][i],
                    rect: [300. + i as f32 * 24., 120. + i as f32 * 24., 270., 340.],
                })
                .collect(),
            left_width: 230.,
            right_width: 250.,
            dragging: None,
            slots: Vec::new(),
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
        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), 23.),
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
        self.headers.push((palette, rect));
        ui.painter().rect_filled(rect, 0., Color32::from_gray(48));
        ui.painter().text(
            rect.left_center() + egui::vec2(6., 0.),
            egui::Align2::LEFT_CENTER,
            format!("≡  {}", palette.title()),
            egui::FontId::proportional(11.),
            Color32::LIGHT_GRAY,
        );
        if response.drag_started() {
            self.dragging = Some(palette);
        }
        response.on_hover_text("Drag to reorder or dock at either side. Drop over the canvas to float. Resize with the bottom grip.");
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
                                self.header(ui, palette);
                                let h = self
                                    .panels
                                    .iter()
                                    .find(|p| p.palette == palette)
                                    .unwrap()
                                    .height;
                                egui::ScrollArea::vertical()
                                    .id_salt("body")
                                    .max_height(h)
                                    .min_scrolled_height(h)
                                    .auto_shrink([false, false])
                                    .show(ui, |ui| app.palette(ui, palette));
                                let (grip, r) = ui.allocate_exact_size(
                                    egui::vec2(ui.available_width(), 6.),
                                    egui::Sense::drag(),
                                );
                                ui.painter().line_segment(
                                    [
                                        grip.center() - egui::vec2(18., 0.),
                                        grip.center() + egui::vec2(18., 0.),
                                    ],
                                    egui::Stroke::new(1., Color32::GRAY),
                                );
                                if r.dragged() {
                                    let delta = ui.input(|i| i.pointer.delta().y);
                                    let item = self
                                        .panels
                                        .iter_mut()
                                        .find(|p| p.palette == palette)
                                        .unwrap();
                                    item.height = (item.height + delta).clamp(60., 2000.);
                                }
                                r.on_hover_cursor(egui::CursorIcon::ResizeVertical);
                                self.slots.push((
                                    palette,
                                    location,
                                    Rect::from_min_max(start, grip.max),
                                ));
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
        self.side(ui, app, Location::Left);
        self.side(ui, app, Location::Right);
        let ctx = ui.ctx().clone();
        let floating: Vec<_> = self
            .panels
            .iter()
            .filter(|p| p.visible && p.location == Location::Floating)
            .cloned()
            .collect();
        for p in floating {
            let response = egui::Window::new(p.palette.title())
                .id(egui::Id::new(("floating", p.palette)))
                .current_pos(egui::pos2(p.rect[0], p.rect[1]))
                .default_size(egui::vec2(p.rect[2], p.rect[3]))
                .min_size(egui::vec2(180., 80.))
                .resizable(true)
                .scroll(true)
                .show(&ctx, |ui| {
                    self.header(ui, p.palette);
                    app.palette(ui, p.palette);
                });
            if let Some(r) = response {
                let rect = r.response.rect;
                let item = self
                    .panels
                    .iter_mut()
                    .find(|s| s.palette == p.palette)
                    .unwrap();
                item.rect = [rect.min.x, rect.min.y, rect.width(), rect.height()];
            }
        }
        if let Some(palette) = self.dragging {
            if let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) {
                let target = self.slots.iter().find(|(_, _, r)| r.contains(pos)).copied();
                let paint = ctx.layer_painter(egui::LayerId::new(
                    egui::Order::Tooltip,
                    egui::Id::new("dock_feedback"),
                ));
                if let Some((_, _, r)) = target {
                    paint.rect_stroke(
                        r,
                        0.,
                        egui::Stroke::new(2., Color32::LIGHT_BLUE),
                        egui::StrokeKind::Inside,
                    );
                }
                paint.text(
                    pos + egui::vec2(14., 14.),
                    egui::Align2::LEFT_TOP,
                    format!("Move {}", palette.title()),
                    egui::FontId::proportional(13.),
                    Color32::WHITE,
                );
                if ctx.input(|i| i.pointer.any_released()) {
                    if let Some((other, side, _)) = target {
                        if other != palette {
                            self.relocate(palette, side, Some(other), pos);
                        }
                    } else {
                        let screen = ctx.content_rect();
                        let side = if pos.x < screen.left() + 100. {
                            Location::Left
                        } else if pos.x > screen.right() - 100. {
                            Location::Right
                        } else {
                            Location::Floating
                        };
                        self.relocate(palette, side, None, pos);
                    }
                    self.dragging = None;
                }
            } else if !ctx.input(|i| i.pointer.any_down()) {
                self.dragging = None;
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
