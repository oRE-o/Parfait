use crate::{Document, Layer};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub(crate) mod paint;

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub enum MaskCoverage {
    Uniform(u8),
    Dense(Arc<Vec<u8>>),
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub struct LayerMask {
    pub enabled: bool,
    pub coverage: MaskCoverage,
}

impl MaskCoverage {
    pub fn value(&self, index: usize) -> u8 {
        match self {
            Self::Uniform(value) => *value,
            Self::Dense(values) => values[index],
        }
    }

    pub(crate) fn same_edit_state(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Uniform(current), Self::Uniform(previous)) => current == previous,
            (Self::Dense(current), Self::Dense(previous)) => Arc::ptr_eq(current, previous),
            _ => false,
        }
    }
}

pub(crate) fn masked_alpha(alpha: u8, coverage: u8) -> u8 {
    ((u16::from(alpha) * u16::from(coverage) + 127) / 255) as u8
}

impl Layer {
    pub(crate) fn effective_alpha(&self, index: usize) -> u8 {
        let alpha = self.pixels[index * 4 + 3];
        self.mask_alpha(index, alpha)
    }

    pub(crate) fn mask_alpha(&self, index: usize, alpha: u8) -> u8 {
        match self.mask.as_ref().filter(|mask| mask.enabled) {
            Some(mask) => masked_alpha(alpha, mask.coverage.value(index)),
            None => alpha,
        }
    }

    pub(crate) fn has_effective_mask(&self) -> bool {
        self.mask.as_ref().is_some_and(|mask| {
            mask.enabled && !matches!(mask.coverage, MaskCoverage::Uniform(255))
        })
    }
}

impl Document {
    pub fn logical_pixel_bytes(&self) -> u64 {
        let pixels = u64::from(self.width) * u64::from(self.height);
        self.layers.iter().fold(0, |bytes, layer| {
            bytes.saturating_add(pixels.saturating_mul(4 + u64::from(layer.mask.is_some())))
        })
    }

    pub fn layer_mask_creation_reason(&self, layer: usize) -> Option<&'static str> {
        if !self.validate() {
            return Some("The drawing has an invalid layer layout.");
        }
        let Some(target) = self.layers.get(layer) else {
            return Some("The target layer no longer exists.");
        };
        if target.locked {
            return Some("Unlock the layer before creating a mask.");
        }
        if target.mask.is_some() {
            return Some("This layer already has a mask.");
        }
        let pixels = u64::from(self.width) * u64::from(self.height);
        (self.logical_pixel_bytes().saturating_add(pixels) > 512 * 1024 * 1024)
            .then_some("The mask would exceed the 512 MiB logical pixel budget.")
    }

    pub fn create_layer_mask(&mut self, layer: usize, coverage: u8) -> bool {
        if self.layer_mask_creation_reason(layer).is_some() {
            return false;
        }
        self.layers[layer].mask = Some(LayerMask {
            enabled: true,
            coverage: MaskCoverage::Uniform(coverage),
        });
        self.touch();
        true
    }

    pub fn remove_layer_mask(&mut self, layer: usize) -> bool {
        let Some(target) = self.layers.get_mut(layer).filter(|layer| !layer.locked) else {
            return false;
        };
        if target.mask.take().is_none() {
            return false;
        }
        self.touch();
        true
    }

    pub fn set_layer_mask_enabled(&mut self, layer: usize, enabled: bool) -> bool {
        let Some(mask) = self
            .layers
            .get_mut(layer)
            .and_then(|layer| layer.mask.as_mut())
        else {
            return false;
        };
        if mask.enabled == enabled {
            return false;
        }
        mask.enabled = enabled;
        self.touch();
        true
    }

    pub fn apply_layer_mask(&mut self, layer: usize) -> bool {
        if !self.validate() {
            return false;
        }
        let Some(target) = self
            .layers
            .get_mut(layer)
            .filter(|layer| !layer.locked && !layer.alpha_locked)
        else {
            return false;
        };
        let Some(mask) = target.mask.take() else {
            return false;
        };
        if mask.enabled && !matches!(mask.coverage, MaskCoverage::Uniform(255)) {
            let changes_alpha =
                target
                    .pixels
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .enumerate()
                    .any(|(index, pixel)| {
                        masked_alpha(pixel[3], mask.coverage.value(index)) != pixel[3]
                    });
            if changes_alpha {
                for (index, pixel) in Arc::make_mut(&mut target.pixels)
                    .as_chunks_mut::<4>()
                    .0
                    .iter_mut()
                    .enumerate()
                {
                    pixel[3] = masked_alpha(pixel[3], mask.coverage.value(index));
                }
            }
        }
        self.touch();
        true
    }
}

#[cfg(test)]
mod tests;
