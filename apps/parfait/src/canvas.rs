use crate::app::Parfait;
use eframe::egui::{self, Color32, Event, Key, PointerButton, Pos2, Rect, Vec2};
use parfait_core::{Point, Stroke};

#[derive(Clone, Copy)]
enum Gesture {
    Pan,
    Rotate,
    Zoom,
}
pub struct View {
    pub zoom: f32,
    pub angle: f32,
    pub pan: Vec2,
    initialized: bool,
    gesture: Option<Gesture>,
    last: Pos2,
}
impl Default for View {
    fn default() -> Self {
        Self {
            zoom: 1.,
            angle: 0.,
            pan: Vec2::ZERO,
            initialized: false,
            gesture: None,
            last: Pos2::ZERO,
        }
    }
}
impl View {
    fn screen(&self, p: Vec2, rect: Rect, size: Vec2) -> Pos2 {
        let p = (p - size / 2.) * self.zoom;
        rect.center() + self.pan + rotate(p, self.angle)
    }
    fn document(&self, p: Pos2, rect: Rect, size: Vec2) -> Vec2 {
        rotate(p - rect.center() - self.pan, -self.angle) / self.zoom + size / 2.
    }
    fn zoom_at(&mut self, factor: f32, anchor: Pos2, rect: Rect, size: Vec2) {
        let before = self.document(anchor, rect, size);
        self.zoom = (self.zoom * factor).clamp(0.05, 12.);
        let after = self.screen(before, rect, size);
        self.pan += anchor - after;
    }
}
fn rotate(v: Vec2, a: f32) -> Vec2 {
    let (s, c) = a.sin_cos();
    egui::vec2(v.x * c - v.y * s, v.x * s + v.y * c)
}
impl Parfait {
    pub fn canvas(&mut self, ui: &mut egui::Ui) {
        let (outer, response) =
            ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());
        let rect = Rect::from_min_max(outer.min + egui::vec2(18., 18.), outer.max);
        let size = egui::vec2(self.doc.width as f32, self.doc.height as f32);
        if !self.view.initialized {
            self.view.zoom = ((rect.width() - 80.) / size.x)
                .min((rect.height() - 80.) / size.y)
                .clamp(0.05, 1.);
            self.view.initialized = true;
        }
        let ctx = ui.ctx().clone();
        let events = ctx.input(|i| i.events.clone());
        let space = ctx.input(|i| i.key_down(Key::Space)) || self.navigation_tool == 1;
        let modifiers = ctx.input(|i| i.modifiers);
        let blocked =
            self.confirm_new || self.error.is_some() || self.show_help || self.show_licenses;
        if response.hovered() && !blocked && self.stroke.is_none() {
            let scroll = ctx.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.
                && let Some(pos) = ctx.input(|i| i.pointer.hover_pos())
            {
                self.view.zoom_at((scroll * 0.003).exp(), pos, rect, size);
            }
        }
        for event in events {
            match event {
                Event::Touch {
                    force: Some(force), ..
                } => {
                    self.pressure = force;
                }
                Event::PointerButton {
                    pos,
                    button,
                    pressed,
                    modifiers,
                } => {
                    if pressed && rect.contains(pos) && response.hovered() && !blocked {
                        if button == PointerButton::Middle
                            || (button == PointerButton::Primary && space)
                        {
                            self.view.gesture = Some(if modifiers.shift {
                                Gesture::Rotate
                            } else if modifiers.command {
                                Gesture::Zoom
                            } else {
                                Gesture::Pan
                            });
                            self.view.last = pos;
                        } else if button == PointerButton::Primary {
                            let p = self.view.document(pos, rect, size);
                            if p.x >= 0. && p.y >= 0. && p.x < size.x && p.y < size.y {
                                if modifiers.alt || self.navigation_tool == 2 {
                                    let pixels = self.doc.composite();
                                    let idx =
                                        ((p.y as u32 * self.doc.width + p.x as u32) * 4) as usize;
                                    self.brush.color.copy_from_slice(&pixels[idx..idx + 3]);
                                } else if !self.doc.layers[self.doc.active].locked
                                    && self.doc.layers[self.doc.active].visible
                                {
                                    self.history.checkpoint(&self.doc);
                                    self.stroke = Some(Stroke::begin(
                                        &mut self.doc,
                                        self.brush.clone(),
                                        Point {
                                            x: p.x,
                                            y: p.y,
                                            pressure: self.pressure,
                                        },
                                    ));
                                }
                            }
                        }
                    }
                    if !pressed
                        && (button == PointerButton::Primary || button == PointerButton::Middle)
                    {
                        if self.stroke.take().is_some() {
                            self.force_save();
                        }
                        self.view.gesture = None;
                        self.pressure = 1.;
                    }
                }
                Event::PointerMoved(pos) => {
                    if let Some(gesture) = self.view.gesture {
                        let delta = pos - self.view.last;
                        match gesture {
                            Gesture::Pan => self.view.pan += delta,
                            Gesture::Rotate => self.view.angle += delta.x * 0.008,
                            Gesture::Zoom => self.view.zoom_at(
                                (delta.x * 0.01).exp(),
                                self.view.last,
                                rect,
                                size,
                            ),
                        }
                        self.view.last = pos;
                    } else if let Some(stroke) = &mut self.stroke {
                        let p = self.view.document(pos, rect, size);
                        stroke.extend(
                            &mut self.doc,
                            Point {
                                x: p.x,
                                y: p.y,
                                pressure: self.pressure,
                            },
                        );
                    }
                }
                _ => {}
            }
        }
        if !ctx.input(|i| i.pointer.primary_down()) && self.stroke.take().is_some() {
            self.force_save();
        }
        if self.rendered != self.doc.revision || self.texture.is_none() {
            let mut pixels = self.doc.composite();
            for (y, row) in pixels
                .chunks_exact_mut(self.doc.width as usize * 4)
                .enumerate()
            {
                for (x, p) in row.as_chunks_mut::<4>().0.iter_mut().enumerate() {
                    let bg = if (x / 16 + y / 16) % 2 == 0 {
                        235.
                    } else {
                        218.
                    };
                    let a = p[3] as f32 / 255.;
                    for c in &mut p[..3] {
                        *c = (*c as f32 * a + bg * (1. - a)).round() as u8;
                    }
                    p[3] = 255;
                }
            }
            let image = egui::ColorImage::from_rgba_unmultiplied(
                [self.doc.width as usize, self.doc.height as usize],
                &pixels,
            );
            if let Some(tex) = &mut self.texture {
                tex.set(image, egui::TextureOptions::LINEAR);
            } else {
                self.texture =
                    Some(ctx.load_texture("drawing", image, egui::TextureOptions::LINEAR));
            }
            self.rendered = self.doc.revision;
        }
        if let Some(texture) = &self.texture {
            let corners = [
                Vec2::ZERO,
                egui::vec2(size.x, 0.),
                size,
                egui::vec2(0., size.y),
            ]
            .map(|p| self.view.screen(p, rect, size));
            let uvs = [
                egui::pos2(0., 0.),
                egui::pos2(1., 0.),
                egui::pos2(1., 1.),
                egui::pos2(0., 1.),
            ];
            let mut mesh = egui::Mesh::with_texture(texture.id());
            for i in 0..4 {
                mesh.vertices.push(egui::epaint::Vertex {
                    pos: corners[i],
                    uv: uvs[i],
                    color: Color32::WHITE,
                });
            }
            mesh.indices = vec![0, 1, 2, 0, 2, 3];
            ui.painter()
                .with_clip_rect(rect)
                .add(egui::Shape::mesh(mesh));
        }
        // Rulers follow the view transform; at rotation they show projected document coordinates.
        let painter = ui.painter();
        painter.rect_filled(
            Rect::from_min_max(outer.min, egui::pos2(outer.right(), rect.top())),
            0.,
            Color32::from_gray(66),
        );
        painter.rect_filled(
            Rect::from_min_max(outer.min, egui::pos2(rect.left(), outer.bottom())),
            0.,
            Color32::from_gray(66),
        );
        let ink = Color32::from_gray(168);
        for x in (0..rect.width() as usize).step_by(50) {
            let p = egui::pos2(rect.left() + x as f32, rect.top());
            let value = self.view.document(p, rect, size).x;
            painter.line_segment([p, p - egui::vec2(0., 5.)], egui::Stroke::new(1., ink));
            painter.text(
                p - egui::vec2(-3., 7.),
                egui::Align2::LEFT_BOTTOM,
                format!("{value:.0}"),
                egui::FontId::monospace(9.),
                ink,
            );
        }
        for y in (0..rect.height() as usize).step_by(50) {
            let p = egui::pos2(rect.left(), rect.top() + y as f32);
            painter.line_segment([p, p - egui::vec2(5., 0.)], egui::Stroke::new(1., ink));
            // Horizontal labels stay legible in the narrow vertical ruler.
            painter.text(
                p - egui::vec2(3., -2.),
                egui::Align2::RIGHT_TOP,
                format!("{:.0}", self.view.document(p, rect, size).y),
                egui::FontId::monospace(7.),
                ink,
            );
        }
        if response.hovered() {
            if space {
                ctx.set_cursor_icon(egui::CursorIcon::Grab);
            } else if modifiers.alt || self.navigation_tool == 2 {
                ctx.set_cursor_icon(egui::CursorIcon::Crosshair);
            } else if let Some(pos) = ctx.input(|i| i.pointer.hover_pos()) {
                ctx.set_cursor_icon(egui::CursorIcon::None);
                let radius = (self.brush.size * self.view.zoom / 2.).max(2.);
                ui.painter().circle_stroke(
                    pos,
                    radius,
                    egui::Stroke::new(2., Color32::from_black_alpha(150)),
                );
                ui.painter()
                    .circle_stroke(pos, radius, egui::Stroke::new(1., Color32::WHITE));
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hand_drag_moves_view_without_painting_then_pen_draws() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open(dir.path().to_path_buf()).unwrap();
        app.doc = parfait_core::Document::new(64, 64);
        let ctx = egui::Context::default();
        let frame = |app: &mut Parfait, events| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(400., 400.))),
                    events,
                    ..Default::default()
                },
                |ui| {
                    egui::CentralPanel::default().show(ui, |ui| app.canvas(ui));
                },
            );
            output.textures_delta.clear();
        };
        let button = |pos, pressed| Event::PointerButton {
            pos,
            pressed,
            button: PointerButton::Primary,
            modifiers: Default::default(),
        };
        frame(&mut app, vec![]);
        let start = egui::pos2(210., 210.);
        let end = start + egui::vec2(15., 10.);
        let before = app.doc.composite();
        app.navigation_tool = 1;
        frame(&mut app, vec![Event::PointerMoved(start)]);
        frame(&mut app, vec![button(start, true)]);
        frame(&mut app, vec![Event::PointerMoved(end)]);
        frame(&mut app, vec![button(end, false)]);
        assert_eq!(app.view.pan, egui::vec2(15., 10.));
        assert_eq!(app.doc.composite(), before);
        app.navigation_tool = 0;
        frame(&mut app, vec![button(end, true)]);
        frame(&mut app, vec![button(end, false)]);
        assert_ne!(app.doc.composite(), before);
    }
    #[test]
    fn coordinate_roundtrip_and_zoom_anchor() {
        let mut v = View {
            angle: 0.7,
            zoom: 1.3,
            pan: egui::vec2(35., -8.),
            ..Default::default()
        };
        let r = Rect::from_min_size(Pos2::ZERO, egui::vec2(900., 700.));
        let size = egui::vec2(1024., 768.);
        let p = egui::vec2(123., 456.);
        let screen = v.screen(p, r, size);
        assert!((v.document(screen, r, size) - p).length() < 0.001);
        v.zoom_at(1.5, screen, r, size);
        assert!((v.document(screen, r, size) - p).length() < 0.001);
    }
}
