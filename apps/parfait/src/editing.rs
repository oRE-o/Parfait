use crate::app::Parfait;
use eframe::egui;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum LayerEditKind {
    Opacity,
    Name,
}

pub(crate) struct LayerEdit {
    kind: LayerEditKind,
    layer: parfait_core::LayerId,
    pointer: bool,
    focus: Option<egui::Id>,
}
impl Parfait {
    pub(crate) fn finish_stroke(&mut self) -> bool {
        if self.stroke.take().is_none() {
            return false;
        }
        if self.history.commit(&self.doc) {
            self.force_save();
        }
        true
    }
    pub(crate) fn cancel_edit(&mut self) {
        self.stroke = None;
        self.layer_edit = None;
        if self.history.cancel(&mut self.doc) {
            self.force_save();
        }
    }
    pub(crate) fn finish_layer_edit(&mut self) {
        if self.layer_edit.take().is_some() {
            self.history.commit(&self.doc);
        }
    }
    pub(crate) fn begin_layer_edit(&mut self, kind: LayerEditKind, layer: usize, pointer: bool) {
        let layer_id = self.doc.layers[layer].id;
        if self
            .layer_edit
            .as_ref()
            .is_some_and(|edit| edit.kind == kind && edit.layer == layer_id)
        {
            return;
        }
        self.finish_stroke();
        self.finish_layer_edit();
        self.history.begin(
            &self.doc,
            match kind {
                LayerEditKind::Opacity => "Layer opacity",
                LayerEditKind::Name => "Rename layer",
            },
        );
        self.layer_edit = Some(LayerEdit {
            kind,
            layer: layer_id,
            pointer,
            focus: None,
        });
    }
    pub(crate) fn layer_edit_response(&mut self, kind: LayerEditKind, response: &egui::Response) {
        let Some(edit) = self.layer_edit.as_mut().filter(|edit| edit.kind == kind) else {
            return;
        };
        edit.focus = Some(response.id);
        if response
            .ctx
            .input(|input| input.key_pressed(egui::Key::Escape))
        {
            self.cancelled_layer_drag =
                edit.pointer && response.ctx.input(|input| input.pointer.any_down());
            response.surrender_focus();
            self.cancel_edit();
        } else if (edit.pointer && !response.ctx.input(|input| input.pointer.any_down()))
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
            self.cancelled_layer_drag = false;
        }
        let finished = self.layer_edit.as_ref().is_some_and(|edit| {
            if edit.pointer {
                !ctx.input(|input| input.pointer.any_down())
            } else {
                edit.focus
                    .is_some_and(|id| ctx.memory(|memory| memory.focused()) != Some(id))
            }
        });
        if finished {
            self.finish_layer_edit();
        }
    }
    pub(crate) fn select_layer(&mut self, layer: usize) {
        if layer >= self.doc.layers.len() || self.doc.active == layer {
            return;
        }
        self.finish_stroke();
        self.finish_layer_edit();
        let revision = self.doc.revision;
        self.doc.active = layer;
        self.mask_target = None;
        self.layer_list = Default::default();
        self.doc.touch();
        if self.rendered == revision {
            self.rendered = self.doc.revision;
        }
        if self.preview_revision == revision {
            self.preview_revision = self.doc.revision;
        }
    }
    pub(crate) fn new_layer(&mut self) {
        let name = format!("Layer {}", self.doc.layers.len());
        if let Some(reason) = parfait_storage::layer_growth_error(&self.doc, name.len()) {
            self.notice = Some(reason.into());
            return;
        }
        self.edit_named("New layer", |document| document.add_layer());
    }
    pub(crate) fn select_history(&mut self, id: u64) {
        if self.stroke.is_some() {
            return;
        }
        self.finish_layer_edit();
        if self.history.restore(&mut self.doc, id) {
            self.layer_list = Default::default();
            self.force_save();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layers_frame(app: &mut Parfait, ctx: &egui::Context, events: Vec<egui::Event>) {
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
                    egui::Modifiers {
                        command: true,
                        ..Default::default()
                    },
                ),
                egui::Event::Text(text.into()),
            ],
        );
    }

    #[test]
    fn rename_focus_session_commits_once_and_escape_retains_future() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open(dir.path().to_owned()).unwrap();
        app.doc = parfait_core::Document::new(4, 4);
        app.new_layer();
        app.undo();
        let original = app.doc.layers[1].name.clone();
        app.begin_layer_rename(1);
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        layers_frame(&mut app, &ctx, vec![]);
        layers_frame(&mut app, &ctx, vec![]);
        let name = ctx
            .data(|data| data.get_temp::<egui::Rect>(egui::Id::new("layer_name")))
            .unwrap();
        click(&mut app, &ctx, name.center());
        replace(&mut app, &ctx, "Draft");
        layers_frame(&mut app, &ctx, vec![egui::Event::Text("!".into())]);
        assert_eq!(app.doc.layers[1].name, "Draft!");
        assert_eq!(app.history.rows().count(), 2);
        assert!(app.history.can_redo());
        layers_frame(
            &mut app,
            &ctx,
            vec![key(egui::Key::Escape, Default::default())],
        );
        assert_eq!(app.doc.layers[1].name, original);
        assert!(app.layer_edit.is_none());
        assert!(app.history.can_redo());
        app.begin_layer_rename(1);
        layers_frame(&mut app, &ctx, vec![]);
        click(&mut app, &ctx, name.center());
        replace(&mut app, &ctx, "Done");
        layers_frame(
            &mut app,
            &ctx,
            vec![key(egui::Key::Enter, Default::default())],
        );
        assert!(app.layer_edit.is_none());
        assert_eq!(app.history.rows().count(), 2);
        assert!(!app.history.can_redo());
        assert_eq!(app.history.rows().last().unwrap().label, "Rename layer");
        app.undo();
        assert_eq!(app.doc.layers[1].name, original);
        app.redo();
        assert_eq!(app.doc.layers[1].name, "Done");
    }

    #[test]
    fn opacity_numeric_percent_input_and_keyboard_session_are_undoable() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open(dir.path().to_owned()).unwrap();
        app.doc = parfait_core::Document::new(4, 4);
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        layers_frame(&mut app, &ctx, vec![]);
        layers_frame(&mut app, &ctx, vec![]);
        let (opacity, _) = ctx
            .data(|data| data.get_temp::<(egui::Rect, f32)>(egui::Id::new("layer_opacity")))
            .unwrap();
        click(&mut app, &ctx, opacity.right_center() - egui::vec2(16., 0.));
        replace(&mut app, &ctx, "42");
        layers_frame(
            &mut app,
            &ctx,
            vec![key(egui::Key::Enter, Default::default())],
        );
        assert!((app.doc.layers[1].opacity - 0.42).abs() < 0.001);
        assert_eq!(app.history.rows().count(), 2);
        app.undo();
        assert_eq!(app.doc.layers[1].opacity, 1.);
        app.redo();
        assert!((app.doc.layers[1].opacity - 0.42).abs() < 0.001);
        layers_frame(&mut app, &ctx, vec![]);
        let (rect, width) = ctx
            .data(|data| data.get_temp::<(egui::Rect, f32)>(egui::Id::new("layer_opacity")))
            .unwrap();
        click(
            &mut app,
            &ctx,
            egui::pos2(rect.left() + width - 2., rect.center().y),
        );
        let before = app.doc.layers[1].opacity;
        let focus = ctx
            .data(|data| data.get_temp::<egui::Id>(egui::Id::new("layer_opacity_focus")))
            .unwrap();
        ctx.memory_mut(|memory| memory.request_focus(focus));
        layers_frame(&mut app, &ctx, vec![]);
        let count = app.history.rows().count();
        for _ in 0..3 {
            layers_frame(
                &mut app,
                &ctx,
                vec![key(egui::Key::ArrowLeft, Default::default())],
            );
        }
        assert!(app.doc.layers[1].opacity < before);
        assert!(app.layer_edit.is_some());
        assert_eq!(app.history.rows().count(), count);
        let name = ctx
            .data(|data| data.get_temp::<egui::Rect>(egui::Id::new(("layer_label", 1usize))))
            .unwrap();
        click(&mut app, &ctx, name.center());
        assert!(app.layer_edit.is_none());
        assert_eq!(app.history.rows().count(), count + 1);
        app.undo();
        assert_eq!(app.doc.layers[1].opacity, before);
    }

    #[test]
    fn escape_cancels_opacity_drag_until_release_without_erasing_future() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open(dir.path().to_owned()).unwrap();
        app.doc = parfait_core::Document::new(4, 4);
        app.new_layer();
        app.undo();
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        layers_frame(&mut app, &ctx, vec![]);
        layers_frame(&mut app, &ctx, vec![]);
        let (rect, width) = ctx
            .data(|data| data.get_temp::<(egui::Rect, f32)>(egui::Id::new("layer_opacity")))
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
        assert!(app.doc.layers[1].opacity < 0.8);
        layers_frame(
            &mut app,
            &ctx,
            vec![key(egui::Key::Escape, Default::default())],
        );
        assert_eq!(app.doc.layers[1].opacity, 1.);
        layers_frame(&mut app, &ctx, vec![button(false)]);
        assert_eq!(app.doc.layers[1].opacity, 1.);
        assert!(app.layer_edit.is_none());
        assert!(!app.cancelled_layer_drag);
        assert!(app.history.can_redo());
        app.redo();
        assert_eq!(app.doc.layers.len(), 3);
    }

    #[test]
    fn growth_budget_blocks_new_and_duplicate_before_mutating_large_layout() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open(dir.path().to_owned()).unwrap();
        app.doc = parfait_core::Document::new(1, 1);
        while app.doc.layers.len() < 7 {
            app.doc.add_layer();
        }
        app.doc.width = 4096;
        app.doc.height = 4096;
        let original = app.doc.clone();
        app.new_layer();
        app.duplicate_layer();
        assert_eq!(app.doc, original);
        assert_eq!(app.history.rows().count(), 0);
        assert!(app.notice.as_ref().unwrap().contains("512 MiB"));
    }
}
