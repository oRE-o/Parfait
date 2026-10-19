use crate::{Document, LayerKind, LayerMask, MaskCoverage, layer_mask::masked_alpha};
use std::sync::Arc;

#[inline]
pub(crate) fn strength(selection: Option<&MaskCoverage>, index: usize, amount: f32) -> f32 {
    match selection {
        None => amount,
        Some(coverage) => amount * (f32::from(coverage.value(index)) / 255.),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectionMode {
    Replace,
    Add,
    Subtract,
    Intersect,
}

impl SelectionMode {
    pub fn combine(self, previous: u8, source: u8) -> u8 {
        match self {
            Self::Replace => source,
            Self::Add => previous + masked_alpha(255 - previous, source),
            Self::Subtract => masked_alpha(previous, 255 - source),
            Self::Intersect => masked_alpha(previous, source),
        }
    }
}

fn canonical(values: Vec<u8>) -> MaskCoverage {
    let first = values[0];
    if values.iter().all(|value| *value == first) {
        MaskCoverage::Uniform(first)
    } else {
        MaskCoverage::Dense(Arc::new(values))
    }
}

pub(crate) fn interpolate_pixel(before: [u8; 4], after: [u8; 4], coverage: u8) -> [u8; 4] {
    if before == after {
        return before;
    }
    if coverage == 0 {
        return before;
    }
    if coverage == 255 {
        return after;
    }
    let previous_weight = u32::from(before[3]) * u32::from(255 - coverage);
    let current_weight = u32::from(after[3]) * u32::from(coverage);
    let weight = previous_weight + current_weight;
    if weight == 0 {
        return [0; 4];
    }
    let mut result = [0; 4];
    for channel in 0..3 {
        result[channel] = ((u32::from(before[channel]) * previous_weight
            + u32::from(after[channel]) * current_weight
            + weight / 2)
            / weight) as u8;
    }
    result[3] = ((weight + 127) / 255) as u8;
    result
}

pub(crate) fn clear_pixels(pixels: &mut Arc<Vec<u8>>, selection: Option<&MaskCoverage>) -> bool {
    let Some(selection) = selection else {
        if pixels.iter().all(|byte| *byte == 0) {
            return false;
        }
        Arc::make_mut(pixels).fill(0);
        return true;
    };
    let Some(first) = pixels
        .as_chunks::<4>()
        .0
        .iter()
        .enumerate()
        .position(|(index, pixel)| {
            interpolate_pixel(*pixel, [0; 4], selection.value(index)) != *pixel
        })
    else {
        return false;
    };
    for (offset, pixel) in Arc::make_mut(pixels).as_chunks_mut::<4>().0[first..]
        .iter_mut()
        .enumerate()
    {
        *pixel = interpolate_pixel(*pixel, [0; 4], selection.value(first + offset));
    }
    true
}

impl Document {
    pub fn selection_value(&self, index: usize) -> u8 {
        self.selection
            .as_ref()
            .map_or(255, |coverage| coverage.value(index))
    }

    pub fn selection_creation_reason(&self) -> Option<&'static str> {
        if !self.validate() {
            return Some("The drawing has invalid image data.");
        }
        (self.selection.is_none()
            && self.logical_pixel_bytes() + u64::from(self.width) * u64::from(self.height)
                > 512 * 1024 * 1024)
            .then_some("The selection would exceed the 512 MiB logical pixel budget.")
    }

    pub fn set_selection(&mut self, coverage: Option<MaskCoverage>) -> bool {
        if !self.validate() || (self.selection_creation_reason().is_some() && coverage.is_some()) {
            return false;
        }
        let count = (self.width * self.height) as usize;
        if coverage.as_ref().is_some_and(
            |coverage| matches!(coverage, MaskCoverage::Dense(values) if values.len() != count),
        ) {
            return false;
        }
        let unchanged = match (&self.selection, &coverage) {
            (None, None) => true,
            (Some(before), Some(after)) => {
                before.same_edit_state(after)
                    || (0..count).all(|index| before.value(index) == after.value(index))
            }
            _ => false,
        };
        if unchanged {
            return false;
        }
        self.selection = coverage;
        self.touch();
        true
    }

    pub fn select_all_pixels(&mut self) -> bool {
        self.set_selection(Some(MaskCoverage::Uniform(255)))
    }

    pub fn deselect_pixels(&mut self) -> bool {
        self.set_selection(None)
    }

    pub fn invert_pixel_selection(&mut self) -> bool {
        if !self.validate() {
            return false;
        }
        let inverted = match &self.selection {
            None => MaskCoverage::Uniform(255),
            Some(MaskCoverage::Uniform(value)) => MaskCoverage::Uniform(255 - value),
            Some(MaskCoverage::Dense(values)) => {
                canonical(values.iter().map(|value| 255 - value).collect())
            }
        };
        self.set_selection(if matches!(inverted, MaskCoverage::Uniform(0)) {
            None
        } else {
            Some(inverted)
        })
    }

    pub fn selection_from_layers(&mut self, selected: u64, mode: SelectionMode) -> bool {
        if self.selection_creation_reason().is_some()
            || selected == 0
            || (self.layers.len() < 64 && selected >> self.layers.len() != 0)
            || (mode != SelectionMode::Replace && self.selection.is_none())
        {
            return false;
        }
        let layout = self.folder_layout().unwrap();
        let mut roots = selected;
        for index in 0..self.layers.len() {
            let mut parent = layout.parents[index];
            while parent != 64 {
                if selected & (1 << parent) != 0 {
                    roots &= !(1 << index);
                    break;
                }
                parent = layout.parents[parent];
            }
        }
        let width = self.width as usize;
        let mut folders: Vec<Vec<u8>> = self
            .layers
            .iter()
            .map(|layer| {
                if layer.kind == LayerKind::Folder {
                    vec![0; width]
                } else {
                    Vec::new()
                }
            })
            .collect();
        let mut result = vec![0; width * self.height as usize];
        for (row, output) in result.chunks_exact_mut(width).enumerate() {
            for (index, layer) in self.layers.iter().enumerate() {
                if layer.kind != LayerKind::Folder {
                    continue;
                }
                let (children, current) = folders.split_at_mut(index);
                current[0].fill(0);
                for (child, layer) in self.layers[..index].iter().enumerate() {
                    if layout.parents[child] != index || !layer.visible {
                        continue;
                    }
                    for (column, target) in current[0].iter_mut().enumerate() {
                        let pixel = row * width + column;
                        let alpha = if layer.kind == LayerKind::Folder {
                            children[child][column]
                        } else {
                            layer.pixels[pixel * 4 + 3]
                        };
                        *target =
                            SelectionMode::Add.combine(*target, layer.mask_alpha(pixel, alpha));
                    }
                }
            }
            for (column, target) in output.iter_mut().enumerate() {
                let pixel = row * width + column;
                let mut alpha = 0;
                for (index, layer) in self.layers.iter().enumerate() {
                    if roots & (1 << index) == 0 {
                        continue;
                    }
                    let source = if layer.kind == LayerKind::Folder {
                        folders[index][column]
                    } else {
                        layer.pixels[pixel * 4 + 3]
                    };
                    alpha = SelectionMode::Add.combine(alpha, layer.mask_alpha(pixel, source));
                }
                *target = mode.combine(self.selection_value(pixel), alpha);
            }
        }
        let result = canonical(result);
        self.set_selection(if matches!(result, MaskCoverage::Uniform(0)) {
            None
        } else {
            Some(result)
        })
    }

    pub fn create_mask_from_selection(&mut self, layer: usize, outside: bool) -> bool {
        if self.layer_mask_creation_reason(layer).is_some() {
            return false;
        }
        let Some(selection) = &self.selection else {
            return self.create_layer_mask(layer, 255);
        };
        let coverage = if outside {
            selection.clone()
        } else {
            match selection {
                MaskCoverage::Uniform(value) => MaskCoverage::Uniform(255 - value),
                MaskCoverage::Dense(values) => {
                    canonical(values.iter().map(|value| 255 - value).collect())
                }
            }
        };
        self.layers[layer].mask = Some(LayerMask {
            enabled: true,
            coverage,
        });
        self.touch();
        true
    }

    pub(crate) fn restore_mask_selection(&mut self, layer: usize) -> Option<bool> {
        let selection = self.selection.as_ref()?;
        let mask = self.layers.get_mut(layer)?.mask.as_mut()?;
        let values: Vec<_> = (0..(self.width * self.height) as usize)
            .map(|index| {
                SelectionMode::Add.combine(mask.coverage.value(index), selection.value(index))
            })
            .collect();
        if values
            .iter()
            .enumerate()
            .all(|(index, value)| *value == mask.coverage.value(index))
        {
            return Some(false);
        }
        mask.coverage = canonical(values);
        self.touch();
        Some(true)
    }
}

#[cfg(test)]
mod tests;
