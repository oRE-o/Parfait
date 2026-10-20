use crate::app::Parfait;
use eframe::egui;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum LayerEditKind {
    Opacity,
    Name,
}

pub(crate) struct LayerEdit {
    owner: crate::document_session::SessionKey,
    kind: LayerEditKind,
    layer: parfait_core::LayerId,
    pointer: bool,
    focus: Option<egui::Id>,
}
impl Parfait {
    pub(crate) fn finish_stroke(&mut self) -> bool {
        let owner = self.documents.session.editing.stroke_owner.take();
        if self.documents.session.editing.stroke.take().is_none() {
            return false;
        }
        if owner.is_some_and(|owner| owner != self.documents.session.key) {
            return true;
        }
        if self
            .documents
            .session
            .history
            .commit(&self.documents.session.document)
        {
            self.force_save();
        }
        true
    }
    pub(crate) fn cancel_edit(&mut self) {
        self.documents.session.editing.stroke = None;
        self.documents.session.editing.stroke_owner = None;
        self.documents.session.editing.layer_edit = None;
        if self
            .documents
            .session
            .history
            .cancel(&mut self.documents.session.document)
        {
            self.force_save();
        }
    }
    pub(crate) fn finish_layer_edit(&mut self) {
        if self
            .documents
            .session
            .editing
            .layer_edit
            .take()
            .is_some_and(|edit| edit.owner == self.documents.session.key)
        {
            self.documents
                .session
                .history
                .commit(&self.documents.session.document);
        }
    }
    pub(crate) fn begin_layer_edit(
        &mut self,
        kind: LayerEditKind,
        layer: usize,
        pointer: bool,
    ) -> bool {
        let layer_id = self.documents.session.document.layers[layer].id;
        if self.documents.session.editing.layer_edit.as_ref().is_some_and(
            |edit| {
                edit.owner == self.documents.session.key
                    && edit.kind == kind
                    && edit.layer == layer_id
            },
        ) {
            return true;
        }
        self.finish_stroke();
        self.finish_layer_edit();
        if !self.begin_history(match kind {
            LayerEditKind::Opacity => "Layer opacity",
            LayerEditKind::Name => "Rename layer",
        }) {
            return false;
        }
        self.documents.session.editing.layer_edit = Some(LayerEdit {
            owner: self.documents.session.key,
            kind,
            layer: layer_id,
            pointer,
            focus: None,
        });
        true
    }
    pub(crate) fn layer_edit_response(
        &mut self,
        kind: LayerEditKind,
        response: &egui::Response,
    ) {
        let Some(edit) =
            self.documents.session.editing.layer_edit.as_mut().filter(|edit| {
                edit.owner == self.documents.session.key && edit.kind == kind
            })
        else {
            return;
        };
        edit.focus = Some(response.id);
        if response.ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
            self.documents.session.editing.cancelled_layer_drag = edit.pointer
                && response.ctx.input(|input| input.pointer.any_down());
            response.surrender_focus();
            self.cancel_edit();
        } else if (edit.pointer
            && !response.ctx.input(|input| input.pointer.any_down()))
            || (!edit.pointer
                && (!response.has_focus()
                    || response
                        .ctx
                        .input(|input| input.key_pressed(egui::Key::Enter))))
        {
            self.finish_layer_edit();
        }
    }
    pub(crate) fn settle_layer_edit(&mut self, ctx: &egui::Context) {
        if !ctx.input(|input| input.pointer.any_down()) {
            self.documents.session.editing.cancelled_layer_drag = false;
        }
        let finished =
            self.documents.session.editing.layer_edit.as_ref().is_some_and(
                |edit| {
                    if edit.pointer {
                        !ctx.input(|input| input.pointer.any_down())
                    } else {
                        edit.focus.is_some_and(|id| {
                            ctx.memory(|memory| memory.focused()) != Some(id)
                        })
                    }
                },
            );
        if finished {
            self.finish_layer_edit();
        }
    }
    pub(crate) fn select_layer(&mut self, layer: usize) {
        if layer >= self.documents.session.document.layers.len()
            || self.documents.session.document.active == layer
        {
            return;
        }
        self.finish_stroke();
        self.finish_layer_edit();
        let revision = self.documents.session.document.revision;
        self.documents.session.document.active = layer;
        self.documents.session.editing.mask_target = None;
        self.reset_layer_selection_view();
        self.reveal_layer(layer);
        self.documents.session.document.touch();
        if self.render_cache.rendered == revision {
            self.render_cache.rendered =
                self.documents.session.document.revision;
        }
        if self.render_cache.preview_revision == revision {
            self.render_cache.preview_revision =
                self.documents.session.document.revision;
        }
    }
    pub(crate) fn new_layer(&mut self) {
        let name =
            format!("Layer {}", self.documents.session.document.layers.len());
        if let Some(reason) = parfait_storage::layer_growth_error(
            &self.documents.session.document,
            name.len(),
        ) {
            self.notice = Some(reason.into());
            return;
        }
        self.edit_named("New layer", |document| document.add_layer());
        self.reveal_layer(self.documents.session.document.active);
    }
    pub(crate) fn select_history(&mut self, id: u64) {
        if self.documents.session.editing.stroke.is_some() {
            return;
        }
        self.finish_layer_edit();
        if self
            .documents
            .session
            .history
            .restore(&mut self.documents.session.document, id)
        {
            self.documents.session.layer_ui.list = Default::default();
            self.force_save();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layers_frame(
        app: &mut Parfait,
        ctx: &egui::Context,
        events: Vec<egui::Event>,
    ) {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(320., 600.),
                )),
                events,
                ..Default::default()
            },
            |ui| {
                app.shortcuts(ui.ctx());
                app.palette(ui, crate::workspace::Palette::Layers, 400.);
                app.settle_layer_edit(ui.ctx());
            },
        );
        output.textures_delta.clear();
    }
    fn click(app: &mut Parfait, ctx: &egui::Context, pos: egui::Pos2) {
        for pressed in [true, false] {
            layers_frame(
                app,
                ctx,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        pressed,
                        button: egui::PointerButton::Primary,
                        modifiers: Default::default(),
                    },
                ],
            );
        }
    }
    fn key(key: egui::Key, modifiers: egui::Modifiers) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }
    }
    fn replace(app: &mut Parfait, ctx: &egui::Context, text: &str) {
        layers_frame(
            app,
            ctx,
            vec![
                key(
                    egui::Key::A,
                    egui::Modifiers { command: true, ..Default::default() },
                ),
                egui::Event::Text(text.into()),
            ],
        );
    }

    #[test]
    fn rename_focus_session_commits_once_and_escape_retains_future() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open(dir.path().to_owned()).unwrap();
        app.documents.session.document = parfait_core::Document::new(4, 4);
        app.new_layer();
        app.undo();
        let original = app.documents.session.document.layers[1].name.clone();
        app.begin_layer_rename(1);
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        layers_frame(&mut app, &ctx, vec![]);
        layers_frame(&mut app, &ctx, vec![]);
        let name = ctx
            .data(|data| {
                data.get_temp::<egui::Rect>(egui::Id::new("layer_name"))
            })
            .unwrap();
        click(&mut app, &ctx, name.center());
        replace(&mut app, &ctx, "Draft");
        layers_frame(&mut app, &ctx, vec![egui::Event::Text("!".into())]);
        assert_eq!(app.documents.session.document.layers[1].name, "Draft!");
        assert_eq!(app.documents.session.history.rows().count(), 2);
        assert!(app.documents.session.history.can_redo());
        layers_frame(
            &mut app,
            &ctx,
            vec![key(egui::Key::Escape, Default::default())],
        );
        assert_eq!(app.documents.session.document.layers[1].name, original);
        assert!(app.documents.session.editing.layer_edit.is_none());
        assert!(app.documents.session.history.can_redo());
        app.begin_layer_rename(1);
        layers_frame(&mut app, &ctx, vec![]);
        click(&mut app, &ctx, name.center());
        replace(&mut app, &ctx, "Done");
        layers_frame(
            &mut app,
            &ctx,
            vec![key(egui::Key::Enter, Default::default())],
        );
        assert!(app.documents.session.editing.layer_edit.is_none());
        assert_eq!(app.documents.session.history.rows().count(), 2);
        assert!(!app.documents.session.history.can_redo());
        assert_eq!(
            app.documents.session.history.rows().last().unwrap().label,
            "Rename layer"
        );
        app.undo();
        assert_eq!(app.documents.session.document.layers[1].name, original);
        app.redo();
        assert_eq!(app.documents.session.document.layers[1].name, "Done");
    }

    #[test]
    fn opacity_numeric_percent_input_and_keyboard_session_are_undoable() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open(dir.path().to_owned()).unwrap();
        app.documents.session.document = parfait_core::Document::new(4, 4);
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        layers_frame(&mut app, &ctx, vec![]);
        layers_frame(&mut app, &ctx, vec![]);
        let (opacity, _) = ctx
            .data(|data| {
                data.get_temp::<(egui::Rect, f32)>(egui::Id::new(
                    "layer_opacity",
                ))
            })
            .unwrap();
        click(&mut app, &ctx, opacity.right_center() - egui::vec2(16., 0.));
        replace(&mut app, &ctx, "42");
        layers_frame(
            &mut app,
            &ctx,
            vec![key(egui::Key::Enter, Default::default())],
        );
        assert!(
            (app.documents.session.document.layers[1].opacity - 0.42).abs()
                < 0.001
        );
        assert_eq!(app.documents.session.history.rows().count(), 2);
        app.undo();
        assert_eq!(app.documents.session.document.layers[1].opacity, 1.);
        app.redo();
        assert!(
            (app.documents.session.document.layers[1].opacity - 0.42).abs()
                < 0.001
        );
        layers_frame(&mut app, &ctx, vec![]);
        let (rect, width) = ctx
            .data(|data| {
                data.get_temp::<(egui::Rect, f32)>(egui::Id::new(
                    "layer_opacity",
                ))
            })
            .unwrap();
        click(
            &mut app,
            &ctx,
            egui::pos2(rect.left() + width - 2., rect.center().y),
        );
        let before = app.documents.session.document.layers[1].opacity;
        let focus = ctx
            .data(|data| {
                data.get_temp::<egui::Id>(egui::Id::new("layer_opacity_focus"))
            })
            .unwrap();
        ctx.memory_mut(|memory| memory.request_focus(focus));
        layers_frame(&mut app, &ctx, vec![]);
        let count = app.documents.session.history.rows().count();
        for _ in 0..3 {
            layers_frame(
                &mut app,
                &ctx,
                vec![key(egui::Key::ArrowLeft, Default::default())],
            );
        }
        assert!(app.documents.session.document.layers[1].opacity < before);
        assert!(app.documents.session.editing.layer_edit.is_some());
        assert_eq!(app.documents.session.history.rows().count(), count);
        let name = ctx
            .data(|data| {
                data.get_temp::<egui::Rect>(egui::Id::new((
                    "layer_label",
                    1usize,
                )))
            })
            .unwrap();
        click(&mut app, &ctx, name.center());
        assert!(app.documents.session.editing.layer_edit.is_none());
        assert_eq!(app.documents.session.history.rows().count(), count + 1);
        app.undo();
        assert_eq!(app.documents.session.document.layers[1].opacity, before);
    }

    #[test]
    fn escape_cancels_opacity_drag_until_release_without_erasing_future() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open(dir.path().to_owned()).unwrap();
        app.documents.session.document = parfait_core::Document::new(4, 4);
        app.new_layer();
        app.undo();
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        layers_frame(&mut app, &ctx, vec![]);
        layers_frame(&mut app, &ctx, vec![]);
        let (rect, width) = ctx
            .data(|data| {
                data.get_temp::<(egui::Rect, f32)>(egui::Id::new(
                    "layer_opacity",
                ))
            })
            .unwrap();
        let start = egui::pos2(rect.left() + width - 2., rect.center().y);
        let end = start - egui::vec2(width * 0.5, 0.);
        let button = |pressed| egui::Event::PointerButton {
            pos: if pressed { start } else { end },
            pressed,
            button: egui::PointerButton::Primary,
            modifiers: Default::default(),
        };
        layers_frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(start), button(true)],
        );
        layers_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        assert!(app.documents.session.document.layers[1].opacity < 0.8);
        layers_frame(
            &mut app,
            &ctx,
            vec![key(egui::Key::Escape, Default::default())],
        );
        assert_eq!(app.documents.session.document.layers[1].opacity, 1.);
        layers_frame(&mut app, &ctx, vec![button(false)]);
        assert_eq!(app.documents.session.document.layers[1].opacity, 1.);
        assert!(app.documents.session.editing.layer_edit.is_none());
        assert!(!app.documents.session.editing.cancelled_layer_drag);
        assert!(app.documents.session.history.can_redo());
        app.redo();
        assert_eq!(app.documents.session.document.layers.len(), 3);
    }

    #[test]
    fn growth_budget_blocks_new_and_duplicate_before_mutating_large_layout() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open(dir.path().to_owned()).unwrap();
        app.documents.session.document = parfait_core::Document::new(1, 1);
        while app.documents.session.document.layers.len() < 7 {
            app.documents.session.document.add_layer();
        }
        app.documents.session.document.width = 4096;
        app.documents.session.document.height = 4096;
        let original = app.documents.session.document.clone();
        app.new_layer();
        app.duplicate_layer();
        assert_eq!(app.documents.session.document, original);
        assert_eq!(app.documents.session.history.rows().count(), 0);
        assert!(app.notice.as_ref().unwrap().contains("512 MiB"));
    }
}
