use super::*;
use crate::{editing::LayerEditKind, shortcuts::Command};
use parfait_core::{LayerMask, MaskCoverage, Point};
use std::sync::Arc;

fn fixture() -> (tempfile::TempDir, Parfait) {
    let dir = tempfile::tempdir().unwrap();
    let mut app = Parfait::open_document_tabs(dir.path().to_owned()).unwrap();
    app.documents.active_session.document = Document::new(8, 8);
    (dir, app)
}
fn pattern(value: u8) -> Document {
    let mut doc = Document::new(8, 8);
    doc.name = format!("Drawing {value}");
    doc.layers[1].pixels = Arc::new([value, 20, 40, 255].repeat(64));
    doc.layers[1].mask = Some(LayerMask {
        enabled: true,
        coverage: MaskCoverage::Dense(Arc::new(vec![128; 64])),
    });
    doc.selection = Some(MaskCoverage::Dense(Arc::new(vec![192; 64])));
    doc
}
fn result(id: u64, doc: Document) -> LoadResult {
    LoadResult {
        id,
        digest: doc.art_digest_v1().map(|d| d.0),
        source_head: None,
        fell_back: false,
        result: Ok(doc),
        thread: std::thread::current().id(),
    }
}
fn rows(app: &Parfait) -> Vec<(u64, bool)> {
    app.documents
        .active_session
        .history
        .rows()
        .map(|row| (row.id, row.future))
        .collect()
}
fn frame(app: &mut Parfait, ctx: &egui::Context, events: Vec<egui::Event>) {
    let themed = egui::Id::new("tabs_test_theme");
    if !ctx.data(|d| d.get_temp::<bool>(themed).unwrap_or(false)) {
        crate::theme::apply(ctx);
        ctx.data_mut(|d| d.insert_temp(themed, true));
    }
    let mut out = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(800., 600.),
            )),
            events,
            ..Default::default()
        },
        |ui| {
            app.document_tabs_ui(ui);
            if app.documents.is_empty() {
                app.menu_bar(ui);
            } else {
                egui::CentralPanel::default().show(ui, |ui| app.canvas(ui));
            }
        },
    );
    out.textures_delta.clear();
}
fn button(pos: egui::Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    }
}
#[test]
fn tab_drag_reorders_without_editing_documents_and_escape_cancels() {
    let (dir, mut app) = fixture();
    app.documents.active_session.untouched_new = false;
    let a = app.documents.active_id().unwrap();
    let b = app.new_document_tab(pattern(70)).unwrap();
    let c = app.new_document_tab(pattern(130)).unwrap();
    let originals: Vec<_> = app
        .documents
        .tab_order
        .iter()
        .map(|id| (*id, app.documents.get(*id).unwrap().document.clone()))
        .collect();
    let ctx = egui::Context::default();
    for _ in 0..3 {
        frame(&mut app, &ctx, vec![]);
    }
    let rect = |id| {
        ctx.data(|d| d.get_temp::<egui::Rect>(egui::Id::new(("tab", id))))
            .unwrap()
    };
    let from = rect(a).center();
    let to = rect(c).right_center() + egui::vec2(22., 0.);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(from), button(from, true)],
    );
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(to)]);
    frame(&mut app, &ctx, vec![button(to, false)]);
    assert_eq!(app.documents.tab_order, [b, c, a]);
    assert_eq!(app.documents.active_id(), Some(c));
    for (id, doc) in &originals {
        assert_eq!(&app.documents.get(*id).unwrap().document, doc);
        assert!(!app.documents.get(*id).unwrap().history.can_undo());
    }
    for _ in 0..2 {
        frame(&mut app, &ctx, vec![]);
    }
    let from = rect(b).center();
    let to = rect(a).right_center() + egui::vec2(22., 0.);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(from), button(from, true)],
    );
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(to)]);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        }],
    );
    frame(&mut app, &ctx, vec![button(to, false)]);
    assert_eq!(app.documents.tab_order, [b, c, a]);
    app.flush_document_recovery(true).unwrap();
    drop(app);
    let restored = Parfait::open_document_tabs(dir.path().into()).unwrap();
    assert_eq!(restored.documents.tab_order, [b, c, a]);
}
#[test]
fn full_workspace_places_tabs_between_command_bar_and_canvas_inside_docks() {
    let (_dir, mut app) = fixture();
    let id = app.documents.active_id().unwrap();
    app.new_document_tab(pattern(70)).unwrap();
    let ctx = egui::Context::default();
    crate::theme::apply(&ctx);
    for _ in 0..3 {
        let mut out = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1440., 920.),
                )),
                ..Default::default()
            },
            |ui| app.draw_ui(ui),
        );
        out.textures_delta.clear();
    }
    let strip = ctx
        .data(|d| d.get_temp::<egui::Rect>(egui::Id::new("canvas_tab_strip")))
        .unwrap();
    let canvas = ctx
        .data(|d| d.get_temp::<egui::Rect>(egui::Id::new("canvas_rect")))
        .unwrap();
    let tab = ctx
        .data(|d| d.get_temp::<egui::Rect>(egui::Id::new(("tab", id))))
        .unwrap();
    assert!(strip.left() > 200. && strip.right() < 1440. - 150., "{strip:?}");
    assert!(strip.bottom() <= canvas.top());
    assert!(strip.contains_rect(tab));
    assert!(strip.top() > 60.);
    assert!(canvas.height() > 300.);
}
#[test]
fn canvas_commands_follow_visual_order_and_dirty_middle_click_requires_a_choice()
 {
    let (_dir, mut app) = fixture();
    let a = app.documents.active_id().unwrap();
    let b = app.new_document_tab(pattern(70)).unwrap();
    let ctx = egui::Context::default();
    app.execute_command(Command::NextCanvas, &ctx);
    assert_eq!(app.documents.active_id(), Some(a));
    app.execute_command(Command::PreviousCanvas, &ctx);
    assert_eq!(app.documents.active_id(), Some(b));
    for _ in 0..3 {
        frame(&mut app, &ctx, vec![]);
    }
    let p = ctx
        .data(|d| d.get_temp::<egui::Rect>(egui::Id::new(("tab", b))))
        .unwrap()
        .center();
    for pressed in [true, false] {
        frame(
            &mut app,
            &ctx,
            vec![
                egui::Event::PointerMoved(p),
                egui::Event::PointerButton {
                    pos: p,
                    button: egui::PointerButton::Middle,
                    pressed,
                    modifiers: Default::default(),
                },
            ],
        );
    }
    assert_eq!(app.tabs.close_prompt, Some(b));
    assert!(app.documents.get(b).is_some());
    assert!(app.dialog_open());
    app.tabs.close_prompt = None;
    assert!(app.documents.get(b).is_some());
}

#[test]
fn multi_document_tabs_isolate_pixels_history_view_and_cache() {
    let (_dir, mut app) = fixture();
    let a = app.documents.active_id().unwrap();
    app.documents.active_session.document = pattern(10);
    app.documents.active_session.view.zoom = 2.;
    let a_before = app.documents.active_session.document.clone();
    let b = app.new_document_tab(pattern(90)).unwrap();
    let b_before = app.documents.active_session.document.clone();
    let b_state = app.documents.active_session.history.state_id();
    app.documents.active_session.view.zoom = 3.;
    assert!(app.activate_document(a, true));
    let ctx = egui::Context::default();
    frame(&mut app, &ctx, vec![]);
    assert_eq!(app.render_cache.key, Some(app.documents.active_session.key));
    assert!(app.begin_canvas_stroke(Point { x: 4., y: 4., pressure: 1. }));
    assert!(app.activate_document(b, true));
    assert!(app.render_cache.texture.is_none());
    assert_eq!(app.documents.active_session.document, b_before);
    assert_eq!(app.documents.active_session.view.zoom, 3.);
    assert_eq!(app.documents.active_session.history.state_id(), b_state);
    assert!(!app.documents.active_session.history.can_undo());
    frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::PointerMoved(egui::pos2(400., 300.)),
            button(egui::pos2(400., 300.), false),
        ],
    );
    assert_eq!(app.documents.active_session.document, b_before);
    assert!(app.tabs.input_barrier);
    assert!(app.activate_document(a, true));
    assert!(app.documents.active_session.history.can_undo());
    app.undo();
    assert_eq!(app.documents.active_session.document.layers, a_before.layers);
    assert_eq!(
        app.documents.active_session.document.selection,
        a_before.selection
    );
    assert_eq!(app.documents.active_session.edit_reservation, 0);
    assert_eq!(app.documents.get(b).unwrap().document, b_before);
}

#[test]
fn multi_document_real_tab_click_commits_opacity_and_blocks_modal() {
    let (_dir, mut app) = fixture();
    let a = app.documents.active_id().unwrap();
    let b = app.new_document_tab(pattern(90)).unwrap();
    app.activate_document(a, true);
    assert!(app.begin_layer_edit(LayerEditKind::Opacity, 1, true));
    app.documents.active_session.document.layers[1].opacity = 0.4;
    let ctx = egui::Context::default();
    frame(&mut app, &ctx, vec![]);
    frame(&mut app, &ctx, vec![]);
    let pos = ctx
        .data(|d| d.get_temp::<egui::Rect>(egui::Id::new(("tab", b))))
        .unwrap()
        .center();
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(pos), button(pos, true)],
    );
    frame(&mut app, &ctx, vec![button(pos, false)]);
    assert_eq!(app.documents.active_id(), Some(b));
    assert_eq!(app.documents.get(a).unwrap().history.rows().count(), 2);
    app.open_layer_details();
    assert!(!app.activate_document(a, true));
    assert_eq!(app.documents.active_id(), Some(b));
    app.documents.active_session.layer_ui.details = None;
    app.activate_document(a, true);
    app.undo();
    assert_eq!(app.documents.active_session.document.layers[1].opacity, 1.);
}

#[test]
fn multi_document_admission_and_last_close_keep_empty_workspace() {
    let (_dir, mut app) = fixture();
    let a = app.documents.active_id().unwrap();
    app.documents.max_tabs = 1;
    assert!(app.new_document_tab(pattern(20)).is_err());
    assert_eq!(app.documents.active_id(), Some(a));
    app.documents.max_tabs = 8;
    let bytes = pattern(20).logical_pixel_bytes();
    app.documents.logical_limit = app.documents.logical_bytes() + bytes - 1;
    assert!(app.new_document_tab(pattern(20)).is_err());
    app.documents.logical_limit += 1;
    let b = app.new_document_tab(pattern(20)).unwrap();
    app.edit(|d| d.layers[1].opacity = 0.5);
    assert!(!app.close_document_tab(b));
    app.documents.active_session.project_saved_state =
        Some(app.documents.active_session.document.clone());
    assert!(app.close_document_tab(b));
    assert_eq!(app.documents.active_id(), Some(a));
    assert!(app.close_document_tab(a));
    assert!(app.documents.is_empty());
    assert!(app.documents.tab_order.is_empty());
    assert_eq!(
        app.command_disabled_reason(Command::Undo),
        Some("Open a drawing first.")
    );
    app.execute_command(Command::Undo, &egui::Context::default());
    let ctx = egui::Context::default();
    crate::theme::apply(&ctx);
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
    output.textures_delta.clear();
    assert!(app.documents.is_empty());
    app.documents.logical_limit = 512 * 1024 * 1024;
    app.execute_command(Command::NewDrawing, &egui::Context::default());
    assert_eq!(app.documents.tab_order.len(), 1);
}

#[test]
fn multi_document_reversed_cancelled_and_duplicate_loads_preserve_order_and_focus()
 {
    let (dir, mut app) = fixture();
    let a = app.documents.active_id().unwrap();
    // Hold dispatch; completions below form a deterministic fake worker.
    app.tabs.in_flight = Some(u64::MAX);
    app.queue_project_open(dir.path().join("one.parfait")).unwrap();
    let first = app.tabs.pending[0].id;
    app.queue_project_open(dir.path().join("two.parfait")).unwrap();
    let second = app.tabs.pending[1].id;
    app.complete_project_open(result(second, pattern(20)));
    let b = app.documents.active_id().unwrap();
    app.complete_project_open(result(first, pattern(10)));
    assert_eq!(app.documents.active_id(), Some(b));
    assert_eq!(
        app.documents
            .tab_order
            .iter()
            .map(|id| app.documents.get(*id).unwrap().title())
            .collect::<Vec<_>>(),
        ["Untitled", "one.parfait", "two.parfait"]
    );
    app.queue_project_open(dir.path().join("three.parfait")).unwrap();
    let third = app.tabs.pending[0].id;
    app.activate_document(a, true);
    app.complete_project_open(result(third, pattern(30)));
    assert_eq!(app.documents.active_id(), Some(a));
    app.queue_project_open(dir.path().join("cancel.parfait")).unwrap();
    let cancel = app.tabs.pending[0].id;
    app.tabs.in_flight = Some(cancel);
    app.cancel_project_open(cancel);
    assert!(app.tabs.reserved_pixels() > 0);
    app.complete_project_open(result(cancel, pattern(40)));
    assert_eq!(app.tabs.reserved_pixels(), 0);
    let count = app.documents.tab_order.len();
    app.complete_project_open(result(cancel, pattern(40)));
    assert_eq!(app.documents.tab_order.len(), count);
    assert_eq!(app.documents.active_id(), Some(a));
}

#[test]
fn multi_document_real_reader_is_off_ui_and_failures_do_not_replace_redo() {
    let (dir, mut app) = fixture();
    app.edit(|d| d.layers[1].opacity = 0.5);
    app.edit(|d| d.layers[1].opacity = 0.2);
    app.undo();
    let before = app.documents.active_session.document.clone();
    let state = app.documents.active_session.history.state_id();
    let history = rows(&app);
    app.queue_project_open(dir.path().join("missing.parfait")).unwrap();
    let loaded = app.tabs.loader.as_ref().unwrap().wait();
    assert_ne!(loaded.thread, std::thread::current().id());
    assert!(loaded.result.is_err());
    app.complete_project_open(loaded);
    assert_eq!(app.documents.active_session.document, before);
    assert_eq!(app.documents.active_session.history.state_id(), state);
    assert_eq!(rows(&app), history);
    assert!(!dir.path().join("missing.parfait").exists());
    app.error = None;
    let path = dir.path().join("읽기.parfait");
    Parfait::write_project_document(&pattern(55), &path).unwrap();
    app.queue_project_open(path.clone()).unwrap();
    assert!(app.queue_project_open(path.clone()).is_err());
    assert!(app.project_path_allowed(&path).is_err());
    let loaded = app.tabs.loader.as_ref().unwrap().wait();
    assert_ne!(loaded.thread, std::thread::current().id());
    app.complete_project_open(loaded);
    assert_eq!(
        app.documents.active_session.document.layers,
        pattern(55).layers
    );
    let count = app.documents.tab_order.len();
    let id = app.documents.active_id();
    app.queue_project_open(path).unwrap();
    assert_eq!(app.documents.active_id(), id);
    assert_eq!(app.documents.tab_order.len(), count);
}

#[test]
fn multi_document_internal_paths_and_other_tab_save_targets_are_rejected() {
    let (dir, mut app) = fixture();
    for name in [
        "session.sqlite",
        "preferences.sqlite",
        "recovery.parfait",
        "session.lock",
    ] {
        for suffix in ["", "-wal", "-shm", "-journal"] {
            assert!(
                app.project_path_allowed(
                    &dir.path().join(format!("{name}{suffix}"))
                )
                .is_err()
            );
        }
    }
    std::fs::create_dir_all(dir.path().join("recovery/abc")).unwrap();
    assert!(
        app.project_path_allowed(
            &dir.path().join("recovery/abc/recovery.parfait")
        )
        .is_err()
    );
    let a = app.documents.active_id().unwrap();
    let first = dir.path().join("first.parfait");
    app.save_project_to(first.clone()).unwrap();
    app.new_document_tab(pattern(20)).unwrap();
    let before = app.documents.active_session.document.clone();
    assert!(app.save_project_to(first.clone()).is_err());
    assert_eq!(app.documents.active_session.document, before);
    assert!(app.documents.active_session.project.is_none());
    app.queue_project_open(first).unwrap();
    assert_eq!(app.documents.active_id(), Some(a));
    #[cfg(unix)]
    {
        let alias = dir.path().join("alias.parfait");
        std::os::unix::fs::symlink(dir.path().join("first.parfait"), &alias)
            .unwrap();
        app.queue_project_open(alias).unwrap();
        assert_eq!(app.documents.active_id(), Some(a));
        let hard = dir.path().join("hard.parfait");
        std::fs::hard_link(dir.path().join("first.parfait"), &hard).unwrap();
        app.queue_project_open(hard.clone()).unwrap();
        assert_eq!(app.documents.active_id(), Some(a));
        app.new_document_tab(Document::new(8, 8)).unwrap();
        assert!(app.save_project_to(hard).is_err());
        // The new durable-tab startup no longer creates a legacy recovery file.
        std::fs::write(dir.path().join("recovery.parfait"), b"legacy fixture")
            .unwrap();
        let internal_alias = dir.path().join("internal-alias.parfait");
        std::fs::hard_link(
            dir.path().join("recovery.parfait"),
            &internal_alias,
        )
        .unwrap();
        assert!(app.project_path_allowed(&internal_alias).is_err());
    }
}

#[test]
fn multi_document_pixel_reservation_boundary_preserves_redo_and_state() {
    let (_dir, mut app) = fixture();
    app.edit(|d| d.layers[1].opacity = 0.5);
    app.edit(|d| d.layers[1].opacity = 0.2);
    app.undo();
    let before = app.documents.active_session.document.clone();
    let state = app.documents.active_session.history.state_id();
    let history = rows(&app);
    let reserve = 64 * 4;
    app.documents.retained_limit =
        app.documents.buffers().bytes() + reserve - 1;
    assert!(!app.begin_canvas_stroke(Point { x: 4., y: 4., pressure: 1. }));
    assert_eq!(app.documents.active_session.document, before);
    assert_eq!(app.documents.active_session.history.state_id(), state);
    assert_eq!(rows(&app), history);
    assert_eq!(app.documents.active_session.edit_reservation, 0);
    app.documents.retained_limit += 1;
    assert!(app.begin_canvas_stroke(Point { x: 4., y: 4., pressure: 1. }));
    assert!(app.documents.active_session.edit_reservation > 0);
    app.cancel_edit();
    assert_eq!(app.documents.active_session.edit_reservation, 0);
    assert_eq!(app.documents.active_session.document.layers, before.layers);
    assert_eq!(app.documents.active_session.history.state_id(), state);
    assert_eq!(rows(&app), history);
}

#[test]
fn multi_document_reclaims_inactive_old_history_but_protects_before_and_redo() {
    let (_dir, mut app) = fixture();
    let a = app.documents.active_id().unwrap();
    for value in [10, 20, 30, 40] {
        app.edit(|d| Arc::make_mut(&mut d.layers[1].pixels)[0] = value);
    }
    let a_rows = rows(&app).len();
    let a_image = app.documents.active_session.document.clone();
    let b = app.new_document_tab(Document::new(8, 8)).unwrap();
    app.documents.retained_limit = app.documents.buffers().bytes();
    assert!(app.reserve_pixel_headroom(256));
    assert!(app.documents.get(a).unwrap().history.rows().count() < a_rows);
    assert_eq!(app.documents.get(a).unwrap().document, a_image);
    assert_eq!(app.documents.active_id(), Some(b));
    let rows_before = app.documents.get(a).unwrap().history.rows().count();
    assert!(!app.reserve_pixel_headroom(u64::MAX));
    assert_eq!(
        app.documents.get(a).unwrap().history.rows().count(),
        rows_before
    );
    app.documents.retained_limit = 1024 * 1024;
    app.begin_history("pending");
    let pinned = app.documents.active_session.document.clone();
    Arc::make_mut(
        &mut app.documents.active_session.document.layers[1].pixels,
    )[0] = 99;
    app.documents.retained_limit = 1;
    assert!(!app.reserve_pixel_headroom(1));
    app.cancel_edit();
    assert_eq!(app.documents.active_session.document.layers, pinned.layers);
}

#[test]
#[ignore = "release-only tab switch and retained-memory workload"]
fn multi_document_tabs_release_workload() {
    if cfg!(debug_assertions) {
        panic!("Run with --release");
    }
    for count in [1, 3, 8] {
        let (_dir, mut app) = fixture();
        app.documents.active_session.document = Document::new(512, 384);
        while app.documents.tab_order.len() < count {
            app.new_document_tab(Document::new(512, 384)).unwrap();
        }
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let expected = vec![255; 512 * 384 * 4];
        let mut samples = Vec::new();
        for iteration in 0..128 {
            let id = app.documents.tab_order[iteration % count];
            let start = std::time::Instant::now();
            assert!(app.activate_document(id, true));
            frame(&mut app, &ctx, vec![]);
            samples.push(start.elapsed().as_secs_f64() * 1000.);
            assert_eq!(
                app.documents.active_session.document.composite(),
                expected
            );
            assert!(
                app.documents.buffers().bytes() <= app.documents.retained_limit
            );
        }
        samples.sort_by(f64::total_cmp);
        let p = |percent: usize| {
            samples[(samples.len() * percent).div_ceil(100) - 1]
        };
        println!(
            "tabs={count} frames={} p50_ms={:.4} p95_ms={:.4} p99_ms={:.4} max_ms={:.4} retained_bytes={} logical_bytes={} checksum={}",
            samples.len(),
            p(50),
            p(95),
            p(99),
            p(100),
            app.documents.buffers().bytes(),
            app.documents.logical_bytes(),
            expected.iter().map(|&b| b as u64).sum::<u64>()
        );
    }
}

#[test]
fn multi_document_save_pin_is_shared_counted_and_released_on_failure() {
    let (dir, mut app) = fixture();
    let pinned = app.documents.active_session.document.clone();
    app.tabs.pinned_save = Some(pinned.clone());
    let bytes = app.documents.buffers().bytes();
    app.documents.retained_limit = bytes;
    assert!(app.reserve_pixel_headroom(0));
    app.documents.active_session.document.layers[1].pixels =
        Arc::new(vec![5; 256]);
    assert!(!app.reserve_pixel_headroom(0));
    assert_eq!(app.tabs.pinned_save.as_ref().unwrap().layers, pinned.layers);
    app.documents.retained_limit += 256;
    assert!(app.reserve_pixel_headroom(0));
    app.tabs.pinned_save = None;
    app.documents.retained_limit = 1024 * 1024 * 1024;
    let invalid = dir.path().join("broken.parfait");
    std::fs::write(&invalid, b"not a database").unwrap();
    let before = app.documents.active_session.document.clone();
    assert!(app.save_project_to(invalid).is_err());
    assert!(app.tabs.pinned_save.is_none());
    assert_eq!(app.documents.active_session.document, before);
}

#[test]
#[ignore = "release-only near-1-GiB retained pixel workload; run serially"]
fn multi_document_retained_budget_release_workload() {
    if cfg!(debug_assertions) {
        panic!("Run with --release");
    }
    let (_dir, mut app) = fixture();
    app.documents.active_session.document = Document::new(2800, 2800);
    while app.documents.tab_order.len() < 8 {
        app.new_document_tab(Document::new(2800, 2800)).unwrap();
    }
    for id in app.documents.tab_order.clone() {
        let session = app.documents.get_mut(id).unwrap();
        session.project_saved_state = Some(session.document.clone());
    }
    app.brush.size = 8.;
    app.brush.pressure = false;
    let mut high_water = 0;
    let mut accepted = 0;
    let start = std::time::Instant::now();
    for index in 0..64 {
        let id = app.documents.tab_order[index % 8];
        assert!(app.activate_document(id, true));
        app.brush.color = [index as u8 * 3, 25, 50];
        let state = app.documents.active_session.history.state_id();
        let before = app.documents.active_session.document.clone();
        if app.begin_canvas_stroke(Point {
            x: 100. + index as f32 * 4.,
            y: 100.,
            pressure: 1.,
        }) {
            app.finish_stroke();
            accepted += 1;
        } else {
            assert_eq!(app.documents.active_session.history.state_id(), state);
            assert_eq!(app.documents.active_session.document, before);
        }
        let bytes = app.documents.buffers().bytes();
        high_water = high_water.max(bytes);
        assert!(bytes <= app.documents.retained_limit);
        assert_eq!(app.documents.active_session.edit_reservation, 0);
    }
    assert!(accepted > 0);
    assert!(high_water > app.documents.retained_limit * 85 / 100);
    println!(
        "near_budget tabs=8 attempts=64 accepted={accepted} retained_high_water={high_water} retained_limit={} logical_bytes={} elapsed_ms={:.3}",
        app.documents.retained_limit,
        app.documents.logical_bytes(),
        start.elapsed().as_secs_f64() * 1000.
    );
}
