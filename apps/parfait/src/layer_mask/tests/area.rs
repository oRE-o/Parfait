use super::*;
use parfait_core::PixelRect;

#[test]
fn delete_and_backspace_clear_mask_target_while_content_clear_remains_explicit()
{
    for clear_key in [egui::Key::Delete, egui::Key::Backspace] {
        let directory = tempfile::tempdir().unwrap();
        let mut app = Parfait::open(directory.path().to_owned()).unwrap();
        app.doc = Document::new(32, 32);
        app.doc.layers[1].pixels = Arc::new([200, 80, 30, 255].repeat(1024));
        app.doc.create_layer_mask(1, 0);
        app.select_layer_target(1, true);
        app.set_layer_selection(3, 1);
        app.doc.layers[1].alpha_locked = true;
        let before = app.doc.clone();
        let context = egui::Context::default();
        crate::theme::apply(&context);
        frame(&mut app, &context, vec![]);
        frame(&mut app, &context, vec![key(clear_key)]);
        assert_eq!(
            app.doc.layers[1].mask.as_ref().unwrap().coverage,
            MaskCoverage::Uniform(255)
        );
        assert!(app.editing_layer_mask());
        assert_eq!(app.selected_layer_mask(), 3);
        assert!(Arc::ptr_eq(
            &app.doc.layers[1].pixels,
            &before.layers[1].pixels
        ));
        assert_eq!(app.doc.layers[0], before.layers[0]);
        assert_eq!(app.history.rows().count(), 2);
        let mut release = key(clear_key);
        if let egui::Event::Key { pressed, .. } = &mut release {
            *pressed = false;
        }
        frame(&mut app, &context, vec![release]);
        app.undo();
        assert!(app.doc.same_edit_state(&before));
        app.redo();
        app.undo();
        app.doc.layers[1].alpha_locked = false;
        app.select_layer_target(1, false);
        frame(&mut app, &context, vec![key(clear_key)]);
        assert_eq!(
            app.doc.layers[1].mask.as_ref().unwrap().coverage,
            MaskCoverage::Uniform(0)
        );
        assert!(
            app.doc.layers[1]
                .pixels
                .as_chunks::<4>()
                .0
                .iter()
                .all(|pixel| pixel[3] == 0)
        );
        assert_eq!(app.doc.layers[0], before.layers[0]);
    }
}

#[test]
fn clear_guards_and_focused_text_do_not_change_the_drawing() {
    let directory = tempfile::tempdir().unwrap();
    let mut app = Parfait::open(directory.path().to_owned()).unwrap();
    app.doc = Document::new(32, 32);
    app.doc.create_layer_mask(1, 0);
    app.select_layer_target(1, true);
    let context = egui::Context::default();
    for blocker in 0..4 {
        app.confirm_new = blocker == 0;
        app.workspace.resizing = blocker == 1;
        app.doc.layers[1].locked = blocker == 2;
        if blocker == 3 {
            app.begin_canvas_stroke(Point { x: 8., y: 8., pressure: 1. });
        }
        let before = app.doc.clone();
        app.execute_command(Command::ClearEditingTarget, &context);
        assert_eq!(app.doc, before);
        app.finish_stroke();
    }
    let before = app.doc.clone();
    let mut text = String::from("sample");
    for events in [vec![], vec![key(egui::Key::Backspace)]] {
        let mut output = context.run_ui(
            egui::RawInput { events, ..Default::default() },
            |ui| {
                ui.text_edit_singleline(&mut text).request_focus();
                app.shortcuts(ui.ctx());
            },
        );
        output.textures_delta.clear();
    }
    assert_eq!(app.doc, before);
    assert_ne!(text, "sample");
}

fn canvas_frame(
    app: &mut Parfait,
    context: &egui::Context,
    events: Vec<egui::Event>,
) -> Option<egui::epaint::ImageDelta> {
    let mut output = context.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(400., 400.),
            )),
            events,
            ..Default::default()
        },
        |ui| {
            egui::CentralPanel::default().show(ui, |ui| app.canvas(ui));
        },
    );
    let drawing = app.texture.as_ref().unwrap().id();
    let delta = output.textures_delta.set.get(&drawing).map(|deltas| {
        assert_eq!(deltas.len(), 1);
        deltas[0].clone()
    });
    output.textures_delta.clear();
    delta
}

fn update_cache(
    app: &Parfait,
    output: &Option<egui::epaint::ImageDelta>,
    cache: &mut [egui::Color32],
    partial: bool,
) {
    let delta = output.as_ref().expect("drawing texture update");
    assert_eq!(delta.pos.is_some(), partial);
    let egui::ImageData::Color(image) = &delta.image;
    let [left, top] = delta.pos.unwrap_or([0, 0]);
    for row in 0..image.size[1] {
        let at = (top + row) * app.doc.width as usize + left;
        cache[at..at + image.size[0]].copy_from_slice(
            &image.pixels[row * image.size[0]..(row + 1) * image.size[0]],
        );
    }
    for (index, pixel) in
        app.doc.composite().as_chunks::<4>().0.iter().enumerate()
    {
        let background = if (index % app.doc.width as usize / 16
            + index / app.doc.width as usize / 16)
            .is_multiple_of(2)
        {
            235.
        } else {
            218.
        };
        let alpha = f32::from(pixel[3]) / 255.;
        let mut expected: [u8; 3] = std::array::from_fn(|channel| {
            (f32::from(pixel[channel]) * alpha + background * (1. - alpha))
                .round() as u8
        });
        if app.active_mask_overlay().is_some() {
            let coverage = app.doc.layers[app.doc.active]
                .mask
                .as_ref()
                .unwrap()
                .coverage
                .value(index);
            let tint_alpha = (u32::from(255 - coverage) * 128 + 127) / 255;
            for (target, tint) in expected.iter_mut().zip([112u32, 96, 255]) {
                *target = ((u32::from(*target) * (255 - tint_alpha)
                    + tint * tint_alpha
                    + 127)
                    / 255) as u8;
            }
        }
        assert_eq!(
            cache[index],
            egui::Color32::from_rgb(expected[0], expected[1], expected[2])
        );
    }
}

#[test]
fn overlay_toggles_and_mask_strokes_keep_full_and_partial_texture_output_equal()
{
    let directory = tempfile::tempdir().unwrap();
    let mut app = Parfait::open(directory.path().to_owned()).unwrap();
    app.doc = Document::new(128, 128);
    app.doc.layers[0].visible = false;
    app.doc.layers[1].pixels = Arc::new([160, 80, 30, 180].repeat(128 * 128));
    app.doc.create_layer_mask(1, 127);
    app.select_layer_target(1, true);
    app.brush = Brush { size: 12., pressure: false, ..Default::default() };
    let context = egui::Context::default();
    let mut cache = vec![egui::Color32::TRANSPARENT; 128 * 128];
    let output = canvas_frame(&mut app, &context, vec![]);
    update_cache(&app, &output, &mut cache, false);
    let before = app.doc.clone();
    let saved = (app.saved, app.submitted, app.history.rows().count());
    app.execute_command(Command::ShowMaskArea, &context);
    let output = canvas_frame(&mut app, &context, vec![]);
    update_cache(&app, &output, &mut cache, false);
    assert_eq!(app.doc, before);
    assert_eq!((app.saved, app.submitted, app.history.rows().count()), saved);
    app.view.pan += egui::vec2(8., 4.);
    let output = canvas_frame(&mut app, &context, vec![]);
    assert!(output.is_none());
    let canvas: egui::Rect = context
        .data(|data| data.get_temp(egui::Id::new("canvas_rect")))
        .unwrap();
    let position = canvas.center();
    let pointer = |pressed| egui::Event::PointerButton {
        pos: position,
        pressed,
        button: egui::PointerButton::Primary,
        modifiers: Default::default(),
    };
    let output = canvas_frame(
        &mut app,
        &context,
        vec![egui::Event::PointerMoved(position), pointer(true)],
    );
    update_cache(&app, &output, &mut cache, true);
    canvas_frame(&mut app, &context, vec![pointer(false)]);
    assert!(Arc::ptr_eq(&app.doc.layers[1].pixels, &before.layers[1].pixels));
    app.undo();
    let output = canvas_frame(&mut app, &context, vec![]);
    update_cache(&app, &output, &mut cache, false);
    app.execute_command(Command::ShowMaskArea, &context);
    let output = canvas_frame(&mut app, &context, vec![]);
    update_cache(&app, &output, &mut cache, false);
    assert_eq!(app.doc.composite(), before.composite());
}

#[test]
fn overlay_follows_stable_active_id_and_does_not_survive_delete_or_reopen() {
    let directory = tempfile::tempdir().unwrap();
    let mut app = Parfait::open(directory.path().to_owned()).unwrap();
    app.doc = Document::new(8, 8);
    app.doc.create_layer_mask(1, 0);
    app.doc.layers[1].locked = true;
    app.doc.set_layer_mask_enabled(1, false);
    let context = egui::Context::default();
    app.execute_command(Command::ShowMaskArea, &context);
    assert_eq!(app.active_mask_overlay(), Some(app.doc.layers[1].id));
    app.select_layer(0);
    assert!(app.active_mask_overlay().is_none());
    app.select_layer(1);
    assert!(app.active_mask_overlay().is_some());
    app.doc.layers[1].locked = false;
    app.execute_command(Command::DeleteLayerMask, &context);
    assert!(app.mask_overlay.is_none());
    app.undo();
    assert!(app.mask_overlay.is_none());
    app.execute_command(Command::ShowMaskArea, &context);
    app.force_save();
    let expected = app.doc.clone();
    drop(app);
    let mut app = Parfait::open(directory.path().to_owned()).unwrap();
    assert_eq!(app.doc, expected);
    assert!(app.mask_overlay.is_none());
    app.execute_command(Command::ShowMaskArea, &context);
    app.new_document();
    assert!(app.mask_overlay.is_none());
}

#[test]
fn old_keymaps_keep_delete_owner_and_new_mask_commands_persist_assignments() {
    let directory = tempfile::tempdir().unwrap();
    let mut app = Parfait::open(directory.path().to_owned()).unwrap();
    app.keymap
        .assign(
            Command::LayerVisibility,
            None,
            Chord::new(egui::Key::Delete, false, false, false),
            true,
        )
        .unwrap();
    app.persist_preferences(true);
    let mut legacy: serde_json::Value =
        serde_json::from_str(&app.saved_preferences_json()).unwrap();
    legacy["version"] = 16.into();
    legacy["keymap"]["bindings"].as_array_mut().unwrap().retain(|binding| {
        !matches!(
            binding["command"].as_str(),
            Some(
                "clear_editing_target" | "clear_layer_mask" | "show_mask_area"
            )
        )
    });
    parfait_storage::save_preferences(
        &directory.path().join("preferences.sqlite"),
        &legacy.to_string(),
    )
    .unwrap();
    drop(app);
    let mut app = Parfait::open(directory.path().to_owned()).unwrap();
    assert!(!app.preferences_read_failed);
    assert_eq!(
        app.keymap
            .resolve(Chord::new(egui::Key::Delete, false, false, false), None),
        Some(Command::LayerVisibility)
    );
    assert!(app.keymap.hint(Command::ClearEditingTarget).is_empty());
    app.keymap
        .assign(
            Command::ShowMaskArea,
            None,
            Chord::new(egui::Key::F9, false, false, false),
            false,
        )
        .unwrap();
    app.persist_preferences(true);
    drop(app);
    let app = Parfait::open(directory.path().to_owned()).unwrap();
    assert_eq!(app.keymap.hint(Command::ShowMaskArea), "F9");
}

#[test]
fn clear_dense_white_mask_does_not_discard_the_redo_branch() {
    let directory = tempfile::tempdir().unwrap();
    let mut app = Parfait::open(directory.path().to_owned()).unwrap();
    app.doc = Document::new(8, 8);
    app.doc.create_layer_mask(1, 255);
    app.doc.layers[1].mask.as_mut().unwrap().coverage =
        MaskCoverage::Dense(Arc::new(vec![255; 64]));
    app.select_layer_target(1, true);
    app.edit_named("Opacity", |document| document.layers[1].opacity = 0.6);
    app.undo();
    let before = app.doc.clone();
    app.execute_command(Command::ClearLayerMask, &egui::Context::default());
    assert!(app.doc.same_edit_state(&before));
    assert_eq!(app.doc.revision, before.revision);
    assert!(app.history.can_redo());
    let region = PixelRect { x0: 0, y0: 0, x1: 8, y1: 8 };
    assert_eq!(
        app.doc.composite_region(region),
        before.composite_region(region)
    );
}

#[test]
fn layer_pick_recomputes_overlay_after_input_before_uploading_a_first_frame() {
    let directory = tempfile::tempdir().unwrap();
    let mut app = Parfait::open(directory.path().to_owned()).unwrap();
    app.doc = Document::new(128, 128);
    app.doc.create_layer_mask(1, 127);
    app.mask_overlay = Some(app.doc.layers[1].id);
    let context = egui::Context::default();
    let position = egui::pos2(200., 200.);
    canvas_frame(&mut app, &context, vec![]);
    canvas_frame(&mut app, &context, vec![]);
    app.texture = None;
    let output = canvas_frame(
        &mut app,
        &context,
        vec![
            egui::Event::PointerMoved(position),
            egui::Event::PointerButton {
                pos: position,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers {
                    command: true,
                    shift: true,
                    ..Default::default()
                },
            },
        ],
    );
    assert_eq!(app.doc.active, 0);
    assert!(app.active_mask_overlay().is_none());
    assert!(app.rendered_overlay.is_none());
    let mut cache = vec![egui::Color32::TRANSPARENT; 128 * 128];
    update_cache(&app, &output, &mut cache, false);
}
