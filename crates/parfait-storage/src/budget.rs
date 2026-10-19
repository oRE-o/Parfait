use parfait_core::{Document, LayerMask, MaskCoverage};

pub(crate) const SNAPSHOT_LIMIT: u64 = 512 * 1024 * 1024;
const PIXEL_LIMIT: u64 = 512 * 1024 * 1024;

pub fn snapshot_size(document: &Document) -> Option<u64> {
    let pixels = u64::from(document.width)
        .checked_mul(u64::from(document.height))?
        .checked_mul(4)?;
    let mut bytes = (super::SNAPSHOT_V5.len() as u64)
        .checked_add(48)?
        .checked_add(document.name.len() as u64)?;
    for layer in &document.layers {
        bytes = bytes
            .checked_add(37)?
            .checked_add(layer.name.len() as u64)?
            .checked_add(pixels)?;
        if let Some(mask) = &layer.mask {
            bytes = bytes.checked_add(mask_payload_size(mask)?)?;
        }
    }
    Some(bytes)
}

pub fn layer_growth_error(document: &Document, name_bytes: usize) -> Option<&'static str> {
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
            .checked_add(37u64.checked_mul(additional as u64)?)?
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

pub fn layer_mask_creation_error(document: &Document, layer: usize) -> Option<&'static str> {
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

pub fn layer_duplication_error(document: &Document, selection: u64) -> Option<&'static str> {
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
    if let Some(reason) = layer_growth_count_error(document, selected.len(), name_bytes) {
        return Some(reason);
    }
    let allowed = || -> Option<bool> {
        let pixels = u64::from(document.width).checked_mul(u64::from(document.height))?;
        let mut logical = document.logical_pixel_bytes();
        let mut snapshot = snapshot_size(document)?;
        for layer in selected {
            logical =
                logical.checked_add(pixels.checked_mul(4 + u64::from(layer.mask.is_some()))?)?;
            snapshot = snapshot
                .checked_add(37)?
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

#[cfg(test)]
mod tests {
    use super::*;
    use bincode::Options;

    #[test]
    fn oversized_valid_shared_buffer_document_is_rejected_before_replacing_saved_generation() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = crate::Store::open(&dir.path().join("budget.parfait")).unwrap();
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
                        + super::super::SNAPSHOT_V5.len() as u64
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
            document.layers[1].mask = coverage.map(|coverage| LayerMask {
                enabled: true,
                coverage,
            });
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
                        + super::super::SNAPSHOT_V5.len() as u64
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
