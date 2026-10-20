use super::*;
use eframe::egui;
use parfait_core::{Document, MaskCoverage};
use std::sync::Arc;

fn drawing() -> Document {
    let mut document = Document::new(8, 4);
    document.layers[1].pixels = Arc::new(
        (0..32)
            .flat_map(|index| [30, 60, 90, if index % 8 < 4 { 128 } else { 0 }])
            .collect(),
    );
    document
}

#[test]
fn readonly_selection_keeps_layer_checks_undo_redo_and_noop_identity() {
    let directory = tempfile::tempdir().unwrap();
    let mut app = Parfait::open(directory.path().to_owned()).unwrap();
    app.doc = drawing();
    app.doc.layers[1].locked = true;
    app.doc.layers[1].visible = false;
    let context = egui::Context::default();
    let original = app.doc.clone();
    assert!(
        app.command_disabled_reason(Command::LayerSelectionCreate).is_none()
    );
    app.execute_command(Command::LayerSelectionCreate, &context);
    assert!(app.doc.selection.is_some());
    assert_eq!(app.doc.layers, original.layers);
    assert_eq!(app.selected_layer_mask(), 2);
    let selected = app.doc.clone();
    let rows = app.history.rows().count();
    app.execute_command(Command::LayerSelectionCreate, &context);
    assert_eq!(app.doc.revision, selected.revision);
    assert!(app.doc.same_edit_state(&selected));
    assert_eq!(app.history.rows().count(), rows);
    app.undo();
    assert!(app.doc.selection.is_none());
    app.execute_command(Command::Deselect, &context);
    assert!(app.history.can_redo());
    app.redo();
    assert!(app.doc.same_edit_state(&selected));
    for command in [
        Command::LayerSelectionAdd,
        Command::LayerSelectionSubtract,
        Command::LayerSelectionIntersect,
    ] {
        assert!(app.command_disabled_reason(command).is_none());
        app.execute_command(command, &context);
        assert!(app.doc.validate());
    }
}

#[test]
fn empty_derivation_clears_prior_selection_and_masks_without_selection_are_usable()
 {
    let directory = tempfile::tempdir().unwrap();
    let mut app = Parfait::open(directory.path().to_owned()).unwrap();
    app.doc = Document::new(8, 4);
    let context = egui::Context::default();
    for command in [
        Command::LayerSelectionAdd,
        Command::LayerSelectionSubtract,
        Command::LayerSelectionIntersect,
        Command::Deselect,
        Command::InvertSelection,
    ] {
        assert!(app.command_disabled_reason(command).is_some());
    }
    let before = app.doc.clone();
    app.execute_command(Command::LayerSelectionCreate, &context);
    assert!(app.doc.same_edit_state(&before));
    assert_eq!(app.doc.revision, before.revision);
    assert!(app.notice.is_some());
    app.execute_command(Command::SelectAll, &context);
    assert_eq!(app.doc.selection, Some(MaskCoverage::Uniform(255)));
    app.execute_command(Command::LayerSelectionCreate, &context);
    assert!(app.doc.selection.is_none());
    app.undo();
    assert_eq!(app.doc.selection, Some(MaskCoverage::Uniform(255)));
    app.execute_command(Command::Deselect, &context);
    for command in
        [Command::LayerMaskOutsideSelection, Command::LayerMaskSelection]
    {
        let original = app.doc.clone();
        app.execute_command(command, &context);
        assert_eq!(
            app.doc.layers[1].mask.as_ref().unwrap().coverage,
            MaskCoverage::Uniform(255)
        );
        assert!(Arc::ptr_eq(
            &app.doc.layers[1].pixels,
            &original.layers[1].pixels
        ));
        app.undo();
        assert!(app.doc.same_edit_state(&original));
    }
}

#[test]
fn selection_masks_use_fractional_coverage_and_locked_targets_refuse_atomically()
 {
    let directory = tempfile::tempdir().unwrap();
    let mut app = Parfait::open(directory.path().to_owned()).unwrap();
    app.doc = drawing();
    let context = egui::Context::default();
    app.execute_command(Command::LayerSelectionCreate, &context);
    for command in
        [Command::LayerMaskOutsideSelection, Command::LayerMaskSelection]
    {
        let original = app.doc.clone();
        app.execute_command(command, &context);
        let coverage = &app.doc.layers[1].mask.as_ref().unwrap().coverage;
        for index in 0..32 {
            let expected = original.selection_value(index);
            assert_eq!(
                coverage.value(index),
                if command == Command::LayerMaskOutsideSelection {
                    expected
                } else {
                    255 - expected
                }
            );
        }
        assert!(app.editing_layer_mask());
        app.undo();
        app.doc.layers[1].locked = true;
        let locked = app.doc.clone();
        app.execute_command(command, &context);
        assert!(app.doc.same_edit_state(&locked));
        assert_eq!(app.doc.revision, locked.revision);
        app.doc.layers[1].locked = false;
    }
}

#[test]
fn shortcuts_are_fresh_defaults_and_old_keymaps_are_not_injected() {
    use crate::shortcuts::Chord;
    let directory = tempfile::tempdir().unwrap();
    let mut app = Parfait::open(directory.path().to_owned()).unwrap();
    let chord = Chord::new(egui::Key::A, true, false, false);
    assert_eq!(app.keymap.resolve(chord, None), Some(Command::SelectAll));
    app.keymap.remove(Command::SelectAll, chord);
    app.keymap
        .assign(
            Command::LayerSelectionCreate,
            None,
            Chord::new(egui::Key::F7, false, false, false),
            false,
        )
        .unwrap();
    app.persist_preferences(true);
    let path = directory.path().join("preferences.sqlite");
    let mut legacy: serde_json::Value = serde_json::from_str(
        &parfait_storage::load_preferences(&path).unwrap().unwrap(),
    )
    .unwrap();
    legacy["version"] = 20.into();
    parfait_storage::save_preferences(&path, &legacy.to_string()).unwrap();
    let keys = app.keymap.clone();
    drop(app);
    let mut app = Parfait::open(directory.path().to_owned()).unwrap();
    assert!(!app.preferences_read_failed);
    assert!(app.keymap == keys);
    assert!(app.keymap.hint(Command::SelectAll).is_empty());
    assert_eq!(app.keymap.hint(Command::LayerSelectionCreate), "F7");
    app.persist_preferences(true);
    assert_eq!(app.preferences_saved.as_ref().unwrap().version, 21);
}

#[test]
fn checked_sources_and_folders_are_readonly_but_masks_require_one_unlocked_target()
 {
    let directory = tempfile::tempdir().unwrap();
    let mut app = Parfait::open(directory.path().to_owned()).unwrap();
    app.doc = drawing();
    app.doc.add_layer();
    app.doc.layers[2].pixels = Arc::new([10, 20, 30, 64].repeat(32));
    app.set_layer_selection(6, 2);
    let original = app.doc.layers.clone();
    let context = egui::Context::default();
    app.execute_command(Command::LayerSelectionCreate, &context);
    assert_eq!(app.selected_layer_mask(), 6);
    assert_eq!(app.doc.layers, original);
    assert_eq!(app.doc.selection_value(0), 160);
    assert_eq!(app.doc.selection_value(7), 64);
    for command in
        [Command::LayerMaskOutsideSelection, Command::LayerMaskSelection]
    {
        let before = app.doc.clone();
        assert!(app.command_disabled_reason(command).is_some());
        app.execute_command(command, &context);
        assert!(app.doc.same_edit_state(&before));
    }
    app.doc.group_layers_in_folder(6, "Source folder".into()).unwrap();
    app.doc.layers[3].locked = true;
    app.doc.layers[3].visible = false;
    app.set_layer_selection(8, 3);
    app.doc.deselect_pixels();
    let original = app.doc.layers.clone();
    app.execute_command(Command::LayerSelectionCreate, &context);
    assert_eq!(app.doc.selection_value(0), 160);
    assert_eq!(app.doc.selection_value(7), 64);
    assert_eq!(app.doc.layers, original);
    app.select_layer(1);
    assert!(
        app.command_disabled_reason(Command::LayerMaskOutsideSelection)
            .is_some()
    );
}

#[test]
fn every_selection_command_refuses_owned_layout_and_modal_edits_without_history_changes()
 {
    let directory = tempfile::tempdir().unwrap();
    let mut app = Parfait::open(directory.path().to_owned()).unwrap();
    app.doc = drawing();
    app.doc.select_all_pixels();
    let context = egui::Context::default();
    for modal in [false, true] {
        if modal {
            app.execute_command(Command::RenameLayer, &context);
            assert!(app.dialog_open());
        } else {
            app.workspace.resizing = true;
        }
        let before = app.doc.clone();
        let rows = app.history.rows().count();
        for &command in
            Command::ALL.iter().filter(|command| command.is_pixel_selection())
        {
            assert!(app.command_disabled_reason(command).is_some());
            app.execute_command(command, &context);
            assert!(app.doc.same_edit_state(&before));
            assert_eq!(app.doc.revision, before.revision);
            assert_eq!(app.history.rows().count(), rows);
        }
        app.workspace.resizing = false;
        app.rename_layer = None;
    }
}

#[test]
fn command_results_reopen_with_exact_layers_masks_and_selection() {
    for &command in
        Command::ALL.iter().filter(|command| command.is_pixel_selection())
    {
        let directory = tempfile::tempdir().unwrap();
        let mut app = Parfait::open(directory.path().to_owned()).unwrap();
        app.doc = drawing();
        if command != Command::SelectAll
            && command != Command::LayerSelectionCreate
        {
            app.doc.set_selection(Some(MaskCoverage::Uniform(64)));
        }
        app.execute_command(command, &egui::Context::default());
        let expected = app.doc.clone();
        let path = directory.path().join("command.parfait");
        {
            let mut store = parfait_storage::Store::open(&path).unwrap();
            store.save(&expected).unwrap();
        }
        let store = parfait_storage::Store::open(&path).unwrap();
        let reopened = store.load().unwrap().unwrap().document;
        assert_eq!(reopened, expected, "{command:?}");
        assert_eq!(reopened.composite(), expected.composite());
    }
}
