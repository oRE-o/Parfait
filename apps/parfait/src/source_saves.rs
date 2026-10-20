//! Asynchronous original saves. Completion always targets the captured session/binding.
use crate::{
    app::Parfait,
    document_session::{DocumentId, SessionKey},
};
use parfait_storage::{JobKey, SnapshotHead, SourceFailure, SourceRequest};
use std::{collections::HashMap, path::PathBuf};
pub(crate) struct PendingSource {
    pub key: JobKey,
    pub path: PathBuf,
}
pub(crate) struct OverwritePrompt {
    pub request: SourceRequest,
    pub observed: Option<SnapshotHead>,
}
#[derive(Default)]
pub(crate) struct SourceSaves {
    pub pending: HashMap<DocumentId, PendingSource>,
    pub prompts: Vec<OverwritePrompt>,
}
impl SourceSaves {
    pub fn reserved_pixels(&self) -> u64 {
        self.pending.len() as u64 * 512 * 1024 * 1024
    }
}
impl Parfait {
    pub(crate) fn queue_source_save(
        &mut self,
        target: SessionKey,
        path: PathBuf,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.documents.get_by_key(target).is_some(),
            "Document is no longer open"
        );
        anyhow::ensure!(
            !self.source_saves.pending.contains_key(&target.document_id)
                && !self.source_saves.prompts.iter().any(|p| p
                    .request
                    .key
                    .document_id
                    == target.document_id.bytes()),
            "Finish the pending original save first"
        );
        let path = crate::project_paths::resolved(&path)?;
        anyhow::ensure!(
            self.reserve_pixel_headroom(512 * 1024 * 1024),
            "Not enough pixel budget to inspect the original before saving"
        );
        let session = self.documents.get_by_key(target).unwrap();
        let same = session
            .project
            .as_ref()
            .is_some_and(|old| crate::project_paths::same_source(old, &path));
        let mut snapshot = session.document.clone();
        if let Some(name) = path.file_stem().and_then(|s| s.to_str())
            && snapshot.name != name
        {
            snapshot.name = name.to_owned();
            snapshot.touch();
        }
        anyhow::ensure!(
            session.binding_epoch < u64::MAX,
            "Original binding epoch exhausted"
        );
        let epoch = session.binding_epoch;
        let state = session.history.state_id();
        let expected = if same { session.source_head.clone() } else { None };
        let confirm = same && session.overwrite_required;
        let runtime = self.recovery_runtime.as_mut().unwrap();
        let number = runtime.next_request()?;
        let request = SourceRequest {
            key: JobKey {
                document_id: target.document_id.bytes(),
                session_epoch: target.epoch.bytes(),
                request_id: number,
            },
            binding_epoch: epoch,
            path,
            snapshot,
            edit_state_id: state,
            expected_head: expected.clone(),
        };
        if confirm {
            self.source_saves
                .prompts
                .push(OverwritePrompt { request, observed: expected });
            return Ok(());
        }
        self.submit_source_request(request)
    }
    fn submit_source_request(
        &mut self,
        request: SourceRequest,
    ) -> anyhow::Result<()> {
        let id = DocumentId::from_bytes(request.key.document_id);
        self.ensure_storage_worker()?;
        let runtime = self.recovery_runtime.as_mut().unwrap();
        anyhow::ensure!(
            runtime.worker.available(),
            "Storage worker unavailable; retry after recovery maintenance"
        );
        runtime.worker.submit_source(request.clone())?;
        self.source_saves
            .pending
            .insert(id, PendingSource { key: request.key, path: request.path });
        Ok(())
    }
    pub(crate) fn poll_source_saves(&mut self) {
        while let Some(event) =
            self.recovery_runtime.as_ref().and_then(|r| r.worker.poll_source())
        {
            let request = event.request;
            let id = DocumentId::from_bytes(request.key.document_id);
            if !self
                .source_saves
                .pending
                .get(&id)
                .is_some_and(|pending| pending.key == request.key)
            {
                continue;
            }
            self.source_saves.pending.remove(&id);
            let Some(session) = self.documents.get_mut(id).filter(|s| {
                s.key.epoch.bytes() == request.key.session_epoch
                    && s.binding_epoch == request.binding_epoch
            }) else {
                continue;
            };
            match event.result {
                Ok(saved) => {
                    let changed =
                        session.project.as_ref() != Some(&request.path);
                    if changed {
                        session.binding_epoch += 1;
                    }
                    session.project = Some(request.path.clone());
                    session.source_head = Some(saved.head.clone());
                    session.saved_edit_state = Some(request.edit_state_id);
                    session.saved_digest = Some(saved.digest);
                    session.overwrite_required = false;
                    session.project_saved_state =
                        Some(request.snapshot.clone());
                    if session.document.name != request.snapshot.name {
                        session.document.name = request.snapshot.name;
                        session.document.touch();
                    }
                    session.recovery.source_hint =
                        parfait_storage::NativePath::encode(&request.path).ok();
                    self.notice = Some(
                        saved
                            .post_commit_warning
                            .unwrap_or_else(|| "Original file saved.".into()),
                    );
                    let key = session.key;
                    if self
                        .recovery_runtime
                        .as_ref()
                        .is_some_and(|r| r.worker.available())
                    {
                        self.force_save_for(key);
                    } else {
                        let session =
                            self.documents.get_by_key_mut(key).unwrap();
                        session.recovery.failed = true;
                        session.recovery.submitted = 0;
                    }
                }
                Err(SourceFailure::Changed(observed)) => self
                    .source_saves
                    .prompts
                    .push(OverwritePrompt { request, observed }),
                Err(SourceFailure::Failed(error)) => {
                    self.error = Some(format!(
                        "Original save failed; recovery remains independent. {error}"
                    ))
                }
            }
        }
    }
    pub(crate) fn source_save_dialogs(&mut self, ctx: &eframe::egui::Context) {
        let Some(prompt) = self.source_saves.prompts.first() else {
            return;
        };
        let path = prompt.request.path.display().to_string();
        let mut action = 0;
        eframe::egui::Window::new("Original file changed").collapsible(false).resizable(false).show(ctx,|ui|{
            ui.label(&path);ui.label("The destination changed or already contains another drawing. Overwrite the inspected version, choose Save As, or cancel this save.");
            ui.horizontal(|ui| {
                if ui.button("Overwrite").clicked() {action=1;}
                if ui.button("Save As").clicked() {action=2;}
                if ui.button("Cancel").clicked() {action=3;}
            });
        });
        if action == 0 {
            return;
        }
        let mut prompt = self.source_saves.prompts.remove(0);
        if action == 3 {
            return;
        }
        let id = DocumentId::from_bytes(prompt.request.key.document_id);
        let Some(session) = self.documents.get(id).filter(|s| {
            s.key.epoch.bytes() == prompt.request.key.session_epoch
                && s.binding_epoch == prompt.request.binding_epoch
        }) else {
            return;
        };
        let target = session.key;
        if action == 2 {
            self.save_project_for(target, true);
            return;
        }
        prompt.request.expected_head = prompt.observed;
        match self.recovery_runtime.as_mut().unwrap().next_request() {
            Ok(number) => prompt.request.key.request_id = number,
            Err(error) => {
                self.error = Some(error.to_string());
                return;
            }
        }
        if !self.reserve_pixel_headroom(512 * 1024 * 1024) {
            self.notice =
                Some("Not enough pixel budget to retry this save.".into());
            return;
        }
        if let Err(error) = self.submit_source_request(prompt.request) {
            self.error = Some(error.to_string());
        }
    }
    #[cfg(test)]
    pub(crate) fn wait_source_saves(&mut self) -> anyhow::Result<()> {
        if let Some(runtime) = &self.recovery_runtime {
            runtime.worker.barrier();
            self.poll_source_saves();
        }
        anyhow::ensure!(
            self.source_saves.pending.is_empty()
                && self.source_saves.prompts.is_empty()
                && self.error.is_none(),
            "Original save needs attention"
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use parfait_core::Document;
    use parfait_storage::Store;
    fn app() -> (tempfile::TempDir, Parfait) {
        let dir = tempfile::tempdir().unwrap();
        let app = Parfait::open(dir.path().to_owned()).unwrap();
        (dir, app)
    }
    fn drain(app: &mut Parfait) {
        app.recovery_runtime.as_ref().unwrap().worker.barrier();
        app.poll_source_saves();
    }
    #[test]
    fn stage4_async_save_keeps_new_edits_other_tab_and_saved_undo_identity() {
        let (_dir, mut app) = app();
        let out = tempfile::tempdir().unwrap();
        let path = out.path().join("drawing.parfait");
        let a = app.documents.active_key().unwrap();
        app.edit_named("First", |d| {
            *d = Document::new(8, 8);
            d.layers[1].opacity = 0.5;
        });
        let saved = app.documents.get_by_key(a).unwrap().history.state_id();
        app.save_project_to_for(a, path.clone()).unwrap();
        assert!(app.source_saves.pending.contains_key(&a.document_id));
        app.edit_named("During original save", |d| d.layers[1].opacity = 0.25);
        let latest = app.documents.get_by_key(a).unwrap().history.state_id();
        let b = app.new_document_tab(Document::new(8, 8)).unwrap();
        drain(&mut app);
        assert_eq!(app.documents.active_id(), Some(b));
        let s = app.documents.get_by_key(a).unwrap();
        assert_eq!(s.document.layers[1].opacity, 0.25);
        assert_eq!(s.document.name, "drawing");
        assert_eq!(s.saved_edit_state, Some(saved));
        assert_eq!(s.history.state_id(), latest);
        assert!(s.modified());
        let disk = Store::open_read_only(&path)
            .unwrap()
            .load()
            .unwrap()
            .unwrap()
            .document;
        assert_eq!(disk.layers[1].opacity, 0.5);
        assert!(app.activate_document(a.document_id, true));
        app.undo();
        assert!(!app.documents.get_by_key(a).unwrap().modified());
        app.redo();
        assert!(app.documents.get_by_key(a).unwrap().modified());
        assert_eq!(
            app.documents.get_by_key(a).unwrap().document.name,
            "drawing"
        );
    }
    #[test]
    fn stage4_external_change_needs_approval_and_is_rechecked_after_approval() {
        let (_dir, mut app) = app();
        let out = tempfile::tempdir().unwrap();
        let path = out.path().join("a.parfait");
        app.edit_named("First", |d| *d = Document::new(8, 8));
        app.save_project_to(path.clone()).unwrap();
        let key = app.documents.active_key().unwrap();
        let mut external = Document::new(8, 8);
        external.layers[1].opacity = 0.5;
        Parfait::write_project_document(&external, &path).unwrap();
        let before = std::fs::read(&path).unwrap();
        app.save_project_to_for(key, path.clone()).unwrap();
        drain(&mut app);
        assert_eq!(app.source_saves.prompts.len(), 1);
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert!(!app.close_document_tab(key.document_id));
        let mut approved = app.source_saves.prompts.remove(0);
        approved.request.expected_head = approved.observed;
        external.layers[1].opacity = 0.25;
        Parfait::write_project_document(&external, &path).unwrap();
        let changed = std::fs::read(&path).unwrap();
        approved.request.key.request_id =
            app.recovery_runtime.as_mut().unwrap().next_request().unwrap();
        app.submit_source_request(approved.request).unwrap();
        drain(&mut app);
        assert_eq!(app.source_saves.prompts.len(), 1);
        assert_eq!(std::fs::read(&path).unwrap(), changed);
        let mut approved = app.source_saves.prompts.remove(0);
        approved.request.expected_head = approved.observed;
        approved.request.key.request_id =
            app.recovery_runtime.as_mut().unwrap().next_request().unwrap();
        app.submit_source_request(approved.request).unwrap();
        drain(&mut app);
        assert!(app.source_saves.prompts.is_empty());
        assert!(!app.documents.active_session.modified());
    }
    #[test]
    fn stage4_failed_original_save_preserves_name_binding_history_and_future_file()
     {
        let (_dir, mut app) = app();
        let out = tempfile::tempdir().unwrap();
        let path = out.path().join("future.parfait");
        std::fs::write(&path, b"not a supported original").unwrap();
        app.edit_named("First", |d| *d = Document::new(8, 8));
        app.edit_named("Second", |d| d.layers[1].opacity = 0.5);
        app.undo();
        let before = app.documents.active_session.document.clone();
        let state = app.documents.active_session.history.state_id();
        let key = app.documents.active_key().unwrap();
        app.save_project_to_for(key, path.clone()).unwrap();
        drain(&mut app);
        assert!(app.error.is_some());
        assert_eq!(app.documents.active_session.document, before);
        assert!(app.documents.active_session.project.is_none());
        assert_eq!(app.documents.active_session.history.state_id(), state);
        assert!(app.documents.active_session.history.can_redo());
        assert_eq!(std::fs::read(path).unwrap(), b"not a supported original");
    }
    #[test]
    fn stage4_abnormal_and_clean_unsaved_restart_keep_recovery_and_valid_source_binding()
     {
        for clean in [false, true] {
            let (dir, mut app) = app();
            let out = tempfile::tempdir().unwrap();
            let path = out.path().join("a.parfait");
            app.edit_named("First", |d| {
                *d = Document::new(8, 8);
                d.layers[1].opacity = 0.5;
            });
            app.save_project_to(path.clone()).unwrap();
            let id = app.documents.active_id().unwrap();
            app.edit_named("Unsaved", |d| d.layers[1].opacity = 0.25);
            app.flush_document_recovery(clean).unwrap();
            drop(app);
            let app = Parfait::open(dir.path().to_owned()).unwrap();
            let s = app.documents.get(id).unwrap();
            assert_eq!(s.document.layers[1].opacity, 0.25);
            assert_eq!(s.project.as_ref(), Some(&path.canonicalize().unwrap()));
            assert!(s.modified());
            assert!(!s.overwrite_required);
        }
    }
    #[test]
    fn stage4_clean_saved_restart_reads_changed_original_but_missing_original_keeps_recovery()
     {
        let (dir, mut app) = app();
        let out = tempfile::tempdir().unwrap();
        let path = out.path().join("a.parfait");
        app.edit_named("First", |d| {
            *d = Document::new(8, 8);
            d.layers[1].opacity = 0.5;
        });
        app.save_project_to(path.clone()).unwrap();
        let id = app.documents.active_id().unwrap();
        app.flush_document_recovery(true).unwrap();
        drop(app);
        let mut external = Document::new(8, 8);
        external.layers[1].opacity = 0.8;
        Parfait::write_project_document(&external, &path).unwrap();
        let mut app = Parfait::open(dir.path().to_owned()).unwrap();
        assert_eq!(
            app.documents.get(id).unwrap().document.layers[1].opacity,
            0.8
        );
        assert!(!app.documents.get(id).unwrap().modified());
        app.flush_document_recovery(false).unwrap();
        drop(app);
        std::fs::remove_file(&path).unwrap();
        let app = Parfait::open(dir.path().to_owned()).unwrap();
        assert_eq!(
            app.documents.get(id).unwrap().document.layers[1].opacity,
            0.8
        );
        assert!(app.documents.get(id).unwrap().project.is_none());
        assert!(app.documents.get(id).unwrap().modified());
    }
    #[test]
    fn stage4_default_launch_is_multi_document_and_shutdown_restores_unsaved_drawings()
     {
        let (dir, mut app) = app();
        assert!(app.tabs.enabled);
        assert!(app.recovery_runtime.is_some());
        app.new_document();
        assert_eq!(app.documents.tab_order.len(), 2);
        app.edit_named("Draw", |d| d.layers[1].opacity = 0.42);
        let id = app.documents.active_id().unwrap();
        eframe::App::on_exit(&mut app, None);
        drop(app);
        let app = Parfait::open(dir.path().to_owned()).unwrap();
        assert_eq!(app.documents.tab_order, [id]);
        assert_eq!(
            app.documents.get(id).unwrap().document.layers[1].opacity,
            0.42
        );
        assert!(!dir.path().join("recovery.parfait").exists());
    }
}

#[cfg(test)]
mod close_tests {
    use super::*;
    #[test]
    fn stage4_cleanup_failure_keeps_closed_tombstones_and_bounds_the_backlog() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open(dir.path().to_owned()).unwrap();
        let id = app.documents.active_id().unwrap();
        for n in 1..=32u8 {
            let runtime = app.recovery_runtime.as_mut().unwrap();
            runtime.store.register([n; 16], None).unwrap();
            runtime.store.close_document([n; 16]).unwrap();
            let path = parfait_storage::recovery_path(dir.path(), [n; 16]);
            std::fs::create_dir_all(path.parent().unwrap().parent().unwrap())
                .unwrap();
            std::fs::write(path.parent().unwrap(), b"broken directory entry")
                .unwrap();
        }
        assert!(!app.close_document_tab(id));
        assert!(app.documents.get(id).is_some());
        for n in 1..=32u8 {
            std::fs::remove_file(
                parfait_storage::recovery_path(dir.path(), [n; 16])
                    .parent()
                    .unwrap(),
            )
            .unwrap();
        }
        assert!(app.close_document_tab(id));
        assert!(app.documents.is_empty());
        assert!(
            app.recovery_runtime
                .as_ref()
                .unwrap()
                .store
                .records()
                .unwrap()
                .iter()
                .all(|r| r.lifecycle == 1)
        );
    }
}

#[cfg(test)]
mod duplicate_binding_tests {
    use super::*;
    use parfait_core::{Document, EditStateId};
    use parfait_storage::{NativePath, RecoveryMetadata, Store};
    #[test]
    fn stage4_duplicate_source_on_clean_restart_does_not_replace_the_second_recovered_artwork()
     {
        let dir = tempfile::tempdir().unwrap();
        let out = tempfile::tempdir().unwrap();
        let path = out.path().join("original.parfait");
        let mut app = Parfait::open(dir.path().to_owned()).unwrap();
        app.edit_named("A", |d| {
            *d = Document::new(8, 8);
            d.layers[1].opacity = 0.5;
        });
        app.save_project_to(path.clone()).unwrap();
        let a = app.documents.active_id().unwrap();
        app.flush_document_recovery(true).unwrap();
        let b = [99; 16];
        let mut document = Document::new(8, 8);
        document.layers[1].opacity = 0.25;
        let state = EditStateId::try_new().unwrap().bytes();
        let hash = document.art_digest_v1().unwrap().0;
        let source = NativePath::encode(&path.canonicalize().unwrap()).unwrap();
        let metadata = RecoveryMetadata {
            document_id: b,
            edit_state_id: state,
            logical_digest_version: 1,
            logical_digest: hash,
            source_path: Some(source.clone()),
            source_saved_state_id: Some(state),
            source_saved_digest: Some(hash),
            source_head: app.documents.get(a).unwrap().source_head.clone(),
        };
        let runtime = app.recovery_runtime.as_mut().unwrap();
        runtime.store.register(b, Some(source)).unwrap();
        Store::open(&parfait_storage::recovery_path(dir.path(), b))
            .unwrap()
            .save_recovery(&document, &metadata)
            .unwrap();
        runtime.store.layout(&[a.bytes(), b], Some(b), true).unwrap();
        drop(app);
        let app = Parfait::open(dir.path().to_owned()).unwrap();
        let b = app.documents.get(DocumentId::from_bytes(b)).unwrap();
        assert_eq!(b.document.layers[1].opacity, 0.25);
        assert!(b.project.is_none());
        assert!(b.modified());
        assert!(app.documents.get(a).unwrap().project.is_some());
    }
}

#[cfg(all(test, unix))]
mod internal_alias_tests {
    use super::*;
    #[test]
    fn stage4_original_target_cannot_alias_a_document_recovery_inode() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open(dir.path().to_owned()).unwrap();
        app.edit_named("First", |d| d.layers[1].opacity = 0.5);
        app.flush_document_recovery(false).unwrap();
        let id = app.documents.active_id().unwrap();
        let internal = parfait_storage::recovery_path(dir.path(), id.bytes());
        let alias = dir.path().join("looks-like-original.parfait");
        std::fs::hard_link(&internal, &alias).unwrap();
        assert!(
            app.save_project_to_for(app.documents.active_key().unwrap(), alias)
                .is_err()
        );
    }
}

#[cfg(test)]
mod reopened_header_tests {
    use super::*;
    use parfait_core::Document;
    use parfait_storage::Store;
    #[test]
    fn stage4_normal_saved_restart_uses_original_header_even_when_art_digest_is_unchanged()
     {
        let dir = tempfile::tempdir().unwrap();
        let out = tempfile::tempdir().unwrap();
        let path = out.path().join("a.parfait");
        let mut app = Parfait::open(dir.path().to_owned()).unwrap();
        app.edit_named("Fixture", |d| *d = Document::new(8, 8));
        app.save_project_to(path.clone()).unwrap();
        let state = app.documents.active_session.history.state_id();
        app.flush_document_recovery(true).unwrap();
        drop(app);
        let mut original = Store::open_read_only(&path)
            .unwrap()
            .load()
            .unwrap()
            .unwrap()
            .document;
        original.active = 0;
        original.name = "External metadata".into();
        Parfait::write_project_document(&original, &path).unwrap();
        let app = Parfait::open(dir.path().to_owned()).unwrap();
        assert_eq!(app.documents.active_session.document.active, 0);
        assert_eq!(
            app.documents.active_session.document.name,
            "External metadata"
        );
        assert_eq!(app.documents.active_session.history.state_id(), state);
        assert!(!app.documents.active_session.modified());
    }
}
