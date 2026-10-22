use parfait_core::{
    BlendMode, Document, Layer, LayerId, LayerKind, LayerMask, MaskCoverage,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Serialize, Deserialize)]
pub(crate) enum LegacyKind {
    Raster,
    Folder,
}

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
    #[serde(deserialize_with = "parfait_core::deserialize_pixel_buffer")]
    pub pixels: Arc<Vec<u8>>,
    pub mask: Option<LayerMask>,
    pub reference: bool,
    pub draft: bool,
    pub kind: LegacyKind,
    pub parent: Option<LayerId>,
}

impl From<LegacyLayer> for Layer {
    fn from(old: LegacyLayer) -> Self {
        Self {
            id: old.id,
            name: old.name,
            visible: old.visible,
            locked: old.locked,
            alpha_locked: old.alpha_locked,
            clipped: old.clipped,
            opacity: old.opacity,
            blend_mode: old.blend_mode,
            pixels: old.pixels,
            mask: old.mask,
            reference: old.reference,
            draft: old.draft,
            parent: old.parent,
            kind: match old.kind {
                LegacyKind::Raster => LayerKind::Raster,
                LegacyKind::Folder => LayerKind::Folder,
            },
        }
    }
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
    pub selection: Option<MaskCoverage>,
}

impl From<LegacyDocument> for Document {
    fn from(old: LegacyDocument) -> Self {
        Self {
            contents: Vec::new(),
            layer_colors: Vec::new(),
            width: old.width,
            height: old.height,
            name: old.name,
            layers: old.layers.into_iter().map(Into::into).collect(),
            active: old.active,
            revision: old.revision,
            next_layer_id: old.next_layer_id,
            selection: old.selection,
        }
    }
}
