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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct SessionKey {
    pub document_id: DocumentId,
    pub epoch: SessionEpoch,
}

impl SessionKey {
    fn new() -> anyhow::Result<Self> {
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
    pub saved: u64,
    pub submitted: u64,
    pub last_submit: Instant,
    pub failed: bool,
}

#[derive(Default)]
pub(crate) struct DocumentEditingState {
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
    pub document: Document,
    pub history: History,
    // Retain the existing baseline semantics until original-save baseline migration (4A).
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
            document,
            history: History::try_new().map_err(|error| {
                anyhow::anyhow!("Could not create edit state: {error}")
            })?,
            project: None,
            project_saved_state: None,
            recovery: RecoveryState {
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
