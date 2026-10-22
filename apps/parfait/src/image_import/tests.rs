use super::*;

fn app() -> (tempfile::TempDir, Parfait) {
    let dir = tempfile::tempdir().unwrap();
    let mut app = Parfait::open(dir.path().join("recovery")).unwrap();
    app.documents.active_session.document = Document::new(8, 8);
    (dir, app)
}
fn png(path: &Path, width: u32, height: u32) -> Vec<u8> {
    let pixels = [24, 72, 128, 120].repeat(width as usize * height as usize);
    image::save_buffer_with_format(
        path,
        &pixels,
        width,
        height,
        image::ColorType::Rgba8,
        ImageFormat::Png,
    )
    .unwrap();
    pixels
}
fn drain(app: &mut Parfait) {
    let deadline =
        std::time::Instant::now() + std::time::Duration::from_secs(10);
    while app.image_import.job.is_some() {
        app.poll_image_import();
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}
#[test]
fn png_new_tab_preserves_alpha_original_tab_and_source_then_recovers_and_reopens()
 {
    let (dir, mut app) = app();
    let path = dir.path().join("한글 이미지.png");
    let pixels = png(&path, 3, 2);
    let original_file = std::fs::read(&path).unwrap();
    let old = app.documents.active_id().unwrap();
    let before = app.documents.active_session.document.clone();
    app.queue_image_import(path.clone(), false).unwrap();
    assert!(app.dialog_open());
    assert_eq!(app.image_import.reserved_bytes(), IMPORT_RESERVATION);
    assert!(app.flush_document_recovery(true).is_err());
    drain(&mut app);
    assert!(app.error.is_none(), "{:?}", app.error);
    let id = app.documents.active_id().unwrap();
    assert_ne!(id, old);
    assert_eq!(app.documents.get(old).unwrap().document, before);
    let session = app.documents.get(id).unwrap();
    assert_eq!(session.document.name, "한글 이미지");
    assert_eq!(&*session.document.layers[1].pixels, &pixels);
    assert!(!session.document.layers[0].visible);
    assert!(session.project.is_none());
    assert!(session.modified());
    assert_eq!(app.image_import.reserved_bytes(), 0);
    let saved = dir.path().join("imported.parfait");
    app.save_project_to(saved.clone()).unwrap();
    let reopened = parfait_storage::Store::open_read_only(&saved)
        .unwrap()
        .load()
        .unwrap()
        .unwrap()
        .document;
    assert_eq!(reopened.layers, app.documents.active_session.document.layers);
    app.edit_named("Unsaved", |d| d.layers[1].opacity = 0.5);
    app.flush_document_recovery(true).unwrap();
    drop(app);
    let app = Parfait::open(dir.path().join("recovery")).unwrap();
    assert_eq!(app.documents.get(id).unwrap().document.layers[1].opacity, 0.5);
    assert_eq!(std::fs::read(path).unwrap(), original_file);
}
#[test]
fn layer_import_is_one_undoable_edit_and_does_not_touch_other_tab() {
    let (dir, mut app) = app();
    let path = dir.path().join("layer.png");
    png(&path, 2, 2);
    let key = app.documents.active_key().unwrap();
    let before = app.documents.active_session.document.clone();
    app.queue_image_import(path, true).unwrap();
    drain(&mut app);
    assert!(app.error.is_none(), "{:?}", app.error);
    let edited = app.documents.active_session.document.clone();
    assert_eq!(edited.layers.len(), before.layers.len() + 1);
    assert_eq!(app.documents.active_session.history.rows().count(), 2);
    app.undo();
    assert!(app.documents.active_session.document.same_edit_state(&before));
    app.redo();
    assert!(app.documents.active_session.document.same_edit_state(&edited));
    let second = app.new_document_tab(Document::new(4, 4)).unwrap();
    let other_before = app.documents.get(second).unwrap().document.clone();
    let path = dir.path().join("late.png");
    let image_pixels = png(&path, 1, 1);
    let owner = app.documents.get(key.document_id).unwrap();
    let target = Target::Layer {
        key,
        state: owner.history.state_id(),
        layer: owner.document.layers[owner.document.active].id,
    };
    app.apply_import(
        target,
        DecodedImage {
            name: "late".into(),
            width: 1,
            height: 1,
            pixels: image_pixels,
        },
    )
    .unwrap();
    assert_eq!(app.documents.active_id(), Some(second));
    assert_eq!(app.documents.get(second).unwrap().document, other_before);
}
#[test]
fn invalid_files_oversized_layer_cancel_and_stale_target_leave_artwork_and_redo_unchanged()
 {
    let (dir, mut app) = app();
    app.edit_named("Opacity", |d| d.layers[1].opacity = 0.5);
    app.undo();
    let before = app.documents.active_session.document.clone();
    for (name, bytes) in
        [("bad.png", b"invalid".as_slice()), ("gif.png", b"GIF89a".as_slice())]
    {
        let path = dir.path().join(name);
        std::fs::write(&path, bytes).unwrap();
        app.queue_image_import(path, true).unwrap();
        drain(&mut app);
        assert!(app.error.take().is_some());
        assert_eq!(app.documents.active_session.document, before);
        assert!(app.documents.active_session.history.can_redo());
    }
    let path = dir.path().join("large.png");
    png(&path, 9, 8);
    app.queue_image_import(path, true).unwrap();
    drain(&mut app);
    assert!(app.error.take().is_some());
    assert_eq!(app.documents.active_session.document, before);
    let path = dir.path().join("cancel.png");
    png(&path, 2, 2);
    app.queue_image_import(path, false).unwrap();
    app.image_import.job.as_mut().unwrap().cancelled = true;
    assert!(!app.dialog_open());
    assert!(app.image_import.reserved_bytes() > 0);
    drain(&mut app);
    assert_eq!(app.documents.tab_order.len(), 1);
    assert_eq!(app.documents.active_session.document, before);
    assert!(app.documents.active_session.history.can_redo());
    let key = app.documents.active_key().unwrap();
    let target = Target::Layer {
        key,
        state: app.documents.active_session.history.state_id(),
        layer: app.documents.active_session.document.layers[1].id,
    };
    app.redo();
    let changed = app.documents.active_session.document.clone();
    assert!(
        app.apply_import(
            target,
            DecodedImage {
                name: "stale".into(),
                width: 1,
                height: 1,
                pixels: vec![0; 4]
            }
        )
        .is_err()
    );
    assert_eq!(app.documents.active_session.document, changed);
}
#[test]
fn encoded_and_dimension_limits_reject_before_publication_and_jpeg_is_opaque() {
    let dir = tempfile::tempdir().unwrap();
    let huge = dir.path().join("huge.png");
    std::fs::File::create(&huge).unwrap().set_len(MAX_FILE_BYTES + 1).unwrap();
    assert!(decode(&huge).is_err());
    let wide = dir.path().join("wide.png");
    png(&wide, 4097, 1);
    assert!(decode(&wide).is_err());
    let jpg = dir.path().join("image.dat");
    image::save_buffer_with_format(
        &jpg,
        &[80, 100, 120].repeat(6),
        3,
        2,
        image::ColorType::Rgb8,
        ImageFormat::Jpeg,
    )
    .unwrap();
    let image = decode(&jpg).unwrap();
    assert_eq!((image.width, image.height), (3, 2));
    assert!(image.pixels.as_chunks::<4>().0.iter().all(|p| p[3] == 255));
}
#[test]
fn tab_layer_and_memory_limits_reject_without_starting_a_worker() {
    let (_dir, mut app) = app();
    let before = app.documents.active_session.document.clone();
    app.documents.max_tabs = 1;
    assert!(app.queue_image_import("not-read.png".into(), false).is_err());
    app.documents.retained_limit = IMPORT_RESERVATION;
    assert!(app.queue_image_import("not-read.png".into(), true).is_err());
    assert!(app.image_import.job.is_none());
    assert_eq!(app.documents.active_session.document, before);
    app.documents.retained_limit = 1024 * 1024 * 1024;
    while app.documents.active_session.document.layers.len() < 64 {
        app.documents.active_session.document.add_layer();
    }
    assert!(app.queue_image_import("not-read.png".into(), true).is_err());
}

#[test]
fn decoder_applies_jpeg_orientation_and_converts_sixteen_bit_png_locally() {
    let dir = tempfile::tempdir().unwrap();
    let jpeg = dir.path().join("rotated.jpg");
    image::save_buffer_with_format(
        &jpeg,
        &[30, 80, 120].repeat(6),
        3,
        2,
        image::ColorType::Rgb8,
        ImageFormat::Jpeg,
    )
    .unwrap();
    let original = std::fs::read(&jpeg).unwrap();
    let mut exif = b"Exif\0\0II\x2a\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0\x06\0\0\0\0\0\0\0".to_vec();
    let mut oriented = original[..2].to_vec();
    oriented.extend_from_slice(&[0xff, 0xe1]);
    oriented.extend_from_slice(&((exif.len() + 2) as u16).to_be_bytes());
    oriented.append(&mut exif);
    oriented.extend_from_slice(&original[2..]);
    std::fs::write(&jpeg, oriented).unwrap();
    let decoded = decode(&jpeg).unwrap();
    assert_eq!((decoded.width, decoded.height), (2, 3));
    let png = dir.path().join("gray16.png");
    let bytes = [0u16, u16::MAX]
        .into_iter()
        .flat_map(u16::to_ne_bytes)
        .collect::<Vec<_>>();
    image::save_buffer_with_format(
        &png,
        &bytes,
        2,
        1,
        image::ColorType::L16,
        ImageFormat::Png,
    )
    .unwrap();
    assert_eq!(
        decode(&png).unwrap().pixels,
        [0, 0, 0, 255, 255, 255, 255, 255]
    );
}
#[test]
fn empty_workspace_exposes_new_image_but_not_layer_import_and_escape_cancels() {
    let (_dir, mut app) = app();
    let id = app.documents.active_id().unwrap();
    assert!(app.close_document_tab(id));
    assert!(
        app.command_disabled_reason(crate::shortcuts::Command::ImportImage)
            .is_none()
    );
    assert!(
        app.command_disabled_reason(
            crate::shortcuts::Command::ImportImageLayer
        )
        .is_some()
    );
    let (sender, receiver) = mpsc::sync_channel(1);
    app.image_import.job =
        Some(ImportJob { target: Target::New, receiver, cancelled: false });
    let ctx = egui::Context::default();
    crate::theme::apply(&ctx);
    for events in [
        vec![],
        vec![egui::Event::Key {
            key: egui::Key::Escape,
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
                    egui::vec2(960., 640.),
                )),
                events,
                ..Default::default()
            },
            |ui| app.draw_ui(ui),
        );
        output.textures_delta.clear();
    }
    assert!(!app.dialog_open());
    assert!(app.image_import.job.as_ref().unwrap().cancelled);
    assert!(app.image_import.reserved_bytes() > 0);
    sender
        .send(Ok(DecodedImage {
            name: "Cancelled".into(),
            width: 1,
            height: 1,
            pixels: vec![0; 4],
        }))
        .unwrap();
    app.poll_image_import();
    assert!(app.documents.is_empty());
    assert_eq!(app.image_import.reserved_bytes(), 0);
}
