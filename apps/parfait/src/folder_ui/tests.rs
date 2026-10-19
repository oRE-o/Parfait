use super::*;
use eframe::egui;
use parfait_core::Document;

#[test]
fn folder_commands_preserve_selection_history_and_project_roundtrip() {
    let directory = tempfile::tempdir().unwrap();
    let mut app = Parfait::open(directory.path().to_owned()).unwrap();
    app.doc = Document::new(8, 8);
    app.doc.add_layer();
    app.set_layer_selection(0b110, 2);
    let before = app.doc.clone();
    let context = egui::Context::default();
    app.execute_command(Command::GroupLayersInFolder, &context);
    let folder = app.doc.layers[3].id;
    assert_eq!(app.doc.layers[1].parent, Some(folder));
    assert_eq!(app.selected_layer_mask(), 1 << 3);
    assert_eq!(app.history.rows().count(), 2);
    let grouped = app.doc.clone();
    app.undo();
    assert!(app.doc.same_edit_state(&before));
    app.redo();
    assert!(app.doc.same_edit_state(&grouped));
    app.execute_command(Command::DuplicateLayer, &context);
    assert_eq!(app.doc.layers.len(), 7);
    assert!(app.doc.validate());
    app.execute_command(Command::DeleteLayer, &context);
    assert!(app.doc.same_edit_state(&grouped));
    app.execute_command(Command::UngroupLayerFolder, &context);
    assert!(app.doc.same_edit_state(&before));
    app.undo();
    app.execute_command(Command::NewLayerFolder, &context);
    assert_eq!(app.doc.layers[app.doc.active].parent, Some(folder));
    let path = directory.path().join("tree.parfait");
    app.save_project_to(path.clone()).unwrap();
    let loaded = parfait_storage::Store::open(&path)
        .unwrap()
        .load()
        .unwrap()
        .unwrap()
        .document;
    assert_eq!(loaded.layers, app.doc.layers);
}

#[test]
fn locked_folder_protects_structure_and_descendants() {
    let directory = tempfile::tempdir().unwrap();
    let mut app = Parfait::open(directory.path().to_owned()).unwrap();
    app.doc = Document::new(8, 8);
    app.doc.group_layers_in_folder(2, "Folder".into()).unwrap();
    app.select_layer(1);
    app.doc.layers[2].locked = true;
    assert!(
        app.command_disabled_reason(Command::NewLayerFolder)
            .is_some()
    );
    assert!(
        app.command_disabled_reason(Command::GroupLayersInFolder)
            .is_some()
    );
    assert!(app.command_disabled_reason(Command::DeleteLayer).is_some());
}

#[test]
fn previous_preferences_keep_existing_keys_and_new_folder_bindings_roundtrip() {
    let directory = tempfile::tempdir().unwrap();
    let mut app = Parfait::open(directory.path().to_owned()).unwrap();
    app.keymap
        .assign(
            Command::LayerLock,
            None,
            crate::shortcuts::Chord::new(egui::Key::F6, false, false, false),
            false,
        )
        .unwrap();
    app.persist_preferences(true);
    let mut legacy: serde_json::Value =
        serde_json::from_str(&app.saved_preferences_json()).unwrap();
    legacy["version"] = 19.into();
    let path = directory.path().join("preferences.sqlite");
    parfait_storage::save_preferences(&path, &legacy.to_string()).unwrap();
    drop(app);
    let mut app = Parfait::open(directory.path().to_owned()).unwrap();
    assert!(!app.preferences_read_failed);
    assert_eq!(app.keymap.hint(Command::LayerLock), "F6");
    for (command, key) in [
        (Command::NewLayerFolder, egui::Key::F7),
        (Command::GroupLayersInFolder, egui::Key::F8),
        (Command::UngroupLayerFolder, egui::Key::F9),
    ] {
        assert!(app.keymap.hint(command).is_empty());
        app.keymap
            .assign(
                command,
                None,
                crate::shortcuts::Chord::new(key, false, false, false),
                false,
            )
            .unwrap();
    }
    app.persist_preferences(true);
    assert_eq!(app.preferences_saved.as_ref().unwrap().version, 21);
    let keys = app.keymap.clone();
    drop(app);
    let app = Parfait::open(directory.path().to_owned()).unwrap();
    assert!(app.keymap == keys);
}
