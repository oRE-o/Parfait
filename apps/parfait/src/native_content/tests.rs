use super::*;
use parfait_core::Document;
fn setup() -> (tempfile::TempDir, Parfait, egui::Context) {
    let dir = tempfile::tempdir().unwrap();
    let mut app = Parfait::open_legacy(dir.path().into()).unwrap();
    app.documents.active_session.document = Document::new(100, 80);
    let ctx = egui::Context::default();
    crate::theme::apply(&ctx);
    (dir, app, ctx)
}
#[test]
fn creating_editing_and_rasterizing_content_each_undo_exactly() {
    for command in [Command::NewTextLayer, Command::NewVectorLayer] {
        let (_dir, mut app, ctx) = setup();
        let before = app.documents.active_session.document.art_digest_v1();
        app.execute_command(command, &ctx);
        let draft = app.native_dialog.take().unwrap();
        assert!(app.apply_native(&draft).unwrap());
        let created = app.documents.active_session.document.art_digest_v1();
        assert_ne!(created, before);
        app.undo();
        assert_eq!(
            app.documents.active_session.document.art_digest_v1(),
            before
        );
        app.redo();
        assert_eq!(
            app.documents.active_session.document.art_digest_v1(),
            created
        );
        app.execute_command(Command::EditNativeLayer, &ctx);
        let mut draft = app.native_dialog.take().unwrap();
        match Arc::make_mut(&mut draft.body) {
            LayerContent::Text(t) => t.text = "편집".into(),
            LayerContent::Vector(v) => {
                Arc::make_mut(&mut v.paths[0].points)[1][0] = 70.
            }
        }
        assert!(app.apply_native(&draft).unwrap());
        let edited = app.documents.active_session.document.art_digest_v1();
        assert_ne!(edited, created);
        app.undo();
        assert_eq!(
            app.documents.active_session.document.art_digest_v1(),
            created
        );
        app.redo();
        let pixels = app.documents.active_session.document.composite();
        app.execute_command(Command::RasterizeNativeLayer, &ctx);
        assert!(app.documents.active_session.document.contents.is_empty());
        assert_eq!(pixels, app.documents.active_session.document.composite());
        app.undo();
        assert_eq!(
            app.documents.active_session.document.art_digest_v1(),
            edited
        );
    }
}
#[test]
fn canceled_invalid_stale_drafts_preserve_document_and_redo() {
    let (_dir, mut app, ctx) = setup();
    app.execute_command(Command::EditFill, &ctx);
    app.undo();
    let before = app.documents.active_session.document.art_digest_v1();
    app.execute_command(Command::NewTextLayer, &ctx);
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
        app.native_content_ui(ui.ctx())
    });
    output.textures_delta.clear();
    let mut output = ctx.run_ui(
        egui::RawInput {
            events: vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            }],
            ..Default::default()
        },
        |ui| app.native_content_ui(ui.ctx()),
    );
    output.textures_delta.clear();
    assert!(app.native_dialog.is_none());
    assert!(app.documents.active_session.history.can_redo());
    assert_eq!(app.documents.active_session.document.art_digest_v1(), before);
    app.execute_command(Command::NewVectorLayer, &ctx);
    let mut draft = app.native_dialog.take().unwrap();
    if let LayerContent::Vector(v) = Arc::make_mut(&mut draft.body) {
        v.matrix.0[0] = f32::NAN;
    }
    assert!(app.apply_native(&draft).is_err());
    assert!(app.documents.active_session.history.can_redo());
    draft.revision = 0;
    assert!(app.apply_native(&draft).is_err());
    assert_eq!(app.documents.active_session.document.art_digest_v1(), before);
}
#[test]
fn bounds_options_toggle_without_artwork_or_history_changes() {
    let (_dir, mut app, ctx) = setup();
    let before = app.documents.active_session.document.clone();
    let rows = app.documents.active_session.history.rows().count();
    app.execute_command(Command::EditTextInkBounds, &ctx);
    app.execute_command(Command::EditVectorCenters, &ctx);
    assert!(
        app.documents.active_session.editing.align.text_ink
            && app.documents.active_session.editing.align.vector_center
    );
    assert!(app.documents.active_session.document.same_edit_state(&before));
    assert_eq!(app.documents.active_session.history.rows().count(), rows);
}
