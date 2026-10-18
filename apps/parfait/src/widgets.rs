//! Compact palette controls with bundled Font Awesome Free icons shared by the desktop palettes.
use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2};

#[derive(Clone, Copy)]
pub enum Icon {
    Pen,
    Eraser,
    Hand,
    Picker,
    Plus,
    Minus,
    Fit,
    Undo,
    Redo,
    Save,
    Folder,
    Copy,
    Trash,
    Up,
    Down,
    Close,
    Maximize,
    Restore,
}

pub fn icon(ui: &mut egui::Ui, kind: Icon, selected: bool, label: &str) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(25.), Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label)
    });
    if selected || response.hovered() {
        ui.painter().rect_filled(
            rect,
            0.,
            if selected {
                ui.visuals().selection.bg_fill
            } else {
                Color32::from_gray(83)
            },
        );
    }
    let glyph = match kind {
        Icon::Pen => "\u{f5ad}",
        Icon::Eraser => "\u{f12d}",
        Icon::Hand => "\u{f256}",
        Icon::Picker => "\u{f1fb}",
        Icon::Plus => "\u{002b}",
        Icon::Minus => "\u{f068}",
        Icon::Fit => "\u{f065}",
        Icon::Undo => "\u{f2ea}",
        Icon::Redo => "\u{f2f9}",
        Icon::Save => "\u{f0c7}",
        Icon::Folder => "\u{f07c}",
        Icon::Copy => "\u{f0c5}",
        Icon::Trash => "\u{f2ed}",
        Icon::Up => "\u{f062}",
        Icon::Down => "\u{f063}",
        Icon::Close => "\u{f00d}",
        Icon::Maximize => "\u{f2d0}",
        Icon::Restore => "\u{f2d2}",
    };
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        glyph,
        egui::FontId::new(14., egui::FontFamily::Name("icons".into())),
        if ui.is_enabled() {
            Color32::from_gray(203)
        } else {
            Color32::from_gray(100)
        },
    );
    response.on_hover_text(label)
}

pub fn heading(ui: &mut egui::Ui, title: &str) {
    let (r, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 21.), Sense::hover());
    ui.painter().rect_filled(r, 0., Color32::from_gray(51));
    ui.painter().text(
        r.left_center() + egui::vec2(6., 0.),
        egui::Align2::LEFT_CENTER,
        format!("≡  {title}"),
        egui::FontId::proportional(11.),
        Color32::from_gray(196),
    );
}

pub fn brush_row(ui: &mut egui::Ui, name: &str, selected: bool, soft: bool) -> bool {
    let (r, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 33.), Sense::click());
    response
        .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::SelectableLabel, true, name));
    let p = ui.painter();
    p.rect_filled(
        r,
        0.,
        if selected {
            ui.visuals().selection.bg_fill
        } else if response.hovered() {
            Color32::from_gray(81)
        } else {
            Color32::from_gray(67)
        },
    );
    let w = (r.width() * 0.46).min(104.);
    let points: Vec<Pos2> = (0..40)
        .map(|i| {
            let t = i as f32 / 39.;
            r.left_center() + egui::vec2(7. + t * w, (t * std::f32::consts::TAU).sin() * 4.)
        })
        .collect();
    if soft {
        p.add(egui::Shape::line(
            points.clone(),
            Stroke::new(7., Color32::from_white_alpha(25)),
        ));
    }
    p.add(egui::Shape::line(
        points,
        Stroke::new(if soft { 3. } else { 1.8 }, Color32::from_gray(220)),
    ));
    p.text(
        r.right_center() - egui::vec2(6., 0.),
        egui::Align2::RIGHT_CENTER,
        name,
        egui::FontId::proportional(11.),
        Color32::from_gray(225),
    );
    response.clicked()
}

pub fn color_wheel(ui: &mut egui::Ui, color: &mut [u8; 3]) {
    let side = ui.available_width().min(204.);
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(side), Sense::click_and_drag());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Other,
            true,
            "Color wheel: hue ring and saturation/value square",
        )
    });
    let c = rect.center();
    let outer = side / 2. - 4.;
    let inner = outer - 14.;
    let mut hsv = egui::ecolor::Hsva::from(Color32::from_rgb(color[0], color[1], color[2]));
    let hue_id = ui.id().with("wheel_hue");
    if hsv.s < 0.001 || hsv.v < 0.001 {
        hsv.h = ui.data(|d| d.get_temp::<f32>(hue_id)).unwrap_or(hsv.h);
    }
    let sq = Rect::from_center_size(c, Vec2::splat(inner * 1.32));
    if (response.dragged() || response.clicked())
        && let Some(pos) = response.interact_pointer_pos()
    {
        let origin = ui.input(|i| i.pointer.press_origin()).unwrap_or(pos);
        if sq.contains(origin) {
            hsv.s = ((pos.x - sq.left()) / sq.width()).clamp(0., 1.);
            hsv.v = (1. - (pos.y - sq.top()) / sq.height()).clamp(0., 1.);
        } else if origin.distance(c) >= inner - 3. && origin.distance(c) <= outer + 4. {
            hsv.h = ((pos - c).angle() / std::f32::consts::TAU).rem_euclid(1.);
        }
        let rgb = Color32::from(hsv);
        *color = [rgb.r(), rgb.g(), rgb.b()];
    }
    ui.data_mut(|d| d.insert_temp(hue_id, hsv.h));
    let mut mesh = egui::Mesh::default();
    for i in 0..=96 {
        let h = i as f32 / 96.;
        let d = Vec2::angled(h * std::f32::consts::TAU);
        let col = Color32::from(egui::ecolor::Hsva::new(h, 1., 1., 1.));
        mesh.colored_vertex(c + d * inner, col);
        mesh.colored_vertex(c + d * outer, col);
        if i < 96 {
            let n = i * 2;
            mesh.add_triangle(n, n + 1, n + 2);
            mesh.add_triangle(n + 1, n + 3, n + 2);
        }
    }
    ui.painter().add(egui::Shape::mesh(mesh));
    let mut square = egui::Mesh::default();
    for y in 0..=16 {
        for x in 0..=16 {
            let s = x as f32 / 16.;
            let v = 1. - y as f32 / 16.;
            square.colored_vertex(
                egui::pos2(
                    sq.left() + s * sq.width(),
                    sq.top() + (1. - v) * sq.height(),
                ),
                Color32::from(egui::ecolor::Hsva::new(hsv.h, s, v, 1.)),
            );
            if x < 16 && y < 16 {
                let n = y * 17 + x;
                square.add_triangle(n, n + 1, n + 17);
                square.add_triangle(n + 1, n + 18, n + 17);
            }
        }
    }
    ui.painter().add(egui::Shape::mesh(square));
    for point in [
        c + Vec2::angled(hsv.h * std::f32::consts::TAU) * (inner + 7.),
        egui::pos2(
            sq.left() + hsv.s * sq.width(),
            sq.bottom() - hsv.v * sq.height(),
        ),
    ] {
        ui.painter()
            .circle_stroke(point, 4., Stroke::new(2., Color32::BLACK));
        ui.painter()
            .circle_stroke(point, 4., Stroke::new(1., Color32::WHITE));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hue_selection_survives_black_until_value_is_raised() {
        let ctx = egui::Context::default();
        let mut color = [0, 0, 0];
        let mut origin = Pos2::ZERO;
        let mut frame = |color: &mut [u8; 3], events| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::splat(300.))),
                    events,
                    ..Default::default()
                },
                |ui| {
                    egui::CentralPanel::default().show(ui, |ui| {
                        ui.set_width(204.);
                        origin = ui.cursor().min;
                        color_wheel(ui, color);
                    });
                },
            );
            output.textures_delta.clear();
            origin
        };
        let origin = frame(&mut color, vec![]);
        let center = origin + Vec2::splat(102.);
        let click = |p| {
            vec![
                egui::Event::PointerMoved(p),
                egui::Event::PointerButton {
                    pos: p,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
                egui::Event::PointerButton {
                    pos: p,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: Default::default(),
                },
            ]
        };
        // Select green on the hue ring while value is zero, then brighten the SV square.
        let ring = center + Vec2::angled(std::f32::consts::TAU / 3.) * 91.;
        frame(&mut color, vec![egui::Event::PointerMoved(ring)]);
        frame(&mut color, click(ring));
        assert_eq!(color, [0, 0, 0]);
        let square = center + egui::vec2(50., -50.);
        frame(&mut color, vec![egui::Event::PointerMoved(square)]);
        frame(&mut color, click(square));
        assert!(
            color[1] > 200
                && color[1] as u16 > 3 * color[0] as u16
                && color[1] as u16 > 3 * color[2] as u16,
            "{color:?}"
        );
    }
}
