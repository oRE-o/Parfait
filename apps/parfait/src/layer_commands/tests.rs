use super::*;

#[test]
fn folder_documents_render_safely_and_refuse_raster_only_edits() {
    let directory = tempfile::tempdir().unwrap();
    let mut app = Parfait::open_legacy(directory.path().to_owned()).unwrap();
    app.documents.active_session.document = parfait_core::Document::new(16, 16);
    app.documents
        .active_session
        .document
        .group_layers_in_folder(2, "Folder".into())
        .unwrap();
    let context = egui::Context::default();
    crate::theme::apply(&context);
    app.update_previews(&context);
    let before = app.documents.active_session.document.clone();
    for command in
        [Command::MoveLayerUp, Command::LayerAlphaLock, Command::ApplyLayerMask]
    {
        assert!(app.command_disabled_reason(command).is_some(), "{command:?}");
        app.execute_command(command, &context);
        assert!(app.documents.active_session.document.same_edit_state(&before));
    }
    assert!(!app.begin_canvas_stroke(parfait_core::Point {
        x: 8.,
        y: 8.,
        pressure: 1.
    }));
    let mut output = context.run_ui(Default::default(), |ui| {
        app.palette(ui, crate::workspace::Palette::Layers, 400.)
    });
    output.textures_delta.clear();
    assert!(app.documents.active_session.document.validate());
    assert_eq!(app.documents.active_session.history.rows().count(), 0);
    assert!(app.command_disabled_reason(Command::NewLayer).is_none());
    app.execute_command(Command::NewLayer, &context);
    assert_eq!(
        app.documents.active_session.document.layers
            [app.documents.active_session.document.active]
            .parent,
        Some(before.layers[2].id)
    );
    assert!(app.documents.active_session.document.validate());
    app.undo();
    assert!(app.documents.active_session.document.same_edit_state(&before));
    app.documents.active_session.document.layers[2].locked = true;
    app.documents.active_session.document.active = 1;
    assert!(app.command_disabled_reason(Command::NewLayer).is_some());
    assert!(!app.begin_canvas_stroke(parfait_core::Point {
        x: 8.,
        y: 8.,
        pressure: 1.
    }));
    app.save_project_to(directory.path().join("folders.parfait")).unwrap();
    let loaded =
        parfait_storage::Store::open(&directory.path().join("folders.parfait"))
            .unwrap()
            .load()
            .unwrap()
            .unwrap()
            .document;
    assert_eq!(loaded.layers, app.documents.active_session.document.layers);
}
use crate::shortcuts::Chord;
use parfait_core::{Brush, Document, Point, Stroke};
use std::sync::Arc;

#[test]
fn masked_document_duplicate_undo_and_autosave_preserve_both_shared_buffers() {
    for multiple in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let mut app =
            Parfait::open_legacy(directory.path().to_owned()).unwrap();
        app.documents.active_session.document = Document::new(8, 8);
        app.documents.active_session.document.layers[0].locked = false;
        for layer in &mut app.documents.active_session.document.layers {
            layer.mask = Some(parfait_core::LayerMask {
                enabled: true,
                coverage: parfait_core::MaskCoverage::Dense(Arc::new(vec![
                    127;
                    64
                ])),
            });
        }
        app.documents.active_session.document.touch();
        app.set_layer_selection(if multiple { 3 } else { 2 }, 1);
        let before = app.documents.active_session.document.clone();
        app.execute_command(Command::DuplicateLayer, &egui::Context::default());
        assert_eq!(
            app.documents.active_session.document.layers.len(),
            if multiple { 4 } else { 3 }
        );
        for layer in app
            .documents
            .active_session
            .document
            .layers
            .iter()
            .filter(|layer| layer.name.ends_with(" copy"))
        {
            let original = before
                .layers
                .iter()
                .find(|original| {
                    layer.name == format!("{} copy", original.name)
                })
                .unwrap();
            assert!(Arc::ptr_eq(&layer.pixels, &original.pixels));
            if let (
                Some(parfait_core::LayerMask {
                    coverage: parfait_core::MaskCoverage::Dense(current),
                    ..
                }),
                Some(parfait_core::LayerMask {
                    coverage: parfait_core::MaskCoverage::Dense(previous),
                    ..
                }),
            ) = (&layer.mask, &original.mask)
            {
                assert!(Arc::ptr_eq(current, previous));
            } else {
                panic!("mask was lost during duplicate");
            }
        }
        app.undo();
        assert!(app.documents.active_session.document.same_edit_state(&before));
        app.redo();
        let expected = app.documents.active_session.document.clone();
        app.force_save();
        drop(app);
        let reopened =
            Parfait::open_legacy(directory.path().to_owned()).unwrap();
        assert_eq!(reopened.documents.active_session.document, expected);
        assert_eq!(
            reopened.documents.active_session.document.composite(),
            expected.composite()
        );
    }
}

#[test]
fn clipping_property_menu_and_key_share_undo_and_durable_state() {
    for route in 0..3 {
        let directory = tempfile::tempdir().unwrap();
        let mut app =
            Parfait::open_legacy(directory.path().to_owned()).unwrap();
        app.documents.active_session.document = Document::new(8, 8);
        app.documents.active_session.document.layers[0].visible = false;
        app.documents.active_session.document.layers[1].pixels =
            Arc::new([255, 20, 50, 255].repeat(64));
        let original = app.documents.active_session.document.clone();
        let context = egui::Context::default();
        crate::theme::apply(&context);
        app.keymap
            .assign(
                Command::LayerClipping,
                None,
                Chord::new(egui::Key::F8, false, false, false),
                false,
            )
            .unwrap();
        let render = |app: &mut Parfait, events| {
            let mut output = context.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(320., 600.),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| {
                    app.prepare_layer_list(ui.ctx());
                    app.shortcuts(ui.ctx());
                    if route == 0 {
                        app.palette(
                            ui,
                            crate::workspace::Palette::Layers,
                            500.,
                        );
                    } else {
                        app.top_menu_toggle(
                            ui,
                            Command::LayerClipping,
                            app.documents.active_session.document.layers
                                [app.documents.active_session.document.active]
                                .clipped,
                        );
                    }
                },
            );
            output.textures_delta.clear();
        };
        render(&mut app, vec![]);
        render(&mut app, vec![]);
        if route == 2 {
            render(&mut app, vec![key(egui::Key::F8, false)]);
        } else {
            let id = if route == 0 {
                egui::Id::new(("layer_property", Command::LayerClipping))
            } else {
                egui::Id::new(("command_toggle", Command::LayerClipping))
            };
            let rect =
                context.data(|data| data.get_temp::<egui::Rect>(id)).unwrap();
            for pressed in [true, false] {
                render(
                    &mut app,
                    vec![
                        egui::Event::PointerMoved(rect.center()),
                        egui::Event::PointerButton {
                            pos: rect.center(),
                            pressed,
                            button: egui::PointerButton::Primary,
                            modifiers: Default::default(),
                        },
                    ],
                );
            }
        }
        assert!(app.documents.active_session.document.layers[1].clipped);
        assert_eq!(app.documents.active_session.history.rows().count(), 2);
        assert!(
            app.documents
                .active_session
                .document
                .composite()
                .iter()
                .all(|byte| *byte == 0)
        );
        assert!(Arc::ptr_eq(
            &app.documents.active_session.document.layers[1].pixels,
            &original.layers[1].pixels
        ));
        app.undo();
        assert!(!app.documents.active_session.document.layers[1].clipped);
        assert_eq!(
            app.documents.active_session.document.composite(),
            original.composite()
        );
        app.redo();
        app.persist_preferences(true);
        let path = directory.path().join("clipped.parfait");
        let mut store = parfait_storage::Store::open(&path).unwrap();
        store.save(&app.documents.active_session.document).unwrap();
        assert_eq!(
            store.load().unwrap().unwrap().document,
            app.documents.active_session.document
        );
        drop(app);
        let app = Parfait::open_legacy(directory.path().to_owned()).unwrap();
        assert_eq!(app.keymap.hint(Command::LayerClipping), "F8");
    }
}

#[test]
fn clipping_selection_is_atomic_and_stroke_and_modal_guards_apply() {
    let directory = tempfile::tempdir().unwrap();
    let mut app = Parfait::open_legacy(directory.path().to_owned()).unwrap();
    app.documents.active_session.document = Document::new(8, 8);
    app.documents.active_session.document.add_layer();
    let context = egui::Context::default();
    app.set_layer_selection(7, 0);
    let before = app.documents.active_session.document.clone();
    assert!(app.command_disabled_reason(Command::LayerClipping).is_some());
    app.execute_command(Command::LayerClipping, &context);
    assert_eq!(app.documents.active_session.document, before);
    app.set_layer_selection(6, 1);
    app.execute_command(Command::LayerClipping, &context);
    assert!(!app.documents.active_session.document.layers[0].clipped);
    assert!(
        app.documents.active_session.document.layers[1..]
            .iter()
            .all(|layer| layer.clipped)
    );
    assert_eq!(app.documents.active_session.history.rows().count(), 2);
    app.undo();
    assert!(
        app.documents
            .active_session
            .document
            .layers
            .iter()
            .all(|layer| !layer.clipped)
    );
    app.redo();
    app.set_layer_selection(6, 1);
    app.execute_command(Command::LayerClipping, &context);
    assert!(
        app.documents
            .active_session
            .document
            .layers
            .iter()
            .all(|layer| !layer.clipped)
    );
    for blocker in 0..3 {
        app.confirm_new = blocker == 0;
        app.workspace.resizing = blocker == 1;
        if blocker == 2 {
            app.documents.active_session.editing.stroke = Some(Stroke::begin(
                &mut app.documents.active_session.document,
                Brush { opacity: 0., ..Default::default() },
                Point { x: 2., y: 2., pressure: 1. },
            ));
        }
        let before = app.documents.active_session.document.clone();
        assert!(app.command_disabled_reason(Command::LayerClipping).is_some());
        app.execute_command(Command::LayerClipping, &context);
        assert_eq!(app.documents.active_session.document, before);
        app.documents.active_session.editing.stroke = None;
    }
}

fn frame(
    app: &mut Parfait,
    ctx: &egui::Context,
    events: Vec<egui::Event>,
    palette: bool,
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
            if palette {
                app.palette(ui, crate::workspace::Palette::Layers, 400.);
            } else {
                app.command_menu_item(ui, Command::MoveLayerDown);
                app.command_toggle(
                    ui,
                    Command::LayerVisibility,
                    app.documents.active_session.document.layers
                        [app.documents.active_session.document.active]
                        .visible,
                );
            }
            app.settle_layer_edit(ui.ctx());
        },
    );
    output.textures_delta.clear();
}

fn click(
    app: &mut Parfait,
    ctx: &egui::Context,
    rect: egui::Rect,
    palette: bool,
) {
    for pressed in [true, false] {
        frame(
            app,
            ctx,
            vec![
                egui::Event::PointerMoved(rect.center()),
                egui::Event::PointerButton {
                    pos: rect.center(),
                    pressed,
                    button: egui::PointerButton::Primary,
                    modifiers: Default::default(),
                },
            ],
            palette,
        );
    }
}

fn key(key: egui::Key, repeat: bool) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat,
        modifiers: Default::default(),
    }
}

#[test]
fn layer_edits_share_buffers_and_survive_undo_redo_and_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = Parfait::open_legacy(dir.path().to_owned()).unwrap();
    app.documents.active_session.document = Document::new(4, 4);
    Arc::make_mut(&mut app.documents.active_session.document.layers[1].pixels)
        [..4]
        .copy_from_slice(&[120, 60, 30, 255]);
    let before = app.documents.active_session.document.clone();
    let ctx = egui::Context::default();
    for (index, command) in
        [Command::MoveLayerDown, Command::LayerLock, Command::LayerVisibility]
            .into_iter()
            .enumerate()
    {
        app.execute_command(command, &ctx);
        assert!(Arc::ptr_eq(
            &app.documents.active_session.document.layers
                [app.documents.active_session.document.active]
                .pixels,
            &before.layers[1].pixels
        ));
        assert_eq!(
            app.documents.active_session.history.rows().count(),
            index + 2
        );
        assert_eq!(
            app.documents.active_session.history.rows().last().unwrap().label,
            command.label()
        );
        let edited = app.documents.active_session.document.clone();
        app.undo();
        app.redo();
        assert_eq!(app.documents.active_session.document.layers, edited.layers);
        assert_eq!(app.documents.active_session.document.active, edited.active);
        assert!(
            app.documents.active_session.document.revision > edited.revision
        );
    }
    assert_eq!(app.documents.active_session.document.active, 0);
    assert!(app.documents.active_session.document.layers[0].locked);
    assert!(!app.documents.active_session.document.layers[0].visible);
    app.execute_command(Command::MoveLayerUp, &ctx);
    assert_eq!(app.documents.active_session.document.active, 1);
    app.execute_command(Command::DeleteLayer, &ctx);
    assert_eq!(app.documents.active_session.document.layers.len(), 1);
    assert_eq!(app.documents.active_session.document.active, 0);
    app.undo();
    assert_eq!(app.documents.active_session.document.layers.len(), 2);
    app.redo();
    let path = dir.path().join("layers.parfait");
    parfait_storage::Store::open(&path)
        .unwrap()
        .save(&app.documents.active_session.document)
        .unwrap();
    let loaded = parfait_storage::Store::open(&path)
        .unwrap()
        .load()
        .unwrap()
        .unwrap()
        .document;
    assert_eq!(loaded, app.documents.active_session.document);
    assert!(loaded.validate());
}

#[test]
fn refused_layer_commands_and_input_owners_preserve_redo() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = Parfait::open_legacy(dir.path().to_owned()).unwrap();
    app.documents.active_session.document = Document::new(4, 4);
    app.documents.active_session.document.layers.truncate(1);
    app.documents.active_session.document.active = 0;
    let ctx = egui::Context::default();
    app.execute_command(Command::LayerVisibility, &ctx);
    app.undo();
    let before = app.documents.active_session.document.clone();
    for command in
        [Command::DeleteLayer, Command::MoveLayerUp, Command::MoveLayerDown]
    {
        assert!(app.command_disabled_reason(command).is_some());
        app.execute_command(command, &ctx);
        assert_eq!(app.documents.active_session.document, before);
        assert!(app.documents.active_session.history.can_redo());
    }
    for owner in 0..4 {
        app.show_help = owner == 0;
        app.workspace.resizing = owner == 1;
        app.workspace.dragging =
            (owner == 2).then_some(crate::workspace::Palette::Layers);
        if owner == 3 {
            app.documents.active_session.editing.stroke = Some(Stroke::begin(
                &mut app.documents.active_session.document,
                Brush { opacity: 0., ..Default::default() },
                Point { x: 2., y: 2., pressure: 1. },
            ));
        }
        for command in [
            Command::NewLayer,
            Command::DuplicateLayer,
            Command::DeleteLayer,
            Command::MoveLayerUp,
            Command::MoveLayerDown,
            Command::LayerLock,
            Command::LayerVisibility,
            Command::LayerAlphaLock,
        ] {
            assert!(app.command_disabled_reason(command).is_some());
            app.execute_command(command, &ctx);
            assert_eq!(app.documents.active_session.document, before);
            assert!(app.documents.active_session.history.can_redo());
        }
    }
    app.documents.active_session.editing.stroke = None;
    app.execute_layer_command(Command::LayerVisibility, usize::MAX);
    assert_eq!(app.documents.active_session.document, before);
    assert!(app.documents.active_session.history.can_redo());
}

#[test]
fn inactive_row_targets_do_not_select_and_reordering_preserves_surviving_target()
 {
    let dir = tempfile::tempdir().unwrap();
    let mut app = Parfait::open_legacy(dir.path().to_owned()).unwrap();
    app.documents.active_session.document = Document::new(4, 4);
    app.new_layer();
    let selected =
        app.documents.active_session.document.layers[2].pixels.clone();
    for command in [Command::LayerLock, Command::LayerVisibility] {
        app.execute_layer_command(command, 0);
        assert_eq!(app.documents.active_session.document.active, 2);
    }
    assert!(!app.documents.active_session.document.layers[0].locked);
    assert!(!app.documents.active_session.document.layers[0].visible);
    app.execute_layer_command(Command::MoveLayerUp, 1);
    assert_eq!(app.documents.active_session.document.active, 1);
    assert!(Arc::ptr_eq(
        &app.documents.active_session.document.layers[1].pixels,
        &selected
    ));
    app.execute_layer_command(Command::DeleteLayer, 0);
    assert_eq!(app.documents.active_session.document.active, 0);
    assert!(Arc::ptr_eq(
        &app.documents.active_session.document.layers[0].pixels,
        &selected
    ));
}

#[test]
fn actual_menu_palette_and_keyboard_routes_produce_the_same_edit() {
    let ctx = egui::Context::default();
    crate::theme::apply(&ctx);
    let mut expected = None;
    for route in 0..3 {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open_legacy(dir.path().to_owned()).unwrap();
        app.documents.active_session.document = Document::new(4, 4);
        frame(&mut app, &ctx, vec![], route == 1);
        frame(&mut app, &ctx, vec![], route == 1);
        match route {
            0 => {
                let rect = ctx
                    .data(|data| {
                        data.get_temp::<egui::Rect>(egui::Id::new((
                            "menu_item",
                            Command::MoveLayerDown.label(),
                        )))
                    })
                    .unwrap();
                click(&mut app, &ctx, rect, false);
            }
            1 => {
                let rect = ctx
                    .data(|data| {
                        data.get_temp::<egui::Rect>(egui::Id::new((
                            "layer_command",
                            Command::MoveLayerDown,
                        )))
                    })
                    .unwrap();
                assert!(rect.right() < 320.);
                click(&mut app, &ctx, rect, true);
            }
            _ => {
                app.keymap
                    .assign(
                        Command::MoveLayerDown,
                        None,
                        Chord::new(egui::Key::F6, false, false, false),
                        false,
                    )
                    .unwrap();
                frame(&mut app, &ctx, vec![key(egui::Key::F6, false)], false);
            }
        }
        assert_eq!(app.documents.active_session.document.active, 0);
        assert_eq!(app.documents.active_session.history.rows().count(), 2);
        if let Some(expected) = &expected {
            assert_eq!(&app.documents.active_session.document, expected);
        } else {
            expected = Some(app.documents.active_session.document.clone());
        }
    }
}

#[test]
fn shared_pixel_layers_with_distinct_ids_reorder_as_a_real_edit() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = Parfait::open_legacy(dir.path().to_owned()).unwrap();
    app.documents.active_session.document = Document::new(4, 4);
    {
        let mut copy = app.documents.active_session.document.layers[1].clone();
        copy.id =
            app.documents.active_session.document.allocate_layer_id().unwrap();
        app.documents.active_session.document.layers.push(copy);
    }
    let ctx = egui::Context::default();
    app.execute_command(Command::LayerVisibility, &ctx);
    app.undo();
    let revision = app.documents.active_session.document.revision;
    let rows = app.documents.active_session.history.rows().count();
    app.execute_command(Command::MoveLayerUp, &ctx);
    assert_eq!(app.documents.active_session.document.active, 2);
    assert!(app.documents.active_session.document.revision > revision);
    assert_eq!(app.documents.active_session.history.rows().count(), rows);
    assert!(!app.documents.active_session.history.can_redo());
    assert_eq!(
        app.documents.active_session.document.layers
            [app.documents.active_session.document.active]
            .id,
        parfait_core::LayerId(2)
    );
    assert!(app.documents.active_session.document.validate());
}

#[test]
fn checkbox_rows_and_menu_toggle_share_state_without_changing_active_layer() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = Parfait::open_legacy(dir.path().to_owned()).unwrap();
    app.documents.active_session.document = Document::new(4, 4);
    let ctx = egui::Context::default();
    crate::theme::apply(&ctx);
    frame(&mut app, &ctx, vec![], true);
    frame(&mut app, &ctx, vec![], true);
    for command in [Command::LayerVisibility, Command::LayerLock] {
        let rect = ctx
            .data(|data| {
                data.get_temp::<egui::Rect>(egui::Id::new((
                    "layer_checkbox",
                    command,
                    0usize,
                )))
            })
            .unwrap();
        click(&mut app, &ctx, rect, true);
        assert_eq!(app.documents.active_session.document.active, 1);
    }
    assert!(!app.documents.active_session.document.layers[0].visible);
    assert!(!app.documents.active_session.document.layers[0].locked);
    frame(&mut app, &ctx, vec![], false);
    let rect = ctx
        .data(|data| {
            data.get_temp::<egui::Rect>(egui::Id::new((
                "command_toggle",
                Command::LayerVisibility,
            )))
        })
        .unwrap();
    click(&mut app, &ctx, rect, false);
    assert!(!app.documents.active_session.document.layers[1].visible);
    assert_eq!(app.documents.active_session.history.rows().count(), 4);
    app.undo();
    assert!(app.documents.active_session.document.layers[1].visible);
}

#[test]
fn new_commands_ignore_key_repeat_and_text_focus() {
    for command in [
        Command::DeleteLayer,
        Command::MoveLayerUp,
        Command::MoveLayerDown,
        Command::LayerVisibility,
        Command::LayerLock,
        Command::LayerAlphaLock,
    ] {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open_legacy(dir.path().to_owned()).unwrap();
        app.documents.active_session.document = Document::new(4, 4);
        app.new_layer();
        app.documents.active_session.document.active = 1;
        app.keymap
            .assign(
                command,
                None,
                Chord::new(egui::Key::F6, false, false, false),
                false,
            )
            .unwrap();
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let before = app.documents.active_session.document.clone();
        frame(&mut app, &ctx, vec![key(egui::Key::F6, false)], false);
        assert!(
            !app.documents.active_session.document.same_edit_state(&before)
        );
        let after_first_press = app.documents.active_session.document.clone();
        frame(&mut app, &ctx, vec![key(egui::Key::F6, true)], false);
        assert_eq!(app.documents.active_session.document, after_first_press);
        app.undo();
        let before_text = app.documents.active_session.document.clone();
        let ctx = egui::Context::default();
        let mut output = ctx.run_ui(
            egui::RawInput {
                events: vec![key(egui::Key::F6, false)],
                ..Default::default()
            },
            |ui| {
                ui.text_edit_singleline(&mut String::new()).request_focus();
                app.shortcuts(ui.ctx());
            },
        );
        output.textures_delta.clear();
        assert_eq!(app.documents.active_session.document, before_text);
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        frame(&mut app, &ctx, vec![key(egui::Key::F6, false)], false);
        assert!(
            !app.documents.active_session.document.same_edit_state(&before)
        );
    }
}

#[test]
fn command_catalog_and_preferences_keep_keys_layout_and_unknown_originals() {
    assert_eq!(Command::ALL.len(), 93);
    let dir = tempfile::tempdir().unwrap();
    let mut app = Parfait::open_legacy(dir.path().to_owned()).unwrap();
    app.documents.active_session.document = Document::new(4, 4);
    app.workspace.panels[0].height = 315.;
    app.persist_preferences(true);
    let mut legacy: serde_json::Value =
        serde_json::from_str(&app.saved_preferences_json()).unwrap();
    legacy["version"] = 10.into();
    let path = dir.path().join("preferences.sqlite");
    parfait_storage::save_preferences(&path, &legacy.to_string()).unwrap();
    let keys = app.keymap.clone();
    drop(app);
    let mut app = Parfait::open_legacy(dir.path().to_owned()).unwrap();
    assert!(!app.preferences_read_failed);
    assert_eq!(app.workspace.panels[0].height, 315.);
    assert!(app.keymap == keys);
    for (command, key) in [
        (Command::DeleteLayer, egui::Key::F6),
        (Command::MoveLayerUp, egui::Key::F7),
        (Command::MoveLayerDown, egui::Key::F8),
        (Command::LayerVisibility, egui::Key::F9),
        (Command::LayerLock, egui::Key::F10),
    ] {
        assert!(app.keymap.hint(command).is_empty());
        app.keymap
            .assign(command, None, Chord::new(key, false, false, false), false)
            .unwrap();
        assert!(app.keymap.tooltip(command).contains(command.label()));
        assert_eq!(app.keymap.hint(command), key.name());
    }
    for &command in Command::ALL {
        assert!(app.keymap.tooltip(command).starts_with(command.label()));
        assert_eq!(
            serde_json::from_value::<Command>(
                serde_json::to_value(command).unwrap()
            )
            .unwrap(),
            command
        );
    }
    app.persist_preferences(true);
    let keys = app.keymap.clone();
    let mut saved: serde_json::Value =
        serde_json::from_str(&app.saved_preferences_json()).unwrap();
    assert_eq!(saved["version"], 21);
    drop(app);
    let mut app = Parfait::open_legacy(dir.path().to_owned()).unwrap();
    assert!(app.keymap == keys);
    assert_eq!(app.workspace.panels[0].height, 315.);
    saved["keymap"]["bindings"][0]["command"] = "unknown_future_command".into();
    let original = saved.to_string();
    parfait_storage::save_preferences(&path, &original).unwrap();
    app.restore_preferences();
    assert!(app.preferences_read_failed);
    app.persist_preferences(true);
    assert_eq!(app.saved_preferences_json(), original);
}

#[test]
fn raster_combine_routes_share_transactions_undo_and_durable_pixels() {
    for command in [
        Command::TransferLayerDown,
        Command::MergeLayerDown,
        Command::MergeSelectedLayers,
        Command::MergeVisibleLayers,
        Command::CopyVisibleLayers,
        Command::FlattenImage,
        Command::ClearLayer,
    ] {
        let mut expected = None;
        for route in 0..3 {
            let dir = tempfile::tempdir().unwrap();
            let mut app = Parfait::open_legacy(dir.path().to_owned()).unwrap();
            app.documents.active_session.document = Document::new(4, 4);
            app.documents.active_session.document.add_layer();
            for (index, layer) in app
                .documents
                .active_session
                .document
                .layers
                .iter_mut()
                .enumerate()
            {
                layer.locked = false;
                Arc::make_mut(&mut layer.pixels)[..4].copy_from_slice(&[
                    index as u8 * 90,
                    70,
                    150,
                    180,
                ]);
            }
            if command == Command::MergeSelectedLayers {
                app.set_layer_selection(0b110, 2);
            }
            let before = app.documents.active_session.document.clone();
            let ctx = egui::Context::default();
            crate::theme::apply(&ctx);
            app.keymap
                .assign(
                    command,
                    None,
                    Chord::new(egui::Key::F6, false, false, false),
                    false,
                )
                .unwrap();
            let render = |app: &mut Parfait, events| {
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
                        app.prepare_layer_list(ui.ctx());
                        app.shortcuts(ui.ctx());
                        if route == 1 {
                            app.layer_command_icon(ui, Icon::Merge, command);
                        } else {
                            app.command_menu_item(ui, command);
                        }
                    },
                );
                output.textures_delta.clear();
            };
            render(&mut app, vec![]);
            render(&mut app, vec![]);
            if route == 2 {
                render(&mut app, vec![key(egui::Key::F6, false)]);
            } else {
                let rect = if route == 1 {
                    ctx.data(|data| {
                        data.get_temp::<egui::Rect>(egui::Id::new((
                            "layer_command",
                            command,
                        )))
                    })
                    .unwrap()
                } else {
                    ctx.data(|data| {
                        data.get_temp::<egui::Rect>(egui::Id::new((
                            "menu_item",
                            command.label(),
                        )))
                    })
                    .unwrap()
                };
                for pressed in [true, false] {
                    render(
                        &mut app,
                        vec![
                            egui::Event::PointerMoved(rect.center()),
                            egui::Event::PointerButton {
                                pos: rect.center(),
                                pressed,
                                button: egui::PointerButton::Primary,
                                modifiers: Default::default(),
                            },
                        ],
                    );
                }
            }
            assert_eq!(
                app.documents.active_session.history.rows().count(),
                2,
                "{command:?} route {route}"
            );
            let result = app.documents.active_session.document.clone();
            if let Some(expected) = &expected {
                assert_eq!(&result, expected);
            } else {
                expected = Some(result.clone());
            }
            app.undo();
            assert_eq!(
                app.documents.active_session.document.layers,
                before.layers
            );
            assert_eq!(
                app.documents.active_session.document.active,
                before.active
            );
            app.redo();
            assert_eq!(
                app.documents.active_session.document.layers,
                result.layers
            );
            assert_eq!(
                app.documents.active_session.document.active,
                result.active
            );
            let path = dir.path().join("combined.parfait");
            let mut store = parfait_storage::Store::open(&path).unwrap();
            store.save(&app.documents.active_session.document).unwrap();
            drop(store);
            assert_eq!(
                parfait_storage::Store::open(&path)
                    .unwrap()
                    .load()
                    .unwrap()
                    .unwrap()
                    .document,
                app.documents.active_session.document
            );
        }
    }
}

#[test]
fn raster_combine_noops_refusals_and_copy_budget_preserve_future_and_shared_pixels()
 {
    let dir = tempfile::tempdir().unwrap();
    let mut app = Parfait::open_legacy(dir.path().to_owned()).unwrap();
    app.documents.active_session.document = Document::new(4, 4);
    let ctx = egui::Context::default();
    app.new_layer();
    app.undo();
    let before = app.documents.active_session.document.clone();
    for command in [
        Command::ClearLayer,
        Command::TransferLayerDown,
        Command::MergeLayerDown,
        Command::MergeSelectedLayers,
        Command::FlattenImage,
    ] {
        app.execute_command(command, &ctx);
        assert_eq!(app.documents.active_session.document, before);
        assert!(app.documents.active_session.history.can_redo());
    }
    app.documents.active_session.document.width = 4096;
    app.documents.active_session.document.height = 4096;
    while app.documents.active_session.document.layers.len() < 7 {
        {
            let mut copy =
                app.documents.active_session.document.layers[1].clone();
            copy.id = app
                .documents
                .active_session
                .document
                .allocate_layer_id()
                .unwrap();
            app.documents.active_session.document.layers.push(copy);
        }
    }
    let before = app.documents.active_session.document.clone();
    assert!(app.command_disabled_reason(Command::CopyVisibleLayers).is_some());
    app.execute_command(Command::CopyVisibleLayers, &ctx);
    assert_eq!(app.documents.active_session.document, before);
    assert!(app.documents.active_session.history.can_redo());
    for (layer, before) in
        app.documents.active_session.document.layers.iter().zip(&before.layers)
    {
        assert!(Arc::ptr_eq(&layer.pixels, &before.pixels));
    }
}

#[test]
fn version_eleven_keys_and_layout_upgrade_to_thirteen_without_new_default_bindings()
 {
    let dir = tempfile::tempdir().unwrap();
    let mut app = Parfait::open_legacy(dir.path().to_owned()).unwrap();
    app.workspace.panels[0].height = 317.;
    app.keymap
        .assign(
            Command::LayerLock,
            None,
            Chord::new(egui::Key::F6, false, false, false),
            false,
        )
        .unwrap();
    app.persist_preferences(true);
    let keys = app.keymap.clone();
    let layout = app.workspace.clone();
    let mut legacy: serde_json::Value =
        serde_json::from_str(&app.saved_preferences_json()).unwrap();
    legacy["version"] = 11.into();
    parfait_storage::save_preferences(
        &dir.path().join("preferences.sqlite"),
        &legacy.to_string(),
    )
    .unwrap();
    drop(app);
    let mut app = Parfait::open_legacy(dir.path().to_owned()).unwrap();
    assert!(!app.preferences_read_failed);
    assert!(app.keymap == keys);
    assert!(app.workspace.same_preferences(&layout));
    for &command in Command::ALL {
        if command.layer_combine().is_some() {
            assert!(app.keymap.hint(command).is_empty());
            assert!(!command.repeatable());
        }
    }
    app.persist_preferences(true);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(
            &app.saved_preferences_json()
        )
        .unwrap()["version"],
        21
    );
}

#[test]
fn alpha_lock_menu_row_and_key_share_one_edit_and_reopen_with_pixels_and_ids() {
    for route in 0..3 {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open_legacy(dir.path().to_owned()).unwrap();
        app.documents.active_session.document = Document::new(8, 8);
        let before = app.documents.active_session.document.clone();
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        app.keymap
            .assign(
                Command::LayerAlphaLock,
                None,
                Chord::new(egui::Key::F6, false, false, false),
                false,
            )
            .unwrap();
        let render = |app: &mut Parfait, events| {
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
                    app.prepare_layer_list(ui.ctx());
                    app.shortcuts(ui.ctx());
                    if route == 1 {
                        app.palette(
                            ui,
                            crate::workspace::Palette::Layers,
                            500.,
                        );
                    } else {
                        app.top_menu_toggle(
                            ui,
                            Command::LayerAlphaLock,
                            app.documents.active_session.document.layers
                                [app.documents.active_session.document.active]
                                .alpha_locked,
                        );
                    }
                },
            );
            output.textures_delta.clear();
        };
        render(&mut app, vec![]);
        render(&mut app, vec![]);
        if route == 2 {
            render(&mut app, vec![key(egui::Key::F6, false)]);
        } else {
            let id = if route == 1 {
                egui::Id::new((
                    "layer_checkbox",
                    Command::LayerAlphaLock,
                    1usize,
                ))
            } else {
                egui::Id::new(("command_toggle", Command::LayerAlphaLock))
            };
            let rect =
                ctx.data(|data| data.get_temp::<egui::Rect>(id)).unwrap();
            for pressed in [true, false] {
                render(
                    &mut app,
                    vec![
                        egui::Event::PointerMoved(rect.center()),
                        egui::Event::PointerButton {
                            pos: rect.center(),
                            pressed,
                            button: egui::PointerButton::Primary,
                            modifiers: Default::default(),
                        },
                    ],
                );
            }
        }
        assert!(app.documents.active_session.document.layers[1].alpha_locked);
        assert_eq!(app.documents.active_session.document.active, 1);
        assert_eq!(
            app.documents.active_session.document.layers[1].id,
            before.layers[1].id
        );
        assert!(Arc::ptr_eq(
            &app.documents.active_session.document.layers[1].pixels,
            &before.layers[1].pixels
        ));
        assert_eq!(app.documents.active_session.history.rows().count(), 2);
        app.undo();
        assert!(!app.documents.active_session.document.layers[1].alpha_locked);
        app.redo();
        assert!(app.documents.active_session.document.layers[1].alpha_locked);
        let path = dir.path().join("alpha.parfait");
        let mut store = parfait_storage::Store::open(&path).unwrap();
        store.save(&app.documents.active_session.document).unwrap();
        drop(store);
        assert_eq!(
            parfait_storage::Store::open(&path)
                .unwrap()
                .load()
                .unwrap()
                .unwrap()
                .document,
            app.documents.active_session.document
        );
    }
}

#[test]
fn selected_ids_and_inline_name_target_survive_reordering_without_index_aliases()
 {
    let dir = tempfile::tempdir().unwrap();
    let mut app = Parfait::open_legacy(dir.path().to_owned()).unwrap();
    app.documents.active_session.document = Document::new(8, 8);
    app.documents.active_session.document.add_layer();
    app.documents.active_session.document.add_layer();
    app.documents.active_session.document.active = 2;
    app.set_layer_selection(0b0110, 1);
    let chosen = app.documents.active_session.document.layers[1].id;
    let active = app.documents.active_session.document.layers[2].id;
    app.documents.active_session.document.layers.swap(1, 3);
    app.documents.active_session.document.touch();
    assert_eq!(
        app.documents.active_session.document.layers
            [app.documents.active_session.document.active]
            .id,
        active
    );
    assert_eq!(app.selected_layer_mask(), 0b1100);
    let ctx = egui::Context::default();
    app.execute_command(Command::LayerAlphaLock, &ctx);
    assert!(
        app.documents.active_session.document.layers[app
            .documents
            .active_session
            .document
            .layer_index(chosen)
            .unwrap()]
        .alpha_locked
    );
    assert!(app.documents.active_session.document.layers[2].alpha_locked);
    assert!(!app.documents.active_session.document.layers[1].alpha_locked);
    app.begin_layer_rename(3);
    let name_id = app.documents.active_session.document.layers[3].id;
    app.reorder_layer_mask(1 << 3, 1);
    assert_eq!(
        app.documents.active_session.document.layer_index(name_id),
        Some(1)
    );
    crate::theme::apply(&ctx);
    for events in [
        vec![],
        vec![egui::Event::Text("Stable name".into())],
        vec![egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        }],
    ] {
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
                app.prepare_layer_list(ui.ctx());
                app.palette(ui, crate::workspace::Palette::Layers, 500.);
                app.settle_layer_edit(ui.ctx());
            },
        );
        output.textures_delta.clear();
    }
    assert_eq!(app.documents.active_session.document.layers[1].id, name_id);
    assert_eq!(
        app.documents.active_session.document.layers[1].name,
        "Stable name"
    );
    assert_eq!(app.documents.active_session.document.layers[3].id, active);
}

#[test]
fn previous_preferences_upgrade_without_reset_and_alpha_key_survives_restart() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = Parfait::open_legacy(dir.path().to_owned()).unwrap();
    app.workspace.panels[0].height = 319.;
    app.persist_preferences(true);
    let keys = app.keymap.clone();
    let mut legacy: serde_json::Value =
        serde_json::from_str(&app.saved_preferences_json()).unwrap();
    legacy["version"] = 12.into();
    parfait_storage::save_preferences(
        &dir.path().join("preferences.sqlite"),
        &legacy.to_string(),
    )
    .unwrap();
    drop(app);
    let mut app = Parfait::open_legacy(dir.path().to_owned()).unwrap();
    assert!(!app.preferences_read_failed);
    assert!(app.keymap == keys);
    assert_eq!(app.workspace.panels[0].height, 319.);
    assert!(app.keymap.hint(Command::LayerAlphaLock).is_empty());
    assert!(!Command::LayerAlphaLock.repeatable());
    app.keymap
        .assign(
            Command::LayerAlphaLock,
            None,
            Chord::new(egui::Key::F7, false, false, false),
            false,
        )
        .unwrap();
    app.persist_preferences(true);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(
            &app.saved_preferences_json()
        )
        .unwrap()["version"],
        21
    );
    drop(app);
    let app = Parfait::open_legacy(dir.path().to_owned()).unwrap();
    assert_eq!(app.keymap.hint(Command::LayerAlphaLock), "F7");
}
