//! Compact palette controls with bundled Font Awesome Free icons shared by the desktop palettes.
use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2};

#[derive(Clone, Copy)]
pub enum Icon {
    Grip,
    Pen,
    Pencil,
    Brush,
    Airbrush,
    Eye,
    EyeSlash,
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
    Lock,
    Unlock,
    Check,
    Close,
    Maximize,
    Restore,
    Search,
    Favorite,
    More,
    Material,
    Settings,
    Blend,
    Transfer,
    Merge,
    AlphaLock,
    Clipping,
}
impl Icon {
    pub fn glyph(self) -> &'static str {
        match self {
            Icon::Grip => "\u{f58e}",
            Icon::Pen => "\u{f5ad}",
            Icon::Pencil => "\u{f303}",
            Icon::Brush => "\u{f55d}",
            Icon::Airbrush => "\u{f5bd}",
            Icon::Eye => "\u{f06e}",
            Icon::EyeSlash => "\u{f070}",
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
            Icon::Lock => "\u{f023}",
            Icon::Unlock => "\u{f09c}",
            Icon::Check => "\u{f00c}",
            Icon::Close => "\u{f00d}",
            Icon::Maximize => "\u{f2d0}",
            Icon::Restore => "\u{f2d2}",
            Icon::Search => "\u{f002}",
            Icon::Favorite => "\u{f005}",
            Icon::More => "\u{f141}",
            Icon::Material => "\u{f03e}",
            Icon::Settings => "\u{f0ad}",
            Icon::Blend => "\u{f043}",
            Icon::Transfer | Icon::Merge | Icon::AlphaLock | Icon::Clipping => "",
        }
    }
}

pub fn action_button(ui: &mut egui::Ui, label: &str, width: f32) -> egui::Response {
    let response = ui.add_sized([width, 24.], egui::Button::new(label));
    #[cfg(test)]
    ui.data_mut(|data| data.insert_temp(egui::Id::new(("action_button", label)), response.rect));
    response
}

pub fn action_row<Result>(
    ui: &mut egui::Ui,
    contents: impl FnOnce(&mut egui::Ui) -> Result,
) -> egui::InnerResponse<Result> {
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 6.;
            ui.spacing_mut().interact_size.y = 24.;
            contents(ui)
        })
        .inner
    })
}

pub fn menu_item(ui: &mut egui::Ui, label: &str, shortcut: &str) -> egui::Response {
    let response = ui.add(egui::Button::new(label).shortcut_text(shortcut));
    #[cfg(test)]
    ui.data_mut(|data| data.insert_temp(egui::Id::new(("menu_item", label)), response.rect));
    response
}

pub fn menu_toggle(
    ui: &mut egui::Ui,
    label: &str,
    shortcut: &str,
    selected: bool,
) -> egui::Response {
    let check = egui::RichText::new(if selected { "\u{f14a}" } else { "\u{f0c8}" })
        .family(egui::FontFamily::Name("icons".into()));
    let response = ui.add(egui::Button::new((check, label)).shortcut_text(shortcut));
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Checkbox, ui.is_enabled(), selected, label)
    });
    response
}

pub fn icon(ui: &mut egui::Ui, kind: Icon, selected: bool, label: &str) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(25.), Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label)
    });
    let visuals = ui.style().interact_selectable(&response, selected);
    ui.painter()
        .rect_filled(rect.shrink(1.), 0., visuals.weak_bg_fill);
    ui.painter().rect_stroke(
        rect.shrink(1.),
        0.,
        visuals.bg_stroke,
        egui::StrokeKind::Inside,
    );
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        kind.glyph(),
        egui::FontId::new(14., egui::FontFamily::Name("icons".into())),
        if ui.is_enabled() {
            Color32::from_gray(203)
        } else {
            Color32::from_gray(100)
        },
    );
    response.on_hover_text(label)
}

pub(crate) fn paint_flat_icon(ui: &egui::Ui, rect: Rect, kind: Icon, selected: bool, side: f32) {
    let color = if !ui.is_enabled() {
        Color32::from_gray(100)
    } else if selected {
        Color32::from_gray(239)
    } else {
        Color32::from_gray(196)
    };
    if matches!(kind, Icon::Clipping) {
        let center = rect.center();
        let stroke = Stroke::new(1.4, color);
        ui.painter().rect_stroke(
            Rect::from_center_size(center + egui::vec2(2., -3.), egui::vec2(10., 8.)),
            1.,
            stroke,
            egui::StrokeKind::Inside,
        );
        ui.painter().line_segment(
            [center + egui::vec2(-7., -1.), center + egui::vec2(-7., 6.)],
            stroke,
        );
        ui.painter().line_segment(
            [center + egui::vec2(-7., 6.), center + egui::vec2(6., 6.)],
            stroke,
        );
        ui.painter().line_segment(
            [center + egui::vec2(3., 3.), center + egui::vec2(6., 6.)],
            stroke,
        );
        return;
    }
    if matches!(kind, Icon::AlphaLock) {
        let origin = rect.center() - egui::vec2(8., 8.);
        for row in 0..4 {
            for column in 0..4 {
                if (row + column) % 2 == 0 {
                    ui.painter().rect_filled(
                        Rect::from_min_size(
                            origin + egui::vec2(column as f32 * 3., row as f32 * 3.),
                            Vec2::splat(3.),
                        ),
                        0.,
                        color.gamma_multiply(0.65),
                    );
                }
            }
        }
        let lock = Rect::from_min_size(rect.center() + egui::vec2(-1., -1.), egui::vec2(8., 8.));
        ui.painter().rect_filled(lock, 1., color);
        ui.painter().rect_stroke(
            lock.translate(egui::vec2(0., -4.))
                .shrink2(egui::vec2(1., 0.)),
            3.,
            Stroke::new(1.3, color),
            egui::StrokeKind::Inside,
        );
        return;
    }
    if matches!(kind, Icon::Transfer | Icon::Merge) {
        let point = |column: f32, row: f32| rect.min + egui::vec2(column, row) * (side / 22.);
        let stroke = Stroke::new(1.2, color);
        for (left, top) in [(4., 3.), (2., 7.)] {
            ui.painter().rect_stroke(
                Rect::from_min_max(point(left, top), point(left + 9., top + 11.)),
                1.,
                stroke,
                egui::StrokeKind::Inside,
            );
        }
        ui.painter()
            .line_segment([point(17., 4.), point(17., 17.)], stroke);
        ui.painter()
            .line_segment([point(14., 14.), point(17., 17.)], stroke);
        ui.painter()
            .line_segment([point(17., 17.), point(20., 14.)], stroke);
        if matches!(kind, Icon::Merge) {
            ui.painter()
                .line_segment([point(13., 20.), point(21., 20.)], stroke);
        }
        return;
    }
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        kind.glyph(),
        egui::FontId::new(
            (side * 0.6).max(12.),
            egui::FontFamily::Name("icons".into()),
        ),
        color,
    );
}

pub fn flat_icon(
    ui: &mut egui::Ui,
    kind: Icon,
    selected: bool,
    label: &str,
    side: f32,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(side), Sense::click());
    #[cfg(test)]
    ui.data_mut(|data| data.insert_temp(egui::Id::new(("flat_icon", label)), rect));
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Button, ui.is_enabled(), selected, label)
    });
    let highlighted =
        response.hovered() || response.has_focus() || response.is_pointer_button_down_on();
    if selected || highlighted {
        let visuals = ui.style().interact_selectable(&response, selected);
        ui.painter().rect_filled(
            rect.shrink(1.),
            0.,
            if selected {
                ui.visuals().selection.bg_fill
            } else {
                visuals.weak_bg_fill
            },
        );
        ui.painter().rect_stroke(
            rect.shrink(1.),
            0.,
            if response.has_focus() {
                ui.visuals().selection.stroke
            } else {
                visuals.bg_stroke
            },
            egui::StrokeKind::Inside,
        );
    }
    paint_flat_icon(ui, rect, kind, selected, side);
    response.on_hover_text(label)
}

pub fn brush_row(
    ui: &mut egui::Ui,
    r: Rect,
    name: &str,
    selected: bool,
    kind: Icon,
    preview: Option<egui::TextureId>,
) -> egui::Response {
    let target = Rect::from_min_max(r.min, r.max - egui::vec2(24., 0.));
    let response = ui.interact(target, ui.id().with("preset"), Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::SelectableLabel,
            ui.is_enabled(),
            selected,
            name,
        )
    });
    let painter = ui.painter().with_clip_rect(ui.clip_rect().intersect(r));
    painter.rect_filled(
        r,
        0.,
        if selected {
            ui.visuals().selection.bg_fill
        } else if response.hovered() || response.has_focus() {
            Color32::from_gray(76)
        } else {
            Color32::from_gray(61)
        },
    );
    if let Some(texture) = preview {
        painter.image(
            texture,
            Rect::from_min_max(
                r.min + egui::vec2(22., 2.),
                target.max - egui::vec2(4., 12.),
            ),
            Rect::from_min_max(Pos2::ZERO, egui::pos2(1., 1.)),
            Color32::WHITE,
        );
    }
    painter.text(
        r.min + egui::vec2(12., 13.),
        egui::Align2::CENTER_CENTER,
        kind.glyph(),
        egui::FontId::new(13., egui::FontFamily::Name("icons".into())),
        Color32::from_gray(190),
    );
    let mut job = egui::text::LayoutJob::simple_singleline(
        name.to_owned(),
        egui::FontId::proportional(11.),
        Color32::from_gray(233),
    );
    job.wrap = egui::text::TextWrapping::truncate_at_width((target.width() - 28.).max(1.));
    let galley = painter.layout_job(job);
    painter.galley(
        target.right_bottom() - egui::vec2(5., 2.) - galley.size(),
        galley,
        Color32::from_gray(233),
    );
    painter.line_segment(
        [r.left_bottom(), r.right_bottom()],
        Stroke::new(1., Color32::from_gray(46)),
    );
    if selected || response.has_focus() {
        painter.rect_stroke(
            r,
            0.,
            Stroke::new(1., ui.visuals().selection.stroke.color),
            egui::StrokeKind::Inside,
        );
    }
    response.on_hover_text(name)
}

#[cfg(test)]
pub fn color_wheel(ui: &mut egui::Ui, color: &mut [u8; 3]) {
    color_wheel_sized(ui, color, 204.);
}

pub fn color_wheel_sized(ui: &mut egui::Ui, color: &mut [u8; 3], max_side: f32) {
    let side = ui.available_width().min(max_side).clamp(80., 204.);
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
    fn action_rows_align_equal_height_buttons_to_the_right_without_wrapping() {
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let mut targets = Vec::new();
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(960., 640.))),
                ..Default::default()
            },
            |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    ui.scope_builder(
                        egui::UiBuilder::new()
                            .max_rect(Rect::from_min_size(ui.cursor().min, egui::vec2(380., 100.))),
                        |ui| {
                            ui.set_width(380.);
                            action_row(ui, |ui| {
                                for (name, width) in
                                    [("Close", 56.), ("Save preset", 82.), ("Restore saved", 96.)]
                                {
                                    targets.push(action_button(ui, name, width).rect);
                                }
                            });
                        },
                    );
                });
            },
        );
        output.textures_delta.clear();
        assert_eq!(targets[0].right(), 388.);
        assert!(
            targets
                .iter()
                .all(|rect| rect.height() == 24. && rect.center().y == targets[0].center().y)
        );
        assert!(
            targets
                .windows(2)
                .all(|pair| pair[1].right() + 6. <= pair[0].left())
        );
        let style = ctx.style_of(egui::Theme::Dark);
        assert!(!style.spacing.scroll.floating);
        assert!(!style.spacing.scroll.foreground_color);
    }

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
