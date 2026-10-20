//! Runtime document state. No UI settings or paths enter the core file format.
use crate::{
    canvas::View, editing::LayerEdit, layer_details::LayerDetails,
    layer_organize::RenameLayer, layers::LayerList,
};
use parfait_core::{Document, History, LayerId, Stroke};
use std::{path::PathBuf, time::Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct DocumentId([u8; 16]);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct SessionEpoch([u8; 16]);

impl DocumentId {
    pub fn bytes(self) -> [u8; 16] {
        self.0
    }
    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }
}
impl SessionEpoch {
    pub fn bytes(self) -> [u8; 16] {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct SessionKey {
    pub document_id: DocumentId,
    pub epoch: SessionEpoch,
}

impl SessionKey {
    pub(crate) fn new() -> anyhow::Result<Self> {
        let mut bytes = [0; 32];
        getrandom::fill(&mut bytes).map_err(|error| {
            anyhow::anyhow!("Could not create document identity: {error}")
        })?;
        Ok(Self {
            document_id: DocumentId(bytes[..16].try_into().unwrap()),
            epoch: SessionEpoch(bytes[16..].try_into().unwrap()),
        })
    }
}

pub(crate) struct RecoveryState {
    pub metadata: Option<parfait_storage::RecoveryMetadata>,
    pub source_hint: Option<parfait_storage::NativePath>,
    pub registered_source: Option<parfait_storage::NativePath>,
    pub registered: bool,
    pub request_id: u64,
    pub saved_request: u64,
    pub saved_state: Option<parfait_core::EditStateId>,
    pub generation: Option<u64>,
    pub saved: u64,
    pub submitted: u64,
    pub last_submit: Instant,
    pub failed: bool,
}

#[derive(Default)]
pub(crate) struct DocumentEditingState {
    pub align: crate::alignment::AlignSettings,
    pub stroke: Option<Stroke>,
    pub stroke_owner: Option<SessionKey>,
    pub layer_edit: Option<LayerEdit>,
    pub cancelled_layer_drag: bool,
    pub mask_target: Option<LayerId>,
    pub mask_overlay: Option<LayerId>,
}

#[derive(Default)]
pub(crate) struct DocumentLayerUi {
    pub list: LayerList,
    pub rename: Option<RenameLayer>,
    pub details: Option<LayerDetails>,
}

pub(crate) struct DocumentSession {
    pub key: SessionKey,
    pub untouched_new: bool,
    pub edit_reservation: u64,
    pub document: Document,
    pub history: History,
    // Retain the existing baseline semantics until original-save baseline migration (4A).
    pub binding_epoch: u64,
    pub source_head: Option<parfait_storage::SnapshotHead>,
    pub saved_edit_state: Option<parfait_core::EditStateId>,
    pub saved_digest: Option<[u8; 32]>,
    pub overwrite_required: bool,
    pub project: Option<PathBuf>,
    pub project_saved_state: Option<Document>,
    pub recovery: RecoveryState,
    pub view: View,
    pub editing: DocumentEditingState,
    pub layer_ui: DocumentLayerUi,
}

impl DocumentSession {
    pub fn new(document: Document) -> anyhow::Result<Self> {
        Ok(Self {
            key: SessionKey::new()?,
            untouched_new: false,
            edit_reservation: 0,
            document,
            history: History::try_new().map_err(|error| {
                anyhow::anyhow!("Could not create edit state: {error}")
            })?,
            binding_epoch: 0,
            source_head: None,
            saved_edit_state: None,
            saved_digest: None,
            overwrite_required: false,
            project: None,
            project_saved_state: None,
            recovery: RecoveryState {
                metadata: None,
                source_hint: None,
                registered_source: None,
                registered: false,
                request_id: 0,
                saved_request: 0,
                saved_state: None,
                generation: None,
                saved: 0,
                submitted: 0,
                last_submit: Instant::now(),
                failed: false,
            },
            view: View::default(),
            editing: DocumentEditingState::default(),
            layer_ui: DocumentLayerUi::default(),
        })
    }
}

impl DocumentSession {
    pub fn modified(&self) -> bool {
        if let Some(saved) = self.saved_edit_state {
            return saved != self.history.state_id()
                || self.history.pending_changed(&self.document);
        }
        self.project_saved_state
            .as_ref()
            .is_none_or(|saved| !self.document.same_edit_state(saved))
    }
    pub fn pristine(&self) -> bool {
        self.untouched_new
            && self.project.is_none()
            && self.history.rows().next().is_none()
            && self.editing.stroke.is_none()
            && self.editing.layer_edit.is_none()
    }
    pub fn title(&self) -> String {
        self.project
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.document.name.clone())
    }
}

impl DocumentSession {
    pub fn commit_edit(&mut self) -> bool {
        self.edit_reservation = 0;
        self.history.commit(&self.document)
    }
    pub fn cancel_pending(&mut self) -> bool {
        self.edit_reservation = 0;
        self.history.cancel(&mut self.document)
    }
    #[cfg(test)]
    pub fn undo_edit(&mut self) -> bool {
        self.history.undo(&mut self.document)
    }
}
