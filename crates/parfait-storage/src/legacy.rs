//! Exact version-1 binary layout. Keep separate: bincode cannot default new struct fields.
use parfait_core::{BlendMode, Document, Layer, LayerId};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
#[derive(Serialize, Deserialize)]
pub(crate) struct LegacyLayer {
    pub name: String,
    pub visible: bool,
    pub locked: bool,
    pub opacity: f32,
    #[serde(deserialize_with = "parfait_core::deserialize_pixel_buffer")]
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
            contents: Vec::new(),
            layer_colors: Vec::new(),
            width: old.width,
            height: old.height,
            name: old.name,
            selection: None,
            active: old.active,
            revision: old.revision,
            next_layer_id: old.layers.len() as u64 + 1,
            layers: old
                .layers
                .into_iter()
                .enumerate()
                .map(|(index, l)| Layer {
                    id: LayerId(index as u64 + 1),
                    name: l.name,
                    visible: l.visible,
                    locked: l.locked,
                    alpha_locked: false,
                    clipped: false,
                    mask: None,
                    reference: false,
                    draft: false,
                    kind: parfait_core::LayerKind::Raster,
                    parent: None,
                    opacity: l.opacity,
                    pixels: l.pixels,
                    blend_mode: BlendMode::Normal,
                })
                .collect(),
        }
    }
}
