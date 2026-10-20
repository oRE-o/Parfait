use super::*;
fn setup(command: Command) -> (tempfile::TempDir, Parfait, egui::Context) {
    let dir = tempfile::tempdir().unwrap();
    let mut app = Parfait::open_legacy(dir.path().into()).unwrap();
    app.documents.active_session.document = Document::new(128, 96);
    app.documents.active_session.document.layers[1].pixels =
        Arc::new([50, 90, 130, 255].repeat(128 * 96));
    let ctx = egui::Context::default();
    crate::theme::apply(&ctx);
    frame(&mut app, &ctx, vec![], Default::default());
    app.execute_command(command, &ctx);
    for _ in 0..3 {
        frame(&mut app, &ctx, vec![], Default::default());
    }
    (dir, app, ctx)
}
fn frame(
    app: &mut Parfait,
    ctx: &egui::Context,
    mut events: Vec<egui::Event>,
    modifiers: egui::Modifiers,
) {
    events.insert(0, egui::Event::ModifiersChanged(modifiers));
    let mut out = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1440., 920.),
            )),
            events,
            ..Default::default()
        },
        |ui| app.draw_ui(ui),
    );
    if app
        .image_edit
        .dialog
        .as_ref()
        .is_some_and(|d| d.command.is_raster_transform())
    {
        let id = app
            .transform_canvas_texture()
            .or(app.render_cache.texture.as_ref())
            .unwrap()
            .id();
        let meshes: Vec<_> = out
            .shapes
            .iter()
            .filter_map(|s| {
                if let egui::Shape::Mesh(m) = &s.shape {
                    (m.texture_id == id).then_some(m)
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
            "Artwork must not inherit the disabled UI tint"
        );
    }
    out.textures_delta.clear();
}
fn pointer(
    pos: egui::Pos2,
    down: bool,
    modifiers: egui::Modifiers,
) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed: down,
        modifiers,
    }
}
fn drag(
    app: &mut Parfait,
    ctx: &egui::Context,
    from: egui::Pos2,
    to: egui::Pos2,
) {
    frame(
        app,
        ctx,
        vec![
            egui::Event::PointerMoved(from),
            pointer(from, true, Default::default()),
        ],
        Default::default(),
    );
    frame(app, ctx, vec![egui::Event::PointerMoved(to)], Default::default());
    frame(
        app,
        ctx,
        vec![pointer(to, false, Default::default())],
        Default::default(),
    );
}
fn click(
    app: &mut Parfait,
    ctx: &egui::Context,
    pos: egui::Pos2,
    modifiers: egui::Modifiers,
) {
    frame(
        app,
        ctx,
        vec![egui::Event::PointerMoved(pos), pointer(pos, true, modifiers)],
        modifiers,
    );
    frame(app, ctx, vec![pointer(pos, false, modifiers)], modifiers);
}
fn doc_point(app: &Parfait, p: egui::Vec2) -> egui::Pos2 {
    let doc = &app.documents.active_session.document;
    app.documents.active_session.view.screen(
        p,
        app.image_edit.canvas_rect.unwrap(),
        egui::vec2(doc.width as f32, doc.height as f32),
    )
}
#[test]
fn canvas_translation_and_scale_are_drafts_and_commit_one_undo() {
    let (_dir, mut app, ctx) = setup(Command::EditTransformScaleRotate);
    let before = app.documents.active_session.document.clone();
    let center = doc_point(&app, egui::vec2(64., 48.));
    drag(&mut app, &ctx, center, center + egui::vec2(8., 6.));
    assert_eq!(
        app.image_edit.dialog.as_ref().unwrap().transform.translation,
        [8., 6.]
    );
    assert!(app.documents.active_session.document.same_edit_state(&before));
    app.execute_command(Command::EditTransformApply, &ctx);
    assert!(app.image_edit.dialog.is_none());
    assert_eq!(app.documents.active_session.history.rows().count(), 2);
    assert!(!app.documents.active_session.document.same_edit_state(&before));
    app.undo();
    assert!(app.documents.active_session.document.same_edit_state(&before));
    app.execute_command(Command::EditTransformScale, &ctx);
    for _ in 0..3 {
        frame(&mut app, &ctx, vec![], Default::default());
    }
    let corner = ctx
        .data(|d| {
            d.get_temp::<egui::Pos2>(egui::Id::new(("transform_handle", 2)))
        })
        .unwrap();
    drag(&mut app, &ctx, corner, corner + egui::vec2(16., 12.));
    assert!(app.image_edit.dialog.as_ref().unwrap().transform.scale[0] > 1.);
    app.execute_command(Command::EditTransformCancel, &ctx);
    assert!(app.documents.active_session.history.can_redo());
    assert!(app.documents.active_session.document.same_edit_state(&before));
}
#[test]
fn pins_can_be_placed_dragged_and_alt_deleted_on_canvas() {
    let (_dir, mut app, ctx) = setup(Command::EditTransformPins);
    let before = app.documents.active_session.document.clone();
    let p = doc_point(&app, egui::vec2(30., 30.));
    let q = doc_point(&app, egui::vec2(95., 70.));
    click(&mut app, &ctx, p, Default::default());
    click(&mut app, &ctx, q, Default::default());
    assert_eq!(app.image_edit.dialog.as_ref().unwrap().transform.pins.len(), 2);
    drag(&mut app, &ctx, p, p + egui::vec2(5., 2.));
    let pin = app.image_edit.dialog.as_ref().unwrap().transform.pins[0];
    assert!((pin.target[0] - 35. / 128.).abs() < 0.0001);
    click(
        &mut app,
        &ctx,
        p + egui::vec2(5., 2.),
        egui::Modifiers { alt: true, ..Default::default() },
    );
    assert_eq!(app.image_edit.dialog.as_ref().unwrap().transform.pins.len(), 1);
    app.execute_command(Command::EditTransformCancel, &ctx);
    assert!(app.documents.active_session.document.same_edit_state(&before));
}
#[test]
fn variable_mesh_handles_follow_rotated_view_without_mutating_source() {
    let (_dir, mut app, ctx) = setup(Command::EditTransformMesh);
    app.documents.active_session.view.angle = 0.4;
    app.image_edit.dialog.as_mut().unwrap().transform.resize_grid(4, 5);
    for _ in 0..3 {
        frame(&mut app, &ctx, vec![], Default::default());
    }
    let point = ctx
        .data(|d| {
            d.get_temp::<egui::Pos2>(egui::Id::new(("transform_handle", 5)))
        })
        .unwrap();
    let before = app.documents.active_session.document.clone();
    drag(&mut app, &ctx, point, point + egui::vec2(3., 2.));
    assert_ne!(
        app.image_edit.dialog.as_ref().unwrap().transform.nodes[5],
        [0.; 2]
    );
    assert!(app.documents.active_session.document.same_edit_state(&before));
}
#[test]
fn late_preview_is_rejected_and_cancel_releases_reservation() {
    let (_dir, mut app, ctx) = setup(Command::EditTransformScaleRotate);
    app.cancel_transform_preview();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while app.image_edit.preview_job.is_some() {
        app.poll_transform_preview(&ctx);
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(app.transform_reserved_bytes(), 0);
    let draft = app.image_edit.dialog.as_mut().unwrap();
    draft.transform.translation = [4., 0.];
    draft.transform_canvas.requested = None;
    frame(&mut app, &ctx, vec![], Default::default());
    assert!(app.image_edit.preview_job.is_some());
    app.image_edit.dialog.as_mut().unwrap().transform.translation = [12., 0.];
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while app.image_edit.preview_job.is_some() {
        app.poll_transform_preview(&ctx);
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_ne!(
        app.image_edit
            .dialog
            .as_ref()
            .unwrap()
            .transform_canvas
            .shown
            .as_ref()
            .map(|t| t.translation),
        Some([4., 0.])
    );
    app.execute_command(Command::EditTransformCancel, &ctx);
    assert!(app.image_edit.dialog.is_none());
}

#[test]
fn rejected_preview_budget_preserves_redo_and_does_not_start_work() {
    let (_dir, mut app, ctx) = setup(Command::EditTransformScaleRotate);
    app.execute_command(Command::EditTransformCancel, &ctx);
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while app.image_edit.preview_job.is_some() {
        app.poll_transform_preview(&ctx);
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    app.edit_named("Opacity", |d| d.layers[1].opacity = 0.5);
    app.undo();
    let before = app.documents.active_session.document.clone();
    let rows = app.documents.active_session.history.rows().count();
    app.documents.retained_limit = 1;
    app.execute_command(Command::EditTransformMesh, &ctx);
    for _ in 0..3 {
        frame(&mut app, &ctx, vec![], Default::default());
    }
    assert!(app.image_edit.preview_job.is_none());
    assert!(app.documents.active_session.history.can_redo());
    assert_eq!(app.documents.active_session.history.rows().count(), rows);
    app.execute_command(Command::EditTransformCancel, &ctx);
    assert!(app.documents.active_session.document.same_edit_state(&before));
}
#[test]
fn mask_thumbnail_transform_applies_to_mask_and_keeps_raster_buffer() {
    let (_dir, mut app, ctx) = setup(Command::EditTransformScaleRotate);
    app.execute_command(Command::EditTransformCancel, &ctx);
    let doc = &mut app.documents.active_session.document;
    assert!(doc.create_layer_mask(1, 255));
    let mut values = vec![255; 128 * 96];
    values[128 * 40 + 40] = 0;
    doc.layers[1].mask.as_mut().unwrap().coverage =
        parfait_core::MaskCoverage::Dense(Arc::new(values));
    doc.touch();
    let before = doc.clone();
    let id = doc.layers[1].id;
    app.documents.active_session.editing.mask_target = Some(id);
    assert!(app.image_edit_reason(Command::EditTransformScaleRotate).is_none());
    app.execute_command(Command::EditTransformScaleRotate, &ctx);
    let draft = app.image_edit.dialog.as_mut().unwrap();
    assert!(draft.mask_target);
    draft.transform.translation = [4., 0.];
    app.execute_command(Command::EditTransformApply, &ctx);
    let doc = &app.documents.active_session.document;
    assert!(Arc::ptr_eq(&doc.layers[1].pixels, &before.layers[1].pixels));
    assert_eq!(
        doc.layers[1].mask.as_ref().unwrap().coverage.value(128 * 40 + 44),
        0
    );
    app.undo();
    assert!(app.documents.active_session.document.same_edit_state(&before));
}

#[test]
fn completed_preview_commits_cached_pixels_without_recomputing_and_keeps_document_name()
 {
    let (_dir, mut app, ctx) = setup(Command::EditTransformScaleRotate);
    let draft = app.image_edit.dialog.as_mut().unwrap();
    draft.transform.translation = [4., 0.];
    draft.transform_canvas.requested = None;
    while app.image_edit.preview_job.is_some() {
        app.poll_transform_preview(&ctx);
        std::thread::sleep(Duration::from_millis(1));
    }
    frame(&mut app, &ctx, vec![], Default::default());
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while app.image_edit.preview_job.is_some() {
        app.poll_transform_preview(&ctx);
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    let cached = app
        .image_edit
        .dialog
        .as_ref()
        .unwrap()
        .cached_transform()
        .unwrap()
        .layers[1]
        .pixels
        .clone();
    app.documents.active_session.document.name = "Saved name".into();
    app.execute_command(Command::EditTransformApply, &ctx);
    assert!(app.image_edit.dialog.is_none());
    assert!(Arc::ptr_eq(
        &cached,
        &app.documents.active_session.document.layers[1].pixels
    ));
    assert_eq!(app.documents.active_session.document.name, "Saved name");
    assert_eq!(app.documents.active_session.history.rows().count(), 2);
}

#[test]
fn shift_selected_mesh_nodes_move_together_after_shift_is_released() {
    let (_dir, mut app, ctx) = setup(Command::EditTransformMesh);
    app.image_edit.dialog.as_mut().unwrap().transform.resize_grid(4, 5);
    for _ in 0..3 {
        frame(&mut app, &ctx, vec![], Default::default());
    }
    let point = |index| {
        ctx.data(|d| {
            d.get_temp::<egui::Pos2>(egui::Id::new(("transform_handle", index)))
        })
        .unwrap()
    };
    let a = point(5_usize);
    let b = point(6_usize);
    click(&mut app, &ctx, a, Default::default());
    click(
        &mut app,
        &ctx,
        b,
        egui::Modifiers { shift: true, ..Default::default() },
    );
    assert_eq!(
        app.image_edit.dialog.as_ref().unwrap().transform_canvas.selected_nodes,
        vec![5, 6]
    );
    drag(&mut app, &ctx, a, a + egui::vec2(3., 2.));
    let t = &app.image_edit.dialog.as_ref().unwrap().transform;
    assert_ne!(t.nodes[5], [0.; 2]);
    assert_eq!(t.nodes[5], t.nodes[6]);
}
