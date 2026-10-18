//! Application-owned window chrome; native viewport commands keep OS window behavior.
use crate::{
    app::Parfait,
    widgets::{self, Icon},
};
use eframe::egui::{self, Color32, Pos2, Rect, ResizeDirection, ViewportCommand};

impl Parfait {
    pub fn title_bar(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        egui::Panel::top("parfait_title")
            .frame(
                egui::Frame::NONE
                    .fill(Color32::from_gray(41))
                    .inner_margin(egui::Margin::symmetric(5, 2)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let (rect, drag) = ui.allocate_exact_size(
                        egui::vec2((ui.available_width() - 90.).max(20.), 25.),
                        egui::Sense::click_and_drag(),
                    );
                    drag.widget_info(|| {
                        egui::WidgetInfo::labeled(
                            egui::WidgetType::Button,
                            true,
                            "Parfait title bar: drag to move, double-click to maximize",
                        )
                    });
                    ui.painter().text(
                        rect.left_center() + egui::vec2(6., 0.),
                        egui::Align2::LEFT_CENTER,
                        "PARFAIT",
                        egui::FontId::proportional(12.),
                        Color32::from_rgb(190, 202, 218),
                    );
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        format!(
                            "{}  —  {} × {} px",
                            self.doc.name, self.doc.width, self.doc.height
                        ),
                        egui::FontId::proportional(11.),
                        Color32::from_gray(170),
                    );
                    let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                    if drag.double_clicked() {
                        ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximized));
                    } else if drag.drag_started() {
                        ctx.send_viewport_cmd(ViewportCommand::StartDrag);
                    }
                    if widgets::icon(ui, Icon::Minus, false, "Minimize window").clicked() {
                        ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
                    }
                    if widgets::icon(
                        ui,
                        if maximized {
                            Icon::Restore
                        } else {
                            Icon::Maximize
                        },
                        false,
                        "Maximize / restore window",
                    )
                    .clicked()
                    {
                        ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximized));
                    }
                    if widgets::icon(ui, Icon::Close, false, "Close Parfait").clicked() {
                        self.request_close(&ctx);
                    }
                });
            });
    }
}
pub fn resize_edges(ui: &mut egui::Ui) {
    if ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false)) {
        return;
    }
    let r = ui.ctx().content_rect();
    let t = 4.;
    let c = 10.;
    let edges = [
        (
            Rect::from_min_max(r.min, Pos2::new(r.left() + c, r.top() + c)),
            ResizeDirection::NorthWest,
            egui::CursorIcon::ResizeNwSe,
        ),
        (
            Rect::from_min_max(
                Pos2::new(r.right() - c, r.top()),
                Pos2::new(r.right(), r.top() + c),
            ),
            ResizeDirection::NorthEast,
            egui::CursorIcon::ResizeNeSw,
        ),
        (
            Rect::from_min_max(
                Pos2::new(r.left(), r.bottom() - c),
                Pos2::new(r.left() + c, r.bottom()),
            ),
            ResizeDirection::SouthWest,
            egui::CursorIcon::ResizeNeSw,
        ),
        (
            Rect::from_min_max(Pos2::new(r.right() - c, r.bottom() - c), r.max),
            ResizeDirection::SouthEast,
            egui::CursorIcon::ResizeNwSe,
        ),
        (
            Rect::from_min_max(
                Pos2::new(r.left() + c, r.top()),
                Pos2::new(r.right() - c, r.top() + t),
            ),
            ResizeDirection::North,
            egui::CursorIcon::ResizeVertical,
        ),
        (
            Rect::from_min_max(
                Pos2::new(r.left() + c, r.bottom() - t),
                Pos2::new(r.right() - c, r.bottom()),
            ),
            ResizeDirection::South,
            egui::CursorIcon::ResizeVertical,
        ),
        (
            Rect::from_min_max(
                Pos2::new(r.left(), r.top() + c),
                Pos2::new(r.left() + t, r.bottom() - c),
            ),
            ResizeDirection::West,
            egui::CursorIcon::ResizeHorizontal,
        ),
        (
            Rect::from_min_max(
                Pos2::new(r.right() - t, r.top() + c),
                Pos2::new(r.right(), r.bottom() - c),
            ),
            ResizeDirection::East,
            egui::CursorIcon::ResizeHorizontal,
        ),
    ];
    for (i, (rect, dir, cursor)) in edges.into_iter().enumerate() {
        let response = ui
            .interact(rect, egui::Id::new(("window_edge", i)), egui::Sense::drag())
            .on_hover_cursor(cursor);
        if response.drag_started() {
            ui.ctx()
                .send_viewport_cmd(ViewportCommand::BeginResize(dir));
        }
    }
}
