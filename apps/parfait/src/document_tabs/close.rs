use super::*;
impl Parfait {
    pub(crate) fn tab_close_dialog_open(&self) -> bool {
        self.tabs.close_prompt.is_some() || self.tabs.close_after_save.is_some()
    }
    pub(crate) fn request_tab_close(&mut self, id: DocumentId) -> bool {
        if self.tab_switch_blocked() {
            return false;
        }
        let dirty = self
            .documents
            .get(id)
            .is_some_and(|s| s.modified() && !s.pristine());
        if dirty
            && !self.source_saves.pending.contains_key(&id)
            && !self
                .source_saves
                .prompts
                .iter()
                .any(|p| p.request.key.document_id == id.bytes())
        {
            self.tabs.close_prompt = Some(id);
            return false;
        }
        self.close_document_tab(id)
    }
    fn save_closing_canvas(&mut self, id: DocumentId) {
        if !self.activate_document(id, true) {
            self.tabs.close_prompt = Some(id);
            return;
        }
        let key = self.documents.active_session.key;
        self.save_project(false);
        if self.source_saves.pending.contains_key(&id)
            || self
                .source_saves
                .prompts
                .iter()
                .any(|p| p.request.key.document_id == id.bytes())
        {
            self.tabs.close_after_save = Some(key);
        } else if self.documents.get(id).is_some_and(|s| !s.modified()) {
            self.close_document_tab(id);
        } else {
            self.tabs.close_prompt = Some(id);
        }
    }
    pub(crate) fn tab_close_prompt_ui(&mut self, ctx: &egui::Context) {
        if let Some(key) = self.tabs.close_after_save {
            if self.documents.get_by_key(key).is_none() {
                self.tabs.close_after_save = None;
                return;
            }
            let busy = self.source_saves.pending.contains_key(&key.document_id);
            let prompt =
                self.source_saves.prompts.iter().any(|p| {
                    p.request.key.document_id == key.document_id.bytes()
                });
            if !busy && !prompt {
                self.tabs.close_after_save = None;
                if self.documents.get_by_key(key).is_some_and(|s| !s.modified())
                    && self.error.is_none()
                {
                    self.close_document_tab(key.document_id);
                } else {
                    self.tabs.close_prompt = Some(key.document_id);
                }
            } else {
                if !prompt {
                    egui::Modal::new(egui::Id::new("closing_canvas_save"))
                        .show(ctx, |ui| {
                            ui.label("파일 저장이 끝나면 그림을 닫습니다.");
                            if ui.button("닫기 취소").clicked() {
                                self.tabs.close_after_save = None;
                            }
                        });
                    if ctx.input_mut(|i| {
                        i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)
                    }) {
                        self.tabs.close_after_save = None;
                    }
                }
                ctx.request_repaint_after(std::time::Duration::from_millis(50));
                return;
            }
        }
        let Some(id) = self.tabs.close_prompt.take() else {
            return;
        };
        let Some(session) = self.documents.get(id) else {
            return;
        };
        let title = session.title();
        let mut choice = None;
        let response = egui::Modal::new(egui::Id::new("close_canvas_prompt"))
            .show(ctx, |ui| {
                ui.set_max_width(380.);
                ui.heading("그림 닫기");
                ui.label(format!("‘{title}’의 변경 내용을 저장할까요?"));
                ui.horizontal(|ui| {
                    for (i, label) in ["저장하고 닫기", "저장 안 함", "취소"]
                        .iter()
                        .enumerate()
                    {
                        let button = ui.button(*label);
                        #[cfg(test)]
                        ui.data_mut(|d| {
                            d.insert_temp(
                                egui::Id::new(("close_canvas_choice", i)),
                                button.rect,
                            )
                        });
                        if button.clicked() {
                            choice = Some(i);
                        }
                    }
                });
            });
        if response.is_top_modal
            && ctx.input_mut(|i| {
                i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)
            })
        {
            choice = Some(2);
        }
        match choice {
            Some(0) => self.save_closing_canvas(id),
            Some(1) => {
                if !self.close_document_tab_checked(id, true) {
                    self.tabs.close_prompt = Some(id);
                }
            }
            Some(2) => {}
            _ => self.tabs.close_prompt = Some(id),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn frame(app: &mut Parfait, ctx: &egui::Context, events: Vec<egui::Event>) {
        let mut out = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(960., 640.),
                )),
                events,
                ..Default::default()
            },
            |ui| app.draw_ui(ui),
        );
        out.textures_delta.clear();
    }
    fn click(app: &mut Parfait, ctx: &egui::Context, index: usize) {
        let p = ctx
            .data(|d| {
                d.get_temp::<egui::Rect>(egui::Id::new((
                    "close_canvas_choice",
                    index,
                )))
            })
            .unwrap()
            .center();
        for pressed in [true, false] {
            frame(
                app,
                ctx,
                vec![
                    egui::Event::PointerMoved(p),
                    egui::Event::PointerButton {
                        pos: p,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: Default::default(),
                    },
                ],
            );
        }
    }
    #[test]
    fn close_choice_cancel_retains_pixels_and_explicit_discard_closes_only_target()
     {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open_document_tabs(dir.path().into()).unwrap();
        app.documents.active_session.document = Document::new(4, 4);
        let a = app.documents.active_id().unwrap();
        let mut doc = Document::new(4, 4);
        doc.layers[1].pixels =
            std::sync::Arc::new([80, 100, 120, 255].repeat(16));
        let b = app.new_document_tab(doc).unwrap();
        let before = app.documents.get(b).unwrap().document.clone();
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        assert!(!app.request_tab_close(b));
        for _ in 0..3 {
            frame(&mut app, &ctx, vec![]);
        }
        click(&mut app, &ctx, 2);
        assert_eq!(app.documents.get(b).unwrap().document, before);
        assert!(!app.tab_close_dialog_open());
        app.request_tab_close(b);
        for _ in 0..3 {
            frame(&mut app, &ctx, vec![]);
        }
        click(&mut app, &ctx, 1);
        assert!(app.documents.get(b).is_none());
        assert!(app.documents.get(a).is_some());
        assert!(!app.tab_close_dialog_open());
    }
    #[test]
    fn save_then_close_waits_for_success_and_reopens_exact_original() {
        let dir = tempfile::tempdir().unwrap();
        let mut app =
            Parfait::open_document_tabs(dir.path().join("recovery")).unwrap();
        app.documents.active_session.document = Document::new(4, 4);
        app.edit_named("Fill", |d| {
            d.edit_layer_pixels(
                1,
                parfait_core::PixelEdit::Fill([70, 90, 130, 255]),
            );
        });
        let id = app.documents.active_id().unwrap();
        let path = dir.path().join("saved.parfait");
        app.save_project_to(path.clone()).unwrap();
        let deadline =
            std::time::Instant::now() + std::time::Duration::from_secs(10);
        while !app.source_saves.pending.is_empty() {
            app.poll_source_saves();
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        app.edit_named("Invert", |d| {
            d.edit_layer_pixels(1, parfait_core::PixelEdit::Invert);
        });
        let expected = app.documents.active_session.document.composite();
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        app.request_tab_close(id);
        for _ in 0..3 {
            frame(&mut app, &ctx, vec![]);
        }
        click(&mut app, &ctx, 0);
        while !app.documents.is_empty() {
            frame(&mut app, &ctx, vec![]);
            assert!(app.error.is_none(), "{:?}", app.error);
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert_eq!(
            parfait_storage::Store::open_read_only(&path)
                .unwrap()
                .load()
                .unwrap()
                .unwrap()
                .document
                .composite(),
            expected
        );
    }
}
