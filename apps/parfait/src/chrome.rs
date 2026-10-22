//! Application-owned window chrome; native viewport commands keep OS window behavior.
use crate::{
    app::Parfait,
    widgets::{self, Icon},
};
use eframe::egui::{
    self, Color32, Pos2, Rect, ResizeDirection, ViewportCommand,
};

impl Parfait {
    pub(crate) fn window_drag(
        &self,
        ui: &mut egui::Ui,
        width: f32,
        logo: bool,
    ) {
        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(width.max(1.), 24.),
            egui::Sense::click_and_drag(),
        );
        if logo {
            let c = rect.center();
            ui.painter().circle_filled(c, 9., Color32::from_rgb(153, 170, 194));
            ui.painter().text(
                c,
                egui::Align2::CENTER_CENTER,
                "P",
                egui::FontId::proportional(13.),
                Color32::from_gray(34),
            );
        } else if rect.width() > 170. && !self.documents.is_empty() {
            ui.painter().with_clip_rect(rect).text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                format!(
                    "{}{}",
                    self.documents.active_session.document.name,
                    if self.project_is_modified() { " *" } else { "" }
                ),
                egui::FontId::proportional(11.),
                Color32::from_gray(165),
            );
        }
        response.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::WidgetType::Button,
                true,
                "창 이동 · 두 번 클릭하여 최대화 / 복원",
            )
        });
        if response.double_clicked() {
            let maximized =
                ui.input(|i| i.viewport().maximized.unwrap_or(false));
            ui.ctx().send_viewport_cmd(ViewportCommand::Maximized(!maximized));
        } else if response.is_pointer_button_down_on()
            && ui.input(|i| i.pointer.delta() != egui::Vec2::ZERO)
        {
            ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
        }
    }
    pub(crate) fn window_controls(&mut self, ui: &mut egui::Ui) {
        let maximized = ui.input(|i| i.viewport().maximized.unwrap_or(false));
        if widgets::icon(ui, Icon::Close, false, "Parfait 닫기").clicked() {
            self.request_close(ui.ctx());
        }
        if widgets::icon(
            ui,
            if maximized { Icon::Restore } else { Icon::Maximize },
            false,
            "최대화 / 복원",
        )
        .clicked()
        {
            ui.ctx().send_viewport_cmd(ViewportCommand::Maximized(!maximized));
        }
        if widgets::icon(ui, Icon::Minus, false, "최소화").clicked() {
            ui.ctx().send_viewport_cmd(ViewportCommand::Minimized(true));
        }
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
            .interact(
                rect,
                egui::Id::new(("window_edge", i)),
                egui::Sense::drag(),
            )
            .on_hover_cursor(cursor);
        if response.drag_started() {
            ui.ctx().send_viewport_cmd(ViewportCommand::BeginResize(dir));
        }
    }
}
