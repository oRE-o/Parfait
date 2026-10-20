use super::*;
use parfait_core::{BlendMode, Document, LayerMask, MaskCoverage};

fn fixture(directory: &std::path::Path) -> Parfait {
    let mut app = Parfait::open(directory.to_owned()).unwrap();
    app.documents.session.document = Document::new(8, 8);
    app.documents.session.document.layers[1].opacity = 0.33333334;
    app.documents.session.document.layers[1].mask =
        Some(LayerMask { enabled: true, coverage: MaskCoverage::Uniform(128) });
    app
}

fn frame(app: &mut Parfait, context: &egui::Context, events: Vec<egui::Event>) {
    let mut output = context.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(960., 640.),
            )),
            events,
            ..Default::default()
        },
        |ui| {
            app.shortcuts(ui.ctx());
            ui.add_enabled_ui(!app.dialog_open(), |ui| app.top_panel(ui));
            app.layer_details_ui(ui.ctx());
        },
    );
    output.textures_delta.clear();
}

fn key(key: egui::Key) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Default::default(),
    }
}

fn click(app: &mut Parfait, context: &egui::Context, name: &str) {
    let target: egui::Rect =
        context.data(|data| data.get_temp(egui::Id::new(name))).unwrap();
    for pressed in [true, false] {
        frame(
            app,
            context,
            vec![
                egui::Event::PointerMoved(target.center()),
                egui::Event::PointerButton {
                    pos: target.center(),
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                },
            ],
        );
    }
}

#[test]
fn draft_settings_apply_once_preserve_content_and_reopen() {
    let directory = tempfile::tempdir().unwrap();
    let mut app = fixture(directory.path());
    let context = egui::Context::default();
    crate::theme::apply(&context);
    let before = app.documents.session.document.clone();
    app.execute_command(Command::LayerDetails, &context);
    assert!(app.dialog_open());
    for _ in 0..3 {
        frame(&mut app, &context, vec![]);
    }
    let dialog = app.documents.session.layer_ui.details.as_mut().unwrap();
    dialog.draft.name = "한글 이름".into();
    dialog.draft.reference = true;
    dialog.draft.draft = true;
    dialog.draft.locked = true;
    dialog.draft.alpha_locked = true;
    dialog.draft.opacity = 0.42;
    dialog.draft.blend_mode = BlendMode::Multiply;
    assert_eq!(app.documents.session.document, before);
    click(&mut app, &context, "layer_details_accept");
    assert!(!app.dialog_open());
    let layer = &app.documents.session.document.layers[1];
    assert_eq!(layer.name, "한글 이름");
    assert!(
        layer.reference && layer.draft && layer.locked && layer.alpha_locked
    );
    assert_eq!(layer.opacity, 0.42);
    assert_eq!(layer.blend_mode, BlendMode::Multiply);
    assert_eq!(layer.mask, before.layers[1].mask);
    assert!(Arc::ptr_eq(&layer.pixels, &before.layers[1].pixels));
    app.undo();
    assert!(app.documents.session.document.same_edit_state(&before));
    assert!(!app.documents.session.history.can_undo());
    app.redo();
    let path = directory.path().join("settings.parfait");
    app.save_project_to(path.clone()).unwrap();
    app.open_project_from(path).unwrap();
    assert!(
        app.documents.session.document.layers[1].reference
            && app.documents.session.document.layers[1].draft
    );
    assert!(!app.project_is_modified());
}

#[test]
fn unchanged_confirmation_and_cancel_preserve_redo_and_fractional_opacity() {
    let directory = tempfile::tempdir().unwrap();
    let mut app = fixture(directory.path());
    let context = egui::Context::default();
    crate::theme::apply(&context);
    app.edit(|document| document.layers[1].visible = false);
    app.undo();
    let before = app.documents.session.document.clone();
    for accept in [true, false] {
        app.open_layer_details();
        for _ in 0..3 {
            frame(&mut app, &context, vec![]);
        }
        if !accept {
            app.documents
                .session
                .layer_ui
                .details
                .as_mut()
                .unwrap()
                .draft
                .name = "discarded".into();
        }
        click(
            &mut app,
            &context,
            if accept {
                "layer_details_accept"
            } else {
                "layer_details_cancel"
            },
        );
        assert_eq!(app.documents.session.document, before);
        assert!(app.documents.session.history.can_redo());
    }
}

#[test]
fn escape_ime_and_stale_target_do_not_apply_drafts() {
    let directory = tempfile::tempdir().unwrap();
    let mut app = fixture(directory.path());
    let context = egui::Context::default();
    crate::theme::apply(&context);
    let before = app.documents.session.document.clone();
    app.open_layer_details();
    frame(&mut app, &context, vec![]);
    frame(
        &mut app,
        &context,
        vec![
            egui::Event::Ime(egui::ImeEvent::Preedit {
                text: "가".into(),
                active_range_chars: None,
            }),
            key(egui::Key::Enter),
        ],
    );
    assert!(app.documents.session.layer_ui.details.is_some());
    frame(
        &mut app,
        &context,
        vec![
            egui::Event::Ime(egui::ImeEvent::Commit("가".into())),
            key(egui::Key::Enter),
        ],
    );
    assert!(app.documents.session.layer_ui.details.is_some());
    frame(&mut app, &context, vec![key(egui::Key::Escape)]);
    assert!(app.documents.session.layer_ui.details.is_none());
    assert_eq!(app.documents.session.document, before);
    app.open_layer_details();
    app.documents.session.document.touch();
    frame(&mut app, &context, vec![]);
    assert!(app.documents.session.layer_ui.details.is_none());
    assert!(app.documents.session.document.same_edit_state(&before));
}

#[test]
fn invalid_names_and_minimum_size_keep_actions_visible() {
    let directory = tempfile::tempdir().unwrap();
    let mut app = fixture(directory.path());
    let context = egui::Context::default();
    crate::theme::apply(&context);
    app.open_layer_details();
    for name in ["", " \t", "invalid\nname"] {
        app.documents.session.layer_ui.details.as_mut().unwrap().draft.name =
            name.into();
        assert!(
            app.documents
                .session
                .layer_ui
                .details
                .as_ref()
                .unwrap()
                .error()
                .is_some()
        );
        for _ in 0..3 {
            frame(&mut app, &context, vec![]);
        }
        click(&mut app, &context, "layer_details_accept");
        assert!(app.documents.session.layer_ui.details.is_some());
    }
    app.documents.session.layer_ui.details.as_mut().unwrap().draft.name =
        "정상 이름".into();
    for opacity in [f32::NAN, f32::INFINITY, -0.1, 1.1] {
        app.documents
            .session
            .layer_ui
            .details
            .as_mut()
            .unwrap()
            .draft
            .opacity = opacity;
        assert!(
            app.documents
                .session
                .layer_ui
                .details
                .as_ref()
                .unwrap()
                .error()
                .is_some()
        );
    }
    app.documents.session.layer_ui.details.as_mut().unwrap().draft.opacity =
        0.5;
    app.documents.session.layer_ui.details.as_mut().unwrap().max_name_bytes = 1;
    assert!(
        app.documents
            .session
            .layer_ui
            .details
            .as_ref()
            .unwrap()
            .error()
            .is_some()
    );
    let modal: egui::Rect = context
        .data(|data| data.get_temp(egui::Id::new("layer_details_bounds")))
        .unwrap();
    let accept: egui::Rect = context
        .data(|data| data.get_temp(egui::Id::new("layer_details_accept")))
        .unwrap();
    assert!(modal.contains_rect(accept));
    assert!(
        modal.left() >= 0.
            && modal.right() <= 960.
            && modal.top() >= 0.
            && modal.bottom() <= 640.
    );
}

#[test]
fn flags_are_independent_of_locks_and_recovery_does_not_clear_file_marker() {
    let directory = tempfile::tempdir().unwrap();
    let mut app = fixture(directory.path());
    let context = egui::Context::default();
    app.documents.session.document.layers[1].locked = true;
    app.save_project_to(directory.path().join("flags.parfait")).unwrap();
    let before = app.documents.session.document.clone();
    app.execute_command(Command::LayerReference, &context);
    app.execute_command(Command::LayerDraft, &context);
    assert!(
        app.documents.session.document.layers[1].reference
            && app.documents.session.document.layers[1].draft
    );
    assert!(app.project_is_modified());
    app.documents.session.recovery.saved =
        app.documents.session.document.revision;
    assert!(app.project_is_modified());
    app.undo();
    app.undo();
    assert!(!app.project_is_modified());
    assert!(app.documents.session.document.same_edit_state(&before));
}

#[test]
fn selected_set_flags_use_active_value_and_one_undo() {
    let directory = tempfile::tempdir().unwrap();
    let mut app = fixture(directory.path());
    let context = egui::Context::default();
    app.documents.session.document.layers[0].draft = true;
    app.set_layer_selection(3, 1);
    let before = app.documents.session.document.clone();
    app.execute_command(Command::LayerDraft, &context);
    assert!(
        app.documents.session.document.layers.iter().all(|layer| layer.draft)
    );
    assert!(app.documents.session.document.layers[0].locked);
    app.undo();
    assert!(app.documents.session.document.same_edit_state(&before));
    assert!(!app.documents.session.history.can_undo());
    app.set_layer_selection(3, 1);
    app.execute_command(Command::LayerReference, &context);
    assert!(
        app.documents
            .session
            .document
            .layers
            .iter()
            .all(|layer| layer.reference)
    );
    app.undo();
    assert!(app.documents.session.document.same_edit_state(&before));
}

#[test]
fn previous_keymap_ownership_survives_and_new_layer_keys_reopen() {
    use crate::shortcuts::Chord;
    let directory = tempfile::tempdir().unwrap();
    let mut app = fixture(directory.path());
    app.keymap
        .assign(
            Command::LayerLock,
            None,
            Chord::new(egui::Key::F6, false, false, false),
            false,
        )
        .unwrap();
    app.persist_preferences(true);
    let mut legacy: serde_json::Value =
        serde_json::from_str(&app.saved_preferences_json()).unwrap();
    legacy["version"] = 18.into();
    let expected = app.keymap.clone();
    parfait_storage::save_preferences(
        &directory.path().join("preferences.sqlite"),
        &legacy.to_string(),
    )
    .unwrap();
    drop(app);
    let mut app = Parfait::open(directory.path().to_owned()).unwrap();
    assert!(!app.preferences_read_failed);
    assert!(app.keymap == expected);
    assert_eq!(app.keymap.hint(Command::LayerLock), "F6");
    for (command, key) in [
        (Command::LayerReference, egui::Key::F7),
        (Command::LayerDraft, egui::Key::F8),
        (Command::LayerDetails, egui::Key::F9),
    ] {
        assert!(app.keymap.hint(command).is_empty());
        app.keymap
            .assign(command, None, Chord::new(key, false, false, false), false)
            .unwrap();
    }
    app.persist_preferences(true);
    assert_eq!(app.preferences_saved.as_ref().unwrap().version, 21);
    drop(app);
    let app = Parfait::open(directory.path().to_owned()).unwrap();
    assert_eq!(app.keymap.hint(Command::LayerReference), "F7");
    assert_eq!(app.keymap.hint(Command::LayerDraft), "F8");
    assert_eq!(app.keymap.hint(Command::LayerDetails), "F9");
}
