use crate::{BlendMode, Document, Layer, LayerId, LayerKind, LayerMask, MaskCoverage};
use std::sync::Arc;

impl LayerKind {
    pub fn solid_color(self) -> Option<[u8; 3]> {
        match self {
            Self::Fill(color) | Self::Paper(color) => Some(color),
            _ => None,
        }
    }

    pub fn is_paper(self) -> bool {
        matches!(self, Self::Paper(_))
    }
}

impl Layer {
    pub fn source_pixel(&self, index: usize) -> [u8; 4] {
        if let Some([red, green, blue]) = self.kind.solid_color() {
            [red, green, blue, 255]
        } else {
            self.pixels
                .get(index.saturating_mul(4)..)
                .and_then(|values| values.get(..4))
                .map_or([0; 4], |pixel| pixel.try_into().unwrap())
        }
    }
}

impl Document {
    pub fn new_with_paper(width: u32, height: u32) -> Self {
        Self::new_background(width, height, true)
    }

    pub fn solid_layer_creation_reason(&self, paper: bool, name: &str) -> Option<&'static str> {
        if !self.validate() {
            return Some("The drawing has invalid image data.");
        }
        if name.trim().is_empty()
            || name.chars().count() > 128
            || name.chars().any(char::is_control)
        {
            return Some("Use a nonempty layer name of at most 128 characters.");
        }
        if self.layers.len() >= 64 || !self.can_allocate_layers(1) {
            return Some("The drawing cannot contain another layer node.");
        }
        if paper {
            return self
                .layers
                .iter()
                .any(|layer| layer.kind.is_paper())
                .then_some("The drawing already has a paper layer.");
        }
        let active = &self.layers[self.active];
        let parent = if active.kind == LayerKind::Folder {
            Some(active.id)
        } else {
            active.parent
        };
        if let Some(reason) = self.folder_edit_reason(parent) {
            return Some(reason);
        }
        (self.logical_pixel_bytes() + u64::from(self.width) * u64::from(self.height)
            > 512 * 1024 * 1024)
            .then_some("The fill mask would exceed the 512 MiB logical pixel budget.")
    }

    pub fn create_solid_layer(
        &mut self,
        color: [u8; 3],
        paper: bool,
        name: String,
    ) -> Option<LayerId> {
        if self.solid_layer_creation_reason(paper, &name).is_some() {
            return None;
        }
        let active = &self.layers[self.active];
        let parent = if paper {
            None
        } else if active.kind == LayerKind::Folder {
            Some(active.id)
        } else {
            active.parent
        };
        let position = if paper {
            0
        } else {
            self.active + usize::from(active.kind != LayerKind::Folder)
        };
        let mask = (!paper).then(|| LayerMask {
            enabled: true,
            coverage: self.selection.clone().unwrap_or(MaskCoverage::Uniform(255)),
        });
        let id = self.allocate_layer_id()?;
        self.layers.insert(
            position,
            Layer {
                id,
                name,
                visible: true,
                locked: false,
                alpha_locked: false,
                clipped: false,
                opacity: 1.,
                blend_mode: BlendMode::Normal,
                pixels: Arc::new(Vec::new()),
                mask,
                reference: false,
                draft: false,
                kind: if paper {
                    LayerKind::Paper(color)
                } else {
                    LayerKind::Fill(color)
                },
                parent,
            },
        );
        self.active = position;
        self.touch();
        Some(id)
    }

    pub fn set_solid_layer_color(&mut self, index: usize, color: [u8; 3]) -> bool {
        if !self.validate() || !self.layer_ancestors_unlocked(index) {
            return false;
        }
        let Some(layer) = self.layers.get_mut(index) else {
            return false;
        };
        if layer
            .kind
            .solid_color()
            .is_none_or(|before| before == color)
        {
            return false;
        }
        layer.kind = if layer.kind.is_paper() {
            LayerKind::Paper(color)
        } else {
            LayerKind::Fill(color)
        };
        self.touch();
        true
    }

    pub fn solid_rasterization_reason(&self, index: usize) -> Option<&'static str> {
        if !self.validate() {
            return Some("The drawing has invalid image data.");
        }
        if self
            .layers
            .get(index)
            .is_none_or(|layer| layer.kind.solid_color().is_none())
        {
            return Some("Select a fill or paper layer to rasterize.");
        }
        if !self.layer_ancestors_unlocked(index) {
            return Some("Unlock the layer and its parent folders before rasterizing.");
        }
        (self.logical_pixel_bytes() + u64::from(self.width) * u64::from(self.height) * 4
            > 512 * 1024 * 1024)
            .then_some("Rasterizing would exceed the 512 MiB logical pixel budget.")
    }

    pub fn rasterize_solid_layer(&mut self, index: usize) -> bool {
        if self.solid_rasterization_reason(index).is_some() {
            return false;
        }
        let layer = &mut self.layers[index];
        let [red, green, blue] = layer.kind.solid_color().unwrap();
        layer.pixels =
            Arc::new([red, green, blue, 255].repeat((self.width * self.height) as usize));
        layer.kind = LayerKind::Raster;
        self.touch();
        true
    }
}

#[cfg(test)]
mod tests;
