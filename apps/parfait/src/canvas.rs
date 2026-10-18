use crate::app::Parfait;
use eframe::egui::{self, Color32, Event, Key, PointerButton, Pos2, Rect, Vec2};
use parfait_core::{PixelRect, Point, Stroke};

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
        let blocked = self.calibration_dialog.is_some()
            || self.brush_details.dialog_open()
            || self.confirm_new
            || self.error.is_some()
            || self.show_help
            || self.show_licenses
            || self.workspace.dragging.is_some();
        // Space takes ownership until pointer-up; painting never resumes mid-gesture.
        if (space || blocked) && self.stroke.take().is_some() {
            self.force_save();
            if space && !blocked && ctx.input(|i| i.pointer.primary_down()) {
                self.view.gesture = Some(Gesture::Pan);
                if let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) {
                    self.view.last = pos;
                }
            }
        }
        if !blocked && self.stroke.is_none() {
            if !ctx.egui_wants_keyboard_input() {
                let step = ctx.input(|i| {
                    if i.modifiers.command || i.modifiers.ctrl {
                        i32::from(i.key_pressed(Key::Plus) || i.key_pressed(Key::Equals))
                            - i32::from(i.key_pressed(Key::Minus))
                    } else {
                        0
                    }
                });
                if step != 0 {
                    self.view
                        .zoom_at(1.2_f32.powi(step), rect.center(), rect, size);
                }
            }
            if response.hovered() {
                // Ctrl/Cmd-wheel and pinch arrive through zoom_delta, not scroll_delta.
                let factor =
                    ctx.input(|i| (i.smooth_scroll_delta.y * 0.003).exp() * i.zoom_delta());
                if factor != 1.
                    && let Some(pos) = ctx.input(|i| i.pointer.hover_pos())
                {
                    self.view.zoom_at(factor, pos, rect, size);
                }
            }
        }
        // Changes made outside this input batch (undo, layer edits, open) invalidate all pixels.
        let full_refresh = self.rendered != self.doc.revision || self.texture.is_none();
        let mut damage: Option<PixelRect> = None;
        let mut stroke_started = false;
        for event in events {
            match event {
                Event::Touch {
                    phase: egui::TouchPhase::Cancel,
                    ..
                }
                | Event::PointerGone
                | Event::WindowFocused(false) => {
                    if self.stroke.take().is_some() {
                        self.force_save();
                    }
                    self.view.gesture = None;
                    self.pressure = 1.;
                }
                Event::Touch {
                    force: Some(force), ..
                } => {
                    if force.is_finite() {
                        self.pressure = force.clamp(0., 1.);
                        self.pressure_seen = true;
                    }
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
                            self.view.gesture = Some(if button == PointerButton::Middle {
                                Gesture::Pan
                            } else if modifiers.shift {
                                Gesture::Rotate
                            } else if modifiers.command || modifiers.ctrl {
                                Gesture::Zoom
                            } else {
                                Gesture::Pan
                            });
                            self.view.last = pos;
                        } else if button == PointerButton::Primary {
                            let p = self.view.document(pos, rect, size);
                            if p.x >= 0. && p.y >= 0. && p.x < size.x && p.y < size.y {
                                if modifiers.alt || self.navigation_tool == 2 {
                                    let pixels = self.doc.composite_region(PixelRect {
                                        x0: p.x as u32,
                                        y0: p.y as u32,
                                        x1: p.x as u32 + 1,
                                        y1: p.y as u32 + 1,
                                    });
                                    self.brush.color.copy_from_slice(&pixels[..3]);
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
                                            pressure: self.calibration.map(self.pressure),
                                        },
                                    ));
                                    stroke_started = true;
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
                    } else if !space
                        && !blocked
                        && let Some(stroke) = &mut self.stroke
                    {
                        let p = self.view.document(pos, rect, size);
                        stroke.extend(
                            &mut self.doc,
                            Point {
                                x: p.x,
                                y: p.y,
                                pressure: self.calibration.map(self.pressure),
                            },
                        );
                    }
                }
                _ => {}
            }
            if let Some(stroke) = &mut self.stroke
                && let Some(r) = stroke.take_damage()
            {
                damage = Some(damage.map_or(r, |old| old.union(r)));
            }
        }
        if !ctx.input(|i| i.pointer.primary_down()) && self.stroke.take().is_some() {
            self.force_save();
        }
        if !stroke_started
            && !blocked
            && !space
            && let Some(stroke) = &mut self.stroke
            && let Some(interval) = stroke.spray_interval()
        {
            stroke.spray(
                &mut self.doc,
                ctx.input(|input| input.unstable_dt),
                self.calibration.map(self.pressure),
            );
            if let Some(region) = stroke.take_damage() {
                damage = Some(damage.map_or(region, |old| old.union(region)));
            }
            ctx.request_repaint_after(std::time::Duration::from_secs_f32(interval));
        } else if stroke_started
            && let Some(interval) = self.stroke.as_ref().and_then(Stroke::spray_interval)
        {
            ctx.request_repaint_after(std::time::Duration::from_secs_f32(interval));
        }
        if self.rendered != self.doc.revision || self.texture.is_none() {
            let region = if let Some(r) = damage.filter(|_| !full_refresh) {
                r
            } else {
                PixelRect {
                    x0: 0,
                    y0: 0,
                    x1: self.doc.width,
                    y1: self.doc.height,
                }
            };
            let width = (region.x1 - region.x0) as usize;
            let height = (region.y1 - region.y0) as usize;
            let mut pixels = self.doc.composite_region(region);
            for (y, row) in pixels.chunks_exact_mut(width * 4).enumerate() {
                for (x, p) in row.as_chunks_mut::<4>().0.iter_mut().enumerate() {
                    if p[3] == 255 {
                        continue;
                    }
                    let bg = if ((x + region.x0 as usize) / 16 + (y + region.y0 as usize) / 16)
                        .is_multiple_of(2)
                    {
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
            let image = egui::ColorImage::from_rgba_unmultiplied([width, height], &pixels);
            if let Some(tex) = &mut self.texture {
                if full_refresh || damage.is_none() {
                    tex.set(image, egui::TextureOptions::LINEAR);
                } else {
                    tex.set_partial(
                        [region.x0 as usize, region.y0 as usize],
                        image,
                        egui::TextureOptions::LINEAR,
                    );
                }
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
        crate::rulers::paint(
            ui,
            outer,
            rect,
            self.view.document(rect.left_top(), rect, size),
            self.view.angle.cos() / self.view.zoom,
        );
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
    #[test]
    fn stationary_spray_ticks_frozen_brush_and_stops_on_release_focus_loss_navigation_and_dialogs()
    {
        for stop in 0..5 {
            let dir = tempfile::tempdir().unwrap();
            let mut app = crate::app::Parfait::open(dir.path().to_path_buf()).unwrap();
            app.doc = parfait_core::Document::new(64, 64);
            app.brush = parfait_core::Brush {
                continuous_spray: true,
                pressure: false,
                size: 20.,
                flow: 0.03,
                ..Default::default()
            };
            let ctx = eframe::egui::Context::default();
            let frame = |app: &mut crate::app::Parfait, time, events| {
                let mut output = ctx.run_ui(
                    eframe::egui::RawInput {
                        time: Some(time),
                        screen_rect: Some(eframe::egui::Rect::from_min_size(
                            eframe::egui::Pos2::ZERO,
                            eframe::egui::vec2(400., 400.),
                        )),
                        events,
                        ..Default::default()
                    },
                    |ui| app.canvas(ui),
                );
                output.textures_delta.clear();
            };
            let position = eframe::egui::pos2(210., 210.);
            let button = |pressed| eframe::egui::Event::PointerButton {
                pos: position,
                button: eframe::egui::PointerButton::Primary,
                pressed,
                modifiers: Default::default(),
            };
            frame(&mut app, 0., vec![]);
            frame(
                &mut app,
                0.01,
                vec![eframe::egui::Event::PointerMoved(position), button(true)],
            );
            let started = app.doc.revision;
            app.brush.continuous_spray = false;
            frame(&mut app, 0.11, vec![]);
            assert!(app.doc.revision > started);
            let painted = app.doc.composite();
            let event = match stop {
                0 => button(false),
                1 => eframe::egui::Event::WindowFocused(false),
                2 => {
                    app.navigation_tool = 1;
                    eframe::egui::Event::PointerMoved(position)
                }
                3 => {
                    app.brush_details.open_stroke_settings();
                    eframe::egui::Event::PointerMoved(position)
                }
                _ => eframe::egui::Event::PointerGone,
            };
            frame(&mut app, 0.21, vec![event]);
            frame(&mut app, 0.31, vec![]);
            assert!(app.stroke.is_none());
            assert_eq!(app.doc.composite(), painted);
        }
    }
    use super::*;
    #[test]
    fn pan_moves_painted_ruler_labels_without_editing_or_uploading_artwork() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open(dir.path().to_path_buf()).unwrap();
        app.doc = parfait_core::Document::new(512, 512);
        app.view.initialized = true;
        app.navigation_tool = 1;
        let original = app.doc.clone();
        let ctx = egui::Context::default();
        let frame = |app: &mut Parfait, events| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(600., 400.))),
                    events,
                    ..Default::default()
                },
                |ui| app.canvas(ui),
            );
            let labels: Vec<_> = output
                .shapes
                .iter()
                .filter_map(|shape| {
                    if let egui::Shape::Text(text) = &shape.shape {
                        Some((text.galley.job.text.clone(), text.pos, text.angle))
                    } else {
                        None
                    }
                })
                .collect();
            let uploaded = output
                .textures_delta
                .set
                .contains_key(&app.texture.as_ref().unwrap().id());
            output.textures_delta.clear();
            (labels, uploaded)
        };
        let (before, _) = frame(&mut app, vec![]);
        let start = egui::pos2(230., 180.);
        let delta = egui::vec2(37.25, 31.5);
        let end = start + delta;
        let button = |position, pressed| Event::PointerButton {
            pos: position,
            pressed,
            button: PointerButton::Primary,
            modifiers: Default::default(),
        };
        frame(&mut app, vec![Event::PointerMoved(start)]);
        frame(&mut app, vec![button(start, true)]);
        frame(&mut app, vec![Event::PointerMoved(end)]);
        let (after, uploaded) = frame(&mut app, vec![button(end, false)]);
        assert_eq!(app.view.pan, delta);
        assert_eq!(app.doc, original);
        assert!(!uploaded);
        for angle in [0., -std::f32::consts::FRAC_PI_2] {
            let before = before
                .iter()
                .find(|(label, _, rotation)| label == "100" && *rotation == angle)
                .unwrap()
                .1;
            let after = after
                .iter()
                .find(|(label, _, rotation)| label == "100" && *rotation == angle)
                .unwrap()
                .1;
            let moved = if angle == 0. {
                after.x - before.x
            } else {
                after.y - before.y
            };
            let expected = if angle == 0. { delta.x } else { delta.y };
            assert!((moved - expected).abs() < 0.001);
        }
        app.history.undo(&mut app.doc);
        assert_eq!(app.doc, original);
    }

    fn zoom_frame(app: &mut Parfait, ctx: &egui::Context, events: Vec<Event>) -> Rect {
        let mut rect = Rect::NOTHING;
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(400., 400.))),
                events,
                ..Default::default()
            },
            |ui| {
                app.shortcuts(ui.ctx());
                egui::CentralPanel::default().show(ui, |ui| {
                    let outer = ui.available_rect_before_wrap();
                    rect = Rect::from_min_max(outer.min + egui::vec2(18., 18.), outer.max);
                    app.canvas(ui);
                });
            },
        );
        output.textures_delta.clear();
        rect
    }
    #[test]
    fn command_plus_equals_minus_zoom_canvas_without_scaling_ui() {
        for (key, shift, factor) in [
            (Key::Plus, true, 1.2),
            (Key::Equals, false, 1.2),
            (Key::Minus, false, 1. / 1.2),
        ] {
            let dir = tempfile::tempdir().unwrap();
            let mut app = Parfait::open(dir.path().to_path_buf()).unwrap();
            app.doc = parfait_core::Document::new(64, 64);
            let ctx = egui::Context::default();
            crate::theme::apply(&ctx);
            let rect = zoom_frame(&mut app, &ctx, vec![]);
            app.view.pan = egui::vec2(35., -18.);
            let size = egui::vec2(64., 64.);
            let center = app.view.document(rect.center(), rect, size);
            let zoom = app.view.zoom;
            let modifiers = egui::Modifiers {
                ctrl: true,
                command: true,
                shift,
                ..Default::default()
            };
            let key_event = |pressed| Event::Key {
                key,
                physical_key: None,
                pressed,
                repeat: false,
                modifiers,
            };
            zoom_frame(
                &mut app,
                &ctx,
                vec![Event::ModifiersChanged(modifiers), key_event(true)],
            );
            assert!((app.view.zoom - zoom * factor).abs() < 1e-6);
            assert!((app.view.document(rect.center(), rect, size) - center).length() < 1e-4);
            assert_eq!(ctx.zoom_factor(), 1.);
            zoom_frame(&mut app, &ctx, vec![key_event(false)]);
            app.brush_details.open = true;
            let zoom = app.view.zoom;
            zoom_frame(&mut app, &ctx, vec![key_event(true)]);
            assert_eq!(app.view.zoom, zoom);
        }
    }
    #[test]
    fn wheel_with_or_without_control_and_pinch_zoom_at_pointer() {
        for ctrl in [false, true] {
            for delta in [-30., 30.] {
                let dir = tempfile::tempdir().unwrap();
                let mut app = Parfait::open(dir.path().to_path_buf()).unwrap();
                app.doc = parfait_core::Document::new(64, 64);
                let ctx = egui::Context::default();
                crate::theme::apply(&ctx);
                let pointer = egui::pos2(170., 240.);
                let rect = zoom_frame(&mut app, &ctx, vec![Event::PointerMoved(pointer)]);
                let size = egui::vec2(64., 64.);
                let anchor = app.view.document(pointer, rect, size);
                let before = app.view.zoom;
                let modifiers = egui::Modifiers {
                    ctrl,
                    command: ctrl,
                    ..Default::default()
                };
                zoom_frame(
                    &mut app,
                    &ctx,
                    vec![
                        Event::ModifiersChanged(modifiers),
                        Event::MouseWheel {
                            unit: egui::MouseWheelUnit::Point,
                            delta: egui::vec2(0., delta),
                            phase: egui::TouchPhase::Move,
                            modifiers,
                        },
                    ],
                );
                assert!(
                    (app.view.zoom - before) * delta > 0.,
                    "ctrl={ctrl}, delta={delta}"
                );
                assert!((app.view.document(pointer, rect, size) - anchor).length() < 1e-4);
                assert_eq!(ctx.zoom_factor(), 1.);
                let before = app.view.zoom;
                zoom_frame(
                    &mut app,
                    &ctx,
                    vec![
                        Event::PointerMoved(egui::pos2(-10., -10.)),
                        Event::Zoom(1.5),
                    ],
                );
                assert_eq!(app.view.zoom, before);
            }
        }
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open(dir.path().to_path_buf()).unwrap();
        app.doc = parfait_core::Document::new(64, 64);
        let ctx = egui::Context::default();
        zoom_frame(
            &mut app,
            &ctx,
            vec![Event::PointerMoved(egui::pos2(150., 160.))],
        );
        let before = app.view.zoom;
        zoom_frame(&mut app, &ctx, vec![Event::Zoom(1.1)]);
        assert!((app.view.zoom - before * 1.1).abs() < 1e-6);
    }
    #[test]
    fn unused_control_pointer_combinations_keep_panning_and_drawing() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open(dir.path().to_path_buf()).unwrap();
        app.doc = parfait_core::Document::new(64, 64);
        let ctx = egui::Context::default();
        let modifiers = egui::Modifiers {
            ctrl: true,
            command: true,
            ..Default::default()
        };
        let start = egui::pos2(210., 210.);
        let end = start + egui::vec2(12., 8.);
        let button = |pos, button, pressed| Event::PointerButton {
            pos,
            button,
            pressed,
            modifiers,
        };
        zoom_frame(
            &mut app,
            &ctx,
            vec![
                Event::ModifiersChanged(modifiers),
                Event::PointerMoved(start),
            ],
        );
        let before = app.doc.composite();
        let zoom = app.view.zoom;
        zoom_frame(
            &mut app,
            &ctx,
            vec![button(start, PointerButton::Middle, true)],
        );
        zoom_frame(&mut app, &ctx, vec![Event::PointerMoved(end)]);
        zoom_frame(
            &mut app,
            &ctx,
            vec![button(end, PointerButton::Middle, false)],
        );
        assert_eq!(app.view.pan, egui::vec2(12., 8.));
        assert_eq!(app.view.zoom, zoom);
        assert_eq!(app.doc.composite(), before);
        zoom_frame(
            &mut app,
            &ctx,
            vec![
                button(end, PointerButton::Primary, true),
                button(end, PointerButton::Primary, false),
            ],
        );
        assert_ne!(app.doc.composite(), before);
    }
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
        // Press Space after starting a pen stroke: subsequent moves must only pan,
        // even if Space is released before the mouse button.
        frame(&mut app, vec![button(end, true)]);
        let painted = app.doc.composite();
        let space = |pressed| Event::Key {
            key: Key::Space,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: Default::default(),
        };
        frame(&mut app, vec![space(true)]);
        frame(
            &mut app,
            vec![Event::PointerMoved(end + egui::vec2(12., 0.))],
        );
        frame(&mut app, vec![space(false)]);
        frame(
            &mut app,
            vec![Event::PointerMoved(end + egui::vec2(24., 0.))],
        );
        assert_eq!(app.doc.composite(), painted);
        assert!(app.stroke.is_none());
        frame(&mut app, vec![button(end + egui::vec2(24., 0.), false)]);
    }
    #[test]
    fn texture_updates_are_partial_for_strokes_and_full_for_external_edits() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open(dir.path().to_path_buf()).unwrap();
        app.doc = parfait_core::Document::new(128, 128);
        app.doc.layers[0].visible = false; // exercise checkerboard origin in partial updates
        let ctx = egui::Context::default();
        let mut cache = vec![Color32::TRANSPARENT; 128 * 128];
        let mut frame = |app: &mut Parfait, events: Vec<Event>, partial: bool| {
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(400., 400.))),
                    events,
                    ..Default::default()
                },
                |ui| {
                    egui::CentralPanel::default().show(ui, |ui| app.canvas(ui));
                },
            );
            let id = app.texture.as_ref().unwrap().id();
            let updates = out.textures_delta.set.get(&id).expect("canvas upload");
            assert_eq!(updates.len(), 1);
            let delta = &updates[0];
            assert_eq!(delta.pos.is_some(), partial);
            let egui::ImageData::Color(image) = &delta.image;
            let [x, y] = delta.pos.unwrap_or([0, 0]);
            if partial {
                assert!(image.size[0] * image.size[1] < 128 * 128);
            }
            for row in 0..image.size[1] {
                let at = (y + row) * 128 + x;
                cache[at..at + image.size[0]]
                    .copy_from_slice(&image.pixels[row * image.size[0]..(row + 1) * image.size[0]]);
            }
            for (i, p) in app.doc.composite().as_chunks::<4>().0.iter().enumerate() {
                let bg = if (i % 128 / 16 + i / 128 / 16).is_multiple_of(2) {
                    235.
                } else {
                    218.
                };
                let a = p[3] as f32 / 255.;
                let rgb: [u8; 3] =
                    std::array::from_fn(|c| (p[c] as f32 * a + bg * (1. - a)).round() as u8);
                assert_eq!(cache[i], Color32::from_rgb(rgb[0], rgb[1], rgb[2]));
            }
            out.textures_delta.clear();
        };
        frame(&mut app, vec![], false);
        let p = egui::pos2(210., 210.);
        let button = |pressed| Event::PointerButton {
            pos: p,
            button: PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        frame(
            &mut app,
            vec![Event::PointerMoved(p), button(true), button(false)],
            true,
        );
        app.doc.layers[1].opacity = 0.2;
        app.doc.touch();
        frame(&mut app, vec![], false);
        app.edit(|d| d.layers[1].blend_mode = parfait_core::BlendMode::Multiply);
        frame(&mut app, vec![], false);
        app.history.undo(&mut app.doc);
        frame(&mut app, vec![], false);
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
