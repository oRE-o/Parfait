use crate::{BlendMode, Document, Layer, LayerId};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

mod render;
#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LayerKind {
    #[default]
    Raster,
    Folder,
}

pub(crate) struct FolderLayout {
    pub parents: [usize; 64],
    pub starts: [usize; 64],
}

impl Document {
    pub fn has_folders(&self) -> bool {
        self.layers
            .iter()
            .any(|layer| layer.kind == LayerKind::Folder)
    }

    pub(crate) fn folder_layout(&self) -> Option<FolderLayout> {
        if self.layers.len() > 64 {
            return None;
        }
        let mut layout = FolderLayout {
            parents: [64; 64],
            starts: std::array::from_fn(|index| index),
        };
        let mut sizes = [1; 64];
        for (index, layer) in self.layers.iter().enumerate() {
            if let Some(parent) = layer.parent {
                let parent = self.layer_index(parent)?;
                if parent <= index || self.layers[parent].kind != LayerKind::Folder {
                    return None;
                }
                layout.parents[index] = parent;
                layout.starts[parent] = layout.starts[parent].min(layout.starts[index]);
                sizes[parent] += sizes[index];
            }
        }
        for (index, layer) in self.layers.iter().enumerate() {
            if sizes[index] != index - layout.starts[index] + 1
                || (layer.kind == LayerKind::Raster && sizes[index] != 1)
            {
                return None;
            }
        }
        Some(layout)
    }

    pub fn layer_subtree_mask(&self, index: usize) -> Option<u64> {
        self.layers.get(index)?;
        let layout = self.folder_layout()?;
        let start = layout.starts[index];
        Some((u64::MAX >> (63 - index + start)) << start)
    }

    pub fn layer_ancestors_allow_edit(&self, index: usize) -> bool {
        self.layer_path_allows_edit(index, true)
    }

    pub fn layer_ancestors_unlocked(&self, index: usize) -> bool {
        self.layer_path_allows_edit(index, false)
    }

    fn layer_path_allows_edit(&self, index: usize, require_visible: bool) -> bool {
        let Some(mut layer) = self.layers.get(index) else {
            return false;
        };
        for _ in 0..=self.layers.len() {
            if layer.locked || (require_visible && !layer.visible) {
                return false;
            }
            let Some(parent) = layer.parent else {
                return true;
            };
            let Some(parent) = self.layer_index(parent) else {
                return false;
            };
            layer = &self.layers[parent];
        }
        false
    }

    pub(crate) fn folder_edit_reason(&self, parent: Option<LayerId>) -> Option<&'static str> {
        if !self.validate() {
            return Some("The drawing has an invalid layer tree.");
        }
        if let Some(parent) = parent {
            let Some(index) = self.layer_index(parent) else {
                return Some("The parent folder no longer exists.");
            };
            if self.layers[index].kind != LayerKind::Folder {
                return Some("The parent is not a folder.");
            }
            if !self.layer_ancestors_unlocked(index) {
                return Some("Unlock the parent folder first.");
            }
        }
        None
    }

    pub fn create_layer_folder(&mut self, name: String) -> Option<LayerId> {
        let active = self.layers.get(self.active)?;
        let parent = if active.kind == LayerKind::Folder {
            Some(active.id)
        } else {
            active.parent
        };
        let position = self.active + usize::from(active.kind != LayerKind::Folder);
        self.insert_layer_folder(name, parent, position)
    }

    fn insert_layer_folder(
        &mut self,
        name: String,
        parent: Option<LayerId>,
        position: usize,
    ) -> Option<LayerId> {
        if self.folder_edit_reason(parent).is_some()
            || self.layers.len() >= 64
            || name.trim().is_empty()
            || name.chars().any(char::is_control)
        {
            return None;
        }
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
                mask: None,
                reference: false,
                draft: false,
                kind: LayerKind::Folder,
                parent,
            },
        );
        self.active = position;
        self.touch();
        Some(id)
    }

    pub fn group_layers_in_folder(&mut self, mut selection: u64, name: String) -> Option<LayerId> {
        if self.layers.is_empty()
            || self.layers.len() >= 64
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
        let first = selection.trailing_zeros() as usize;
        let parent = self.layers.get(first)?.parent;
        if self.folder_edit_reason(parent).is_some() {
            return None;
        }
        let mut selected = 0;
        for (index, layer) in self.layers.iter().enumerate() {
            if selection & (1 << index) != 0 {
                if layer.parent != parent {
                    return None;
                }
                selected |= self.layer_subtree_mask(index)?;
            }
        }
        let highest = 63 - selected.leading_zeros() as usize;
        let insertion = (0..=highest)
            .filter(|index| selected & (1 << index) == 0)
            .count();
        let before = self.clone();
        let id = self.insert_layer_folder(name, parent, self.layers.len())?;
        let folder = self.layers.pop().unwrap();
        let mut remaining = Vec::with_capacity(self.layers.len() + 1);
        let mut children = Vec::new();
        for (index, mut layer) in self.layers.drain(..).enumerate() {
            if selected & (1 << index) != 0 {
                if layer.parent == parent {
                    layer.parent = Some(id);
                }
                children.push(layer);
            } else {
                remaining.push(layer);
            }
        }
        let count = children.len();
        children.push(folder);
        remaining.splice(insertion..insertion, children);
        self.layers = remaining;
        self.active = insertion + count;
        if !self.validate() {
            *self = before;
            return None;
        }
        Some(id)
    }

    pub fn ungroup_layer_folder(&mut self, index: usize) -> bool {
        let Some(folder) = self.layers.get(index) else {
            return false;
        };
        if folder.kind != LayerKind::Folder
            || folder.locked
            || self.folder_edit_reason(folder.parent).is_some()
            || self.layers.len() == 1
        {
            return false;
        }
        let parent = folder.parent;
        let id = folder.id;
        let active = self.layers[self.active].id;
        self.layers.remove(index);
        for layer in &mut self.layers {
            if layer.parent == Some(id) {
                layer.parent = parent;
            }
        }
        self.active = self
            .layer_index(active)
            .unwrap_or_else(|| index.saturating_sub(1).min(self.layers.len() - 1));
        self.touch();
        true
    }
}
