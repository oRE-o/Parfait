use crate::legacy_v9::LegacyLayer;
use parfait_core::Document;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub(crate) struct LegacyDocument {
    pub width: u32,
    pub height: u32,
    pub name: String,
    pub layers: Vec<LegacyLayer>,
    pub active: usize,
    pub revision: u64,
    pub next_layer_id: u64,
}

impl From<LegacyDocument> for Document {
    fn from(old: LegacyDocument) -> Self {
        Self {
            width: old.width,
            height: old.height,
            name: old.name,
            layers: old.layers.into_iter().map(Into::into).collect(),
            active: old.active,
            revision: old.revision,
            next_layer_id: old.next_layer_id,
            selection: None,
        }
    }
}
