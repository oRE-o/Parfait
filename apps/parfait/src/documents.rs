//! Single-session manager for the extraction increment; tabs arrive separately.
use crate::document_session::DocumentSession;
use parfait_core::Document;

pub(crate) struct DocumentManager {
    // A concrete field preserves disjoint borrowing of document state and app settings.
    pub session: DocumentSession,
}

impl DocumentManager {
    pub fn new(document: Document) -> anyhow::Result<Self> {
        Ok(Self { session: DocumentSession::new(document)? })
    }
}
