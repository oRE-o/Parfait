use parfait_core::{BlendMode, Document, Layer, LayerId};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Serialize, Deserialize)]
pub(crate) struct LegacyLayer {
    pub name: String,
    pub visible: bool,
    pub locked: bool,
    pub opacity: f32,
    pub blend_mode: BlendMode,
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
            next_layer_id: old.layers.len() as u64 + 1,
            layers: old
                .layers
                .into_iter()
                .enumerate()
                .map(|(index, layer)| Layer {
                    id: LayerId(index as u64 + 1),
                    name: layer.name,
                    visible: layer.visible,
                    locked: layer.locked,
                    alpha_locked: false,
                    clipped: false,
                    mask: None,
                    reference: false,
                    draft: false,
                    kind: parfait_core::LayerKind::Raster,
                    parent: None,
                    opacity: layer.opacity,
                    blend_mode: layer.blend_mode,
                    pixels: layer.pixels,
                })
                .collect(),
        }
    }
}
