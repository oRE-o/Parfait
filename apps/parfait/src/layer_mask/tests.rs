use super::*;
use crate::shortcuts::Chord;
use parfait_core::{Brush, Document, MaskCoverage};
use std::sync::Arc;

mod area;

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
            app.prepare_layer_list(ui.ctx());
            app.shortcuts(ui.ctx());
            app.update_previews(ui.ctx());
            egui::Panel::left("mask_test_layers")
                .exact_size(240.)
                .show(ui, |ui| {
                    app.palette(ui, crate::workspace::Palette::Layers, 500.);
                });
            egui::CentralPanel::default().show(ui, |ui| app.canvas(ui));
        },
    );
    output.textures_delta.clear();
}

fn click(app: &mut Parfait, context: &egui::Context, position: egui::Pos2) {
    for pressed in [true, false] {
        frame(
            app,
            context,
            vec![
                egui::Event::PointerMoved(position),
                egui::Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                },
            ],
        );
    }
}

fn target(context: &egui::Context, layer: usize, mask: bool) -> egui::Rect {
    context
        .data(|data| data.get_temp(egui::Id::new(("layer_target", layer, mask))))
        .unwrap()
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

#[test]
fn mask_commands_are_active_only_undoable_and_durably_reopened() {
    let directory = tempfile::tempdir().unwrap();
    let mut app = Parfait::open(directory.path().to_owned()).unwrap();
    app.doc = Document::new(32, 32);
    app.set_layer_selection(3, 1);
    let context = egui::Context::default();
    let before = app.doc.clone();
    app.execute_command(Command::LayerMaskHideAll, &context);
    assert!(app.editing_layer_mask());
    assert!(app.doc.layers[0].mask.is_none());
    assert_eq!(
        app.doc.layers[1].mask.as_ref().unwrap().coverage,
        MaskCoverage::Uniform(0)
    );
    assert_eq!(app.history.rows().count(), 2);
    assert_eq!(app.selected_layer_mask(), 3);
    app.brush = Brush {
        size: 12.,
        pressure: false,
        ..Default::default()
    };
    assert!(app.begin_canvas_stroke(Point {
        x: 16.,
        y: 16.,
        pressure: 1.
    }));
    app.finish_stroke();
    let painted = app.doc.clone();
    assert!(Arc::ptr_eq(
        &painted.layers[1].pixels,
        &before.layers[1].pixels
    ));
    app.undo();
    assert_eq!(
        app.doc.layers[1].mask.as_ref().unwrap().coverage,
        MaskCoverage::Uniform(0)
    );
    app.redo();
    assert!(app.doc.same_edit_state(&painted));
    app.execute_command(Command::LayerMaskEnabled, &context);
    assert!(!app.doc.layers[1].mask.as_ref().unwrap().enabled);
    assert!(!app.begin_canvas_stroke(Point {
        x: 16.,
        y: 16.,
        pressure: 1.
    }));
    app.execute_command(Command::LayerMaskEnabled, &context);
    app.force_save();
    let expected = app.doc.clone();
    drop(app);
    let mut app = Parfait::open(directory.path().to_owned()).unwrap();
    assert_eq!(app.doc, expected);
    assert!(!app.editing_layer_mask());
    let reopened = app.doc.clone();
    let pixels = app.doc.composite();
    app.execute_command(Command::ApplyLayerMask, &context);
    assert!(app.doc.layers[1].mask.is_none());
    assert_eq!(app.doc.composite(), pixels);
    app.undo();
    assert!(app.doc.same_edit_state(&reopened));
    app.execute_command(Command::DeleteLayerMask, &context);
    assert!(app.doc.layers[1].mask.is_none());
    assert!(Arc::ptr_eq(
        &app.doc.layers[1].pixels,
        &reopened.layers[1].pixels
    ));
}

#[test]
fn actual_thumbnail_clicks_route_canvas_input_and_keep_layout_disjoint() {
    let directory = tempfile::tempdir().unwrap();
    let mut app = Parfait::open(directory.path().to_owned()).unwrap();
    app.doc = Document::new(32, 32);
    app.doc.layers[1].pixels = Arc::new([200, 50, 90, 255].repeat(1024));
    app.doc.create_layer_mask(1, 255);
    app.brush = Brush {
        erase: true,
        size: 12.,
        pressure: false,
        ..Default::default()
    };
    let context = egui::Context::default();
    crate::theme::apply(&context);
    frame(&mut app, &context, vec![]);
    frame(&mut app, &context, vec![]);
    let content = target(&context, 1, false);
    let mask = target(&context, 1, true);
    let name: egui::Rect = context
        .data(|data| data.get_temp(egui::Id::new(("layer_label", 1usize))))
        .unwrap();
    assert!(content.right() < mask.left());
    assert!(mask.right() < name.left());
    assert!(name.width() > 0.);
    let before = app.doc.clone();
    click(&mut app, &context, mask.center());
    assert!(app.editing_layer_mask());
    assert_eq!(app.doc, before);
    let canvas: egui::Rect = context
        .data(|data| data.get_temp(egui::Id::new("canvas_rect")))
        .unwrap();
    click(&mut app, &context, canvas.center());
    assert_ne!(app.doc.layers[1].mask, before.layers[1].mask);
    assert!(Arc::ptr_eq(
        &app.doc.layers[1].pixels,
        &before.layers[1].pixels
    ));
    let masked = app.doc.clone();
    click(&mut app, &context, content.center());
    assert!(!app.editing_layer_mask());
    click(&mut app, &context, canvas.center());
    assert!(!Arc::ptr_eq(
        &app.doc.layers[1].pixels,
        &masked.layers[1].pixels
    ));
    assert_eq!(app.doc.layers[1].mask, masked.layers[1].mask);
}

#[test]
fn mask_stroke_escape_and_noop_preserve_redo_and_never_fall_back_to_content() {
    let directory = tempfile::tempdir().unwrap();
    let mut app = Parfait::open(directory.path().to_owned()).unwrap();
    app.doc = Document::new(32, 32);
    app.doc.create_layer_mask(1, 127);
    app.select_layer_target(1, true);
    app.edit_named("Opacity", |document| document.layers[1].opacity = 0.6);
    app.undo();
    let before = app.doc.clone();
    let context = egui::Context::default();
    crate::theme::apply(&context);
    app.brush = Brush {
        size: 12.,
        pressure: false,
        ..Default::default()
    };
    frame(&mut app, &context, vec![]);
    let canvas: egui::Rect = context
        .data(|data| data.get_temp(egui::Id::new("canvas_rect")))
        .unwrap();
    frame(
        &mut app,
        &context,
        vec![
            egui::Event::PointerMoved(canvas.center()),
            egui::Event::PointerButton {
                pos: canvas.center(),
                pressed: true,
                button: egui::PointerButton::Primary,
                modifiers: Default::default(),
            },
        ],
    );
    assert!(app.stroke.is_some());
    frame(&mut app, &context, vec![key(egui::Key::Escape)]);
    assert!(app.stroke.is_none());
    assert!(app.doc.same_edit_state(&before));
    assert!(app.history.can_redo());
    frame(
        &mut app,
        &context,
        vec![egui::Event::PointerButton {
            pos: canvas.center(),
            pressed: false,
            button: egui::PointerButton::Primary,
            modifiers: Default::default(),
        }],
    );
    app.brush.opacity = 0.;
    click(&mut app, &context, canvas.center());
    assert!(app.history.can_redo());
    for operation in [
        BrushOperation::Blend,
        BrushOperation::Blur,
        BrushOperation::Smudge,
    ] {
        app.brush.operation = operation;
        assert!(!app.begin_canvas_stroke(Point {
            x: 16.,
            y: 16.,
            pressure: 1.
        }));
        assert!(app.doc.same_edit_state(&before));
        assert!(app.history.can_redo());
    }
}

#[test]
fn mask_menu_keys_and_preferences_version_keep_existing_bindings() {
    let directory = tempfile::tempdir().unwrap();
    let mut app = Parfait::open(directory.path().to_owned()).unwrap();
    app.doc = Document::new(32, 32);
    let context = egui::Context::default();
    crate::theme::apply(&context);
    app.keymap
        .assign(
            Command::LayerMaskHideAll,
            None,
            Chord::new(egui::Key::F8, false, false, false),
            false,
        )
        .unwrap();
    frame(&mut app, &context, vec![key(egui::Key::F8)]);
    assert!(app.editing_layer_mask());
    assert_eq!(
        app.doc.layers[1].mask.as_ref().unwrap().coverage,
        MaskCoverage::Uniform(0)
    );
    app.persist_preferences(true);
    let mut legacy: serde_json::Value =
        serde_json::from_str(&app.saved_preferences_json()).unwrap();
    assert_eq!(legacy["version"], 21);
    legacy["version"] = 15.into();
    legacy["keymap"]["bindings"]
        .as_array_mut()
        .unwrap()
        .retain(|binding| binding["command"] != "layer_mask_hide_all");
    parfait_storage::save_preferences(
        &directory.path().join("preferences.sqlite"),
        &legacy.to_string(),
    )
    .unwrap();
    drop(app);
    let mut app = Parfait::open(directory.path().to_owned()).unwrap();
    assert!(!app.preferences_read_failed);
    assert!(app.keymap.hint(Command::LayerMaskHideAll).is_empty());
    assert!(!app.keymap.hint(Command::Undo).is_empty());
    app.keymap
        .assign(
            Command::LayerMaskEnabled,
            None,
            Chord::new(egui::Key::F7, false, false, false),
            false,
        )
        .unwrap();
    app.persist_preferences(true);
    drop(app);
    let app = Parfait::open(directory.path().to_owned()).unwrap();
    assert_eq!(app.keymap.hint(Command::LayerMaskEnabled), "F7");
}

#[test]
fn mask_command_guards_and_target_changes_preserve_atomic_state() {
    let directory = tempfile::tempdir().unwrap();
    let mut app = Parfait::open(directory.path().to_owned()).unwrap();
    app.doc = Document::new(32, 32);
    let context = egui::Context::default();
    for blocker in 0..3 {
        app.confirm_new = blocker == 0;
        app.workspace.resizing = blocker == 1;
        app.doc.layers[1].locked = blocker == 2;
        let before = app.doc.clone();
        app.execute_command(Command::LayerMaskRevealAll, &context);
        assert_eq!(app.doc, before);
    }
    app.doc.layers[1].locked = false;
    app.execute_command(Command::LayerMaskRevealAll, &context);
    app.doc.layers[1].alpha_locked = true;
    assert!(
        app.command_disabled_reason(Command::ApplyLayerMask)
            .is_some()
    );
    assert!(app.begin_canvas_stroke(Point {
        x: 16.,
        y: 16.,
        pressure: 1.
    }));
    let before = app.doc.clone();
    app.select_layer_target(1, false);
    assert!(app.editing_layer_mask());
    app.execute_command(Command::DeleteLayerMask, &context);
    assert_eq!(app.doc, before);
    app.finish_stroke();
    app.select_layer(0);
    assert!(!app.editing_layer_mask());
    app.select_layer(1);
    assert!(!app.editing_layer_mask());
    app.select_layer_target(1, true);
    app.execute_command(Command::DeleteLayerMask, &context);
    assert!(!app.editing_layer_mask());
}
