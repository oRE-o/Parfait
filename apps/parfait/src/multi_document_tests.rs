//! Single-document oracle and workload retained across the session extraction.
use crate::app::Parfait;
use eframe::egui::{self, Event, Pos2, Rect};
use parfait_core::{Document, History, LayerMask, MaskCoverage, PixelRect};
use parfait_storage::Store;
use std::{sync::Arc, time::Instant};

fn pattern(width: u32, height: u32, variant: u8) -> Document {
    let mut document = Document::new(width, height);
    document.name = format!("Baseline {variant}");
    document.layers[1].pixels = Arc::new(
        (0..width * height)
            .flat_map(|index| {
                [
                    (index % 97) as u8 ^ variant,
                    (index % 61) as u8,
                    180 - variant,
                    if index % 3 == 0 { 0 } else { 255 },
                ]
            })
            .collect(),
    );
    document
}

fn masked(width: u32, height: u32) -> Document {
    let mut document = pattern(width, height, 7);
    document.group_layers_in_folder(2, "Baseline folder".into()).unwrap();
    document.layers[1].mask = Some(LayerMask {
        enabled: true,
        coverage: MaskCoverage::Dense(Arc::new(
            (0..width * height)
                .map(|index| [0, 128, 255][index as usize % 3])
                .collect(),
        )),
    });
    document.layers[2].mask =
        Some(LayerMask { enabled: true, coverage: MaskCoverage::Uniform(193) });
    document.selection = Some(MaskCoverage::Dense(Arc::new(
        (0..width * height)
            .map(|index| [0, 64, 255, 128][index as usize % 4])
            .collect(),
    )));
    document.active = 1;
    assert!(document.validate());
    document
}

// Integer oracle for this fixture only: one masked child, one masked Normal
// folder and opaque white paper. It calls no production compositing routine.
fn expected_composite(document: &Document) -> Vec<u8> {
    document.layers[1]
        .pixels
        .as_chunks::<4>()
        .0
        .iter()
        .enumerate()
        .flat_map(|(index, pixel)| {
            let masked = (u32::from(pixel[3])
                * u32::from([0_u8, 128, 255][index % 3])
                + 127)
                / 255;
            let alpha = (masked * 193 + 127) / 255;
            [
                ((u32::from(pixel[0]) * alpha + 255 * (255 - alpha) + 127)
                    / 255) as u8,
                ((u32::from(pixel[1]) * alpha + 255 * (255 - alpha) + 127)
                    / 255) as u8,
                ((u32::from(pixel[2]) * alpha + 255 * (255 - alpha) + 127)
                    / 255) as u8,
                255,
            ]
        })
        .collect()
}

fn checksum(bytes: &[u8]) -> u64 {
    bytes.iter().enumerate().fold(0, |sum: u64, (index, byte)| {
        sum.wrapping_add((index as u64 + 1).wrapping_mul(u64::from(*byte)))
    })
}

fn rows(history: &History) -> Vec<(u64, String, bool, bool)> {
    history
        .rows()
        .map(|row| (row.id, row.label.to_owned(), row.current, row.future))
        .collect()
}

fn assert_artwork(actual: &Document, expected: &Document) {
    let mut expected = expected.clone();
    expected.revision = actual.revision;
    assert_eq!(*actual, expected);
}

fn frame(
    app: &mut Parfait,
    context: &egui::Context,
    time: f64,
    events: Vec<Event>,
) -> egui::FullOutput {
    context.run_ui(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(
                Pos2::ZERO,
                egui::vec2(800., 600.),
            )),
            time: Some(time),
            events,
            ..Default::default()
        },
        |ui| app.canvas(ui),
    )
}

fn button(position: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos: position,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    }
}

#[test]
fn multi_document_baseline_fixtures_have_independent_pixels_and_fixed_oracle() {
    let a = pattern(32, 24, 7);
    let b = pattern(32, 24, 17);
    assert_eq!(a.revision, b.revision);
    assert_eq!(a.layers[1].id, b.layers[1].id);
    assert_ne!(a.name, b.name);
    assert_ne!(a.layers[1].pixels, b.layers[1].pixels);
    assert!(!Arc::ptr_eq(&a.layers[1].pixels, &b.layers[1].pixels));
    let m = masked(32, 24);
    let original = m.clone();
    let expected = expected_composite(&m);
    assert_eq!(checksum(&m.layers[1].pixels), 498_223_128);
    assert_eq!(checksum(&expected), 973_944_324);
    assert_eq!(m.composite(), expected);
    for region in [
        PixelRect { x0: 3, y0: 2, x1: 19, y1: 13 },
        PixelRect { x0: 31, y0: 23, x1: 32, y1: 24 },
    ] {
        let crop: Vec<u8> = (region.y0..region.y1)
            .flat_map(|y| {
                let start = (y * m.width + region.x0) as usize * 4;
                expected[start..start + (region.x1 - region.x0) as usize * 4]
                    .to_vec()
            })
            .collect();
        assert_eq!(m.composite_region(region), crop);
    }
    assert_eq!(m, original);
    assert!(Arc::ptr_eq(&m.layers[1].pixels, &original.layers[1].pixels));
    println!(
        "multi_document_baseline fixture=M raw={} composite={} selection={}",
        checksum(&m.layers[1].pixels),
        checksum(&expected),
        checksum(match m.selection.as_ref().unwrap() {
            MaskCoverage::Dense(values) => values,
            _ => unreachable!(),
        })
    );
}

#[test]
fn multi_document_baseline_history_failure_and_noop_preserve_future() {
    let directory = tempfile::tempdir().unwrap();
    let mut app = Parfait::open(directory.path().to_owned()).unwrap();
    app.documents.session.document = masked(32, 24);
    app.edit_named("Baseline opacity", |document| {
        document.layers[1].opacity = 0.5
    });
    let first = app.documents.session.document.clone();
    app.edit_named("Baseline name", |document| {
        document.layers[1].name = "Second edit".into()
    });
    let second = app.documents.session.document.clone();
    app.undo();
    assert_artwork(&app.documents.session.document, &first);
    let before = app.documents.session.document.clone();
    let history = rows(&app.documents.session.history);
    app.edit_named("Baseline no-op", |_| {});
    assert_eq!(rows(&app.documents.session.history), history);
    assert_eq!(app.documents.session.document, before);
    assert!(
        app.save_project_to(directory.path().join("missing/output.parfait"))
            .is_err()
    );
    assert_eq!(app.documents.session.document, before);
    assert_eq!(rows(&app.documents.session.history), history);
    assert!(app.documents.session.history.can_redo());
    app.redo();
    assert_artwork(&app.documents.session.document, &second);
    assert!(Arc::ptr_eq(
        &app.documents.session.document.layers[1].pixels,
        &before.layers[1].pixels
    ));
}

#[test]
fn multi_document_baseline_pointer_undo_save_and_recovery_keep_exact_artwork() {
    let directory = tempfile::tempdir().unwrap();
    let mut app = Parfait::open(directory.path().to_owned()).unwrap();
    app.documents.session.document = masked(32, 24);
    app.brush.size = 8.;
    app.brush.pressure = false;
    let original = app.documents.session.document.clone();
    let context = egui::Context::default();
    frame(&mut app, &context, 0., vec![]).textures_delta.clear();
    frame(&mut app, &context, 0.016, vec![]).textures_delta.clear();
    let rect = context
        .data(|data| data.get_temp::<Rect>(egui::Id::new("canvas_rect")))
        .unwrap();
    let position = app.documents.session.view.screen(
        egui::vec2(16., 12.),
        rect,
        egui::vec2(32., 24.),
    );
    frame(
        &mut app,
        &context,
        0.032,
        vec![Event::PointerMoved(position), button(position, true)],
    )
    .textures_delta
    .clear();
    frame(&mut app, &context, 0.048, vec![button(position, false)])
        .textures_delta
        .clear();
    assert!(app.documents.session.editing.stroke.is_none());
    assert_ne!(
        app.documents.session.document.layers[1].pixels,
        original.layers[1].pixels
    );
    assert_eq!(app.documents.session.document.layers[0], original.layers[0]);
    assert_eq!(app.documents.session.document.layers[2], original.layers[2]);
    assert_eq!(app.documents.session.document.selection, original.selection);
    for index in (0..32 * 24).step_by(4) {
        assert_eq!(
            &app.documents.session.document.layers[1].pixels
                [index * 4..index * 4 + 4],
            &original.layers[1].pixels[index * 4..index * 4 + 4]
        );
    }
    let painted = app.documents.session.document.clone();
    app.undo();
    assert_artwork(&app.documents.session.document, &original);
    app.redo();
    assert_artwork(&app.documents.session.document, &painted);
    app.save_project_to(directory.path().join("baseline.parfait")).unwrap();
    let saved = app.documents.session.document.clone();
    let loaded = Store::open(&directory.path().join("baseline.parfait"))
        .unwrap()
        .load()
        .unwrap()
        .unwrap()
        .document;
    assert_artwork(&loaded, &saved);
    let recovered_pixels = saved.composite();
    assert_eq!(checksum(&saved.layers[1].pixels), 496_657_091);
    assert_eq!(checksum(&recovered_pixels), 972_926_990);
    app.autosave.finish(saved.clone());
    drop(app);
    let reopened = Parfait::open(directory.path().to_owned()).unwrap();
    assert_artwork(&reopened.documents.session.document, &saved);
    assert_eq!(
        reopened.documents.session.document.composite(),
        recovered_pixels
    );
    assert!(reopened.documents.session.project.is_none());
    assert_eq!(reopened.documents.session.history.rows().count(), 0);
    println!(
        "multi_document_baseline fixture=painted raw={} composite={}",
        checksum(&saved.layers[1].pixels),
        checksum(&recovered_pixels)
    );
}

#[test]
#[ignore = "release-only headless canvas-frame and synchronous save baseline"]
fn multi_document_baseline_release_workload() {
    if cfg!(debug_assertions) {
        panic!("Run with --release");
    }
    let directory = tempfile::tempdir().unwrap();
    let document = masked(512, 384);
    let expected = expected_composite(&document);
    assert_eq!(document.composite(), expected);
    let mut store =
        Store::open(&directory.path().join("recovery.parfait")).unwrap();
    store.save(&document).unwrap();
    drop(store);
    let mut app = Parfait::open(directory.path().to_owned()).unwrap();
    app.brush.size = 32.;
    app.brush.pressure = false;
    let context = egui::Context::default();
    for index in 0..3 {
        frame(&mut app, &context, index as f64 / 60., vec![])
            .textures_delta
            .clear();
    }
    let rect = context
        .data(|data| data.get_temp::<Rect>(egui::Id::new("canvas_rect")))
        .unwrap();
    let mut samples = Vec::new();
    for index in 0..256 {
        let position = app.documents.session.view.screen(
            egui::vec2(32. + index as f32 * 1.5, 180.),
            rect,
            egui::vec2(512., 384.),
        );
        let mut events = vec![Event::PointerMoved(position)];
        if index == 0 {
            events.push(button(position, true));
        }
        let start = Instant::now();
        let mut output =
            frame(&mut app, &context, (index + 3) as f64 / 60., events);
        samples.push(start.elapsed().as_secs_f64() * 1000.);
        output.textures_delta.clear();
    }
    let position = context.input(|input| input.pointer.latest_pos()).unwrap();
    frame(&mut app, &context, 5., vec![button(position, false)])
        .textures_delta
        .clear();
    assert!(app.documents.session.editing.stroke.is_none());
    let final_pixels = app.documents.session.document.layers[1].pixels.clone();
    let final_composite = app.documents.session.document.composite();
    let mut save_samples = Vec::new();
    for _ in 0..9 {
        let start = Instant::now();
        app.save_project_to(directory.path().join("baseline.parfait")).unwrap();
        save_samples.push(start.elapsed().as_secs_f64() * 1000.);
        let loaded = Store::open(&directory.path().join("baseline.parfait"))
            .unwrap()
            .load()
            .unwrap()
            .unwrap()
            .document;
        assert_artwork(&loaded, &app.documents.session.document);
    }
    assert_eq!(app.documents.session.document.layers[1].pixels, final_pixels);
    assert_eq!(app.documents.session.document.composite(), final_composite);
    samples.sort_by(f64::total_cmp);
    save_samples.sort_by(f64::total_cmp);
    let percentile = |values: &[f64], percent: usize| {
        values[(values.len() * percent).div_ceil(100) - 1]
    };
    println!(
        "multi_document_baseline workload=512x384-folder-mask-selection updates=256 frame_scope=headless_canvas_run_ui frame_p50_ms={:.6} frame_p95_ms={:.6} frame_p99_ms={:.6} frame_max_ms={:.6} save_samples=9 save_p50_ms={:.6} save_p95_ms={:.6} raw={} composite={} logical_bytes={}",
        percentile(&samples, 50),
        percentile(&samples, 95),
        percentile(&samples, 99),
        samples[255],
        percentile(&save_samples, 50),
        percentile(&save_samples, 95),
        checksum(&final_pixels),
        checksum(&final_composite),
        app.documents.session.document.logical_pixel_bytes()
    );
    app.autosave.finish(app.documents.session.document.clone());
}

#[test]
fn multi_document_session_move_and_replacement_preserve_owned_buffers_and_settings()
 {
    use crate::document_session::DocumentSession;
    let document = masked(32, 24);
    let expected = document.clone();
    let pixels = document.layers[1].pixels.clone();
    let references = Arc::strong_count(&pixels);
    let session = DocumentSession::new(document).unwrap();
    assert_eq!(session.document, expected);
    assert!(Arc::ptr_eq(&pixels, &session.document.layers[1].pixels));
    assert_eq!(Arc::strong_count(&pixels), references);
    let MaskCoverage::Dense(selection) =
        session.document.selection.as_ref().unwrap()
    else {
        unreachable!()
    };
    let MaskCoverage::Dense(original_selection) =
        expected.selection.as_ref().unwrap()
    else {
        unreachable!()
    };
    assert!(Arc::ptr_eq(selection, original_selection));
    let MaskCoverage::Dense(mask) =
        &session.document.layers[1].mask.as_ref().unwrap().coverage
    else {
        unreachable!()
    };
    let MaskCoverage::Dense(original_mask) =
        &expected.layers[1].mask.as_ref().unwrap().coverage
    else {
        unreachable!()
    };
    assert!(Arc::ptr_eq(mask, original_mask));

    let directory = tempfile::tempdir().unwrap();
    let mut app = Parfait::open(directory.path().to_owned()).unwrap();
    app.brush.color = [18, 37, 91];
    app.brush.size = 19.;
    app.display.background = 45;
    let brush = app.brush.clone();
    let keys = app.keymap.clone();
    let key = app.documents.session.key;
    app.documents.session = session;
    assert_ne!(app.documents.session.key, key);
    assert!(app.brush == brush);
    assert!(app.keymap == keys);
    assert_eq!(app.display.background, 45);
    assert!(app.documents.session.history.rows().next().is_none());
}

#[test]
fn multi_document_session_key_invalidates_cache_and_stale_commands_and_dialogs()
{
    use crate::{document_session::DocumentSession, shortcuts::Command};
    let directory = tempfile::tempdir().unwrap();
    let mut app = Parfait::open(directory.path().to_owned()).unwrap();
    app.documents.session = DocumentSession::new(masked(32, 24)).unwrap();
    let context = egui::Context::default();
    crate::theme::apply(&context);
    let key = app.documents.session.key;
    frame(&mut app, &context, 0., vec![]).textures_delta.clear();
    app.update_previews(&context);
    assert_eq!(app.render_cache.key, Some(key));
    let texture = app.render_cache.texture.as_ref().unwrap().id();
    assert!(!app.render_cache.layer_previews.is_empty());
    app.execute_command(Command::RenameLayer, &context);
    let mut output =
        context.run_ui(Default::default(), |ui| app.layer_rename_ui(ui.ctx()));
    output.textures_delta.clear();
    let widget = context
        .data(|data| {
            data.get_temp::<egui::Id>(egui::Id::new("rename_field_id"))
        })
        .unwrap();
    let stale_rename = app.documents.session.layer_ui.rename.take();
    app.open_layer_details();
    let stale_details = app.documents.session.layer_ui.details.take();
    let mut replacement =
        DocumentSession::new(app.documents.session.document.clone()).unwrap();
    // Same document ID and revision; only the memory session has changed.
    replacement.key.document_id = key.document_id;
    let replacement_key = replacement.key;
    assert_ne!(key.epoch, replacement_key.epoch);
    app.documents.session = replacement;
    let before = app.documents.session.document.clone();
    app.execute_command_for(key, Command::NewLayer, &context);
    assert_eq!(app.documents.session.document, before);
    assert!(app.documents.session.history.rows().next().is_none());
    app.documents.session.layer_ui.rename = stale_rename;
    app.documents.session.layer_ui.details = stale_details;
    let mut output = context.run_ui(Default::default(), |ui| {
        app.layer_rename_ui(ui.ctx());
        app.layer_details_ui(ui.ctx());
    });
    output.textures_delta.clear();
    assert!(app.documents.session.layer_ui.rename.is_none());
    assert!(app.documents.session.layer_ui.details.is_none());
    assert_eq!(app.documents.session.document, before);
    frame(&mut app, &context, 0.1, vec![]).textures_delta.clear();
    assert_eq!(app.render_cache.key, Some(replacement_key));
    assert_ne!(app.render_cache.texture.as_ref().unwrap().id(), texture);
    assert!(app.render_cache.layer_previews.is_empty());
    assert!(app.render_cache.mask_previews.is_empty());
    app.execute_command(Command::RenameLayer, &context);
    let mut output =
        context.run_ui(Default::default(), |ui| app.layer_rename_ui(ui.ctx()));
    output.textures_delta.clear();
    let replacement_widget = context
        .data(|data| {
            data.get_temp::<egui::Id>(egui::Id::new("rename_field_id"))
        })
        .unwrap();
    assert_ne!(widget, replacement_widget);
}

#[test]
fn multi_document_new_and_open_replace_identity_only_on_success() {
    let directory = tempfile::tempdir().unwrap();
    let mut app = Parfait::open(directory.path().to_owned()).unwrap();
    app.documents.session.document = masked(32, 24);
    let first = app.documents.session.key;
    let original = app.documents.session.document.clone();
    let path = directory.path().join("baseline.parfait");
    app.save_project_to(path.clone()).unwrap();
    assert_eq!(app.documents.session.key, first);
    let corrupt = directory.path().join("corrupt.parfait");
    std::fs::write(&corrupt, b"not a database").unwrap();
    assert!(app.open_project_from(corrupt).is_err());
    assert_eq!(app.documents.session.key, first);
    app.open_project_from(path).unwrap();
    assert_ne!(app.documents.session.key.document_id, first.document_id);
    assert_ne!(app.documents.session.key.epoch, first.epoch);
    assert_eq!(app.documents.session.document.layers, original.layers);
    let opened = app.documents.session.key;
    app.new_document();
    assert!(app.error.is_none());
    assert_ne!(app.documents.session.key.document_id, opened.document_id);
    assert_ne!(app.documents.session.key.epoch, opened.epoch);
}

#[test]
fn multi_document_stale_input_owner_cannot_edit_a_replacement_session() {
    use crate::{document_session::DocumentSession, editing::LayerEditKind};
    let directory = tempfile::tempdir().unwrap();
    let mut app = Parfait::open(directory.path().to_owned()).unwrap();
    app.documents.session.document = masked(32, 24);
    let owner = app.documents.session.key;
    assert!(app.begin_canvas_stroke(parfait_core::Point {
        x: 16.,
        y: 12.,
        pressure: 1.
    }));
    let stroke = app.documents.session.editing.stroke.take();
    app.documents.session.editing.stroke_owner = None;
    app.finish_layer_edit();
    app.begin_layer_edit(LayerEditKind::Opacity, 1, true);
    let layer_edit = app.documents.session.editing.layer_edit.take();
    let replacement = DocumentSession::new(pattern(32, 24, 17)).unwrap();
    let before = replacement.document.clone();
    app.documents.session = replacement;
    app.documents.session.editing.stroke = stroke;
    app.documents.session.editing.stroke_owner = Some(owner);
    app.documents.session.editing.layer_edit = layer_edit;
    let context = egui::Context::default();
    frame(
        &mut app,
        &context,
        0.,
        vec![Event::PointerMoved(egui::pos2(400., 300.))],
    )
    .textures_delta
    .clear();
    app.finish_layer_edit();
    assert!(app.documents.session.editing.stroke.is_none());
    assert!(app.documents.session.editing.stroke_owner.is_none());
    assert!(app.documents.session.editing.layer_edit.is_none());
    assert_eq!(app.documents.session.document, before);
    assert!(app.documents.session.history.rows().next().is_none());
}

#[test]
fn multi_document_edit_id_and_save_as_keep_history_and_filename_independent() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = Parfait::open(dir.path().to_owned()).unwrap();
    app.edit_named("First", |d| d.layers[1].opacity = 0.5);
    app.save_project_to(dir.path().join("first.parfait")).unwrap();
    let saved = app.documents.session.history.state_id();
    app.edit_named("Second", |d| d.layers[1].opacity = 0.25);
    let second = app.documents.session.history.state_id();
    assert_ne!(second, saved);
    app.undo();
    assert_eq!(app.documents.session.history.state_id(), saved);
    assert!(!app.project_is_modified());
    app.save_project_to(dir.path().join("renamed.parfait")).unwrap();
    assert_eq!(app.documents.session.history.state_id(), saved);
    assert!(app.documents.session.history.can_redo());
    app.redo();
    assert_eq!(app.documents.session.history.state_id(), second);
    assert!(app.project_is_modified());
    assert_eq!(app.documents.session.document.name, "renamed");
    app.undo();
    assert!(!app.project_is_modified());
    assert_eq!(app.documents.session.document.name, "renamed");
}
