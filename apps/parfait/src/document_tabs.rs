//! Default multi-document integration with durable session recovery.
use crate::{
    app::Parfait,
    document_session::{DocumentId, DocumentSession},
    project_loader::{LoadResult, ProjectLoader, RequestId},
};
use eframe::egui;
mod close;
mod ui;
use parfait_core::Document;
use std::{collections::HashMap, path::PathBuf};

pub(crate) struct PendingOpen {
    pub id: RequestId,
    pub path: PathBuf,
    pub focus: u64,
    pub cancelled: bool,
}
#[derive(Default)]
pub(crate) struct DocumentTabs {
    pub enabled: bool,
    pub pending: Vec<PendingOpen>,
    pub in_flight: Option<RequestId>,
    pub loader: Option<ProjectLoader>,
    next_order: u64,
    orders: HashMap<DocumentId, u64>,
    latest_focus: Option<RequestId>,
    pub input_barrier: bool,
    pub pinned_save: Option<Document>,
    drag: Option<ui::TabDrag>,
    drawn_active: Option<DocumentId>,
    close_prompt: Option<DocumentId>,
    close_after_save: Option<crate::document_session::SessionKey>,
}
impl DocumentTabs {
    fn next(&mut self) -> u64 {
        self.next_order += 1;
        self.next_order
    }
    pub fn reserved_pixels(&self) -> u64 {
        if self.in_flight.is_some() { 512 * 1024 * 1024 } else { 0 }
    }
}
impl Parfait {
    fn fresh_document_session(
        &self,
        document: Document,
    ) -> anyhow::Result<DocumentSession> {
        let known = if let Some(runtime) = &self.recovery_runtime {
            runtime
                .store
                .records()?
                .into_iter()
                .map(|r| r.document_id)
                .collect::<std::collections::HashSet<_>>()
        } else {
            Default::default()
        };
        let mut session = DocumentSession::new(document)?;
        while known.contains(&session.key.document_id.bytes())
            || self.documents.get(session.key.document_id).is_some()
        {
            session.key = crate::document_session::SessionKey::new()?;
        }
        Ok(session)
    }
    fn tab_switch_blocked(&self) -> bool {
        self.dialog_open()
            || (!self.documents.is_empty()
                && self
                    .documents
                    .active_session
                    .layer_ui
                    .list
                    .has_name_editor())
    }
    #[cfg(test)]
    pub(crate) fn enable_document_tabs(&mut self) -> anyhow::Result<()> {
        // Drain the legacy worker before enabling tabs; never feed it a different document.
        self.autosave.finish(self.documents.active_session.document.clone());
        self.tabs.loader = Some(ProjectLoader::new()?);
        self.start_document_recovery()?;
        self.tabs.enabled = true;
        Ok(())
    }
    fn settle_tab_input(&mut self) {
        if !self.documents.is_empty() {
            self.finish_stroke();
            self.finish_layer_edit();
            self.documents.active_session.layer_ui.list.cancel_pointer();
            self.documents.active_session.view.cancel_gesture();
        }
        self.tabs.input_barrier = true;
        self.pressure = 1.;
    }
    pub(crate) fn activate_document(
        &mut self,
        id: DocumentId,
        user: bool,
    ) -> bool {
        if self.tab_switch_blocked() || self.documents.get(id).is_none() {
            return false;
        }
        if user {
            self.documents.focus_generation += 1;
        }
        if self.documents.active_id() == Some(id) {
            return true;
        }
        self.settle_tab_input();
        self.documents.activate(id);
        self.render_cache = Default::default();
        true
    }
    pub(crate) fn new_document_tab(
        &mut self,
        document: Document,
    ) -> anyhow::Result<DocumentId> {
        self.new_document_tab_as(document, false)
    }
    pub(crate) fn new_document_tab_as(
        &mut self,
        document: Document,
        untouched: bool,
    ) -> anyhow::Result<DocumentId> {
        anyhow::ensure!(
            !self.tab_switch_blocked(),
            "Finish or cancel the current dialog first"
        );
        self.documents.check_admission(&document, self.tabs.pending.len())?;
        anyhow::ensure!(
            self.documents
                .buffers()
                .bytes()
                .saturating_add(document.logical_pixel_bytes())
                .saturating_add(self.tabs.reserved_pixels())
                <= self.documents.retained_limit,
            "A reader or edit holds the available pixel budget"
        );
        anyhow::ensure!(
            self.reserve_pixel_headroom(document.logical_pixel_bytes()),
            "Recovery snapshots hold the available pixel budget"
        );
        let mut session = self.fresh_document_session(document)?;
        session.untouched_new = untouched;
        let id =
            self.documents.insert(session, self.documents.tab_order.len())?;
        self.settle_tab_input();
        let order = self.tabs.next();
        self.tabs.orders.insert(id, order);
        self.documents.focus_generation += 1;
        self.documents.activate(id);
        self.render_cache = Default::default();
        if !untouched {
            self.force_save_for(self.documents.get(id).unwrap().key);
        }
        Ok(id)
    }
    pub(crate) fn close_document_tab(&mut self, id: DocumentId) -> bool {
        self.close_document_tab_checked(id, false)
    }
    fn close_document_tab_checked(
        &mut self,
        id: DocumentId,
        discard: bool,
    ) -> bool {
        if self.tab_switch_blocked() {
            return false;
        }
        if self.documents.active_id() == Some(id) {
            self.settle_tab_input();
        }
        if self.source_saves.pending.contains_key(&id)
            || self
                .source_saves
                .prompts
                .iter()
                .any(|p| p.request.key.document_id == id.bytes())
        {
            self.notice=Some("Finish or cancel the original save before closing this drawing.".into());
            return false;
        }
        let Some(session) = self.documents.get(id) else {
            return false;
        };
        if session.modified() && !session.pristine() && !discard {
            self.notice =
                Some("Save this drawing manually before closing it.".into());
            return false;
        }
        let key = session.key;
        if let Err(error) = self.persist_tab_close(key) {
            self.notice = Some(format!("Close cancelled: {error:#}"));
            return false;
        }
        let was_active = self.documents.active_id() == Some(id);
        self.documents.remove(id);
        self.cleanup_closed_recoveries();
        self.tabs.orders.remove(&id);
        self.documents.focus_generation += 1;
        if was_active {
            self.render_cache = Default::default();
        }
        true
    }
    pub(crate) fn queue_project_open(
        &mut self,
        path: PathBuf,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            !self.tab_switch_blocked(),
            "Finish or cancel the current dialog first"
        );
        let path = crate::project_paths::allowed(
            &path,
            self.recovery_path.parent().unwrap(),
        )?;
        let existing = self
            .documents
            .sessions()
            .find(|s| {
                s.project.as_ref().is_some_and(|p| {
                    crate::project_paths::same_source(p, &path)
                })
            })
            .map(|s| s.key.document_id);
        if let Some(id) = existing {
            self.activate_document(id, true);
            return Ok(());
        }
        anyhow::ensure!(
            !self
                .tabs
                .pending
                .iter()
                .any(|p| crate::project_paths::same_source(&p.path, &path)),
            "This source is already being opened"
        );
        anyhow::ensure!(
            self.documents.tab_order.len() + self.tabs.pending.len()
                < self.documents.max_tabs,
            "Open tab limit reached"
        );
        anyhow::ensure!(
            !self
                .source_saves
                .pending
                .values()
                .any(|p| crate::project_paths::same_source(&p.path, &path))
                && !self.source_saves.prompts.iter().any(|p| {
                    crate::project_paths::same_source(&p.request.path, &path)
                }),
            "This file is reserved by an original save"
        );
        let id = self.tabs.next();
        self.tabs.latest_focus = Some(id);
        self.tabs.pending.push(PendingOpen {
            id,
            path,
            focus: self.documents.focus_generation,
            cancelled: false,
        });
        self.start_next_open();
        Ok(())
    }
    pub(crate) fn cancel_project_open(&mut self, id: RequestId) {
        if self.tabs.in_flight == Some(id) {
            if let Some(pending) =
                self.tabs.pending.iter_mut().find(|p| p.id == id)
            {
                pending.cancelled = true;
            }
        } else {
            self.tabs.pending.retain(|p| p.id != id);
        }
    }
    fn start_next_open(&mut self) {
        if self.tabs.in_flight.is_some() {
            return;
        }
        let Some(pending) = self.tabs.pending.first() else {
            return;
        };
        let (id, path) = (pending.id, pending.path.clone());
        if !self.reserve_pixel_headroom(512 * 1024 * 1024) {
            self.tabs.pending.remove(0);
            self.notice = Some(
                "Not enough retained pixel budget to read another drawing."
                    .into(),
            );
            return;
        }
        match self.tabs.loader.as_ref().unwrap().submit(id, path) {
            Ok(()) => self.tabs.in_flight = Some(id),
            Err(error) => {
                self.tabs.pending.remove(0);
                self.error = Some(error.to_string());
            }
        }
    }
    pub(crate) fn poll_project_opens(&mut self) {
        if !self.tabs.enabled {
            return;
        }
        match self.tabs.loader.as_ref().unwrap().poll() {
            Ok(Some(result)) => self.complete_project_open(result),
            Ok(None) => {}
            Err(_) => {
                self.tabs.pending.clear();
                self.tabs.in_flight = None;
                self.error = Some("Project reader disconnected".into());
            }
        }
        self.start_next_open();
    }
    pub(crate) fn complete_project_open(&mut self, loaded: LoadResult) {
        if self.tabs.in_flight == Some(loaded.id) {
            self.tabs.in_flight = None;
        }
        let Some(position) =
            self.tabs.pending.iter().position(|p| p.id == loaded.id)
        else {
            return;
        };
        let request = self.tabs.pending.remove(position);
        if request.cancelled {
            return;
        }
        let result = (|| -> anyhow::Result<DocumentId> {
            let document = loaded.result.map_err(anyhow::Error::msg)?;
            self.documents
                .check_admission(&document, self.tabs.pending.len())?;
            anyhow::ensure!(
                self.reserve_pixel_headroom(document.logical_pixel_bytes()),
                "Recovery snapshots hold the available pixel budget"
            );
            let mut session = self.fresh_document_session(document)?;
            if !loaded.fell_back {
                session.project = Some(request.path);
                session.source_head = loaded.source_head;
                session.saved_digest = loaded.digest;
                session.saved_edit_state = Some(session.history.state_id());
                session.project_saved_state = Some(session.document.clone());
            } else {
                session.recovery.source_hint =
                    parfait_storage::NativePath::encode(&request.path).ok();
                self.notice=Some("Opened an earlier valid source snapshot without a writable binding. Save As to preserve the damaged original.".into());
            }
            let position = self
                .documents
                .tab_order
                .iter()
                .position(|id| {
                    self.tabs.orders.get(id).copied().unwrap_or(0) > request.id
                })
                .unwrap_or(self.documents.tab_order.len());
            let id = self.documents.insert(session, position)?;
            self.force_save_for(self.documents.get(id).unwrap().key);
            self.tabs.orders.insert(id, request.id);
            Ok(id)
        })();
        match result {
            Ok(id) => {
                if request.focus == self.documents.focus_generation
                    && self.tabs.latest_focus == Some(request.id)
                {
                    self.activate_document(id, false);
                }
            }
            Err(error) => self.error = Some(format!("Open failed: {error:#}")),
        }
    }
}

#[cfg(test)]
mod tests;
