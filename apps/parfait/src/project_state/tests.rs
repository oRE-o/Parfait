use super::*;
use eframe::egui;
use parfait_core::{LayerMask, MaskCoverage, RasterConversion};
use std::sync::Arc;

fn fixture(directory: &Path) -> Parfait {
    let mut app = Parfait::open(directory.to_owned()).unwrap();
    app.doc = Document::new(32, 32);
    app
}

#[test]
fn recovery_completion_does_not_mark_a_new_or_edited_project_saved() {
    let directory = tempfile::tempdir().unwrap();
    let mut app = fixture(directory.path());
    assert_eq!(app.project_status(), "파일 저장 필요");
    app.saved = app.doc.revision;
    assert_eq!(app.recovery_status(), "복구 저장됨");
    assert!(app.project_is_modified());
    app.save_project_to(directory.path().join("drawing.parfait"))
        .unwrap();
    assert_eq!(app.project_status(), "파일 저장됨");
    app.edit(|document| document.layers[1].opacity = 0.5);
    assert_eq!(app.project_status(), "파일 수정됨");
    app.saved = app.doc.revision - 1;
    assert_eq!(app.recovery_status(), "복구 저장 중");
    app.saved = app.doc.revision;
    assert_eq!(app.recovery_status(), "복구 저장됨");
    assert!(app.project_is_modified());
    app.saving_failed = true;
    assert_eq!(app.recovery_status(), "복구 저장 실패");
    assert_eq!(app.project_status(), "파일 수정됨");
    app.undo();
    assert_eq!(app.project_status(), "파일 저장됨");
    app.redo();
    assert_eq!(app.project_status(), "파일 수정됨");
}

#[test]
fn save_as_commits_candidate_name_path_and_shared_baseline_only_after_success() {
    let directory = tempfile::tempdir().unwrap();
    let mut app = fixture(directory.path());
    let path = directory.path().join("첫 그림.parfait");
    let before_pixels = app.doc.layers[1].pixels.clone();
    app.save_project_to(path.clone()).unwrap();
    assert_eq!(app.doc.name, "첫 그림");
    assert_eq!(app.project.as_ref(), Some(&path));
    assert!(!app.project_is_modified());
    let baseline = app.project_saved_state.as_ref().unwrap();
    assert!(Arc::ptr_eq(&app.doc.layers[1].pixels, &before_pixels));
    assert!(Arc::ptr_eq(&baseline.layers[1].pixels, &before_pixels));
    let store = Store::open(&path).unwrap();
    let loaded = store.load().unwrap().unwrap().document;
    let mut expected = app.doc.clone();
    expected.revision = loaded.revision;
    assert_eq!(loaded, expected);
    assert!(!store.unclean().unwrap());
    let saved = app.doc.clone();
    let invalid = directory.path().join("cannot-save-to-directory");
    std::fs::create_dir(&invalid).unwrap();
    assert!(app.save_project_to(invalid).is_err());
    assert_eq!(app.doc, saved);
    assert_eq!(app.project.as_ref(), Some(&path));
    assert!(!app.project_is_modified());
    assert!(Arc::ptr_eq(
        &app.project_saved_state.as_ref().unwrap().layers[1].pixels,
        &before_pixels
    ));
}

#[test]
fn failed_save_and_open_keep_dirty_state_future_and_existing_file() {
    let directory = tempfile::tempdir().unwrap();
    let mut app = fixture(directory.path());
    let path = directory.path().join("drawing.parfait");
    app.save_project_to(path.clone()).unwrap();
    let file_before = std::fs::read(&path).unwrap();
    app.edit(|document| document.layers[1].opacity = 0.4);
    app.edit(|document| document.layers[1].opacity = 0.8);
    app.undo();
    let before = app.doc.clone();
    let rows = app.history.rows().count();
    let invalid = directory.path().join("missing/renamed.parfait");
    assert!(app.save_project_to(invalid).is_err());
    let corrupt = directory.path().join("corrupt.parfait");
    std::fs::write(&corrupt, b"not a database").unwrap();
    assert!(app.open_project_from(corrupt).is_err());
    assert_eq!(app.doc, before);
    assert_eq!(app.project.as_ref(), Some(&path));
    assert!(app.project_is_modified());
    assert_eq!(app.history.rows().count(), rows);
    assert!(app.history.can_redo());
    assert_eq!(std::fs::read(path).unwrap(), file_before);
}

#[test]
fn explicit_open_sets_clean_baseline_and_new_resets_only_after_backup() {
    let directory = tempfile::tempdir().unwrap();
    let mut app = fixture(directory.path());
    let path = directory.path().join("drawing.parfait");
    app.doc.create_layer_mask(1, 100);
    app.save_project_to(path.clone()).unwrap();
    let mut saved = app.doc.clone();
    app.edit(|document| document.layers[1].name = "changed".into());
    app.open_project_from(path).unwrap();
    saved.revision = app.doc.revision;
    assert_eq!(app.doc, saved);
    assert!(!app.project_is_modified());
    let before = app.doc.clone();
    let blocked_backup = directory.path().join("before-new.parfait");
    std::fs::create_dir(&blocked_backup).unwrap();
    app.new_document();
    assert!(app.error.is_some());
    assert_eq!(app.doc, before);
    assert!(!app.project_is_modified());
    std::fs::remove_dir(blocked_backup).unwrap();
    app.error = None;
    app.new_document();
    assert!(app.project.is_none() && app.project_saved_state.is_none());
    assert_eq!(app.project_status(), "파일 저장 필요");
}

#[test]
fn marker_ignores_selection_revision_and_view_but_tracks_content_and_masks() {
    let directory = tempfile::tempdir().unwrap();
    let mut app = fixture(directory.path());
    app.doc.layers[1].pixels = Arc::new([20, 40, 60, 180].repeat(1024));
    app.doc.layers[1].mask = Some(LayerMask {
        enabled: true,
        coverage: MaskCoverage::Dense(Arc::new(vec![127; 1024])),
    });
    app.save_project_to(directory.path().join("drawing.parfait"))
        .unwrap();
    app.select_layer(0);
    app.doc.next_layer_id += 1;
    app.doc.touch();
    app.view.zoom = 0.3;
    app.navigation_tool = 2;
    app.brush.color = [70, 80, 90];
    app.mask_overlay = Some(app.doc.layers[1].id);
    assert!(!app.project_is_modified());
    app.select_layer(1);
    app.edit(|document| {
        document.convert_layer_pixels(1, RasterConversion::DrawingColor([70, 80, 90]));
    });
    assert!(app.project_is_modified());
    app.undo();
    assert!(!app.project_is_modified());
    app.edit(|document| {
        document.set_layer_mask_enabled(1, false);
    });
    assert!(app.project_is_modified());
    app.undo();
    assert!(!app.project_is_modified());
    let before = app.doc.clone();
    for _ in 0..1000 {
        assert!(!app.project_is_modified());
    }
    assert_eq!(app.doc, before);
    assert!(Arc::ptr_eq(
        &app.doc.layers[1].pixels,
        &before.layers[1].pixels
    ));
}

#[test]
fn unchanged_save_preserves_redo_and_recovery_reopen_requires_explicit_file_choice() {
    let directory = tempfile::tempdir().unwrap();
    let mut app = fixture(directory.path());
    let path = directory.path().join("drawing.parfait");
    app.save_project_to(path.clone()).unwrap();
    app.edit(|document| document.layers[1].opacity = 0.5);
    app.undo();
    let before = app.doc.clone();
    app.save_project_to(path.clone()).unwrap();
    assert!(app.history.can_redo());
    assert_eq!(app.doc, before);
    assert!(!app.project_is_modified());
    app.force_save();
    drop(app);
    let mut app = Parfait::open(directory.path().to_owned()).unwrap();
    assert_eq!(app.doc, before);
    assert_eq!(app.project_status(), "파일 저장 필요");
    app.open_project_from(path).unwrap();
    assert!(!app.project_is_modified());
}

#[test]
fn working_file_targets_and_symbolic_aliases_are_refused() {
    let directory = tempfile::tempdir().unwrap();
    let mut app = fixture(directory.path());
    let before = app.doc.clone();
    for name in [
        "recovery.parfait",
        "before-open.parfait",
        "before-new.parfait",
        "preferences.sqlite",
        "session.lock",
    ] {
        assert!(app.save_project_to(directory.path().join(name)).is_err());
        assert!(app.open_project_from(directory.path().join(name)).is_err());
        assert_eq!(app.doc, before);
        for suffix in ["-wal", "-shm", "-journal"] {
            let sidecar = directory.path().join(format!("{name}{suffix}"));
            assert!(app.save_project_to(sidecar.clone()).is_err());
            assert!(app.open_project_from(sidecar).is_err());
            assert_eq!(app.doc, before);
        }
    }
    #[cfg(unix)]
    {
        let alias = directory.path().join("alias.parfait");
        std::os::unix::fs::symlink(&app.recovery_path, &alias).unwrap();
        assert!(app.save_project_to(alias.clone()).is_err());
        assert!(app.open_project_from(alias).is_err());
        let broken_alias = directory.path().join("broken-alias.parfait");
        std::os::unix::fs::symlink(directory.path().join("missing.parfait"), &broken_alias)
            .unwrap();
        assert!(app.save_project_to(broken_alias.clone()).is_err());
        assert!(app.open_project_from(broken_alias).is_err());
        assert!(!directory.path().join("missing.parfait").exists());
        assert_eq!(app.doc, before);
    }
}

fn visible_text(shape: &egui::Shape, text: &mut Vec<String>) {
    match shape {
        egui::Shape::Text(shape) => text.push(shape.galley.text().to_owned()),
        egui::Shape::Vec(shapes) => {
            for shape in shapes {
                visible_text(shape, text);
            }
        }
        _ => {}
    }
}

#[test]
fn labels_and_unsaved_star_fit_the_minimum_and_normal_window_widths() {
    let directory = tempfile::tempdir().unwrap();
    let mut app = fixture(directory.path());
    let context = egui::Context::default();
    crate::theme::apply(&context);
    for width in [960., 1440.] {
        for saved in [false, true] {
            if saved {
                app.save_project_to(directory.path().join("drawing.parfait"))
                    .unwrap();
            } else {
                app.project = None;
                app.project_saved_state = None;
            }
            app.saved = app.doc.revision;
            for _ in 0..2 {
                let mut output = context.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(width, 640.),
                        )),
                        ..Default::default()
                    },
                    |ui| {
                        app.title_bar(ui);
                        app.top_panel(ui);
                    },
                );
                let mut text = Vec::new();
                for shape in &output.shapes {
                    visible_text(&shape.shape, &mut text);
                }
                output.textures_delta.clear();
                assert!(text.iter().any(|text| text == "복구 저장됨"));
                assert!(text.iter().any(|text| text == app.project_status()));
                let caption = format!("{}{}", app.doc.name, if saved { "" } else { " *" });
                assert!(text.iter().any(|text| text == &caption));
            }
            let recovery: egui::Rect = context
                .data(|data| data.get_temp(egui::Id::new("menu_save_status")))
                .unwrap();
            let project: egui::Rect = context
                .data(|data| data.get_temp(egui::Id::new("project_save_status")))
                .unwrap();
            let help: egui::Rect = context
                .data(|data| {
                    data.get_temp(egui::Id::new(("top_menu", crate::menus::TopMenu::Help)))
                })
                .unwrap();
            assert!(help.right() <= project.left());
            assert!(project.right() <= recovery.left());
            assert!(recovery.right() <= width);
        }
    }
}
