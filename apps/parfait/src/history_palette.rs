use crate::{
    app::Parfait,
    shortcuts::Command,
    widgets::{self, Icon},
};
use eframe::egui;

impl Parfait {
    pub(crate) fn history_palette(
        &mut self,
        ui: &mut egui::Ui,
        body_height: f32,
    ) {
        ui.horizontal(|ui| {
            let undo = ui.add_enabled_ui(self.stroke.is_none() && self.history.can_undo(), |ui| {
                widgets::flat_icon(ui, Icon::Undo, false, self.keymap.tooltip(Command::Undo), 24.)
            }).inner;
            if undo.clicked() { self.execute_command(Command::Undo, ui.ctx()); }
            let redo = ui.add_enabled_ui(self.stroke.is_none() && self.history.can_redo(), |ui| {
                widgets::flat_icon(ui, Icon::Redo, false, self.keymap.tooltip(Command::Redo), 24.)
            }).inner;
            if redo.clicked() { self.execute_command(Command::Redo, ui.ctx()); }
            ui.add(egui::Label::new("Session history").truncate()).on_hover_text("Up to 30 edits under a conservative 128 MiB logical history budget. History is not saved with the drawing.");
        });
        ui.separator();
        let mut selected = None;
        ui.add_enabled_ui(self.stroke.is_none(), |ui| {
            egui::ScrollArea::vertical()
                .id_salt("history_rows")
                .max_height((body_height - 36.).max(24.))
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let mut has_rows = false;
                    for row in self.history.rows() {
                        has_rows = true;
                        let text = egui::RichText::new(row.label).color(
                            if row.future {
                                ui.visuals().weak_text_color()
                            } else {
                                ui.visuals().text_color()
                            },
                        );
                        let response = ui
                            .push_id(row.id, |ui| {
                                ui.add_sized(
                                    [ui.available_width(), 22.],
                                    egui::Button::selectable(row.current, "")
                                        .left_text(text)
                                        .truncate(),
                                )
                            })
                            .inner
                            .on_hover_text(row.label);
                        #[cfg(test)]
                        ui.data_mut(|data| {
                            data.insert_temp(
                                egui::Id::new(("history_row", row.id)),
                                response.rect,
                            )
                        });
                        if response.clicked() {
                            selected = Some(row.id);
                        }
                    }
                    if !has_rows {
                        ui.add_sized(
                            [ui.available_width(), 22.],
                            egui::Button::selectable(true, "")
                                .left_text("Initial state")
                                .truncate(),
                        );
                    }
                });
        });
        if let Some(id) = selected {
            self.select_history(id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_and_commands_restore_one_cursor_and_preserve_future_until_new_edit()
    {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open(dir.path().to_owned()).unwrap();
        app.doc = parfait_core::Document::new(4, 4);
        for label in ["A", "B"] {
            app.edit_named(label, |document| document.add_layer());
        }
        let ids: Vec<_> = app.history.rows().map(|row| row.id).collect();
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let frame = |app: &mut Parfait, events| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(960., 640.),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| {
                    ui.set_width(180.);
                    app.history_palette(ui, 190.);
                },
            );
            output.textures_delta.clear();
        };
        frame(&mut app, vec![]);
        frame(&mut app, vec![]);
        app.execute_command(Command::Undo, &ctx);
        assert!(app.history.rows().last().unwrap().future);
        let click = |app: &mut Parfait, id| {
            let rect = ctx
                .data(|data| {
                    data.get_temp::<egui::Rect>(egui::Id::new((
                        "history_row",
                        id,
                    )))
                })
                .unwrap();
            assert!(rect.right() <= 180.1);
            let event = |pressed| egui::Event::PointerButton {
                pos: rect.center(),
                pressed,
                button: egui::PointerButton::Primary,
                modifiers: Default::default(),
            };
            frame(
                app,
                vec![egui::Event::PointerMoved(rect.center()), event(true)],
            );
            frame(app, vec![event(false)]);
        };
        click(&mut app, ids[2]);
        assert_eq!(app.doc.layers.len(), 4);
        click(&mut app, ids[1]);
        app.execute_command(Command::Undo, &ctx);
        assert_eq!(app.doc.layers.len(), 2);
        assert!(app.history.rows().next().unwrap().current);
        app.execute_command(Command::Redo, &ctx);
        app.edit_named("C", |document| document.layers[1].locked = true);
        assert_eq!(app.history.rows().last().unwrap().label, "C");
        assert!(!app.history.can_redo());
        assert!(app.doc.validate());
    }
}
