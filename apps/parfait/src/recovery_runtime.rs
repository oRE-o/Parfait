//! Stage-3 durability integration. Original binding validation remains stage 4.
use crate::{
    app::Parfait,
    document_session::{DocumentId, DocumentSession, SessionKey},
    documents::DocumentManager,
};
use parfait_core::{Document, EditStateId, History};
use parfait_storage::{
    JobKey, NativePath, RecoveryRequest, SessionRecord, SessionStore,
    StorageCoordinator, Store,
};
use std::time::Instant;

pub(crate) struct RestoreItem {
    pub record: SessionRecord,
    pub error: String,
}
type SessionLayout = (Vec<[u8; 16]>, Option<[u8; 16]>);
pub(crate) struct RecoveryRuntime {
    pub store: SessionStore,
    pub worker: StorageCoordinator,
    pub pending_restore: Vec<RestoreItem>,
    pub shutdown_ready: bool,
    next_request_id: u64,
    cleanup_needed: bool,
    cleanup_tick: Instant,
    last_layout: Option<SessionLayout>,
}
impl RecoveryRuntime {
    pub fn next_request(&mut self) -> anyhow::Result<u64> {
        self.next_request_id = self
            .next_request_id
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("Storage request limit reached"))?;
        Ok(self.next_request_id)
    }
}
impl Parfait {
    pub(crate) fn start_document_recovery(&mut self) -> anyhow::Result<()> {
        let dir = self.recovery_path.parent().unwrap().to_owned();
        let mut store = SessionStore::open(&dir.join("session.sqlite"))?;
        let was_clean = store.clean_shutdown()?;
        let desired = store.active()?;
        let mut messages = Vec::new();
        if let Err(error) = parfait_storage::migrate_legacy(&dir, &mut store) {
            messages.push(format!(
                "Legacy recovery preserved; migration failed: {error:#}"
            ));
        }
        let records = store.records()?;
        let desired_order = desired.and_then(|id| {
            records.iter().find(|r| r.document_id == id).map(|r| r.order)
        });
        let mut restored_order = Vec::new();
        let mut documents = DocumentManager::new(Document::new(1, 1))?;
        documents.remove(documents.active_id().unwrap());
        let mut pending_restore = Vec::new();
        for record in records.into_iter().filter(|record| record.lifecycle == 0)
        {
            if documents.tab_order.len() >= documents.max_tabs {
                pending_restore.push(RestoreItem {
                    record,
                    error: "Tab limit reached; preserved for later restoration"
                        .into(),
                });
                continue;
            }
            match restore_session(&dir, &record, was_clean) {
                Ok(mut session) => {
                    if let Err(error) =
                        documents.check_admission(&session.document, 0)
                    {
                        pending_restore.push(RestoreItem {
                            record,
                            error: error.to_string(),
                        });
                        continue;
                    }
                    let duplicate_hint = session
                        .recovery
                        .metadata
                        .as_ref()
                        .and_then(|m| m.source_path.as_ref())
                        .and_then(|p| p.decode().ok())
                        .is_some_and(|path| {
                            documents.sessions().any(|other| {
                                other.project.as_ref().is_some_and(|other| {
                                    crate::project_paths::same_source(
                                        other, &path,
                                    )
                                })
                            })
                        });
                    if duplicate_hint {
                        messages.push("Duplicate original left unbound; recovered artwork is unchanged.".into());
                    }
                    if !duplicate_hint
                        && let Err(error) = validate_restored_source(
                            &dir,
                            &mut session,
                            was_clean,
                        )
                    {
                        messages.push(format!("Original binding unavailable; recovery retained: {error:#}"));
                    }
                    if session.project.as_ref().is_some_and(|path| {
                        documents.sessions().any(|other| {
                            other.project.as_ref().is_some_and(|other| {
                                crate::project_paths::same_source(other, path)
                            })
                        })
                    }) {
                        session.project = None;
                        session.project_saved_state = None;
                        session.saved_edit_state = None;
                        session.saved_digest = None;
                        session.source_head = None;
                        session.binding_epoch += 1;
                        messages.push("Duplicate original binding left unbound; both drawings are retained.".into());
                    }
                    let id = session.key.document_id;
                    match documents.insert(session, documents.tab_order.len()) {
                        Ok(inserted) => {
                            anyhow::ensure!(
                                inserted == id,
                                "Restoration identity collision"
                            );
                            restored_order.push((record.order, id));
                        }
                        Err(error) => pending_restore.push(RestoreItem {
                            record,
                            error: error.to_string(),
                        }),
                    }
                }
                Err(error) => pending_restore
                    .push(RestoreItem { record, error: format!("{error:#}") }),
            }
        }
        if let Some(desired) = desired
            && !documents.activate(DocumentId::from_bytes(desired))
            && let Some((_, id)) =
                restored_order.iter().rev().find(|(order, _)| {
                    desired_order.is_some_and(|desired| *order < desired)
                })
        {
            documents.activate(*id);
        }
        if documents.is_empty() && pending_restore.is_empty() {
            let mut session = DocumentSession::new(Document::new(1024, 768))?;
            session.untouched_new = true;
            documents.insert(session, 0)?;
        }
        // Never enable editing against an unreadable/future session registry.
        store.begin_run()?;
        let worker = StorageCoordinator::start(dir)?;
        self.documents = documents;
        self.render_cache = Default::default();
        self.recovery_runtime = Some(RecoveryRuntime {
            store,
            worker,
            pending_restore,
            shutdown_ready: false,
            next_request_id: 0,
            cleanup_needed: true,
            cleanup_tick: Instant::now(),
            last_layout: None,
        });
        if !was_clean {
            messages.push("Recovered the last committed drawings after an unexpected exit.".into());
        }
        if !messages.is_empty() {
            messages.push("All readable drawings are retained; original and recovery save status are independent.".into());
        }
        self.notice = (!messages.is_empty()).then(|| messages.join("\n"));
        self.cleanup_closed_recoveries();
        Ok(())
    }
    pub(crate) fn ensure_storage_worker(&mut self) -> anyhow::Result<()> {
        if self.recovery_runtime.as_ref().is_some_and(|r| !r.worker.available())
        {
            // Drain every old owner's result before replacing its terminal queue.
            self.poll_document_recovery();
            self.poll_source_saves();
            let worker = StorageCoordinator::start(
                self.recovery_path.parent().unwrap().to_owned(),
            )?;
            self.recovery_runtime.as_mut().unwrap().worker = worker;
        }
        Ok(())
    }
    pub(crate) fn submit_document_recovery(&mut self, target: SessionKey) {
        if let Err(error) = self.ensure_storage_worker() {
            if let Some(session) = self.documents.get_by_key_mut(target) {
                session.recovery.failed = true;
                session.recovery.last_submit = Instant::now();
            }
            self.notice =
                Some(format!("Storage worker could not restart: {error:#}"));
            return;
        }
        let Some(session) = self.documents.get_by_key(target) else {
            return;
        };
        if session.pristine()
            || session.editing.stroke.is_some()
            || session.editing.layer_edit.is_some()
        {
            return;
        }
        let source = session
            .project
            .as_ref()
            .map(|path| {
                let absolute = if path.is_absolute() {
                    path.clone()
                } else {
                    std::env::current_dir()?.join(path)
                };
                NativePath::encode(&absolute)
            })
            .transpose()
            .map(|source| {
                source.or_else(|| session.recovery.source_hint.clone())
            });
        let result = (|| -> anyhow::Result<()> {
            let source = source?;
            let session = self.documents.get_by_key_mut(target).unwrap();
            let runtime = self.recovery_runtime.as_mut().unwrap();
            runtime.shutdown_ready = false;
            if !session.recovery.registered
                || session.recovery.registered_source != source
            {
                runtime
                    .store
                    .register(target.document_id.bytes(), source.clone())?;
                session.recovery.registered = true;
                session.recovery.registered_source = source.clone();
            }
            let next = runtime.next_request()?;
            let previous = session
                .recovery
                .metadata
                .as_ref()
                .filter(|m| m.source_path == source);
            runtime.worker.submit(RecoveryRequest {
                key: JobKey {
                    document_id: target.document_id.bytes(),
                    session_epoch: target.epoch.bytes(),
                    request_id: next,
                },
                edit_state_id: session.history.state_id(),
                snapshot: session.document.clone(),
                source_path: source,
                source_saved_state_id: session
                    .saved_edit_state
                    .map(|id| id.bytes())
                    .or_else(|| previous.and_then(|m| m.source_saved_state_id)),
                source_saved_digest: session
                    .saved_digest
                    .or_else(|| previous.and_then(|m| m.source_saved_digest)),
                source_head: session
                    .source_head
                    .clone()
                    .or_else(|| previous.and_then(|m| m.source_head.clone())),
            })?;
            session.recovery.request_id = next;
            session.recovery.submitted = session.document.revision;
            session.recovery.last_submit = Instant::now();
            Ok(())
        })();
        if let Err(error) = result {
            let session = self.documents.get_by_key_mut(target).unwrap();
            session.recovery.failed = true;
            session.recovery.last_submit = Instant::now();
            self.notice = Some(format!(
                "Recovery failed; drawing remains in memory. Retry automatic recovery or save an original manually. {error:#}"
            ));
        }
    }
    pub(crate) fn poll_document_recovery(&mut self) {
        while let Some(event) = self
            .recovery_runtime
            .as_ref()
            .and_then(|runtime| runtime.worker.poll())
        {
            let id = DocumentId::from_bytes(event.key.document_id);
            let Some(session) = self
                .documents
                .get_mut(id)
                .filter(|s| s.key.epoch.bytes() == event.key.session_epoch)
            else {
                continue;
            };
            if event.key.request_id < session.recovery.saved_request
                || event.key.request_id > session.recovery.request_id
            {
                continue;
            }
            let result=event.result.and_then(|head|{
                self.recovery_runtime.as_ref().unwrap().store.recovered(event.key.document_id,head.generation).map_err(|e|format!("Recovery committed, but registry update failed: {e:#}"))?;
                Ok(head)
            });
            match result {
                Ok(head) => {
                    session.recovery.saved_request = event.key.request_id;
                    session.recovery.saved =
                        session.recovery.saved.max(event.revision);
                    session.recovery.saved_state = Some(event.edit_state_id);
                    session.recovery.generation = Some(head.generation);
                    session.recovery.failed = false;
                }
                Err(error) => {
                    session.recovery.failed = true;
                    self.notice =
                        Some(format!("Automatic recovery failed: {error}"));
                }
            }
        }
    }
    pub(crate) fn maintain_document_recovery(&mut self) {
        if self.recovery_runtime.as_ref().is_some_and(|r| {
            r.cleanup_needed
                && r.cleanup_tick.elapsed() > std::time::Duration::from_secs(5)
        }) {
            self.cleanup_closed_recoveries();
        }
        self.poll_project_opens();
        self.poll_source_saves();
        self.poll_document_recovery();
        let ids = self.documents.tab_order.clone();
        for id in ids {
            let session = self.documents.get(id).unwrap();
            let needs = !session.pristine()
                && (!session.recovery.registered
                    || session.recovery.failed
                    || session.document.revision != session.recovery.submitted);
            let interval = if session.recovery.failed {
                std::time::Duration::from_secs(3)
            } else {
                std::time::Duration::from_millis(u64::from(
                    self.options.autosave_delay_ms,
                ))
            };
            if needs && session.recovery.last_submit.elapsed() >= interval {
                self.submit_document_recovery(session.key);
            }
        }
        let order: Vec<_> =
            self.documents.tab_order.iter().map(|id| id.bytes()).collect();
        let active = self.documents.active_id().map(|id| id.bytes());
        let layout = (order, active);
        let runtime = self.recovery_runtime.as_mut().unwrap();
        if runtime.last_layout.as_ref() != Some(&layout)
            || runtime.shutdown_ready
        {
            match runtime.store.layout(&layout.0, layout.1, false) {
                Ok(()) => {
                    runtime.last_layout = Some(layout);
                    runtime.shutdown_ready = false;
                }
                Err(error) => {
                    self.notice =
                        Some(format!("Session layout was not saved: {error:#}"))
                }
            }
        }
    }
    pub(crate) fn flush_document_recovery(
        &mut self,
        clean: bool,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            !self.image_import.dialog_open(),
            "Finish or cancel the image import first"
        );
        anyhow::ensure!(
            self.tabs.pending.is_empty(),
            "Finish or cancel pending opens first"
        );
        anyhow::ensure!(
            self.source_saves.prompts.is_empty(),
            "Resolve or cancel the overwrite prompt first"
        );
        self.recovery_runtime.as_ref().unwrap().worker.barrier();
        self.poll_source_saves();
        anyhow::ensure!(
            self.source_saves.prompts.is_empty(),
            "Resolve or cancel the overwrite prompt first"
        );
        anyhow::ensure!(
            self.source_saves.pending.is_empty(),
            "An original save is incomplete"
        );
        let keys: Vec<_> = self
            .documents
            .tab_order
            .iter()
            .map(|&id| self.documents.get(id).unwrap().key)
            .collect();
        for &key in &keys {
            self.finish_stroke_for(key);
            self.finish_layer_edit_for(key);
            self.submit_document_recovery(key);
        }
        self.recovery_runtime.as_ref().unwrap().worker.barrier();
        self.poll_document_recovery();
        for key in keys {
            let session = self.documents.get_by_key(key).unwrap();
            anyhow::ensure!(
                session.pristine()
                    || (!session.recovery.failed
                        && session.recovery.registered
                        && session.recovery.saved_request
                            == session.recovery.request_id
                        && session.recovery.saved_state
                            == Some(session.history.state_id())),
                "Final recovery is incomplete; exit cancelled"
            );
        }
        let order: Vec<_> =
            self.documents.tab_order.iter().map(|id| id.bytes()).collect();
        let active = self.documents.active_id().map(|id| id.bytes());
        let runtime = self.recovery_runtime.as_mut().unwrap();
        runtime.store.layout(&order, active, clean)?;
        runtime.shutdown_ready = clean;
        Ok(())
    }
    pub(crate) fn persist_tab_close(
        &mut self,
        key: SessionKey,
    ) -> anyhow::Result<()> {
        if self.recovery_runtime.is_none() {
            return Ok(());
        }
        self.cleanup_closed_recoveries();
        let dir = self.recovery_path.parent().unwrap();
        let pending = self
            .recovery_runtime
            .as_ref()
            .unwrap()
            .store
            .records()?
            .iter()
            .filter(|r| {
                r.lifecycle == 1
                    && parfait_storage::recovery_path(dir, r.document_id)
                        .parent()
                        .unwrap()
                        .exists()
            })
            .count();
        anyhow::ensure!(
            pending < 32,
            "Recovery cleanup backlog reached 32; retry cleanup before closing more drawings"
        );
        if self.documents.get_by_key(key).is_some_and(|s| !s.pristine()) {
            self.submit_document_recovery(key);
            self.recovery_runtime.as_ref().unwrap().worker.barrier();
            self.poll_document_recovery();
            let session = self.documents.get_by_key(key).unwrap();
            anyhow::ensure!(
                !session.recovery.failed
                    && session.recovery.saved_request
                        == session.recovery.request_id
                    && session.recovery.saved_state
                        == Some(session.history.state_id()),
                "Latest recovery could not be committed; tab retained"
            );
        }
        let runtime = self.recovery_runtime.as_mut().unwrap();
        runtime.worker.forget(key.document_id.bytes(), key.epoch.bytes());
        let closed = runtime.store.close_document(key.document_id.bytes());
        runtime.last_layout = None;
        if closed.is_err()
            && let Some(session) = self.documents.get_by_key_mut(key)
        {
            session.recovery.failed = true;
            session.recovery.submitted = 0;
        }
        closed
    }
    pub(crate) fn cleanup_closed_recoveries(&mut self) {
        let Some(runtime) = self.recovery_runtime.as_mut() else {
            return;
        };
        runtime.cleanup_tick = Instant::now();
        if let Ok(records) = runtime.store.records() {
            let paths: Vec<_> = records
                .iter()
                .filter(|r| r.lifecycle == 1)
                .map(|r| {
                    parfait_storage::recovery_path(
                        self.recovery_path.parent().unwrap(),
                        r.document_id,
                    )
                    .parent()
                    .unwrap()
                    .to_owned()
                })
                .filter(|path| path.exists())
                .collect();
            runtime.cleanup_needed = false;
            for path in paths.iter().take(32) {
                if let Err(error) = std::fs::remove_dir_all(path) {
                    runtime.cleanup_needed = true;
                    self.notice = Some(format!(
                        "Closed recovery retained for cleanup retry: {error}"
                    ));
                }
            }
            if paths.len() > 32 {
                runtime.cleanup_needed = true;
            }
        }
    }
    pub(crate) fn recovery_items_ui(&mut self, ui: &mut eframe::egui::Ui) {
        let items: Vec<_> = self
            .recovery_runtime
            .as_ref()
            .map(|r| {
                r.pending_restore
                    .iter()
                    .map(|item| (item.record.document_id, item.error.clone()))
                    .collect()
            })
            .unwrap_or_default();
        if items.is_empty() {
            return;
        }
        eframe::egui::CollapsingHeader::new(format!(
            "Recovery items waiting ({})",
            items.len()
        ))
        .show(ui, |ui| {
            for (id, error) in items {
                ui.label(format!(
                    "{}: {error}",
                    parfait_storage::document_hex(id)
                ));
                ui.horizontal(|ui| {
                    if ui.button("Retry").clicked() {
                        self.retry_recovery_item(id, None);
                    }
                    if ui.button("Locate source").clicked()
                        && let Some(path) = rfd::FileDialog::new()
                            .add_filter("Parfait project", &["parfait"])
                            .pick_file()
                    {
                        self.retry_recovery_item(id, Some(path));
                    }
                    if ui.button("Skip for this run").clicked() {
                        self.recovery_runtime
                            .as_mut()
                            .unwrap()
                            .pending_restore
                            .retain(|item| item.record.document_id != id);
                    }
                });
            }
        });
    }
    fn retry_recovery_item(
        &mut self,
        id: [u8; 16],
        source: Option<std::path::PathBuf>,
    ) {
        let runtime = self.recovery_runtime.as_ref().unwrap();
        let Some(item) = runtime
            .pending_restore
            .iter()
            .find(|item| item.record.document_id == id)
        else {
            return;
        };
        let mut record = item.record.clone();
        if let Some(path) = source {
            match crate::project_paths::allowed(
                &path,
                self.recovery_path.parent().unwrap(),
            )
            .and_then(|p| NativePath::encode(&p))
            {
                Ok(path) => record.source_path = Some(path),
                Err(error) => {
                    self.notice = Some(error.to_string());
                    return;
                }
            }
        }
        if !self.reserve_pixel_headroom(512 * 1024 * 1024) {
            self.notice =
                Some("Not enough pixel budget to retry recovery.".into());
            return;
        }
        let result=restore_session(self.recovery_path.parent().unwrap(),&record,false).and_then(|mut session| {
            self.documents.check_admission(&session.document,0)?;
            if let Err(error)=validate_restored_source(self.recovery_path.parent().unwrap(),&mut session,false){self.notice=Some(format!("Recovered artwork retained without original binding: {error:#}"));}
            if session.project.as_ref().is_some_and(|p|self.documents.sessions().any(|s|s.project.as_ref().is_some_and(|other|crate::project_paths::same_source(p,other)))){session.project=None;session.saved_edit_state=None;session.saved_digest=None;session.project_saved_state=None;session.source_head=None;}
            self.documents.insert(session,self.documents.tab_order.len())
        });
        match result {
            Ok(_) => self
                .recovery_runtime
                .as_mut()
                .unwrap()
                .pending_restore
                .retain(|item| item.record.document_id != id),
            Err(error) => {
                if let Some(item) = self
                    .recovery_runtime
                    .as_mut()
                    .unwrap()
                    .pending_restore
                    .iter_mut()
                    .find(|item| item.record.document_id == id)
                {
                    item.error = error.to_string();
                }
            }
        }
    }
}
fn restore_session(
    dir: &std::path::Path,
    record: &SessionRecord,
    _was_clean: bool,
) -> anyhow::Result<DocumentSession> {
    let path = parfait_storage::recovery_path(dir, record.document_id);
    let recovered = if path.exists() {
        Store::open_read_only(&path)
            .and_then(|store| store.load_recovery(record.document_id))
            .unwrap_or(None)
    } else {
        None
    };
    if let Some(recovered) = recovered {
        let mut session = DocumentSession::new(recovered.loaded.document)?;
        session.key.document_id = DocumentId::from_bytes(record.document_id);
        if let Some(metadata) = recovered.metadata {
            session.history = History::restored(EditStateId::from_bytes(
                metadata.edit_state_id,
            ));
            session.recovery.source_hint = metadata.source_path.clone();
            session.recovery.metadata = Some(metadata);
        }
        session.recovery.registered = true;
        session.recovery.registered_source = record.source_path.clone();
        session.recovery.saved = session.document.revision;
        session.recovery.submitted = session.document.revision;
        session.recovery.saved_state = Some(session.history.state_id());
        session.recovery.generation = Some(recovered.loaded.head.generation);
        return Ok(session);
    }
    if let Some(source) = &record.source_path {
        let source = source.decode()?;
        let source = crate::project_paths::allowed(&source, dir)?;
        let loaded =
            Store::open_read_only(&source)?.load()?.ok_or_else(|| {
                anyhow::anyhow!("Source contains no valid snapshot")
            })?;
        let mut session = DocumentSession::new(loaded.document)?;
        session.key.document_id = DocumentId::from_bytes(record.document_id);
        session.recovery.registered = true;
        session.recovery.registered_source = record.source_path.clone();
        session.recovery.source_hint = record.source_path.clone();
        if !loaded.fell_back {
            session.project = Some(source);
            session.source_head = Some(loaded.head);
            session.saved_edit_state = Some(session.history.state_id());
            session.saved_digest =
                session.document.art_digest_v1().map(|d| d.0);
            session.project_saved_state = Some(session.document.clone());
        }
        session.recovery.failed = true;
        return Ok(session);
    }
    anyhow::bail!(
        "No valid recovery or readable source; original records preserved"
    )
}

fn validate_restored_source(
    dir: &std::path::Path,
    session: &mut DocumentSession,
    was_clean: bool,
) -> anyhow::Result<()> {
    let Some(metadata) = session.recovery.metadata.clone() else {
        return Ok(());
    };
    let Some(path) = metadata.source_path else {
        return Ok(());
    };
    let path = crate::project_paths::allowed(&path.decode()?, dir)?;
    let loaded = Store::open_read_only(&path)?
        .load()?
        .ok_or_else(|| anyhow::anyhow!("Original has no valid snapshot"))?;
    anyhow::ensure!(
        !loaded.fell_back,
        "Original has a damaged newest generation"
    );
    let hash = loaded
        .document
        .art_digest_v1()
        .ok_or_else(|| anyhow::anyhow!("Invalid original"))?
        .0;
    let was_saved = metadata.source_saved_state_id
        == Some(metadata.edit_state_id)
        && metadata.source_saved_digest == Some(metadata.logical_digest);
    let reopen_original = was_clean && was_saved;
    if reopen_original {
        // Source revisions belong to another file. Advance past the recovered
        // revision too, including load()'s repair bump after a damaged head.
        let revision = loaded
            .document
            .revision
            .max(session.document.revision)
            .max(session.recovery.generation.unwrap_or(0))
            .checked_add(1)
            .filter(|revision| i64::try_from(*revision).is_ok())
            .ok_or_else(|| {
                anyhow::anyhow!("Recovery revision limit reached")
            })?;
        if hash != metadata.logical_digest {
            session.history = History::try_new().map_err(|e| {
                anyhow::anyhow!("Could not identify reopened original: {e}")
            })?;
        }
        session.document = loaded.document;
        session.document.revision = revision;
        session.recovery.saved = 0;
        session.recovery.submitted = 0;
        session.recovery.saved_state = None;
    }
    let matches_current = reopen_original || hash == metadata.logical_digest;
    session.saved_edit_state = if matches_current {
        Some(session.history.state_id())
    } else if metadata.source_saved_digest == Some(hash) {
        metadata.source_saved_state_id.map(EditStateId::from_bytes)
    } else {
        None
    };
    session.saved_digest = Some(hash);
    session.project_saved_state =
        matches_current.then(|| session.document.clone());
    session.overwrite_required = !matches_current
        && (metadata.source_saved_digest != Some(hash)
            || metadata.source_head.as_ref() != Some(&loaded.head));
    session.source_head = Some(loaded.head);
    session.project = Some(path);
    session.binding_epoch = 1;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    fn doc(value: u8) -> Document {
        let mut d = Document::new(8, 8);
        d.layers[1].pixels = Arc::new([value, 20, 40, 255].repeat(64));
        d.selection = Some(parfait_core::MaskCoverage::Uniform(128));
        d
    }
    fn flush(app: &mut Parfait) {
        app.flush_document_recovery(false).unwrap();
    }
    #[test]
    fn stage3_app_restores_independent_tabs_order_active_ids_and_state_not_history()
     {
        let dir = tempfile::tempdir().unwrap();
        let mut app =
            Parfait::open_document_tabs(dir.path().to_owned()).unwrap();
        assert!(
            app.recovery_runtime
                .as_ref()
                .unwrap()
                .store
                .records()
                .unwrap()
                .is_empty()
        );
        let a = app.documents.active_id().unwrap();
        let epoch = app.documents.get(a).unwrap().key.epoch;
        app.edit_named("A paint", |d| *d = doc(10));
        let a_state = app.documents.get(a).unwrap().history.state_id();
        let b = app.new_document_tab(doc(90)).unwrap();
        let b_state = app.documents.get(b).unwrap().history.state_id();
        flush(&mut app);
        assert_eq!(app.recovery_status(), "복구 저장됨");
        let before_a = app.documents.get(a).unwrap().document.clone();
        let before_b = app.documents.get(b).unwrap().document.clone();
        assert!(
            !app.recovery_runtime
                .as_ref()
                .unwrap()
                .store
                .clean_shutdown()
                .unwrap()
        );
        drop(app);
        let app = Parfait::open_document_tabs(dir.path().to_owned()).unwrap();
        assert_eq!(app.documents.tab_order, [a, b]);
        assert_eq!(app.documents.active_id(), Some(b));
        assert_eq!(app.documents.get(a).unwrap().document, before_a);
        assert_eq!(app.documents.get(b).unwrap().document, before_b);
        assert_ne!(app.documents.get(a).unwrap().key.epoch, epoch);
        assert_eq!(app.documents.get(a).unwrap().history.state_id(), a_state);
        assert_eq!(app.documents.get(b).unwrap().history.state_id(), b_state);
        assert!(!app.documents.get(a).unwrap().history.can_undo());
        assert!(app.documents.get(a).unwrap().project.is_none());
    }
    #[test]
    fn stage3_normal_shutdown_retains_unsaved_tabs_but_not_untouched_empty_tabs()
     {
        let dir = tempfile::tempdir().unwrap();
        let mut app =
            Parfait::open_document_tabs(dir.path().to_owned()).unwrap();
        let empty = app.documents.active_id().unwrap();
        let a = app.new_document_tab(doc(20)).unwrap();
        app.new_document();
        let second_empty = app.documents.active_id().unwrap();
        assert!(app.documents.get(second_empty).unwrap().pristine());
        app.flush_document_recovery(true).unwrap();
        assert!(
            app.recovery_runtime
                .as_ref()
                .unwrap()
                .store
                .clean_shutdown()
                .unwrap()
        );
        drop(app);
        let app = Parfait::open_document_tabs(dir.path().to_owned()).unwrap();
        assert_eq!(app.documents.tab_order, [a]);
        assert!(app.documents.get(empty).is_none());
        assert!(app.documents.get(second_empty).is_none());
    }
    #[test]
    fn stage3_closed_tab_does_not_resurrect_and_orphan_files_are_not_automatically_opened()
     {
        let dir = tempfile::tempdir().unwrap();
        let mut app =
            Parfait::open_document_tabs(dir.path().to_owned()).unwrap();
        let a = app.new_document_tab(doc(10)).unwrap();
        let b = app.new_document_tab(doc(20)).unwrap();
        app.documents.get_mut(b).unwrap().project_saved_state =
            Some(app.documents.get(b).unwrap().document.clone());
        flush(&mut app);
        assert!(app.close_document_tab(b));
        flush(&mut app);
        let orphan = parfait_storage::recovery_path(dir.path(), [99; 16]);
        Store::open(&orphan).unwrap().save(&doc(99)).unwrap();
        drop(app);
        let app = Parfait::open_document_tabs(dir.path().to_owned()).unwrap();
        assert!(app.documents.get(a).is_some());
        assert!(app.documents.get(b).is_none());
        assert_eq!(app.documents.tab_order.len(), 1);
        assert!(orphan.exists());
    }
    #[test]
    fn stage3_failed_restore_is_visible_and_does_not_block_other_drawings_or_delete_records()
     {
        let dir = tempfile::tempdir().unwrap();
        let mut app =
            Parfait::open_document_tabs(dir.path().to_owned()).unwrap();
        let a = app.new_document_tab(doc(10)).unwrap();
        flush(&mut app);
        app.recovery_runtime
            .as_mut()
            .unwrap()
            .store
            .register([80; 16], None)
            .unwrap();
        drop(app);
        let mut app =
            Parfait::open_document_tabs(dir.path().to_owned()).unwrap();
        assert!(app.documents.get(a).is_some());
        assert_eq!(
            app.recovery_runtime.as_ref().unwrap().pending_restore.len(),
            1
        );
        let count = app
            .recovery_runtime
            .as_ref()
            .unwrap()
            .store
            .records()
            .unwrap()
            .len();
        app.flush_document_recovery(true).unwrap();
        assert_eq!(
            app.recovery_runtime
                .as_ref()
                .unwrap()
                .store
                .records()
                .unwrap()
                .len(),
            count
        );
        let mut out = eframe::egui::Context::default()
            .run_ui(Default::default(), |ui| app.recovery_items_ui(ui));
        out.textures_delta.clear();
    }
    #[test]
    fn stage3_stale_epoch_result_and_other_document_completion_cannot_mark_current_tab_saved()
     {
        let dir = tempfile::tempdir().unwrap();
        let mut app =
            Parfait::open_document_tabs(dir.path().to_owned()).unwrap();
        let a = app.new_document_tab(doc(10)).unwrap();
        let a_key = app.documents.get(a).unwrap().key;
        app.recovery_runtime.as_ref().unwrap().worker.barrier();
        // Replace only the memory lifetime before consuming the completed old-owner event.
        let replacement = DocumentSession::new(doc(11)).unwrap().key.epoch;
        app.documents.get_mut(a).unwrap().key.epoch = replacement;
        app.poll_document_recovery();
        assert_eq!(app.documents.get(a).unwrap().recovery.saved_request, 0);
        app.recovery_runtime
            .as_ref()
            .unwrap()
            .worker
            .forget(a.bytes(), a_key.epoch.bytes());
        let b = app.new_document_tab(doc(20)).unwrap();
        app.force_save_for(app.documents.get(a).unwrap().key);
        app.recovery_runtime.as_ref().unwrap().worker.barrier();
        app.poll_document_recovery();
        assert_eq!(app.documents.active_id(), Some(b));
        assert_eq!(
            app.documents.get(a).unwrap().recovery.saved_state,
            Some(app.documents.get(a).unwrap().history.state_id())
        );
        assert_eq!(
            app.documents.get(b).unwrap().document.layers[1].pixels[0],
            20
        );
    }
    #[test]
    fn stage3_failed_initial_recovery_blocks_clean_exit_and_recovers_after_retry()
     {
        let dir = tempfile::tempdir().unwrap();
        let mut app =
            Parfait::open_document_tabs(dir.path().to_owned()).unwrap();
        let a = app.documents.active_id().unwrap();
        let path = parfait_storage::recovery_path(dir.path(), a.bytes());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"bad database").unwrap();
        app.edit_named("First", |d| *d = doc(30));
        assert!(app.flush_document_recovery(true).is_err());
        assert!(app.documents.get(a).unwrap().recovery.failed);
        assert!(
            !app.recovery_runtime
                .as_ref()
                .unwrap()
                .store
                .clean_shutdown()
                .unwrap()
        );
        std::fs::remove_file(path).unwrap();
        app.flush_document_recovery(true).unwrap();
        assert!(!app.documents.get(a).unwrap().recovery.failed);
    }
    #[test]
    fn stage3_future_session_registry_and_second_process_are_rejected_without_legacy_writes()
     {
        let dir = tempfile::tempdir().unwrap();
        let app = Parfait::open_document_tabs(dir.path().to_owned()).unwrap();
        assert!(Parfait::open_document_tabs(dir.path().to_owned()).is_err());
        drop(app);
        let path = dir.path().join("session.sqlite");
        // A corrupt registry is rejected before a legacy recovery file can be created.
        std::fs::write(&path, b"future or corrupted registry").unwrap();
        assert!(Parfait::open_document_tabs(dir.path().to_owned()).is_err());
        assert!(!dir.path().join("recovery.parfait").exists());
    }
}

#[cfg(test)]
mod extra_tests {
    use super::*;
    #[test]
    fn stage3_first_edit_registers_immediately_and_worker_restart_keeps_the_owner()
     {
        let dir = tempfile::tempdir().unwrap();
        let mut app =
            Parfait::open_document_tabs(dir.path().to_owned()).unwrap();
        let key = app.documents.active_key().unwrap();
        app.edit_named("First opacity", |d| d.layers[1].opacity = 0.5);
        assert!(app.documents.get_by_key(key).unwrap().recovery.registered);
        assert_eq!(app.recovery_status(), "복구 저장 중");
        assert_eq!(
            app.recovery_runtime
                .as_ref()
                .unwrap()
                .store
                .records()
                .unwrap()
                .len(),
            1
        );
        app.recovery_runtime.as_mut().unwrap().worker.finish();
        app.edit_named("Second opacity", |d| d.layers[1].opacity = 0.25);
        app.flush_document_recovery(false).unwrap();
        assert!(app.recovery_runtime.as_ref().unwrap().worker.available());
        assert_eq!(app.recovery_status(), "복구 저장됨");
        assert_eq!(app.documents.active_key(), Some(key));
    }
    #[test]
    fn stage3_restore_over_tab_limit_is_deferred_not_deleted_and_retryable() {
        let dir = tempfile::tempdir().unwrap();
        let mut registry =
            SessionStore::open(&dir.path().join("session.sqlite")).unwrap();
        for n in 1..=9u8 {
            let id = [n; 16];
            registry.register(id, None).unwrap();
            let document = Document::new(1, 1);
            let metadata = parfait_storage::RecoveryMetadata {
                document_id: id,
                edit_state_id: EditStateId::try_new().unwrap().bytes(),
                logical_digest_version: 1,
                logical_digest: document.art_digest_v1().unwrap().0,
                source_path: None,
                source_saved_state_id: None,
                source_saved_digest: None,
                source_head: None,
            };
            Store::open(&parfait_storage::recovery_path(dir.path(), id))
                .unwrap()
                .save_recovery(&document, &metadata)
                .unwrap();
        }
        drop(registry);
        let mut app =
            Parfait::open_document_tabs(dir.path().to_owned()).unwrap();
        assert_eq!(app.documents.tab_order.len(), 8);
        assert_eq!(
            app.recovery_runtime.as_ref().unwrap().pending_restore.len(),
            1
        );
        assert_eq!(
            app.recovery_runtime
                .as_ref()
                .unwrap()
                .store
                .records()
                .unwrap()
                .len(),
            9
        );
        // Leave a tab's original unmodified baseline so the existing safe-close policy permits removal.
        let first = app.documents.active_id().unwrap();
        let saved = app.documents.get(first).unwrap().document.clone();
        app.documents.get_mut(first).unwrap().project_saved_state = Some(saved);
        assert!(app.close_document_tab(first));
        app.retry_recovery_item([9; 16], None);
        assert_eq!(app.documents.tab_order.len(), 8);
        assert!(
            app.recovery_runtime.as_ref().unwrap().pending_restore.is_empty()
        );
    }
}

#[cfg(test)]
mod source_hint_tests {
    use super::*;
    #[test]
    fn stage4_source_hint_fallback_is_validated_and_gets_its_own_initial_recovery()
     {
        let dir = tempfile::tempdir().unwrap();
        let source_dir = tempfile::tempdir().unwrap();
        let source = source_dir.path().join("original.parfait");
        let mut document = Document::new(8, 8);
        document.layers[1].opacity = 0.42;
        Parfait::write_project_document(&document, &source).unwrap();
        let before = std::fs::read(&source).unwrap();
        let mut registry =
            SessionStore::open(&dir.path().join("session.sqlite")).unwrap();
        let encoded = NativePath::encode(&source).unwrap();
        registry.register([77; 16], Some(encoded.clone())).unwrap();
        drop(registry);
        let mut app =
            Parfait::open_document_tabs(dir.path().to_owned()).unwrap();
        let session = &*app.documents.active_session;
        assert_eq!(session.document.layers, document.layers);
        assert_eq!(
            session.project.as_ref(),
            Some(&source.canonicalize().unwrap())
        );
        assert!(session.recovery.failed);
        app.flush_document_recovery(false).unwrap();
        let recovered = Store::open_read_only(&parfait_storage::recovery_path(
            dir.path(),
            [77; 16],
        ))
        .unwrap()
        .load_recovery([77; 16])
        .unwrap()
        .unwrap();
        assert_eq!(
            recovered.metadata.unwrap().source_path,
            Some(NativePath::encode(&source.canonicalize().unwrap()).unwrap())
        );
        assert_eq!(std::fs::read(source).unwrap(), before);
    }
}

#[cfg(test)]
mod reopened_revision_tests {
    use super::*;

    fn verify_reopened_original(same_art: bool) {
        for revision_offset in [None, Some(0), Some(100)] {
            for clean in [false, true] {
                for edit in [false, true] {
                    let dir = tempfile::tempdir().unwrap();
                    let out = tempfile::tempdir().unwrap();
                    let path = out.path().join("drawing.parfait");
                    let mut app = Parfait::open(dir.path().to_owned()).unwrap();
                    app.edit_named("Fixture", |d| {
                        *d = Document::new(8, 8);
                        d.revision = 100;
                        d.layers[1].opacity = 0.8;
                    });
                    app.save_project_to(path.clone()).unwrap();
                    app.flush_document_recovery(true).unwrap();
                    let id = app.documents.active_id().unwrap();
                    let session = app.documents.get(id).unwrap();
                    let old_generation = session.recovery.generation.unwrap();
                    let saved_state = session.history.state_id();
                    let mut replacement = session.document.clone();
                    drop(app);

                    replacement.revision =
                        revision_offset.map_or(1, |n| old_generation + n);
                    replacement.name = "Replacement header".into();
                    replacement.active = 0;
                    if !same_art {
                        replacement.layers[1].opacity = 0.25;
                    }
                    std::fs::remove_file(&path).unwrap();
                    Parfait::write_project_document(&replacement, &path)
                        .unwrap();
                    let source_bytes = std::fs::read(&path).unwrap();
                    let source_head = Store::open_read_only(&path)
                        .unwrap()
                        .load()
                        .unwrap()
                        .unwrap()
                        .head;

                    let mut app = Parfait::open(dir.path().to_owned()).unwrap();
                    let session = app.documents.get(id).unwrap();
                    assert_eq!(session.document.layers, replacement.layers);
                    assert_eq!(session.document.name, replacement.name);
                    assert_eq!(session.document.active, replacement.active);
                    assert!(session.document.revision > old_generation);
                    assert!(session.document.revision >= replacement.revision);
                    assert_eq!(
                        session.source_head.as_ref(),
                        Some(&source_head)
                    );
                    assert!(!session.modified());
                    assert_eq!(
                        session.history.state_id() == saved_state,
                        same_art
                    );
                    if edit {
                        app.edit_named("Edit replacement", |d| {
                            d.layers[1].opacity = 0.35;
                        });
                    }
                    let expected =
                        app.documents.get(id).unwrap().document.clone();
                    app.flush_document_recovery(clean).unwrap();
                    assert_eq!(app.recovery_status(), "복구 저장됨");
                    let recovered = Store::open_read_only(
                        &parfait_storage::recovery_path(dir.path(), id.bytes()),
                    )
                    .unwrap()
                    .load_recovery(id.bytes())
                    .unwrap()
                    .unwrap();
                    assert!(!recovered.loaded.fell_back);
                    assert!(recovered.loaded.head.generation > old_generation);
                    assert_eq!(recovered.loaded.document, expected);
                    assert_eq!(
                        recovered.metadata.unwrap().source_head,
                        Some(source_head.clone())
                    );
                    drop(app);

                    let app = Parfait::open(dir.path().to_owned()).unwrap();
                    let session = app.documents.get(id).unwrap();
                    assert_eq!(session.document.layers, expected.layers);
                    assert_eq!(session.document.name, expected.name);
                    assert_eq!(session.document.active, expected.active);
                    assert_eq!(session.modified(), edit);
                    assert_eq!(
                        session.source_head.as_ref(),
                        Some(&source_head)
                    );
                    assert_eq!(std::fs::read(&path).unwrap(), source_bytes);
                }
            }
        }
    }

    #[test]
    fn reopened_original_revision_keeps_new_artwork_and_edits_recoverable() {
        verify_reopened_original(false);
    }

    #[test]
    fn reopened_original_revision_keeps_header_changes_with_same_artwork() {
        verify_reopened_original(true);
    }

    #[test]
    fn reopened_original_revision_exhaustion_preserves_recovered_session() {
        let dir = tempfile::tempdir().unwrap();
        let out = tempfile::tempdir().unwrap();
        let path = out.path().join("drawing.parfait");
        let mut app = Parfait::open(dir.path().to_owned()).unwrap();
        app.edit_named("Fixture", |d| *d = Document::new(8, 8));
        app.save_project_to(path).unwrap();
        app.flush_document_recovery(true).unwrap();
        let record = app
            .recovery_runtime
            .as_ref()
            .unwrap()
            .store
            .records()
            .unwrap()
            .remove(0);
        for limit in [i64::MAX as u64, u64::MAX] {
            let mut session =
                restore_session(dir.path(), &record, true).unwrap();
            session.document.revision = limit;
            let before = session.document.clone();
            let state = session.history.state_id();
            let saved = session.recovery.saved;
            let error =
                validate_restored_source(dir.path(), &mut session, true)
                    .unwrap_err();
            assert!(error.to_string().contains("Recovery revision limit"));
            assert_eq!(session.document, before);
            assert_eq!(session.history.state_id(), state);
            assert_eq!(session.recovery.saved, saved);
            assert!(session.project.is_none());
            assert!(session.saved_edit_state.is_none());
        }
    }
}
