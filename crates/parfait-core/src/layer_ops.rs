use crate::{BlendMode, Document, Layer, PixelRect};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayerCombine {
    TransferDown,
    MergeDown,
    MergeSelected,
    MergeVisible,
    CopyVisible,
    Flatten,
    Clear,
}

impl Document {
    pub fn layer_combine_reason(
        &self,
        action: LayerCombine,
        selection: u64,
    ) -> Option<&'static str> {
        if !self.validate() {
            return Some("The drawing has an invalid layer layout.");
        }
        let all = u64::MAX >> (64 - self.layers.len());
        if selection == 0 || selection & !all != 0 {
            return Some("Select an existing layer first.");
        }
        let Some(mask) = self.combine_mask(action, selection) else {
            return Some(
                if matches!(action, LayerCombine::TransferDown | LayerCombine::MergeDown) {
                    "There is no layer below the active layer."
                } else {
                    "Show a layer first."
                },
            );
        };
        if matches!(
            action,
            LayerCombine::CopyVisible | LayerCombine::MergeVisible | LayerCombine::Flatten
        ) && !self.can_allocate_layers(1)
        {
            return Some("The layer identifier space is exhausted.");
        }
        if matches!(action, LayerCombine::Clear | LayerCombine::TransferDown)
            && self
                .layers
                .iter()
                .enumerate()
                .any(|(index, layer)| mask & (1 << index) != 0 && layer.alpha_locked)
        {
            return Some("Unlock transparent pixels before clearing or transferring these layers.");
        }
        if action == LayerCombine::CopyVisible {
            if self
                .logical_pixel_bytes()
                .saturating_add(u64::from(self.width) * u64::from(self.height) * 4)
                > 512 * 1024 * 1024
            {
                return Some("The visible copy would exceed the 512 MiB logical pixel budget.");
            }
            return (self.layers.len() >= 64).then_some("The drawing already contains 64 layers.");
        }
        if action == LayerCombine::TransferDown
            && self
                .layers
                .iter()
                .enumerate()
                .any(|(index, layer)| mask & (1 << index) != 0 && layer.mask.is_some())
        {
            return Some("Apply or remove the affected masks before transferring pixels.");
        }
        if self
            .layers
            .iter()
            .enumerate()
            .any(|(index, layer)| mask & (1 << index) != 0 && layer.locked)
        {
            return Some("Unlock the affected layers first.");
        }
        if matches!(
            action,
            LayerCombine::TransferDown | LayerCombine::MergeDown | LayerCombine::MergeSelected
        ) {
            if mask.count_ones() < 2 {
                return Some("Select at least two adjacent layers.");
            }
            let first = mask.trailing_zeros();
            let last = 63 - mask.leading_zeros();
            if last - first + 1 != mask.count_ones() {
                return Some("Select a continuous range of layers to merge.");
            }
            if self.layers[first as usize].clipped
                || self
                    .layers
                    .get(last as usize + 1)
                    .is_some_and(|layer| layer.clipped)
            {
                return Some(
                    "Include the complete clipping chain and its base layer before combining.",
                );
            }
            if self.layers.iter().enumerate().any(|(index, layer)| {
                mask & (1 << index) != 0
                    && (!layer.visible || layer.blend_mode != BlendMode::Normal)
            }) {
                return Some(
                    "This operation currently requires visible Normal layers. Other blend modes are preserved by Merge visible or Flatten image.",
                );
            }
        }
        if action == LayerCombine::MergeVisible && mask.count_ones() < 2 {
            return Some("Show at least two layers to merge.");
        }
        None
    }

    fn combine_mask(&self, action: LayerCombine, selection: u64) -> Option<u64> {
        match action {
            LayerCombine::TransferDown | LayerCombine::MergeDown => self
                .active
                .checked_sub(1)
                .map(|below| (1 << below) | (1 << self.active)),
            LayerCombine::MergeSelected | LayerCombine::Clear => Some(selection),
            LayerCombine::Flatten => Some(u64::MAX >> (64 - self.layers.len())),
            LayerCombine::MergeVisible | LayerCombine::CopyVisible => {
                let mask = self
                    .layers
                    .iter()
                    .enumerate()
                    .fold(0, |mask, (index, layer)| {
                        mask | if layer.visible { 1 << index } else { 0 }
                    });
                (mask != 0).then_some(mask)
            }
        }
    }

    pub fn combine_layers(&mut self, action: LayerCombine, selection: u64) -> bool {
        if matches!(action, LayerCombine::TransferDown | LayerCombine::MergeDown)
            && self.active == 0
            || matches!(
                action,
                LayerCombine::CopyVisible | LayerCombine::MergeVisible
            ) && !self.layers.iter().any(|layer| layer.visible)
            || self.layer_combine_reason(action, selection).is_some()
        {
            return false;
        }
        let Some(mask) = self.combine_mask(action, selection) else {
            return false;
        };
        if action == LayerCombine::Clear {
            let mut changed = false;
            for (index, layer) in self.layers.iter_mut().enumerate() {
                if mask & (1 << index) != 0 && layer.pixels.iter().any(|byte| *byte != 0) {
                    Arc::make_mut(&mut layer.pixels).fill(0);
                    changed = true;
                }
            }
            if changed {
                self.touch();
            }
            return changed;
        }
        if action == LayerCombine::TransferDown
            && self.layers[self.active]
                .pixels
                .iter()
                .all(|byte| *byte == 0)
        {
            return false;
        }
        let pixels = self.composite_region_layers(
            PixelRect {
                x0: 0,
                y0: 0,
                x1: self.width,
                y1: self.height,
            },
            mask,
        );
        match action {
            LayerCombine::TransferDown => {
                let target = &mut self.layers[self.active - 1];
                target.pixels = Arc::new(pixels);
                target.opacity = 1.;
                target.blend_mode = BlendMode::Normal;
                Arc::make_mut(&mut self.layers[self.active].pixels).fill(0);
            }
            LayerCombine::MergeDown | LayerCombine::MergeSelected => {
                let bottom = mask.trailing_zeros() as usize;
                let top = 63 - mask.leading_zeros() as usize;
                let target = &mut self.layers[bottom];
                target.pixels = Arc::new(pixels);
                target.opacity = 1.;
                target.blend_mode = BlendMode::Normal;
                target.mask = None;
                self.layers.drain(bottom + 1..=top);
                self.active = bottom;
            }
            LayerCombine::MergeVisible | LayerCombine::CopyVisible | LayerCombine::Flatten => {
                let name = match action {
                    LayerCombine::MergeVisible => "Merged visible",
                    LayerCombine::CopyVisible => "Visible copy",
                    _ => "Flattened image",
                };
                let merged = Layer {
                    id: self.allocate_layer_id().unwrap(),
                    name: name.into(),
                    visible: true,
                    locked: false,
                    alpha_locked: false,
                    clipped: false,
                    opacity: 1.,
                    blend_mode: BlendMode::Normal,
                    pixels: Arc::new(pixels),
                    mask: None,
                };
                if action != LayerCombine::CopyVisible {
                    let mut index = 0;
                    self.layers.retain(|_| {
                        let keep = mask & (1 << index) == 0;
                        index += 1;
                        keep
                    });
                }
                self.layers.push(merged);
                self.active = self.layers.len() - 1;
            }
            LayerCombine::Clear => unreachable!(),
        }
        self.touch();
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Document {
        let mut document = Document::new(12, 9);
        document.layers[0].locked = false;
        for _ in 0..3 {
            document.add_layer();
        }
        for (index, layer) in document.layers.iter_mut().enumerate() {
            layer.opacity = 0.2 + index as f32 * 0.18;
            for (offset, pixel) in Arc::make_mut(&mut layer.pixels)
                .as_chunks_mut::<4>()
                .0
                .iter_mut()
                .enumerate()
            {
                pixel.copy_from_slice(&[
                    (offset * 71 + index * 39) as u8,
                    (offset * 17) as u8,
                    (index * 53) as u8,
                    (offset * 97 + index * 31) as u8,
                ]);
            }
        }
        document
    }

    #[test]
    fn visible_merge_and_flatten_match_all_modes_and_keep_hidden_buffers() {
        for mode in BlendMode::ALL {
            for action in [
                LayerCombine::MergeVisible,
                LayerCombine::Flatten,
                LayerCombine::CopyVisible,
            ] {
                let mut document = sample();
                document.layers[2].visible = false;
                document.layers[3].blend_mode = mode;
                let original = document.clone();
                let expected = document.composite();
                assert!(document.combine_layers(action, 1 << document.active));
                assert!(document.validate());
                if action == LayerCombine::CopyVisible {
                    assert_eq!(*document.layers.last().unwrap().pixels, expected);
                    for (layer, before) in document.layers.iter().zip(&original.layers) {
                        assert!(Arc::ptr_eq(&layer.pixels, &before.pixels));
                    }
                } else {
                    assert_eq!(document.composite(), expected);
                    assert_eq!(
                        document.layers.len(),
                        if action == LayerCombine::Flatten {
                            1
                        } else {
                            2
                        }
                    );
                    if action == LayerCombine::MergeVisible {
                        assert_eq!(document.layers[0], original.layers[2]);
                        assert!(Arc::ptr_eq(
                            &document.layers[0].pixels,
                            &original.layers[2].pixels
                        ));
                    }
                }
            }
        }
    }

    #[test]
    fn adjacent_operations_bake_opacity_keep_names_and_bound_rounding() {
        for action in [
            LayerCombine::MergeDown,
            LayerCombine::TransferDown,
            LayerCombine::MergeSelected,
        ] {
            let mut document = sample();
            document.active = 3;
            let original = document.clone();
            let expected = document.composite();
            assert!(document.combine_layers(action, 0b1100));
            assert!(
                document
                    .composite()
                    .iter()
                    .zip(&expected)
                    .all(|(actual, expected)| actual.abs_diff(*expected) <= 2)
            );
            assert_eq!(document.layers[2].name, original.layers[2].name);
            assert_eq!(document.layers[2].opacity, 1.);
            if action == LayerCombine::TransferDown {
                assert_eq!(document.layers.len(), original.layers.len());
                assert_eq!(document.active, 3);
                assert!(document.layers[3].pixels.iter().all(|byte| *byte == 0));
                assert_eq!(*original.layers[3].pixels, *sample().layers[3].pixels);
            } else {
                assert_eq!(document.layers.len(), original.layers.len() - 1);
                assert_eq!(document.active, 2);
            }
            assert!(Arc::ptr_eq(
                &document.layers[0].pixels,
                &original.layers[0].pixels
            ));
        }
    }

    #[test]
    fn refused_operations_and_empty_clears_are_atomic() {
        for case in 0..7 {
            let mut document = sample();
            let action = if case == 6 {
                LayerCombine::Clear
            } else {
                LayerCombine::MergeSelected
            };
            match case {
                0 => document.layers[2].locked = true,
                1 => document.layers[2].visible = false,
                2 => document.layers[3].blend_mode = BlendMode::Multiply,
                6 => {
                    for layer in &mut document.layers {
                        Arc::make_mut(&mut layer.pixels).fill(0);
                    }
                }
                _ => {}
            }
            let selection = match case {
                3 => 0b1010,
                4 => 0,
                5 => 1 << 63,
                _ => 0b1100,
            };
            let before = document.clone();
            assert!(!document.combine_layers(action, selection));
            assert_eq!(document, before);
            for (layer, before) in document.layers.iter().zip(&before.layers) {
                assert!(Arc::ptr_eq(&layer.pixels, &before.pixels));
            }
        }
        let mut document = Document::new(2, 2);
        document.layers[0].locked = false;
        let before = document.clone();
        assert!(!document.combine_layers(LayerCombine::TransferDown, 2));
        assert_eq!(document, before);
        document.active = 0;
        assert!(!document.combine_layers(LayerCombine::MergeDown, 1));
        document.layers[0].locked = false;
        while document.layers.len() < 64 {
            document.add_layer();
        }
        assert!(
            document
                .layer_combine_reason(LayerCombine::CopyVisible, 1 << 63)
                .is_some()
        );
        assert!(document.combine_layers(LayerCombine::MergeSelected, u64::MAX));
        assert_eq!(document.layers.len(), 1);
        assert!(document.validate());
    }
}
