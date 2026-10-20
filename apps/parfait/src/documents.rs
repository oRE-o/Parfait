//! Active slot plus ID-indexed parked sessions. Each session has exactly one owner.
use crate::document_session::{DocumentId, DocumentSession, SessionKey};
use parfait_core::Document;
use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
};

#[derive(Default)]
pub(crate) struct ActiveSession(Option<DocumentSession>);
impl Deref for ActiveSession {
    type Target = DocumentSession;
    fn deref(&self) -> &Self::Target {
        self.0.as_ref().expect("document command without active session")
    }
}
impl DerefMut for ActiveSession {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.as_mut().expect("document command without active session")
    }
}
impl ActiveSession {
    pub fn replace(&mut self, session: DocumentSession) {
        self.0 = Some(session);
    }
}

pub(crate) struct DocumentManager {
    /// The active tab; all other open documents live in `parked`.
    pub active_session: ActiveSession,
    parked: HashMap<DocumentId, DocumentSession>,
    pub tab_order: Vec<DocumentId>,
    pub max_tabs: usize,
    pub logical_limit: u64,
    pub retained_limit: u64,
    pub focus_generation: u64,
}
impl DocumentManager {
    pub fn new(document: Document) -> anyhow::Result<Self> {
        let session = DocumentSession::new(document)?;
        let id = session.key.document_id;
        Ok(Self {
            active_session: ActiveSession(Some(session)),
            parked: HashMap::new(),
            tab_order: vec![id],
            max_tabs: 8,
            logical_limit: 512 * 1024 * 1024,
            retained_limit: 1024 * 1024 * 1024,
            focus_generation: 0,
        })
    }
    pub fn replace_single(&mut self, session: DocumentSession) {
        assert!(
            self.parked.is_empty(),
            "single-document replacement with parked tabs"
        );
        self.tab_order = vec![session.key.document_id];
        self.active_session.replace(session);
    }
    pub fn active_id(&self) -> Option<DocumentId> {
        self.active_session.0.as_ref().map(|s| s.key.document_id)
    }
    pub fn active_key(&self) -> Option<SessionKey> {
        self.active_session.0.as_ref().map(|session| session.key)
    }
    pub fn is_empty(&self) -> bool {
        self.active_session.0.is_none()
    }
    pub fn get(&self, id: DocumentId) -> Option<&DocumentSession> {
        self.active_session
            .0
            .as_ref()
            .filter(|s| s.key.document_id == id)
            .or_else(|| self.parked.get(&id))
    }
    pub fn get_mut(&mut self, id: DocumentId) -> Option<&mut DocumentSession> {
        if self.active_id() == Some(id) {
            self.active_session.0.as_mut()
        } else {
            self.parked.get_mut(&id)
        }
    }
    /// Resolve a captured target, never whichever tab happens to be active now.
    pub fn get_by_key(&self, key: SessionKey) -> Option<&DocumentSession> {
        self.get(key.document_id).filter(|session| session.key == key)
    }
    pub fn get_by_key_mut(
        &mut self,
        key: SessionKey,
    ) -> Option<&mut DocumentSession> {
        self.get_mut(key.document_id).filter(|session| session.key == key)
    }
    pub fn sessions(&self) -> impl Iterator<Item = &DocumentSession> {
        self.active_session.0.iter().chain(self.parked.values())
    }
    pub fn logical_bytes(&self) -> u64 {
        self.sessions().map(|s| s.document.logical_pixel_bytes()).sum()
    }
    pub fn check_admission(
        &self,
        document: &Document,
        pending: usize,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.tab_order.len() + pending < self.max_tabs,
            "At most {} drawings can be open",
            self.max_tabs
        );
        anyhow::ensure!(document.validate(), "Invalid drawing");
        anyhow::ensure!(
            self.logical_bytes().saturating_add(document.logical_pixel_bytes())
                <= self.logical_limit,
            "Open drawings exceed the logical pixel budget"
        );
        let mut census = self.buffers();
        census.add_document(document);
        anyhow::ensure!(
            census.bytes().saturating_add(
                self.sessions().map(|s| s.edit_reservation).sum::<u64>()
            ) <= self.retained_limit,
            "Open drawings exceed the retained pixel budget"
        );
        Ok(())
    }
    pub fn insert(
        &mut self,
        mut session: DocumentSession,
        position: usize,
    ) -> anyhow::Result<DocumentId> {
        self.check_admission(&session.document, 0)?;
        while self.get(session.key.document_id).is_some() {
            session.key = crate::document_session::SessionKey::new()?;
        }
        let id = session.key.document_id;
        self.tab_order.insert(position.min(self.tab_order.len()), id);
        if self.is_empty() {
            self.active_session.replace(session);
        } else {
            self.parked.insert(id, session);
        }
        Ok(id)
    }
    pub fn activate(&mut self, id: DocumentId) -> bool {
        if self.active_id() == Some(id) {
            return true;
        }
        let Some(next) = self.parked.remove(&id) else {
            return false;
        };
        if let Some(old) = self.active_session.0.replace(next) {
            self.parked.insert(old.key.document_id, old);
        }
        true
    }
    pub fn remove(&mut self, id: DocumentId) -> bool {
        let Some(position) = self.tab_order.iter().position(|&tab| tab == id)
        else {
            return false;
        };
        self.tab_order.remove(position);
        if self.active_id() == Some(id) {
            self.active_session.0 = None;
            if let Some(&next) = self
                .tab_order
                .get(position.min(self.tab_order.len().saturating_sub(1)))
            {
                self.active_session.0 = self.parked.remove(&next);
            }
        } else {
            self.parked.remove(&id);
        }
        true
    }
    pub fn buffers(&self) -> parfait_core::BufferCensus {
        let mut census = parfait_core::BufferCensus::default();
        for session in self.sessions() {
            census.add_document(&session.document);
            if let Some(saved) = &session.project_saved_state {
                census.add_document(saved);
            }
            session.history.visit_documents(|doc| census.add_document(doc));
        }
        census
    }
}
