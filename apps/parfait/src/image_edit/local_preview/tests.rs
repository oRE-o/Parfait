use super::*;
fn setup(command: Command) -> (tempfile::TempDir, Parfait, egui::Context) {
    let dir = tempfile::tempdir().unwrap();
    let mut app = Parfait::open_legacy(dir.path().into()).unwrap();
    app.documents.active_session.document = Document::new(32, 24);
    let mut pixels = vec![0; 32 * 24 * 4];
    for y in 3..21 {
        for x in 4..28 {
            let i = (y * 32 + x) * 4;
            pixels[i..i + 4].copy_from_slice(&[100, 140, 190, 180]);
        }
    }
    app.documents.active_session.document.layers[1].pixels = Arc::new(pixels);
    let ctx = egui::Context::default();
    crate::theme::apply(&ctx);
    app.execute_command(command, &ctx);
    (dir, app, ctx)
}
fn frame(app: &mut Parfait, ctx: &egui::Context) {
    let mut out = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(960., 640.),
            )),
            ..Default::default()
        },
        |ui| app.draw_ui(ui),
    );
    out.textures_delta.clear();
}
fn drain(app: &mut Parfait, ctx: &egui::Context) {
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while app.image_edit.local_job.is_some() {
        app.poll_local_preview(ctx);
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn input_frame(
    app: &mut Parfait,
    ctx: &egui::Context,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(960., 640.),
            )),
            events,
            ..Default::default()
        },
        |ui| app.draw_ui(ui),
    )
}
fn pointer(pos: egui::Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    }
}
#[test]
fn real_hsv_slider_drags_change_canvas_preview_and_cancel_restores_original() {
    for label in ["색조", "채도", "명도"] {
        let (_dir, mut app, ctx) = setup(Command::EditHsl);
        let before = app.documents.active_session.document.clone();
        for _ in 0..3 {
            frame(&mut app, &ctx);
        }
        let track = ctx
            .data(|d| {
                d.get_temp::<egui::Rect>(egui::Id::new((
                    "image_edit_slider",
                    label,
                )))
            })
            .unwrap();
        let p =
            egui::pos2(track.left() + track.width() * 0.78, track.center().y);
        for pressed in [true, false] {
            let mut out = input_frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(p), pointer(p, pressed)],
            );
            out.textures_delta.clear();
        }
        drain(&mut app, &ctx);
        frame(&mut app, &ctx);
        drain(&mut app, &ctx);
        let draft = app.image_edit.dialog.as_ref().unwrap();
        let cache = &draft.local_preview.cached.as_ref().unwrap().1;
        assert_ne!(cache.composite(), before.composite(), "{label}");
        assert!(app.documents.active_session.document.same_edit_state(&before));
        let texture = app.local_canvas_texture().unwrap().id();
        let mut out = input_frame(&mut app, &ctx, vec![]);
        assert!(out.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Mesh(m) if m.texture_id==texture)),"The changed image must appear on the canvas: {label}");
        let preview_meshes=out.shapes.iter().filter(|s|matches!(&s.shape,egui::Shape::Mesh(m) if m.texture_id==texture)).count();
        assert!(
            preview_meshes >= 2,
            "Canvas and navigator must share the current preview: {label}"
        );
        out.textures_delta.clear();
        let cancel = ctx
            .data(|d| {
                d.get_temp::<egui::Rect>(egui::Id::new("image_edit_cancel"))
            })
            .unwrap()
            .center();
        for pressed in [true, false] {
            let mut out = input_frame(
                &mut app,
                &ctx,
                vec![
                    egui::Event::PointerMoved(cancel),
                    pointer(cancel, pressed),
                ],
            );
            out.textures_delta.clear();
        }
        assert!(app.image_edit.dialog.is_none());
        assert!(app.local_canvas_texture().is_none());
        assert!(app.documents.active_session.document.same_edit_state(&before));
        assert!(!app.documents.active_session.history.can_undo());
    }
}
#[test]
fn each_editor_remembers_its_own_position_without_moving_other_tools() {
    let (_dir, mut app, ctx) = setup(Command::EditHsl);
    for _ in 0..3 {
        frame(&mut app, &ctx);
    }
    let rect = |command| {
        ctx.data(|d| {
            d.get_temp::<egui::Rect>(egui::Id::new((
                "image_edit_window",
                command,
            )))
        })
        .unwrap()
    };
    let original = rect(Command::EditHsl);
    let p = original.left_top() + egui::vec2(65., 13.);
    let moved = p + egui::vec2(85., 25.);
    for events in [
        vec![egui::Event::PointerMoved(p), pointer(p, true)],
        vec![egui::Event::PointerMoved(moved)],
        vec![pointer(moved, false)],
    ] {
        let mut out = input_frame(&mut app, &ctx, events);
        out.textures_delta.clear();
    }
    frame(&mut app, &ctx);
    let hsv = rect(Command::EditHsl);
    assert!(hsv.left() > original.left() + 50.);
    app.cancel_local_preview();
    app.image_edit.dialog = None;
    drain(&mut app, &ctx);
    app.execute_command(Command::EditLevels, &ctx);
    for _ in 0..3 {
        frame(&mut app, &ctx);
    }
    let levels = rect(Command::EditLevels);
    assert!((levels.left() - hsv.left()).abs() > 40.);
    app.cancel_local_preview();
    app.image_edit.dialog = None;
    drain(&mut app, &ctx);
    app.execute_command(Command::EditHsl, &ctx);
    for _ in 0..3 {
        frame(&mut app, &ctx);
    }
    assert!(rect(Command::EditHsl).min.distance(hsv.min) < 3.);
}
#[test]
fn latest_preview_rejects_stale_parameters_and_confirmation_reuses_exact_buffers()
 {
    let (_dir, mut app, ctx) = setup(Command::EditLevels);
    frame(&mut app, &ctx);
    app.image_edit.dialog.as_mut().unwrap().values[3] = 35.;
    drain(&mut app, &ctx);
    assert!(
        app.image_edit.dialog.as_ref().unwrap().local_preview.cached.is_none()
    );
    frame(&mut app, &ctx);
    drain(&mut app, &ctx);
    let draft = app.image_edit.dialog.take().unwrap();
    let cached = draft.local_preview.cached.as_ref().unwrap().1.clone();
    let source = app.documents.active_session.document.clone();
    assert_eq!(app.documents.active_session.history.rows().count(), 0);
    assert!(app.apply_image_edit(&draft));
    assert!(Arc::ptr_eq(
        &cached.layers[1].pixels,
        &app.documents.active_session.document.layers[1].pixels
    ));
    assert_eq!(app.documents.active_session.history.rows().count(), 2);
    app.undo();
    assert!(app.documents.active_session.document.same_edit_state(&source));
}
#[test]
fn preview_refusal_and_cancel_preserve_redo_and_comparison_shows_original() {
    let (_dir, mut app, ctx) = setup(Command::EditColorBalance);
    app.image_edit.dialog = None;
    app.edit_named("Opacity", |d| d.layers[1].opacity = 0.6);
    app.undo();
    let before = app.documents.active_session.document.clone();
    app.execute_command(Command::EditColorBalance, &ctx);
    app.documents.retained_limit = 1;
    frame(&mut app, &ctx);
    assert!(app.image_edit.local_job.is_none());
    assert!(app.documents.active_session.history.can_redo());
    assert!(app.documents.active_session.document.same_edit_state(&before));
    app.documents.retained_limit = 512 * 1024 * 1024;
    app.image_edit.dialog.as_mut().unwrap().local_preview.requested = None;
    app.image_edit.dialog.as_mut().unwrap().values[0] = 20.;
    frame(&mut app, &ctx);
    drain(&mut app, &ctx);
    assert!(app.local_canvas_texture().is_some());
    app.image_edit.dialog.as_mut().unwrap().local_preview.compare = true;
    assert!(app.local_canvas_texture().is_none());
    app.image_edit.dialog = None;
    assert!(app.documents.active_session.history.can_redo());
    assert!(app.documents.active_session.document.same_edit_state(&before));
}
#[test]
fn multi_selected_layer_color_correction_has_one_undo_and_locked_members_refuse()
 {
    let (_dir, mut app, ctx) = setup(Command::EditBrightnessContrast);
    app.image_edit.dialog = None;
    app.documents.active_session.document.add_layer();
    app.documents.active_session.document.layers[2].pixels =
        app.documents.active_session.document.layers[1].pixels.clone();
    app.set_layer_selection(6, 2);
    app.execute_command(Command::EditBrightnessContrast, &ctx);
    let mut draft = app.image_edit.dialog.take().unwrap();
    assert_eq!(draft.tone_ui.layers, 6);
    draft.values[0] = 20.;
    let before = app.documents.active_session.document.clone();
    assert!(app.apply_image_edit(&draft));
    assert_ne!(
        app.documents.active_session.document.layers[1].pixels,
        before.layers[1].pixels
    );
    assert_eq!(
        app.documents.active_session.document.layers[1].pixels,
        app.documents.active_session.document.layers[2].pixels
    );
    app.undo();
    assert!(app.documents.active_session.document.same_edit_state(&before));
    app.documents.active_session.document.layers[1].locked = true;
    app.set_layer_selection(6, 2);
    assert!(app.image_edit_reason(Command::EditLevels).is_some());
}
#[test]
fn disabled_then_reenabled_preview_does_not_get_stuck_on_a_cancelled_request() {
    let (_dir, mut app, ctx) = setup(Command::EditGradientMap);
    frame(&mut app, &ctx);
    app.cancel_local_preview();
    drain(&mut app, &ctx);
    assert!(
        app.image_edit
            .dialog
            .as_ref()
            .unwrap()
            .local_preview
            .requested
            .is_none()
    );
    frame(&mut app, &ctx);
    drain(&mut app, &ctx);
    assert!(
        app.image_edit.dialog.as_ref().unwrap().local_preview.cached.is_some()
    );
}
#[test]
fn shading_preview_retains_original_and_commits_two_clipped_layers_with_one_undo()
 {
    let (_dir, mut app, ctx) = setup(Command::EditAutoShade);
    let before = app.documents.active_session.document.clone();
    frame(&mut app, &ctx);
    drain(&mut app, &ctx);
    assert_eq!(app.documents.active_session.document, before);
    let draft = app.image_edit.dialog.take().unwrap();
    assert!(app.apply_image_edit(&draft));
    assert_eq!(app.documents.active_session.document.layers.len(), 4);
    assert_eq!(app.documents.active_session.document.active, 3);
    app.undo();
    assert!(app.documents.active_session.document.same_edit_state(&before));
}
#[test]
fn all_color_and_size_editors_keep_footers_visible_and_artwork_undimmed() {
    for command in [
        Command::EditBrightnessContrast,
        Command::EditHsl,
        Command::EditPosterize,
        Command::EditLevels,
        Command::EditToneCurve,
        Command::EditColorBalance,
        Command::EditThreshold,
        Command::EditGradientMap,
        Command::EditResolution,
        Command::EditCanvasSize,
        Command::EditAutoShade,
    ] {
        let (_dir, mut app, ctx) = setup(command);
        frame(&mut app, &ctx);
        frame(&mut app, &ctx);
        for name in ["image_edit_accept", "image_edit_cancel"] {
            let rect = ctx
                .data(|d| d.get_temp::<egui::Rect>(egui::Id::new(name)))
                .unwrap();
            assert!(
                ctx.content_rect().contains_rect(rect),
                "{command:?} {rect:?}"
            );
        }
        app.cancel_local_preview();
        drain(&mut app, &ctx);
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(960., 640.),
                )),
                ..Default::default()
            },
            |ui| app.draw_ui(ui),
        );
        let texture = app
            .local_canvas_texture()
            .or(app.render_cache.texture.as_ref())
            .unwrap()
            .id();
        let meshes: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|s| {
                if let egui::Shape::Mesh(m) = &s.shape {
                    (m.texture_id == texture).then_some(m)
                } else {
                    None
                }
            })
            .collect();
        assert!(!meshes.is_empty());
        assert!(
            meshes.iter().all(|m| m
                .vertices
                .iter()
                .all(|v| v.color == egui::Color32::WHITE)),
            "{command:?}"
        );
        output.textures_delta.clear();
    }
}
