use crate::{app::Parfait, canvas::ViewZoom, widgets};
use eframe::egui;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DisplaySettings {
    pub rulers: bool,
    pub pixel_grid: bool,
    pub brush_cursor: bool,
    pub background: u8,
    pub rotation_step: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Event, Pos2, Rect, Vec2};

    #[test]
    fn menu_actions_and_settings_footer_work_at_minimum_workspace_size() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open_legacy(dir.path().to_owned()).unwrap();
        let artwork = app.documents.active_session.document.clone();
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let bounds = Rect::from_min_size(Pos2::ZERO, Vec2::new(960., 640.));
        let frame = |app: &mut Parfait, events, menu| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(bounds),
                    events,
                    ..Default::default()
                },
                |ui| {
                    if menu {
                        app.display_menu(ui);
                    }
                    app.display_settings_ui(ui.ctx());
                },
            );
            output.textures_delta.clear();
        };
        let click = |app: &mut Parfait, target: Rect, menu| {
            let position = target.center();
            frame(app, vec![Event::PointerMoved(position)], menu);
            for pressed in [true, false] {
                frame(
                    app,
                    vec![Event::PointerButton {
                        pos: position,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: Default::default(),
                    }],
                    menu,
                );
            }
        };
        frame(&mut app, vec![], true);
        let menu_path =
            egui::Id::new(("catalog_root", crate::menus::TopMenu::View));
        for _ in 0..20 {
            let target = ctx.data(|data| {
                data.get_temp::<Rect>(egui::Id::new((
                    "menu_item",
                    "Display settings…",
                )))
                .unwrap()
            });
            let viewport = ctx.data(|data| {
                data.get_temp::<Rect>(egui::Id::new((
                    "catalog_viewport",
                    menu_path,
                )))
                .unwrap()
            });
            if viewport.contains(target.center()) {
                break;
            }
            frame(
                &mut app,
                vec![
                    Event::PointerMoved(viewport.center()),
                    Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        phase: egui::TouchPhase::Start,
                        delta: Vec2::ZERO,
                        modifiers: Default::default(),
                    },
                    Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        phase: egui::TouchPhase::Move,
                        delta: egui::vec2(0., -240.),
                        modifiers: Default::default(),
                    },
                ],
                true,
            );
        }
        frame(
            &mut app,
            vec![Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                phase: egui::TouchPhase::End,
                delta: Vec2::ZERO,
                modifiers: Default::default(),
            }],
            true,
        );
        let settings = ctx
            .data_mut(|data| {
                data.get_temp::<Rect>(egui::Id::new((
                    "menu_item",
                    "Display settings…",
                )))
            })
            .unwrap();
        click(&mut app, settings, true);
        assert!(app.show_display_settings);
        frame(&mut app, vec![], false);
        let button = |label: &str| {
            ctx.data_mut(|data| {
                data.get_temp::<Rect>(egui::Id::new(("action_button", label)))
            })
            .unwrap()
        };
        assert!(bounds.contains_rect(button("Close")));
        assert!(bounds.contains_rect(button("Restore defaults")));
        app.display.rotation_step = 17.;
        app.display.pixel_grid = true;
        click(&mut app, button("Restore defaults"), false);
        assert_eq!(app.display, Default::default());
        click(&mut app, button("Close"), false);
        assert!(!app.show_display_settings);
        assert_eq!(app.documents.active_session.document, artwork);
    }
}

impl Default for DisplaySettings {
    fn default() -> Self {
        Self {
            rulers: true,
            pixel_grid: false,
            brush_cursor: true,
            background: 44,
            rotation_step: 5.,
        }
    }
}

impl DisplaySettings {
    pub fn validate(self) -> bool {
        (20..=96).contains(&self.background)
            && self.rotation_step.is_finite()
            && (1.0..=90.).contains(&self.rotation_step)
    }
}

#[derive(Clone, Copy)]
pub enum ViewCommand {
    Fit,
    Reset,
    ActualSize,
    DoubleSize,
    RotateLeft,
    RotateRight,
    ResetRotation,
    ResetOrientation,
    FlipHorizontal,
    FlipVertical,
}

impl Parfait {
    pub fn view_command(&mut self, command: ViewCommand) {
        let session = &mut *self.documents.active_session;
        if session.editing.stroke.is_some() {
            return;
        }
        match command {
            ViewCommand::Fit => session.view.request_zoom(ViewZoom::Fit),
            ViewCommand::Reset => session.view = Default::default(),
            ViewCommand::ActualSize => {
                session.view.request_zoom(ViewZoom::Scale(1.))
            }
            ViewCommand::DoubleSize => {
                session.view.request_zoom(ViewZoom::Scale(2.))
            }
            ViewCommand::RotateLeft | ViewCommand::RotateRight => {
                let direction = if matches!(command, ViewCommand::RotateLeft) {
                    -1.
                } else {
                    1.
                };
                session.view.angle = (session.view.angle
                    + direction * self.display.rotation_step.to_radians())
                .rem_euclid(std::f32::consts::TAU);
            }
            ViewCommand::ResetRotation => session.view.angle = 0.,
            ViewCommand::ResetOrientation => {
                session.view.angle = 0.;
                session.view.flip_horizontal = false;
                session.view.flip_vertical = false;
            }
            ViewCommand::FlipHorizontal => {
                session.view.flip_horizontal = !session.view.flip_horizontal
            }
            ViewCommand::FlipVertical => {
                session.view.flip_vertical = !session.view.flip_vertical
            }
        }
    }

    pub fn display_menu(&mut self, ui: &mut egui::Ui) {
        ui.add_enabled_ui(
            self.documents.is_empty()
                || self.documents.active_session.editing.stroke.is_none(),
            |ui| self.expanded_view_menu(ui),
        );
    }

    pub fn display_settings_ui(&mut self, ctx: &egui::Context) {
        if !self.show_display_settings {
            return;
        }
        let mut open = true;
        egui::Window::new("Display settings")
            .open(&mut open)
            .collapsible(false)
            .default_pos(ctx.content_rect().center() - egui::vec2(170., 155.))
            .default_width(340.)
            .min_width(280.)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .max_height((ctx.content_rect().height() - 180.).clamp(100., 400.))
                    .show(ui, |ui| {
                        ui.label("Changes apply immediately and are saved locally.");
                        ui.separator();
                        ui.checkbox(&mut self.display.rulers, "Ruler bar (document pixels)");
                        ui.checkbox(&mut self.display.pixel_grid, "Pixel grid at 800% or higher");
                        ui.checkbox(&mut self.display.brush_cursor, "Brush cursor outline");
                        ui.separator();
                        widgets::numeric_slider(ui, &mut self.display.background, 20..=96, "캔버스 바깥 밝기", 0, "");
                        widgets::numeric_slider(ui, &mut self.display.rotation_step, 1.0..=90., "보기 회전 간격", 1, "°");
                        ui.weak("View changes never transform artwork. Pixel grid is a display guide, not snapping.");
                    });
                ui.separator();
                widgets::action_row(ui, |ui| {
                    if widgets::action_button(ui, "Close", 64.).clicked() {
                        self.show_display_settings = false;
                    }
                    if widgets::action_button(ui, "Restore defaults", 118.).clicked() {
                        self.display = Default::default();
                    }
                });
            });
        self.show_display_settings &=
            open && !ctx.input(|input| input.key_pressed(egui::Key::Escape));
    }
}
