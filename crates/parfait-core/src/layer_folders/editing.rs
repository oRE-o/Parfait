use crate::{Document, LayerId};

impl Document {
    pub fn layer_folder_creation_reason(&self, selection: Option<u64>) -> Option<&'static str> {
        if !self.validate() {
            return Some("The drawing has an invalid layer tree.");
        }
        if self.layers.len() >= 64 || !self.can_allocate_layers(1) {
            return Some("The drawing cannot contain another layer node.");
        }
        let parent = if let Some(selection) = selection {
            let Some(roots) = self.layer_selection_roots(selection) else {
                return Some("Select existing layers first.");
            };
            if self
                .layers
                .iter()
                .enumerate()
                .any(|(index, layer)| roots & (1 << index) != 0 && layer.kind.is_paper())
            {
                return Some("Paper must stay outside folders at the bottom of the drawing.");
            }
            let parent = self.layers[roots.trailing_zeros() as usize].parent;
            if self
                .layers
                .iter()
                .enumerate()
                .any(|(index, layer)| roots & (1 << index) != 0 && layer.parent != parent)
            {
                return Some("Select sibling layers to group them.");
            }
            parent
        } else {
            let active = &self.layers[self.active];
            if active.kind == crate::LayerKind::Folder {
                Some(active.id)
            } else {
                active.parent
            }
        };
        self.folder_edit_reason(parent)
    }

    pub fn layer_selection_roots(&self, mut selection: u64) -> Option<u64> {
        if self.layers.is_empty()
            || selection == 0
            || (self.layers.len() < 64 && selection >> self.layers.len() != 0)
        {
            return None;
        }
        let layout = self.folder_layout()?;
        for index in 0..self.layers.len() {
            let mut ancestor = layout.parents[index];
            while ancestor != 64 {
                if selection & (1 << ancestor) != 0 {
                    selection &= !(1 << index);
                    break;
                }
                ancestor = layout.parents[ancestor];
            }
        }
        Some(selection)
    }

    pub fn layer_selection_subtrees(&self, selection: u64) -> Option<u64> {
        let roots = self.layer_selection_roots(selection)?;
        let mut result = 0;
        for index in 0..self.layers.len() {
            if roots & (1 << index) != 0 {
                result |= self.layer_subtree_mask(index)?;
            }
        }
        Some(result)
    }

    pub fn layer_reparent_reason(
        &self,
        selection: u64,
        boundary: usize,
        parent: Option<LayerId>,
    ) -> Option<&'static str> {
        if !self.validate() {
            return Some("The drawing has an invalid layer tree.");
        }
        let Some(roots) = self.layer_selection_roots(selection) else {
            return Some("Select existing layers first.");
        };
        let expanded = self.layer_selection_subtrees(roots).unwrap();
        if self
            .layers
            .iter()
            .enumerate()
            .any(|(index, layer)| expanded & (1 << index) != 0 && layer.kind.is_paper())
            || (boundary == 0 && self.layers[0].kind.is_paper())
        {
            return Some("Paper must stay at the bottom of the drawing.");
        }
        if let Some(reason) = self.folder_edit_reason(parent) {
            return Some(reason);
        }
        let parent_index = parent.and_then(|id| self.layer_index(id)).unwrap_or(64);
        if parent_index != 64 && expanded & (1 << parent_index) != 0 {
            return Some("A folder cannot be placed inside itself.");
        }
        for (index, layer) in self.layers.iter().enumerate() {
            if roots & (1 << index) != 0
                && let Some(reason) = self.folder_edit_reason(layer.parent)
            {
                return Some(reason);
            }
        }
        let layout = self.folder_layout().unwrap();
        let top = if parent_index == 64 {
            self.layers.len()
        } else {
            parent_index
        };
        if boundary != top
            && !self.layers.iter().enumerate().any(|(index, _)| {
                layout.parents[index] == parent_index && layout.starts[index] == boundary
            })
        {
            return Some("Drop at a sibling boundary or inside a folder.");
        }
        None
    }

    pub fn move_layer_subtrees(
        &mut self,
        selection: u64,
        boundary: usize,
        parent: Option<LayerId>,
    ) -> bool {
        if self
            .layer_reparent_reason(selection, boundary, parent)
            .is_some()
        {
            return false;
        }
        let roots = self.layer_selection_roots(selection).unwrap();
        let expanded = self.layer_selection_subtrees(roots).unwrap();
        let before = self.clone();
        let active = self.layers[self.active].id;
        let insertion = (0..boundary)
            .filter(|index| expanded & (1 << index) == 0)
            .count();
        let mut kept = Vec::with_capacity(self.layers.len());
        let mut moved = Vec::new();
        for (index, mut layer) in self.layers.drain(..).enumerate() {
            if expanded & (1 << index) != 0 {
                if roots & (1 << index) != 0 {
                    layer.parent = parent;
                }
                moved.push(layer);
            } else {
                kept.push(layer);
            }
        }
        kept.splice(insertion..insertion, moved);
        self.layers = kept;
        self.active = self.layer_index(active).unwrap();
        if !self.validate() || self.same_edit_state(&before) {
            *self = before;
            return false;
        }
        self.touch();
        true
    }

    pub fn layer_tree_delete_reason(&self, selection: u64) -> Option<&'static str> {
        if !self.validate() {
            return Some("The drawing has an invalid layer tree.");
        }
        let Some(expanded) = self.layer_selection_subtrees(selection) else {
            return Some("Select existing layers first.");
        };
        if expanded.count_ones() as usize >= self.layers.len() {
            return Some("Keep at least one layer in the drawing.");
        }
        if self
            .layers
            .iter()
            .enumerate()
            .any(|(index, _)| expanded & (1 << index) != 0 && !self.layer_ancestors_unlocked(index))
        {
            return Some("Unlock the selected layers and their folders before deleting them.");
        }
        None
    }

    pub fn delete_layer_subtrees(&mut self, selection: u64) -> bool {
        if self.layer_tree_delete_reason(selection).is_some() {
            return false;
        }
        let expanded = self.layer_selection_subtrees(selection).unwrap();
        let active = self.layers[self.active].id;
        let fallback = expanded.trailing_zeros() as usize;
        let mut index = 0;
        self.layers.retain(|_| {
            let keep = expanded & (1 << index) == 0;
            index += 1;
            keep
        });
        self.active = self
            .layer_index(active)
            .unwrap_or(fallback.min(self.layers.len() - 1));
        self.touch();
        true
    }

    pub fn layer_tree_duplication_reason(&self, selection: u64) -> Option<&'static str> {
        if !self.validate() {
            return Some("The drawing has an invalid layer tree.");
        }
        let Some(expanded) = self.layer_selection_subtrees(selection) else {
            return Some("Select existing layers first.");
        };
        let count = expanded.count_ones() as usize;
        if self
            .layers
            .iter()
            .enumerate()
            .any(|(index, layer)| expanded & (1 << index) != 0 && layer.kind.is_paper())
        {
            return Some("Paper cannot be duplicated.");
        }
        if self.layers.len() + count > 64 || !self.can_allocate_layers(count) {
            return Some("The duplicated subtrees would exceed the layer or identifier limit.");
        }
        let roots = self.layer_selection_roots(selection).unwrap();
        for (index, layer) in self.layers.iter().enumerate() {
            if roots & (1 << index) != 0
                && let Some(reason) = self.folder_edit_reason(layer.parent)
            {
                return Some(reason);
            }
        }
        let pixels = u64::from(self.width) * u64::from(self.height);
        let added: u64 = self
            .layers
            .iter()
            .enumerate()
            .filter(|(index, _)| expanded & (1 << index) != 0)
            .map(|(_, layer)| {
                pixels
                    * (4 * u64::from(layer.kind == crate::LayerKind::Raster)
                        + u64::from(layer.mask.is_some()))
            })
            .sum();
        (self.logical_pixel_bytes().saturating_add(added) > 512 * 1024 * 1024)
            .then_some("Duplicating these subtrees would exceed the 512 MiB logical pixel budget.")
    }

    pub fn duplicate_layer_subtrees(&mut self, selection: u64) -> Option<u64> {
        if self.layer_tree_duplication_reason(selection).is_some() {
            return None;
        }
        let roots = self.layer_selection_roots(selection)?;
        let expanded = self.layer_selection_subtrees(roots)?;
        let layout = self.folder_layout()?;
        let mut identities = [LayerId(0); 64];
        for (index, identity) in identities.iter_mut().enumerate().take(self.layers.len()) {
            if expanded & (1 << index) != 0 {
                *identity = self.allocate_layer_id().unwrap();
            }
        }
        let mut output = Vec::with_capacity(self.layers.len() + expanded.count_ones() as usize);
        let mut copied = 0u64;
        for (index, layer) in self.layers.iter().enumerate() {
            output.push(layer.clone());
            if roots & (1 << index) != 0 {
                for child in layout.starts[index]..=index {
                    let mut copy = self.layers[child].clone();
                    copy.id = identities[child];
                    if roots & (1 << child) != 0 {
                        copy.name.push_str(" copy");
                    }
                    if let Some(parent) = copy.parent.and_then(|id| self.layer_index(id))
                        && identities[parent].0 != 0
                    {
                        copy.parent = Some(identities[parent]);
                    }
                    output.push(copy);
                }
                copied |= 1 << (output.len() - 1);
            }
        }
        self.layers = output;
        self.active = 63 - copied.leading_zeros() as usize;
        self.touch();
        Some(copied)
    }

    pub fn layer_sibling_order(
        &self,
        selection: u64,
        upward: bool,
        edge: bool,
    ) -> Option<Vec<usize>> {
        if !self.validate() {
            return None;
        }
        let roots = self.layer_selection_roots(selection)?;
        let first = roots.trailing_zeros() as usize;
        let parent = self.layers[first].parent;
        if self
            .layers
            .iter()
            .enumerate()
            .any(|(index, layer)| roots & (1 << index) != 0 && layer.kind.is_paper())
        {
            return None;
        }
        if self.folder_edit_reason(parent).is_some()
            || self
                .layers
                .iter()
                .enumerate()
                .any(|(index, layer)| roots & (1 << index) != 0 && layer.parent != parent)
        {
            return None;
        }
        let layout = self.folder_layout()?;
        let parent_index = parent.and_then(|id| self.layer_index(id)).unwrap_or(64);
        let siblings: Vec<_> = (0..self.layers.len())
            .filter(|index| {
                layout.parents[*index] == parent_index && !self.layers[*index].kind.is_paper()
            })
            .collect();
        let mut reordered = siblings.clone();
        let selected = |index: usize| roots & (1 << index) != 0;
        if edge {
            reordered.sort_by_key(|index| selected(*index) == upward);
        } else if upward {
            for index in (0..reordered.len().saturating_sub(1)).rev() {
                if selected(reordered[index]) && !selected(reordered[index + 1]) {
                    reordered.swap(index, index + 1);
                }
            }
        } else {
            for index in 1..reordered.len() {
                if selected(reordered[index]) && !selected(reordered[index - 1]) {
                    reordered.swap(index, index - 1);
                }
            }
        }
        if reordered == siblings {
            return None;
        }
        let start = if parent_index == 64 {
            usize::from(self.layers[0].kind.is_paper())
        } else {
            layout.starts[parent_index]
        };
        let end = if parent_index == 64 {
            self.layers.len()
        } else {
            parent_index
        };
        let mut order: Vec<_> = (0..start).collect();
        for sibling in reordered {
            order.extend(layout.starts[sibling]..=sibling);
        }
        order.extend(end..self.layers.len());
        Some(order)
    }
}
