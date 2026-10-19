use parfait_core::{BlendMode, Document, Layer, LayerId, LayerMask};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Serialize, Deserialize)]
pub(crate) struct LegacyLayer {
    pub id: LayerId,
    pub name: String,
    pub visible: bool,
    pub locked: bool,
    pub alpha_locked: bool,
    pub clipped: bool,
    pub opacity: f32,
    pub blend_mode: BlendMode,
    pub pixels: Arc<Vec<u8>>,
    pub mask: Option<LayerMask>,
}

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
            active: old.active,
            revision: old.revision,
            next_layer_id: old.next_layer_id,
            layers: old
                .layers
                .into_iter()
                .map(|layer| Layer {
                    id: layer.id,
                    name: layer.name,
                    visible: layer.visible,
                    locked: layer.locked,
                    alpha_locked: layer.alpha_locked,
                    clipped: layer.clipped,
                    opacity: layer.opacity,
                    blend_mode: layer.blend_mode,
                    pixels: layer.pixels,
                    mask: layer.mask,
                    reference: false,
                    draft: false,
                    kind: parfait_core::LayerKind::Raster,
                    parent: None,
                })
                .collect(),
        }
    }
}
