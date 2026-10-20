//! One window-owned cache, never serialized or retained per inactive document.
use crate::{document_session::SessionKey, pixel_selection::BoundaryCache};
use eframe::egui;
use parfait_core::LayerId;

pub(crate) struct WindowRenderCache {
    pub key: Option<SessionKey>,
    pub layer_previews: Vec<egui::TextureHandle>,
    pub mask_previews: Vec<Option<egui::TextureHandle>>,
    pub rendered_overlay: Option<LayerId>,
    pub preview_revision: u64,
    pub selection_boundary: BoundaryCache,
    pub texture: Option<egui::TextureHandle>,
    pub rendered: u64,
}

impl Default for WindowRenderCache {
    fn default() -> Self {
        Self {
            key: None,
            layer_previews: Vec::new(),
            mask_previews: Vec::new(),
            rendered_overlay: None,
            preview_revision: u64::MAX,
            selection_boundary: BoundaryCache::default(),
            texture: None,
            rendered: 0,
        }
    }
}

impl WindowRenderCache {
    pub fn ensure_session(&mut self, key: SessionKey) {
        if self.key != Some(key) {
            *self = Self { key: Some(key), ..Self::default() };
        }
    }
}
