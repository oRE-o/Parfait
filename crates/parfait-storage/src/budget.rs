use parfait_core::{Document, LayerMask, MaskCoverage};

pub(crate) const SNAPSHOT_LIMIT: u64 = 512 * 1024 * 1024;
const PIXEL_LIMIT: u64 = 512 * 1024 * 1024;

fn coverage_size(coverage: &MaskCoverage) -> Option<u64> {
    match coverage {
        MaskCoverage::Uniform(_) => Some(5),
        MaskCoverage::Dense(values) => 12u64.checked_add(values.len() as u64),
    }
}

pub fn solid_layer_growth_error(
    document: &Document,
    paper: bool,
    name: &str,
) -> Option<&'static str> {
    if let Some(reason) = document.solid_layer_creation_reason(paper, name) {
        return Some(reason);
    }
    let active = &document.layers[document.active];
    let parent = !paper
        && (active.parent.is_some()
            || active.kind == parfait_core::LayerKind::Folder);
    let mask_bytes = if paper {
        Some(0)
    } else {
        document
            .selection
            .as_ref()
            .map_or(Some(5), coverage_size)
            .and_then(|bytes| bytes.checked_add(1))
    };
    let allowed = snapshot_size(document).and_then(|bytes| {
        bytes
            .checked_add(47 + 8 * u64::from(parent))?
            .checked_add(name.len() as u64)?
            .checked_add(mask_bytes?)
    });
    allowed.is_none_or(|bytes| bytes > SNAPSHOT_LIMIT).then_some(
        "Creating this layer would exceed the 512 MiB snapshot budget.",
    )
}

pub fn solid_rasterization_error(
    document: &Document,
    index: usize,
) -> Option<&'static str> {
    if let Some(reason) = document.solid_rasterization_reason(index) {
        return Some(reason);
    }
    snapshot_size(document)
        .and_then(|bytes| {
            bytes.checked_sub(3)?.checked_add(
                u64::from(document.width) * u64::from(document.height) * 4,
            )
        })
        .is_none_or(|bytes| bytes > SNAPSHOT_LIMIT)
        .then_some(
            "Rasterizing this layer would exceed the 512 MiB snapshot budget.",
        )
}

pub fn selection_update_error(
    document: &Document,
    dense: bool,
) -> Option<&'static str> {
    if let Some(reason) = document.selection_creation_reason() {
        return Some(reason);
    }
    let allowed = || -> Option<bool> {
        let previous =
            document.selection.as_ref().map_or(Some(0), coverage_size)?;
        let proposed = if dense {
            12 + u64::from(document.width) * u64::from(document.height)
        } else {
            5
        };
        Some(
            snapshot_size(document)?
                .checked_sub(previous)?
                .checked_add(proposed)?
                <= SNAPSHOT_LIMIT,
        )
    };
    (allowed() != Some(true))
        .then_some("The selection would exceed the 512 MiB snapshot budget.")
}

pub fn mask_edit_error(
    document: &Document,
    layer: usize,
) -> Option<&'static str> {
    if !document.validate() {
        return Some("The drawing has invalid image data.");
    }
    let mask = document.layers.get(layer)?.mask.as_ref()?;
    let allowed = || -> Option<bool> {
        Some(
            snapshot_size(document)?
                .checked_sub(mask_payload_size(mask)?)?
                .checked_add(
                    13 + u64::from(document.width) * u64::from(document.height),
                )?
                <= SNAPSHOT_LIMIT,
        )
    };
    (allowed() != Some(true)).then_some(
        "Editing this mask would exceed the 512 MiB snapshot budget.",
    )
}

pub fn snapshot_size(document: &Document) -> Option<u64> {
    let pixels = u64::from(document.width)
        .checked_mul(u64::from(document.height))?
        .checked_mul(4)?;
    let mut bytes = (super::SNAPSHOT_V10.len() as u64)
        .checked_add(49)?
        .checked_add(document.name.len() as u64)?;
    if let Some(coverage) = &document.selection {
        bytes = bytes.checked_add(match coverage {
            MaskCoverage::Uniform(_) => 5,
            MaskCoverage::Dense(values) => {
                12u64.checked_add(values.len() as u64)?
            }
        })?;
    }
    for layer in &document.layers {
        bytes = bytes
            .checked_add(44 + 8 * u64::from(layer.parent.is_some()))?
            .checked_add(layer.name.len() as u64)?
            .checked_add(3 * u64::from(layer.kind.solid_color().is_some()))?
            .checked_add(if layer.kind == parfait_core::LayerKind::Raster {
                pixels
            } else {
                0
            })?;
        if let Some(mask) = &layer.mask {
            bytes = bytes.checked_add(mask_payload_size(mask)?)?;
        }
    }
    Some(bytes)
}

pub fn layer_growth_error(
    document: &Document,
    name_bytes: usize,
) -> Option<&'static str> {
    layer_growth_count_error(document, 1, name_bytes)
}

pub fn layer_growth_count_error(
    document: &Document,
    additional: usize,
    name_bytes: usize,
) -> Option<&'static str> {
    if document.layers.len().saturating_add(additional) > 64 {
        return Some("The drawing cannot contain more than 64 layers.");
    }
    if !document.can_allocate_layers(additional) {
        return Some("The layer identifier space is exhausted.");
    }
    let allowed = || -> Option<bool> {
        let pixels = u64::from(document.width)
            .checked_mul(u64::from(document.height))?
            .checked_mul(4)?;
        let logical = document
            .logical_pixel_bytes()
            .checked_add(pixels.checked_mul(additional as u64)?)?;
        let snapshot = snapshot_size(document)?
            .checked_add(
                (44u64
                    + 8 * u64::from(
                        document.layers.get(document.active).is_some_and(
                            |layer| {
                                layer.parent.is_some()
                                    || layer.kind
                                        == parfait_core::LayerKind::Folder
                            },
                        ),
                    ))
                .checked_mul(additional as u64)?,
            )?
            .checked_add(name_bytes as u64)?
            .checked_add(pixels.checked_mul(additional as u64)?)?;
        Some(logical <= PIXEL_LIMIT && snapshot <= SNAPSHOT_LIMIT)
    };
    if allowed() == Some(true) {
        None
    } else {
        Some(
            "Adding this layer would exceed the 512 MiB snapshot or logical pixel budget. The drawing has not been changed.",
        )
    }
}

fn mask_payload_size(mask: &LayerMask) -> Option<u64> {
    match &mask.coverage {
        MaskCoverage::Uniform(_) => Some(6),
        MaskCoverage::Dense(values) => 13u64.checked_add(values.len() as u64),
    }
}

pub fn layer_mask_creation_error(
    document: &Document,
    layer: usize,
) -> Option<&'static str> {
    if let Some(reason) = document.layer_mask_creation_reason(layer) {
        return Some(reason);
    }
    let pixels = u64::from(document.width) * u64::from(document.height);
    if snapshot_size(document)
        .and_then(|bytes| bytes.checked_add(13)?.checked_add(pixels))
        .is_some_and(|bytes| bytes <= SNAPSHOT_LIMIT)
    {
        None
    } else {
        Some("Creating this mask would exceed the 512 MiB snapshot budget.")
    }
}

pub fn layer_duplication_error(
    document: &Document,
    selection: u64,
) -> Option<&'static str> {
    if document.has_non_raster_layers() {
        if let Some(reason) = document.layer_tree_duplication_reason(selection)
        {
            return Some(reason);
        }
        let expanded = document.layer_selection_subtrees(selection).unwrap();
        let roots = document.layer_selection_roots(selection).unwrap();
        let allowed = || -> Option<bool> {
            let mut bytes = snapshot_size(document)?;
            for (index, layer) in document
                .layers
                .iter()
                .enumerate()
                .filter(|(index, _)| expanded & (1 << index) != 0)
            {
                bytes = bytes
                    .checked_add(44 + 8 * u64::from(layer.parent.is_some()))?
                    .checked_add(layer.name.len() as u64)?
                    .checked_add(
                        3 * u64::from(layer.kind.solid_color().is_some()),
                    )?
                    .checked_add(5 * u64::from(roots & (1 << index) != 0))?
                    .checked_add(layer.pixels.len() as u64)?;
                if let Some(mask) = &layer.mask {
                    bytes = bytes.checked_add(mask_payload_size(mask)?)?;
                }
            }
            Some(bytes <= SNAPSHOT_LIMIT)
        };
        return (allowed() != Some(true))
            .then_some("Duplicating these subtrees would exceed the 512 MiB snapshot budget.");
    }
    if document.layers.is_empty()
        || document.layers.len() > 64
        || selection == 0
        || selection & !(u64::MAX >> (64 - document.layers.len())) != 0
    {
        return Some("Select existing layers first.");
    }
    let selected: Vec<_> = document
        .layers
        .iter()
        .enumerate()
        .filter(|(index, _)| selection & (1 << index) != 0)
        .map(|(_, layer)| layer)
        .collect();
    if selected.is_empty() {
        return Some("Select an existing layer first.");
    }
    let Some(name_bytes) = selected.iter().try_fold(0usize, |bytes, layer| {
        bytes.checked_add(layer.name.len())?.checked_add(5)
    }) else {
        return Some("The duplicated layer names exceed the snapshot budget.");
    };
    if let Some(reason) =
        layer_growth_count_error(document, selected.len(), name_bytes)
    {
        return Some(reason);
    }
    let allowed = || -> Option<bool> {
        let pixels = u64::from(document.width)
            .checked_mul(u64::from(document.height))?;
        let mut logical = document.logical_pixel_bytes();
        let mut snapshot = snapshot_size(document)?;
        for layer in selected {
            logical = logical.checked_add(
                pixels.checked_mul(4 + u64::from(layer.mask.is_some()))?,
            )?;
            snapshot = snapshot
                .checked_add(44)?
                .checked_add(layer.name.len() as u64)?
                .checked_add(5)?
                .checked_add(pixels.checked_mul(4)?)?;
            if let Some(mask) = &layer.mask {
                snapshot = snapshot.checked_add(mask_payload_size(mask)?)?;
            }
        }
        Some(logical <= PIXEL_LIMIT && snapshot <= SNAPSHOT_LIMIT)
    };
    (allowed() != Some(true)).then_some(
        "Duplicating these layers would exceed the 512 MiB snapshot or logical pixel budget.",
    )
}

pub fn layer_folder_growth_error(
    document: &Document,
    selection: Option<u64>,
    name_bytes: usize,
) -> Option<&'static str> {
    if let Some(reason) = document.layer_folder_creation_reason(selection) {
        return Some(reason);
    }
    let (parent, children) = if let Some(selection) = selection {
        let roots = document.layer_selection_roots(selection).unwrap();
        let parent = document.layers[roots.trailing_zeros() as usize].parent;
        (
            parent,
            if parent.is_none() { roots.count_ones() as u64 * 8 } else { 0 },
        )
    } else {
        let active = &document.layers[document.active];
        (
            if active.kind == parfait_core::LayerKind::Folder {
                Some(active.id)
            } else {
                active.parent
            },
            0,
        )
    };
    let bytes = snapshot_size(document).and_then(|bytes| {
        bytes
            .checked_add(44 + 8 * u64::from(parent.is_some()))?
            .checked_add(name_bytes as u64)?
            .checked_add(children)
    });
    (!bytes.is_some_and(|bytes| bytes <= SNAPSHOT_LIMIT))
        .then_some("The folder would exceed the 512 MiB snapshot budget.")
}

pub fn layer_reparent_error(
    document: &Document,
    selection: u64,
    boundary: usize,
    parent: Option<parfait_core::LayerId>,
) -> Option<&'static str> {
    if let Some(reason) =
        document.layer_reparent_reason(selection, boundary, parent)
    {
        return Some(reason);
    }
    let roots = document.layer_selection_roots(selection).unwrap();
    let mut bytes = snapshot_size(document).unwrap_or(u64::MAX);
    for (index, layer) in document.layers.iter().enumerate() {
        if roots & (1 << index) != 0 {
            if layer.parent.is_none() && parent.is_some() {
                bytes = bytes.saturating_add(8);
            }
            if layer.parent.is_some() && parent.is_none() {
                bytes = bytes.saturating_sub(8);
            }
        }
    }
    (bytes > SNAPSHOT_LIMIT).then_some(
        "Moving these layers would exceed the 512 MiB snapshot budget.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use bincode::Options;

    #[test]
    fn selection_and_mask_edit_guards_reserve_dense_payload_metadata() {
        use std::sync::Arc;
        let mut document = Document::new(1, 1);
        document.width = 4096;
        document.height = 4096;
        document.layers.resize(7, document.layers[1].clone());
        document.next_layer_id = 8;
        let pixels = Arc::new(vec![0; 4096 * 4096 * 4]);
        let coverage = Arc::new(vec![255; 4096 * 4096]);
        for (index, layer) in document.layers.iter_mut().enumerate() {
            layer.id = parfait_core::LayerId(index as u64 + 1);
            layer.pixels = pixels.clone();
            if index < 3 {
                layer.mask = Some(LayerMask {
                    enabled: true,
                    coverage: MaskCoverage::Dense(coverage.clone()),
                });
            }
        }
        assert!(document.validate());
        assert!(selection_update_error(&document, false).is_none());
        assert!(selection_update_error(&document, true).is_some());
        assert!(mask_edit_error(&document, 0).is_none());
        assert!(document.select_all_pixels());
        assert!(selection_update_error(&document, false).is_none());
        assert!(selection_update_error(&document, true).is_some());
        document.layers[3].mask = Some(LayerMask {
            enabled: true,
            coverage: MaskCoverage::Uniform(255),
        });
        assert!(mask_edit_error(&document, 3).is_some());
        assert!(mask_edit_error(&document, 0).is_some());
        document.selection = None;
        assert!(document.validate());
        assert!(mask_edit_error(&document, 0).is_none());
        assert!(selection_update_error(&document, false).is_some());
        assert!(mask_edit_error(&document, 3).is_some());
        document.layers[2].mask = None;
        assert!(selection_update_error(&document, true).is_none());
        assert!(mask_edit_error(&document, 3).is_none());
        assert!(
            document
                .layers
                .iter()
                .all(|layer| Arc::ptr_eq(&layer.pixels, &pixels))
        );
        document.width = u32::MAX;
        assert!(selection_update_error(&document, true).is_some());
    }

    #[test]
    fn folder_tree_edits_count_parent_links_masks_and_no_folder_canvas() {
        let mut document = Document::new(7, 5);
        document.add_layer();
        assert!(layer_folder_growth_error(&document, Some(6), 6).is_none());
        let folder =
            document.group_layers_in_folder(6, "Folder".into()).unwrap();
        document.create_layer_mask(3, 127);
        let bytes = snapshot_size(&document).unwrap();
        assert_eq!(
            bytes,
            bincode::DefaultOptions::new()
                .with_fixint_encoding()
                .serialized_size(&document)
                .unwrap()
                + super::super::SNAPSHOT_V10.len() as u64
        );
        assert!(layer_reparent_error(&document, 2, 4, None).is_none());
        assert!(document.move_layer_subtrees(2, 4, None));
        assert_eq!(snapshot_size(&document).unwrap(), bytes - 8);
        assert!(document.move_layer_subtrees(8, 2, Some(folder)));
        assert_eq!(snapshot_size(&document).unwrap(), bytes);
        assert!(layer_duplication_error(&document, 8).is_none());
        let before = snapshot_size(&document).unwrap();
        let added = document.layers[1..=3]
            .iter()
            .map(|layer| {
                bincode::DefaultOptions::new()
                    .with_fixint_encoding()
                    .serialized_size(layer)
                    .unwrap()
            })
            .sum::<u64>()
            + 5;
        document.duplicate_layer_subtrees(8).unwrap();
        assert_eq!(snapshot_size(&document).unwrap() - before, added);
        assert_eq!(
            snapshot_size(&document).unwrap(),
            bincode::DefaultOptions::new()
                .with_fixint_encoding()
                .serialized_size(&document)
                .unwrap()
                + super::super::SNAPSHOT_V10.len() as u64
        );
    }

    #[test]
    fn empty_folder_still_fits_when_another_raster_snapshot_would_not() {
        let mut document = Document::new(1, 1);
        document.width = 4096;
        document.height = 4096;
        document.layers.resize(7, document.layers[1].clone());
        document.next_layer_id = 8;
        let pixels = std::sync::Arc::new(vec![0; 4096 * 4096 * 4]);
        for (index, layer) in document.layers.iter_mut().enumerate() {
            layer.id = parfait_core::LayerId(index as u64 + 1);
            layer.pixels = pixels.clone();
        }
        assert!(document.validate());
        assert!(layer_growth_error(&document, 7).is_some());
        assert!(layer_folder_growth_error(&document, None, 6).is_none());
        let folder = document.create_layer_folder("Folder".into()).unwrap();
        assert!(
            document.layers[document.layer_index(folder).unwrap()]
                .pixels
                .is_empty()
        );
        assert!(layer_duplication_error(&document, 4).is_none());
        assert!(layer_duplication_error(&document, 2).is_some());
    }

    #[test]
    fn oversized_valid_shared_buffer_document_is_rejected_before_replacing_saved_generation()
     {
        let dir = tempfile::tempdir().unwrap();
        let mut store =
            crate::Store::open(&dir.path().join("budget.parfait")).unwrap();
        let original = Document::new(4, 4);
        store.save(&original).unwrap();
        let mut oversized = Document::new(1, 1);
        oversized.width = 4096;
        oversized.height = 4096;
        oversized.layers.resize(8, oversized.layers[1].clone());
        oversized.next_layer_id = 9;
        let pixels = std::sync::Arc::new(vec![0; 4096 * 4096 * 4]);
        for (index, layer) in oversized.layers.iter_mut().enumerate() {
            layer.id = parfait_core::LayerId(index as u64 + 1);
            layer.pixels = pixels.clone();
        }
        assert!(oversized.validate());
        assert!(snapshot_size(&oversized).unwrap() > SNAPSHOT_LIMIT);
        let error = store.save(&oversized).unwrap_err().to_string();
        assert!(error.contains("before encoding"));
        assert_eq!(store.load().unwrap().unwrap().document, original);
    }

    #[test]
    fn metadata_estimate_matches_fixed_integer_serialization() {
        for mode in parfait_core::BlendMode::ALL {
            let mut document = Document::new(7, 5);
            document.name = "그림 café".into();
            document.layers[1].name = "반투명 layer".into();
            document.layers[1].blend_mode = mode;
            for _ in 0..3 {
                assert_eq!(
                    snapshot_size(&document).unwrap(),
                    bincode::DefaultOptions::new()
                        .with_fixint_encoding()
                        .serialized_size(&document)
                        .unwrap()
                        + super::super::SNAPSHOT_V10.len() as u64
                );
                document.add_layer();
            }
        }
    }

    #[test]
    fn mask_sizes_match_serialization_and_duplication_reserves_mask_memory() {
        for coverage in [
            None,
            Some(MaskCoverage::Uniform(255)),
            Some(MaskCoverage::Dense(std::sync::Arc::new(vec![127; 35]))),
        ] {
            let mut document = Document::new(7, 5);
            document.layers[1].mask =
                coverage.map(|coverage| LayerMask { enabled: true, coverage });
            for enabled in [true, false] {
                if let Some(mask) = &mut document.layers[1].mask {
                    mask.enabled = enabled;
                }
                assert_eq!(
                    snapshot_size(&document).unwrap(),
                    bincode::DefaultOptions::new()
                        .with_fixint_encoding()
                        .serialized_size(&document)
                        .unwrap()
                        + super::super::SNAPSHOT_V10.len() as u64
                );
                assert!(layer_duplication_error(&document, 2).is_none());
            }
        }
        let mut document = Document::new(1, 1);
        document.width = 4096;
        document.height = 4096;
        document.layers.resize(7, document.layers[1].clone());
        document.next_layer_id = 8;
        for (index, layer) in document.layers.iter_mut().enumerate() {
            layer.id = parfait_core::LayerId(index as u64 + 1);
        }
        document.layers[1].mask = Some(LayerMask {
            enabled: true,
            coverage: MaskCoverage::Uniform(255),
        });
        assert!(layer_growth_error(&document, 7).is_some());
        assert!(layer_duplication_error(&document, 2).is_some());
        document.layers.truncate(6);
        assert!(layer_duplication_error(&document, 2).is_none());
        assert!(layer_duplication_error(&document, 0).is_some());
        assert!(layer_duplication_error(&document, 1 << 63).is_some());
        document.layers[1].mask = Some(LayerMask {
            enabled: true,
            coverage: MaskCoverage::Dense(std::sync::Arc::new(vec![127; 1])),
        });
        assert!(layer_duplication_error(&document, 2).is_none());
    }

    #[test]
    fn large_layout_boundary_is_checked_without_allocating_large_images() {
        let mut document = Document::new(1, 1);
        document.width = 4096;
        document.height = 4096;
        while document.layers.len() < 6 {
            {
                let mut copy = document.layers[1].clone();
                copy.id = document.allocate_layer_id().unwrap();
                document.layers.push(copy);
            }
        }
        assert!(layer_growth_error(&document, 7).is_none());
        {
            let mut copy = document.layers[1].clone();
            copy.id = document.allocate_layer_id().unwrap();
            document.layers.push(copy);
        }
        assert!(snapshot_size(&document).unwrap() < SNAPSHOT_LIMIT);
        assert!(layer_growth_error(&document, 7).is_some());
        {
            let mut copy = document.layers[1].clone();
            copy.id = document.allocate_layer_id().unwrap();
            document.layers.push(copy);
        }
        assert!(snapshot_size(&document).unwrap() > SNAPSHOT_LIMIT);
        document.width = u32::MAX;
        document.height = u32::MAX;
        assert!(snapshot_size(&document).is_none());
        assert!(layer_growth_error(&document, usize::MAX).is_some());
        let mut document = Document::new(1, 1);
        while document.layers.len() < 64 {
            document.add_layer();
        }
        assert!(layer_growth_error(&document, 7).is_some());
    }
}
