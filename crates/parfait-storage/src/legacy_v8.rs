use parfait_core::{Document, Layer};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub(crate) struct LegacyDocument {
    pub width: u32,
    pub height: u32,
    pub name: String,
    pub layers: Vec<Layer>,
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
            layers: old.layers,
            active: old.active,
            revision: old.revision,
            next_layer_id: old.next_layer_id,
            selection: None,
        }
    }
}
