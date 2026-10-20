use super::*;
use parfait_core::{Document, MaskCoverage};
use std::sync::Arc;

fn setup() -> (tempfile::TempDir, Parfait, egui::Context) {
    let dir = tempfile::tempdir().unwrap();
    let mut app = Parfait::open_legacy(dir.path().to_owned()).unwrap();
    app.documents.active_session.document = Document::new(3, 2);
    app.documents.active_session.document.layers[1].pixels =
        Arc::new([80, 100, 160, 180].repeat(6));
    let ctx = egui::Context::default();
    crate::theme::apply(&ctx);
    (dir, app, ctx)
}
#[test]
fn immediate_pixel_and_canvas_commands_have_one_history_step_and_exact_undo_redo()
 {
    for command in [
        Command::EditFill,
        Command::EditInvert,
        Command::EditClearOutside,
        Command::EditCrop,
        Command::EditCanvasRotateRight,
        Command::EditCanvasRotateHalf,
        Command::EditCanvasRotateLeft,
        Command::EditCanvasFlipX,
        Command::EditCanvasFlipY,
    ] {
        let (_dir, mut app, ctx) = setup();
        // Nonuniform fixture makes flips and rotations actual changes.
        Arc::make_mut(
            &mut app.documents.active_session.document.layers[1].pixels,
        )[0] = 17;
        app.documents.active_session.document.selection =
            Some(MaskCoverage::Dense(Arc::new(vec![0, 255, 255, 0, 255, 255])));
        let before = app.documents.active_session.document.clone();
        app.execute_command(command, &ctx);
        assert!(
            !app.documents.active_session.document.same_edit_state(&before),
            "{command:?}"
        );
        assert!(app.documents.active_session.document.validate());
        assert_eq!(
            app.documents.active_session.history.rows().count(),
            2,
            "{command:?}"
        );
        let edited = app.documents.active_session.document.clone();
        app.undo();
        assert!(
            app.documents.active_session.document.same_edit_state(&before),
            "{command:?}"
        );
        app.redo();
        assert!(
            app.documents.active_session.document.same_edit_state(&edited),
            "{command:?}"
        );
    }
}
#[test]
fn dialog_cancel_discards_settings_and_preserves_redo_at_minimum_window_size() {
    for command in [
        Command::EditBrightnessContrast,
        Command::EditHsl,
        Command::EditPosterize,
        Command::EditLevels,
        Command::EditToneCurve,
        Command::EditColorBalance,
        Command::EditThreshold,
        Command::EditGradientMap,
        Command::EditAdvancedFill,
        Command::EditBorder,
        Command::EditResolution,
        Command::EditCanvasSize,
        Command::EditClearHistory,
    ] {
        let (_dir, mut app, ctx) = setup();
        app.documents.active_session.document.select_all_pixels();
        app.edit_named("Opacity", |d| d.layers[1].opacity = 0.5);
        app.undo();
        let before = app.documents.active_session.document.clone();
        app.execute_command(command, &ctx);
        assert!(app.image_edit.dialog.is_some(), "{command:?}");
        assert!(app.dialog_open());
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(960., 640.),
                )),
                ..Default::default()
            },
            |ui| app.image_edit_dialog_ui(ui.ctx()),
        );
        output.textures_delta.clear();
        for name in ["image_edit_accept", "image_edit_cancel"] {
            let rect = ctx
                .data(|d| d.get_temp::<egui::Rect>(egui::Id::new(name)))
                .unwrap();
            assert!(
                ctx.content_rect().contains_rect(rect),
                "{command:?} {rect:?}"
            );
        }
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
            |ui| app.image_edit_dialog_ui(ui.ctx()),
        );
        output.textures_delta.clear();
        assert!(app.image_edit.dialog.is_none());
        assert!(app.documents.active_session.document.same_edit_state(&before));
        assert!(app.documents.active_session.history.can_redo());
    }
}
#[test]
fn dialog_confirmation_and_stale_session_guards_preserve_correct_document() {
    let (_dir, mut app, _ctx) = setup();
    let before = app.documents.active_session.document.clone();
    let mut draft = EditDialog::new(&app, Command::EditBrightnessContrast);
    draft.values[0] = 25.;
    assert!(app.apply_image_edit(&draft));
    assert!(!app.documents.active_session.document.same_edit_state(&before));
    let edited = app.documents.active_session.document.clone();
    assert!(!app.apply_image_edit(&draft));
    assert_eq!(app.documents.active_session.document, edited);
    let draft = EditDialog::new(&app, Command::EditCanvasSize);
    let replacement =
        crate::document_session::DocumentSession::new(Document::new(1, 1))
            .unwrap();
    app.documents.active_session.replace(replacement);
    let before = app.documents.active_session.document.clone();
    assert!(!app.apply_image_edit(&draft));
    assert_eq!(app.documents.active_session.document, before);
}
#[test]
fn invalid_parameters_limits_locks_and_mask_target_refuse_atomically() {
    let (_dir, mut app, ctx) = setup();
    app.documents.active_session.document.layers[1].locked = true;
    let before = app.documents.active_session.document.clone();
    app.execute_command(Command::EditFill, &ctx);
    assert_eq!(app.documents.active_session.document, before);
    app.documents.active_session.document.layers[1].locked = false;
    let mut draft = EditDialog::new(&app, Command::EditLevels);
    draft.values[0] = 230.;
    draft.values[1] = 100.;
    let before = app.documents.active_session.document.clone();
    assert!(!app.apply_image_edit(&draft));
    assert_eq!(app.documents.active_session.document, before);
    let mut draft = EditDialog::new(&app, Command::EditResolution);
    draft.width = 4097;
    assert!(!app.apply_image_edit(&draft));
    assert_eq!(app.documents.active_session.document, before);
    app.documents.logical_limit = 1;
    let mut draft = EditDialog::new(&app, Command::EditResolution);
    draft.width = 100;
    assert!(!app.apply_image_edit(&draft));
    assert_eq!(app.documents.active_session.document, before);
    app.documents.logical_limit = 512 * 1024 * 1024;
    app.documents.active_session.document.create_layer_mask(1, 255);
    app.documents.active_session.editing.mask_target =
        Some(app.documents.active_session.document.layers[1].id);
    assert!(app.image_edit_reason(Command::EditFill).is_some());
    assert_eq!(app.documents.active_session.history.rows().count(), 0);
}
#[test]
fn history_clear_discards_undo_redo_without_changing_saved_identity_or_pixels()
{
    let (_dir, mut app, _ctx) = setup();
    app.edit_named("Opacity", |d| d.layers[1].opacity = 0.3);
    app.documents.active_session.saved_edit_state =
        Some(app.documents.active_session.history.state_id());
    let before = app.documents.active_session.document.clone();
    let draft = EditDialog::new(&app, Command::EditClearHistory);
    assert!(app.apply_image_edit(&draft));
    assert_eq!(app.documents.active_session.document, before);
    assert!(!app.documents.active_session.modified());
    assert!(!app.documents.active_session.history.can_undo());
    assert!(!app.documents.active_session.history.can_redo());
}
#[test]
fn resized_project_and_dense_selection_reopen_with_exact_pixels_and_masks() {
    let (dir, mut app, _ctx) = setup();
    app.documents.active_session.document.create_layer_mask(1, 128);
    app.documents.active_session.document.selection =
        Some(MaskCoverage::Dense(Arc::new(vec![0, 50, 100, 150, 200, 255])));
    let mut draft = EditDialog::new(&app, Command::EditResolution);
    draft.width = 7;
    draft.height = 5;
    assert!(app.apply_image_edit(&draft));
    let expected = app.documents.active_session.document.clone();
    let path = dir.path().join("resized.parfait");
    app.save_project_to(path.clone()).unwrap();
    let actual = parfait_storage::Store::open_read_only(&path)
        .unwrap()
        .load()
        .unwrap()
        .unwrap()
        .document;
    assert_eq!((actual.width, actual.height), (7, 5));
    assert_eq!(actual.layers, expected.layers);
    assert_eq!(actual.selection, expected.selection);
    assert_eq!(actual.composite(), expected.composite());
}
#[test]
fn old_preferences_keep_existing_assignments_and_new_commands_can_be_bound() {
    let (dir, mut app, _ctx) = setup();
    let mut old: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&app.keymap).unwrap())
            .unwrap();
    old["bindings"].as_array_mut().unwrap().retain(|b| {
        !matches!(
            b["command"].as_str(),
            Some("edit_copy" | "edit_cut" | "edit_paste")
        )
    });
    app.keymap = serde_json::from_value(old).unwrap();
    app.keymap.refresh();
    let expected = app.keymap.clone();
    app.persist_preferences(true);
    let path = dir.path().join("preferences.sqlite");
    let mut json: serde_json::Value =
        serde_json::from_str(&app.saved_preferences_json()).unwrap();
    json["version"] = 21.into();
    parfait_storage::save_preferences(&path, &json.to_string()).unwrap();
    // Release the session lock before reopening the preferences fixture.
    drop(app);
    let restored = Parfait::open_legacy(dir.path().to_owned()).unwrap();
    assert!(restored.keymap == expected);
    assert!(restored.keymap.hint(Command::EditCopy).is_empty());
}

#[test]
fn cut_publishes_clipboard_before_mutation_and_copy_failure_keeps_history_and_pixels()
 {
    let (_dir, mut app, _ctx) = setup();
    app.edit_named("Opacity", |d| d.layers[1].opacity = 0.5);
    app.undo();
    let before = app.documents.active_session.document.clone();
    app.copy_image_with(Command::EditCut, |_, _| {
        Err("test clipboard unavailable".into())
    });
    assert!(app.documents.active_session.document.same_edit_state(&before));
    assert!(app.documents.active_session.history.can_redo());
    assert!(app.image_edit.clipboard.is_none());
    app.error = None;
    app.copy_image_with(Command::EditCopy, |_, clip| {
        assert!(clip.validate());
        Ok(())
    });
    assert!(app.documents.active_session.document.same_edit_state(&before));
    assert!(app.documents.active_session.history.can_redo());
    app.copy_image_with(Command::EditCut, |app, clip| {
        assert_eq!(
            app.documents.active_session.document.layers[1].pixels.as_slice(),
            clip.pixels
        );
        Ok(())
    });
    assert!(
        app.documents.active_session.document.layers[1]
            .pixels
            .iter()
            .all(|v| *v == 0)
    );
    assert!(app.image_edit.clipboard.is_some());
    app.undo();
    assert!(app.documents.active_session.document.same_edit_state(&before));
}

#[test]
fn clipboard_native_events_in_text_fields_do_not_read_image_clipboard() {
    let (_dir, mut app, ctx) = setup();
    let before = app.documents.active_session.document.clone();
    for event in
        [egui::Event::Copy, egui::Event::Cut, egui::Event::Paste("text".into())]
    {
        let mut output = ctx.run_ui(
            egui::RawInput { events: vec![event], ..Default::default() },
            |ui| {
                ui.text_edit_singleline(&mut String::new()).request_focus();
                app.shortcuts(ui.ctx());
            },
        );
        output.textures_delta.clear();
        assert!(app.image_edit.system_clipboard.is_none());
        assert!(app.documents.active_session.document.same_edit_state(&before));
    }
}

#[test]
fn transform_dialog_apply_cancel_and_invalid_mesh_share_one_transaction() {
    for commit in [false, true] {
        let (_dir, mut app, ctx) = setup();
        Arc::make_mut(
            &mut app.documents.active_session.document.layers[1].pixels,
        )[0] = 1;
        let before = app.documents.active_session.document.clone();
        app.execute_command(Command::EditTransformRotate, &ctx);
        assert!(app.image_edit.dialog.is_some());
        app.image_edit.dialog.as_mut().unwrap().transform.angle = 90.;
        assert!(app.documents.active_session.document.same_edit_state(&before));
        app.execute_command(
            if commit {
                Command::EditTransformApply
            } else {
                Command::EditTransformCancel
            },
            &ctx,
        );
        assert!(app.image_edit.dialog.is_none());
        if commit {
            assert_eq!(app.documents.active_session.history.rows().count(), 2);
            app.undo();
        }
        assert!(app.documents.active_session.document.same_edit_state(&before));
    }
    let (_dir, mut app, ctx) = setup();
    let before = app.documents.active_session.document.clone();
    app.execute_command(Command::EditTransformMesh, &ctx);
    app.image_edit.dialog.as_mut().unwrap().transform.nodes[4] = [2., 2.];
    app.execute_command(Command::EditTransformApply, &ctx);
    assert!(app.image_edit.dialog.is_some());
    assert!(app.documents.active_session.document.same_edit_state(&before));
    app.execute_command(Command::EditTransformCancel, &ctx);
    assert!(app.image_edit.dialog.is_none());
}

#[test]
fn alignment_references_and_guides_are_session_state_and_pixel_moves_are_undoable()
 {
    let (_dir, mut app, ctx) = setup();
    app.documents.active_session.document = Document::new(8, 4);
    Arc::make_mut(&mut app.documents.active_session.document.layers[1].pixels)
        [4..8]
        .copy_from_slice(&[100, 50, 20, 255]);
    let before = app.documents.active_session.document.clone();
    app.execute_command(Command::EditRefCanvas, &ctx);
    assert_eq!(app.documents.active_session.history.rows().count(), 0);
    app.execute_command(Command::EditAlignRight, &ctx);
    assert_eq!(
        app.documents.active_session.document.layer_pixel_bounds(1).unwrap().x1,
        8
    );
    app.undo();
    assert!(app.documents.active_session.document.same_edit_state(&before));
    app.execute_command(Command::EditRefGuides, &ctx);
    let draft = app.image_edit.dialog.take().unwrap();
    assert!(app.apply_image_edit(&draft));
    assert!(app.documents.active_session.history.can_redo());
    assert!(app.documents.active_session.document.same_edit_state(&before));
}

#[test]
fn material_registration_roundtrips_images_templates_and_never_rebinds_source()
{
    use crate::local_materials::MaterialKind;
    let (_dir, mut app, _ctx) = setup();
    let before = app.documents.active_session.document.clone();
    let image =
        app.register_local_material("색/../소재", MaterialKind::Image).unwrap();
    assert_eq!(image.file_name().unwrap(), "image.png");
    let decoded = crate::local_materials::read_local_image(&image).unwrap();
    assert_eq!(decoded.pixels, before.composite());
    let template =
        app.register_local_material("템플릿", MaterialKind::Template).unwrap();
    let loaded = parfait_storage::Store::open_read_only(&template)
        .unwrap()
        .load()
        .unwrap()
        .unwrap()
        .document;
    assert_eq!(loaded.layers, before.layers);
    assert_eq!(loaded.name, "템플릿");
    assert!(app.documents.active_session.document.same_edit_state(&before));
    assert!(app.documents.active_session.project.is_none());
    assert_eq!(app.local_materials().len(), 2);
    assert!(app.register_local_material(" ", MaterialKind::Image).is_err());
    app.use_local_material(MaterialKind::Image, &image).unwrap();
    assert_eq!(app.documents.active_session.document.layers.len(), 3);
    app.undo();
    assert!(app.documents.active_session.document.same_edit_state(&before));
}

#[test]
fn imported_reference_header_limits_are_enforced_before_use() {
    let (dir, _app, _ctx) = setup();
    let path = dir.path().join("too-wide.png");
    image::save_buffer_with_format(
        &path,
        &[255; 5000 * 4],
        5000,
        1,
        image::ColorType::Rgba8,
        image::ImageFormat::Png,
    )
    .unwrap();
    assert!(crate::local_materials::read_local_image(&path).is_err());
}
