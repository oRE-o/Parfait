//! Explicit target routing, including a tab switch between capture and execution.
use crate::{app::Parfait, document_session::DocumentSession};
use parfait_core::{Document, Point};
use parfait_storage::Store;
use std::sync::Arc;

fn fixture() -> (tempfile::TempDir, Parfait) {
    let dir = tempfile::tempdir().unwrap();
    let mut app = Parfait::open_legacy(dir.path().to_owned()).unwrap();
    app.documents.active_session.document = Document::new(8, 8);
    app.enable_document_tabs().unwrap();
    (dir, app)
}

#[test]
fn multi_document_captured_target_edit_and_history_do_not_follow_active_tab() {
    let (_dir, mut app) = fixture();
    let a = app.documents.active_key().unwrap();
    let a_state = app.documents.get_by_key(a).unwrap().history.state_id();
    let b_id = app.new_document_tab(Document::new(8, 8)).unwrap();
    let b = app.documents.active_key().unwrap();
    let b_before = app.documents.get_by_key(b).unwrap().document.clone();
    let b_state = app.documents.get_by_key(b).unwrap().history.state_id();
    app.edit_named_for(a, "Captured A", |document| {
        Arc::make_mut(&mut document.layers[1].pixels)[0..4]
            .copy_from_slice(&[25, 50, 75, 255]);
    });
    let session = app.documents.get_by_key(a).unwrap();
    assert_ne!(session.history.state_id(), a_state);
    assert_eq!(&session.document.layers[1].pixels[..4], &[25, 50, 75, 255]);
    assert_eq!(session.edit_reservation, 0);
    assert!(
        app.navigate_history(a, |history, document| history.undo(document))
    );
    assert_eq!(
        app.documents.get_by_key(a).unwrap().history.state_id(),
        a_state
    );
    assert!(
        app.navigate_history(a, |history, document| history.redo(document))
    );
    assert_eq!(app.documents.active_id(), Some(b_id));
    let session = app.documents.get_by_key(b).unwrap();
    assert_eq!(session.document, b_before);
    assert_eq!(session.history.state_id(), b_state);
    assert!(!session.history.can_undo());
}

#[test]
fn multi_document_captured_stroke_finish_and_cancel_only_touch_the_owner() {
    let (_dir, mut app) = fixture();
    let a = app.documents.active_key().unwrap();
    let b_id = app.new_document_tab(Document::new(8, 8)).unwrap();
    let b = app.documents.active_key().unwrap();
    let b_before = app.documents.get_by_key(b).unwrap().document.clone();
    let b_state = app.documents.get_by_key(b).unwrap().history.state_id();
    app.activate_document(a.document_id, true);
    assert!(app.begin_canvas_stroke(Point { x: 4., y: 4., pressure: 1. }));
    // Bypass the UI's normal settling to model a delayed owner-targeted finish.
    assert!(app.documents.activate(b_id));
    assert!(app.finish_stroke_for(a));
    let committed = app.documents.get_by_key(a).unwrap().document.clone();
    let committed_state =
        app.documents.get_by_key(a).unwrap().history.state_id();
    assert!(app.documents.get_by_key(a).unwrap().history.can_undo());
    assert!(app.documents.activate(a.document_id));
    app.brush.color = [200, 50, 80];
    assert!(app.begin_canvas_stroke(Point { x: 4., y: 4., pressure: 1. }));
    assert!(app.documents.activate(b_id));
    app.cancel_edit_for(a);
    let session = app.documents.get_by_key(a).unwrap();
    assert_eq!(session.document.layers, committed.layers);
    assert_eq!(session.history.state_id(), committed_state);
    assert!(session.editing.stroke.is_none());
    assert_eq!(session.edit_reservation, 0);
    let session = app.documents.get_by_key(b).unwrap();
    assert_eq!(session.document, b_before);
    assert_eq!(session.history.state_id(), b_state);
    assert!(session.editing.stroke.is_none());
    assert_eq!(app.documents.active_key(), Some(b));
}

#[test]
fn multi_document_captured_save_uses_owner_binding_and_leaves_active_tab_unchanged()
 {
    let (dir, mut app) = fixture();
    let a = app.documents.active_key().unwrap();
    let a_path = dir.path().join("a.parfait");
    app.save_project_to_for(a, a_path.clone()).unwrap();
    app.wait_source_saves().unwrap();
    app.new_document_tab(Document::new(8, 8)).unwrap();
    let b = app.documents.active_key().unwrap();
    let b_path = dir.path().join("b.parfait");
    app.save_project_to_for(b, b_path.clone()).unwrap();
    app.wait_source_saves().unwrap();
    app.edit_named_for(a, "Edit A", |document| {
        Arc::make_mut(&mut document.layers[1].pixels)[0..4]
            .copy_from_slice(&[11, 22, 33, 255]);
    });
    let b_before = app.documents.get_by_key(b).unwrap().document.clone();
    let b_state = app.documents.get_by_key(b).unwrap().history.state_id();
    app.save_project_to_for(a, a_path.clone()).unwrap();
    app.wait_source_saves().unwrap();
    let stored = Store::open_read_only(&a_path)
        .unwrap()
        .load()
        .unwrap()
        .unwrap()
        .document;
    let session = app.documents.get_by_key(a).unwrap();
    assert_eq!(stored.layers, session.document.layers);
    assert_eq!(stored.name, "a");
    assert_eq!(session.project.as_ref(), Some(&a_path.canonicalize().unwrap()));
    assert!(!session.modified());
    assert!(app.save_project_to_for(a, b_path).is_err());
    let session = app.documents.get_by_key(b).unwrap();
    assert_eq!(session.document, b_before);
    assert_eq!(session.history.state_id(), b_state);
    assert_eq!(app.documents.active_key(), Some(b));
    assert!(app.tabs.pinned_save.is_none());
}

#[test]
fn multi_document_stale_or_removed_target_never_falls_back_to_the_active_session()
 {
    let (dir, mut app) = fixture();
    let old = app.documents.active_key().unwrap();
    let mut replacement = DocumentSession::new(Document::new(8, 8)).unwrap();
    replacement.key.document_id = old.document_id;
    let current = replacement.key;
    app.documents.replace_single(replacement);
    let before = app.documents.get_by_key(current).unwrap().document.clone();
    let state = app.documents.get_by_key(current).unwrap().history.state_id();
    assert!(app.documents.get_by_key(old).is_none());
    assert!(app.documents.get_by_key_mut(old).is_none());
    app.edit_named_for(old, "Stale", |_| panic!("stale edit ran"));
    assert!(!app.begin_history_for(old, "Stale"));
    assert!(!app.navigate_history(old, |_, _| panic!("stale history ran")));
    assert!(!app.finish_stroke_for(old));
    app.cancel_edit_for(old);
    app.finish_layer_edit_for(old);
    let path = dir.path().join("stale.parfait");
    assert!(app.save_project_to_for(old, path.clone()).is_err());
    assert!(!path.exists());
    let session = app.documents.get_by_key(current).unwrap();
    assert_eq!(session.document, before);
    assert_eq!(session.history.state_id(), state);
    app.new_document_tab(Document::new(8, 8)).unwrap();
    assert!(app.documents.remove(current.document_id));
    let active = app.documents.active_key().unwrap();
    let before = app.documents.get_by_key(active).unwrap().document.clone();
    app.edit_named_for(current, "Removed", |_| panic!("removed edit ran"));
    assert!(app.save_project_to_for(current, path.clone()).is_err());
    assert!(!path.exists());
    assert_eq!(app.documents.get_by_key(active).unwrap().document, before);
}
