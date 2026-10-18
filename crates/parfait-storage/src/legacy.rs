//! Exact version-1 binary layout. Keep separate: bincode cannot default new struct fields.
use parfait_core::{BlendMode, Document, Layer};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
#[derive(Serialize, Deserialize)]
pub(crate) struct LegacyLayer {
    pub name: String,
    pub visible: bool,
    pub locked: bool,
    pub opacity: f32,
    pub pixels: Arc<Vec<u8>>,
}
#[derive(Serialize, Deserialize)]
pub(crate) struct LegacyDocument {
    pub width: u32,
    pub height: u32,
    pub name: String,
    pub layers: Vec<LegacyLayer>,
    pub active: usize,
    pub revision: u64,
}
impl From<LegacyDocument> for Document {
    fn from(old: LegacyDocument) -> Self {
        Self {
            width: old.width,
            height: old.height,
            name: old.name,
            active: old.active,
            revision: old.revision,
            layers: old
                .layers
                .into_iter()
                .map(|l| Layer {
                    name: l.name,
                    visible: l.visible,
                    locked: l.locked,
                    opacity: l.opacity,
                    pixels: l.pixels,
                    blend_mode: BlendMode::Normal,
                })
                .collect(),
        }
    }
}
