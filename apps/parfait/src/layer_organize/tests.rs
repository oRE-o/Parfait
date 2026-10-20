use super::*;
use crate::shortcuts::Chord;
use parfait_core::{Brush, Document, Point, Stroke};

fn sample(directory: &std::path::Path) -> Parfait {
    let mut app = Parfait::open(directory.to_owned()).unwrap();
    app.documents.session.document = Document::new(8, 8);
    for _ in 0..3 {
        app.documents.session.document.add_layer();
    }
    for (index, layer) in
        app.documents.session.document.layers.iter_mut().enumerate()
    {
        layer.name = format!("Layer {index}");
        layer.locked = index % 2 == 0;
        layer.visible = index % 2 == 1;
        Arc::make_mut(&mut layer.pixels)[..4].copy_from_slice(&[
            index as u8,
            80,
            120,
            128,
        ]);
    }
    app
}

fn key(
    key: egui::Key,
    modifiers: egui::Modifiers,
    repeat: bool,
) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat,
        modifiers,
    }
}

fn frame(
    app: &mut Parfait,
    context: &egui::Context,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    context.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(960., 640.),
            )),
            events,
            ..Default::default()
        },
        |ui| {
            app.prepare_layer_list(ui.ctx());
            app.shortcuts(ui.ctx());
            ui.add_enabled_ui(!app.dialog_open(), |ui| app.canvas(ui));
            app.layer_rename_ui(ui.ctx());
        },
    )
}

fn settle(app: &mut Parfait, context: &egui::Context) {
    for _ in 0..3 {
        frame(app, context, vec![]).textures_delta.clear();
    }
}

fn click(app: &mut Parfait, context: &egui::Context, name: &str) {
    let rect = context
        .data(|data| data.get_temp::<egui::Rect>(egui::Id::new(name)))
        .unwrap();
    for pressed in [true, false] {
        frame(
            app,
            context,
            vec![
                egui::Event::PointerMoved(rect.center()),
                egui::Event::PointerButton {
                    pos: rect.center(),
                    pressed,
                    button: egui::PointerButton::Primary,
                    modifiers: Default::default(),
                },
            ],
        )
        .textures_delta
        .clear();
    }
}

#[test]
fn active_ordering_preserves_other_selection_relative_order_ids_pixels_and_one_undo()
 {
    for (command, destination) in [
        (Command::MoveActiveLayerUp, 3),
        (Command::MoveActiveLayerDown, 1),
        (Command::MoveActiveLayerToTop, 4),
        (Command::MoveActiveLayerToBottom, 0),
    ] {
        let directory = tempfile::tempdir().unwrap();
        let mut app = sample(directory.path());
        app.select_layer(2);
        app.set_layer_selection(0b10101, 0);
        app.documents.session.document.layers[3].clipped = true;
        let before = app.documents.session.document.clone();
        let active = before.layers[2].id;
        let mut expected: Vec<_> =
            before.layers.iter().map(|layer| layer.id).collect();
        expected.remove(2);
        expected.insert(destination, active);
        app.execute_command(command, &egui::Context::default());
        assert_eq!(
            app.documents
                .session
                .document
                .layers
                .iter()
                .map(|layer| layer.id)
                .collect::<Vec<_>>(),
            expected
        );
        assert_eq!(app.documents.session.document.active, destination);
        assert_eq!(app.documents.session.history.rows().count(), 2);
        for layer in &app.documents.session.document.layers {
            let original =
                &before.layers[before.layer_index(layer.id).unwrap()];
            assert_eq!(layer, original);
            assert!(Arc::ptr_eq(&layer.pixels, &original.pixels));
        }
        assert_eq!(app.selected_layer_mask().count_ones(), 3);
        app.undo();
        assert!(app.documents.session.document.same_edit_state(&before));
        app.redo();
        assert_eq!(
            app.documents.session.document.layers
                [app.documents.session.document.active]
                .id,
            active
        );
        let path = directory.path().join("ordering.parfait");
        let mut store = parfait_storage::Store::open(&path).unwrap();
        store.save(&app.documents.session.document).unwrap();
        assert_eq!(
            store.load().unwrap().unwrap().document,
            app.documents.session.document
        );
    }
}

#[test]
fn selected_moves_remain_distinct_and_boundary_commands_are_true_no_ops() {
    let directory = tempfile::tempdir().unwrap();
    let mut app = sample(directory.path());
    app.select_layer(2);
    app.set_layer_selection(0b00101, 0);
    let before = app.documents.session.document.clone();
    app.execute_command(Command::MoveLayerUp, &egui::Context::default());
    assert_eq!(
        app.documents.session.document.layers[1].id,
        before.layers[0].id
    );
    assert_eq!(
        app.documents.session.document.layers[3].id,
        before.layers[2].id
    );
    app.undo();
    assert!(app.documents.session.history.can_redo());
    for (active, command) in [
        (4, Command::MoveActiveLayerToTop),
        (4, Command::MoveActiveLayerUp),
        (0, Command::MoveActiveLayerToBottom),
        (0, Command::MoveActiveLayerDown),
    ] {
        app.select_layer(active);
        let before = app.documents.session.document.clone();
        assert!(app.command_disabled_reason(command).is_some());
        app.execute_command(command, &egui::Context::default());
        assert_eq!(app.documents.session.document, before);
        assert!(app.documents.session.history.can_redo());
    }
}

#[test]
fn cyclic_navigation_clears_additional_selection_without_uploads_history_or_brush_changes()
 {
    let directory = tempfile::tempdir().unwrap();
    let mut app = sample(directory.path());
    let context = egui::Context::default();
    crate::theme::apply(&context);
    app.documents.session.document.layers[0].locked = true;
    app.documents.session.document.layers[0].visible = false;
    app.set_layer_selection(0b10001, 0);
    settle(&mut app, &context);
    let image = app.render_cache.texture.as_ref().unwrap().id();
    let before = app.documents.session.document.clone();
    let brush = app.brush.clone();
    let output = frame(
        &mut app,
        &context,
        vec![key(egui::Key::CloseBracket, egui::Modifiers::ALT, false)],
    );
    assert_eq!(app.documents.session.document.active, 0);
    assert_eq!(app.selected_layer_mask(), 1);
    assert!(app.documents.session.document.same_edit_state(&before));
    assert_eq!(app.documents.session.history.rows().count(), 0);
    assert!(!output.textures_delta.set.iter().any(|(id, _)| *id == image));
    assert!(app.brush == brush);
    frame(
        &mut app,
        &context,
        vec![key(egui::Key::OpenBracket, egui::Modifiers::ALT, true)],
    )
    .textures_delta
    .clear();
    assert_eq!(app.documents.session.document.active, 4);
    for _ in 0..5 {
        app.execute_command(Command::SelectLayerBelow, &context);
    }
    assert_eq!(app.documents.session.document.active, 4);
}

#[test]
fn organization_guards_and_one_layer_or_sixty_four_layer_boundaries() {
    let directory = tempfile::tempdir().unwrap();
    let mut app = sample(directory.path());
    let commands = [
        Command::RenameLayer,
        Command::MoveActiveLayerUp,
        Command::MoveActiveLayerDown,
        Command::MoveActiveLayerToTop,
        Command::MoveActiveLayerToBottom,
        Command::SelectLayerAbove,
        Command::SelectLayerBelow,
    ];
    for blocker in 0..3 {
        app.select_layer(2);
        app.confirm_new = blocker == 0;
        app.workspace.resizing = blocker == 1;
        if blocker == 2 {
            app.documents.session.editing.stroke = Some(Stroke::begin(
                &mut app.documents.session.document,
                Brush { opacity: 0., ..Default::default() },
                Point { x: 2., y: 2., pressure: 1. },
            ));
        }
        let before = app.documents.session.document.clone();
        for command in commands {
            assert!(app.command_disabled_reason(command).is_some());
            app.execute_command(command, &egui::Context::default());
            assert_eq!(app.documents.session.document, before);
        }
        assert!(app.documents.session.layer_ui.rename.is_none());
        app.documents.session.editing.stroke = None;
    }
    app.workspace.resizing = false;
    app.confirm_new = false;
    while app.documents.session.document.layers.len() < 64 {
        app.documents.session.document.add_layer();
    }
    app.select_layer(63);
    app.execute_command(Command::SelectLayerAbove, &egui::Context::default());
    assert_eq!(app.documents.session.document.active, 0);
    app.execute_command(Command::SelectLayerBelow, &egui::Context::default());
    assert_eq!(app.documents.session.document.active, 63);
    app.documents.session.document.layers.truncate(1);
    app.documents.session.document.active = 0;
    let before = app.documents.session.document.clone();
    for command in [Command::SelectLayerAbove, Command::SelectLayerBelow] {
        assert!(app.command_disabled_reason(command).is_some());
        app.execute_command(command, &egui::Context::default());
        assert_eq!(app.documents.session.document, before);
    }
}

#[test]
fn rename_confirm_is_one_durable_unicode_edit_even_with_the_palette_hidden() {
    for enter in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let mut app = sample(directory.path());
        app.hide_panels = true;
        let context = egui::Context::default();
        crate::theme::apply(&context);
        let before = app.documents.session.document.clone();
        app.execute_command(Command::RenameLayer, &context);
        settle(&mut app, &context);
        frame(
            &mut app,
            &context,
            vec![egui::Event::Text("새 레이어 café".into())],
        )
        .textures_delta
        .clear();
        assert_eq!(app.documents.session.document, before);
        assert_eq!(app.documents.session.history.rows().count(), 0);
        if enter {
            frame(
                &mut app,
                &context,
                vec![key(egui::Key::Enter, Default::default(), false)],
            )
            .textures_delta
            .clear();
        } else {
            click(&mut app, &context, "rename_accept");
        }
        assert!(app.documents.session.layer_ui.rename.is_none());
        assert_eq!(
            app.documents.session.document.layers[4].name,
            "새 레이어 café"
        );
        assert!(Arc::ptr_eq(
            &app.documents.session.document.layers[4].pixels,
            &before.layers[4].pixels
        ));
        assert_eq!(
            app.documents.session.document.layers[4].id,
            before.layers[4].id
        );
        assert_eq!(app.documents.session.history.rows().count(), 2);
        app.undo();
        assert_eq!(
            app.documents.session.document.layers[4].name,
            before.layers[4].name
        );
        app.redo();
        let path = directory.path().join("renamed.parfait");
        let mut store = parfait_storage::Store::open(&path).unwrap();
        store.save(&app.documents.session.document).unwrap();
        assert_eq!(
            store.load().unwrap().unwrap().document,
            app.documents.session.document
        );
    }
}

#[test]
fn rename_cancel_no_op_blank_and_backdrop_preserve_document_and_redo() {
    for exit in ["rename_cancel", "rename_close", "escape", "unchanged"] {
        let directory = tempfile::tempdir().unwrap();
        let mut app = sample(directory.path());
        app.edit_named("Earlier edit", |document| {
            document.layers[0].name = "Before".into()
        });
        app.undo();
        let context = egui::Context::default();
        crate::theme::apply(&context);
        let before = app.documents.session.document.clone();
        app.execute_command(Command::RenameLayer, &context);
        settle(&mut app, &context);
        if exit != "unchanged" {
            frame(
                &mut app,
                &context,
                vec![egui::Event::Text("Draft only".into())],
            )
            .textures_delta
            .clear();
        }
        if exit == "escape" {
            frame(
                &mut app,
                &context,
                vec![key(egui::Key::Escape, Default::default(), false)],
            )
            .textures_delta
            .clear();
        } else {
            click(
                &mut app,
                &context,
                if exit == "unchanged" { "rename_accept" } else { exit },
            );
        }
        assert!(app.documents.session.layer_ui.rename.is_none());
        assert_eq!(app.documents.session.document, before);
        assert!(app.documents.session.history.can_redo());
    }
    let directory = tempfile::tempdir().unwrap();
    let mut app = sample(directory.path());
    let context = egui::Context::default();
    crate::theme::apply(&context);
    let before = app.documents.session.document.clone();
    app.execute_command(Command::RenameLayer, &context);
    settle(&mut app, &context);
    frame(
        &mut app,
        &context,
        vec![
            egui::Event::Text("   ".into()),
            key(egui::Key::Enter, Default::default(), false),
        ],
    )
    .textures_delta
    .clear();
    assert!(
        app.documents
            .session
            .layer_ui
            .rename
            .as_ref()
            .unwrap()
            .error()
            .is_some()
    );
    assert_eq!(app.documents.session.document, before);
    for pressed in [true, false] {
        frame(
            &mut app,
            &context,
            vec![egui::Event::PointerButton {
                pos: egui::pos2(20., 20.),
                pressed,
                button: egui::PointerButton::Primary,
                modifiers: Default::default(),
            }],
        )
        .textures_delta
        .clear();
    }
    assert!(app.documents.session.layer_ui.rename.is_some());
    assert!(app.documents.session.editing.stroke.is_none());
    assert_eq!(app.documents.session.document, before);
}

#[test]
fn rename_is_bounded_target_checked_and_ime_enter_does_not_commit_preedit() {
    let directory = tempfile::tempdir().unwrap();
    let mut app = sample(directory.path());
    let context = egui::Context::default();
    crate::theme::apply(&context);
    app.documents.session.document.layers[4].name = "가".repeat(5000);
    app.execute_command(Command::RenameLayer, &context);
    assert_eq!(
        app.documents
            .session
            .layer_ui
            .rename
            .as_ref()
            .unwrap()
            .draft
            .chars()
            .count(),
        128
    );
    assert!(
        app.documents.session.layer_ui.rename.as_ref().unwrap().draft.len()
            <= 512
    );
    assert!(
        app.documents
            .session
            .layer_ui
            .rename
            .as_ref()
            .unwrap()
            .error()
            .is_some()
    );
    settle(&mut app, &context);
    frame(
        &mut app,
        &context,
        vec![
            egui::Event::Ime(egui::ImeEvent::Preedit {
                text: "한".into(),
                active_range_chars: None,
            }),
            key(egui::Key::Enter, Default::default(), false),
        ],
    )
    .textures_delta
    .clear();
    assert!(app.documents.session.layer_ui.rename.is_some());
    assert_eq!(
        app.documents.session.document.layers[4].name.chars().count(),
        5000
    );
    frame(
        &mut app,
        &context,
        vec![
            egui::Event::Ime(egui::ImeEvent::Commit("한글".into())),
            key(egui::Key::Enter, Default::default(), false),
        ],
    )
    .textures_delta
    .clear();
    assert!(app.documents.session.layer_ui.rename.is_some());
    frame(
        &mut app,
        &context,
        vec![key(egui::Key::Enter, Default::default(), false)],
    )
    .textures_delta
    .clear();
    assert!(app.documents.session.layer_ui.rename.is_none());
    assert_eq!(app.documents.session.document.layers[4].name, "한글");
    app.execute_command(Command::RenameLayer, &context);
    app.documents.session.document = Document::new(8, 8);
    let before = app.documents.session.document.clone();
    frame(&mut app, &context, vec![]).textures_delta.clear();
    assert!(app.documents.session.layer_ui.rename.is_none());
    assert_eq!(app.documents.session.document, before);
}

#[test]
fn rename_layout_stays_inside_minimum_window_and_rejects_stale_revision() {
    let directory = tempfile::tempdir().unwrap();
    let mut app = sample(directory.path());
    let context = egui::Context::default();
    crate::theme::apply(&context);
    app.execute_command(Command::RenameLayer, &context);
    for _ in 0..30 {
        frame(&mut app, &context, vec![]).textures_delta.clear();
    }
    let modal = context
        .data(|data| data.get_temp::<egui::Rect>(egui::Id::new("rename_modal")))
        .unwrap();
    assert!(
        modal.left() >= 0.
            && modal.top() >= 0.
            && modal.right() <= 960.
            && modal.bottom() <= 640.
    );
    for name in
        ["rename_field", "rename_accept", "rename_cancel", "rename_close"]
    {
        let rect = context
            .data(|data| data.get_temp::<egui::Rect>(egui::Id::new(name)))
            .unwrap();
        assert!(modal.contains_rect(rect));
    }
    app.documents.session.document.touch();
    let before = app.documents.session.document.clone();
    frame(&mut app, &context, vec![]).textures_delta.clear();
    assert!(app.documents.session.layer_ui.rename.is_none());
    assert_eq!(app.documents.session.document, before);
}

#[test]
fn version_fourteen_keys_are_preserved_and_new_assignments_reopen_in_version_fifteen()
 {
    let directory = tempfile::tempdir().unwrap();
    let mut app = sample(directory.path());
    app.persist_preferences(true);
    let mut legacy: serde_json::Value =
        serde_json::from_str(&app.saved_preferences_json()).unwrap();
    legacy["version"] = 14.into();
    legacy["keymap"]["bindings"].as_array_mut().unwrap().retain(|binding| {
        !matches!(
            binding["command"].as_str(),
            Some("select_layer_above" | "select_layer_below")
        )
    });
    let expected: crate::preferences::Preferences =
        serde_json::from_value(legacy.clone()).unwrap();
    parfait_storage::save_preferences(
        &directory.path().join("preferences.sqlite"),
        &legacy.to_string(),
    )
    .unwrap();
    drop(app);
    let mut app = Parfait::open(directory.path().to_owned()).unwrap();
    assert!(!app.preferences_read_failed);
    assert!(app.keymap == expected.keymap);
    assert!(app.keymap.hint(Command::SelectLayerAbove).is_empty());
    app.keymap
        .assign(
            Command::RenameLayer,
            None,
            Chord::new(egui::Key::F6, false, false, false),
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
    let app = Parfait::open(directory.path().to_owned()).unwrap();
    assert_eq!(app.keymap.hint(Command::RenameLayer), "F6");
}
