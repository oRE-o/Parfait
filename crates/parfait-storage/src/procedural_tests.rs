use super::*;
use parfait_core::{BlendMode, LayerId, LayerKind, LayerMask, MaskCoverage};
use std::sync::Arc;

fn inject(store: &Store, revision: u64, raw: &[u8]) {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(raw).unwrap();
    store
        .connection
        .execute(
            "INSERT INTO snapshots(revision,checksum,payload) VALUES(?1,?2,?3)",
            params![
                revision as i64,
                Sha256::digest(raw).as_slice(),
                encoder.finish().unwrap()
            ],
        )
        .unwrap();
}

fn legacy_bytes(document: &Document, version: u8) -> Vec<u8> {
    let layers: Vec<_> = document
        .layers
        .iter()
        .map(|layer| {
            (
                layer.id,
                &layer.name,
                layer.visible,
                layer.locked,
                layer.alpha_locked,
                layer.clipped,
                layer.opacity,
                layer.blend_mode,
                &layer.pixels,
                &layer.mask,
                layer.reference,
                layer.draft,
                u32::from(layer.kind == LayerKind::Folder),
                layer.parent,
            )
        })
        .collect();
    let mut raw = match version {
        7 => SNAPSHOT_V7,
        8 => SNAPSHOT_V8,
        _ => SNAPSHOT_V9,
    }
    .to_vec();
    let options = bincode::DefaultOptions::new().with_fixint_encoding();
    if version == 9 {
        options
            .serialize_into(
                &mut raw,
                &(
                    document.width,
                    document.height,
                    &document.name,
                    layers,
                    document.active,
                    document.revision,
                    document.next_layer_id,
                    &document.selection,
                ),
            )
            .unwrap();
    } else {
        options
            .serialize_into(
                &mut raw,
                &(
                    document.width,
                    document.height,
                    &document.name,
                    layers,
                    document.active,
                    document.revision,
                    document.next_layer_id,
                ),
            )
            .unwrap();
    }
    raw
}

#[test]
fn exact_prior_kind_layouts_preserve_artwork_and_selection_without_converting_old_paper() {
    for version in [7, 8, 9] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("legacy.parfait");
        let mut store = Store::open(&path).unwrap();
        let mut document = Document::new(8, 5);
        document.layers[1].pixels = Arc::new([20, 80, 150, 128].repeat(40));
        document.group_layers_in_folder(2, "Folder".into()).unwrap();
        document.layers[2].blend_mode = if version == 7 {
            BlendMode::Multiply
        } else {
            BlendMode::Through
        };
        document.create_layer_mask(1, 193);
        if version == 9 {
            document.set_selection(Some(MaskCoverage::Dense(Arc::new(
                (0..40).map(|index| (index * 7) as u8).collect(),
            ))));
        }
        inject(&store, document.revision, &legacy_bytes(&document, version));
        store
            .connection
            .pragma_update(None, "user_version", version)
            .unwrap();
        let restored = store.load().unwrap().unwrap().document;
        assert_eq!(restored, document);
        assert_eq!(restored.layers[0].kind, LayerKind::Raster);
        store.save(&restored).unwrap();
        assert_eq!(
            store
                .connection
                .pragma_query_value::<i64, _>(None, "user_version", |row| row.get(0))
                .unwrap(),
            10
        );
        drop(store);
        assert_eq!(
            Store::open(&path)
                .unwrap()
                .load()
                .unwrap()
                .unwrap()
                .document,
            document
        );
    }
}

#[test]
fn solid_parameters_and_shared_masks_roundtrip_with_exact_version_ten_byte_accounting() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("solid.parfait");
    let mut document = Document::new_with_paper(8, 5);
    document.set_solid_layer_color(0, [240, 230, 210]);
    document.set_selection(Some(MaskCoverage::Dense(Arc::new(
        (0..40).map(|index| (index * 7) as u8).collect(),
    ))));
    document
        .create_solid_layer([30, 80, 160], false, "Fill".into())
        .unwrap();
    document.group_layers_in_folder(4, "Group".into()).unwrap();
    document.layers[3].blend_mode = BlendMode::Through;
    document.layers[2].opacity = 0.65;
    let mut store = Store::open(&path).unwrap();
    store.save(&document).unwrap();
    assert_eq!(
        snapshot_size(&document).unwrap(),
        bincode::DefaultOptions::new()
            .with_fixint_encoding()
            .serialized_size(&document)
            .unwrap()
            + SNAPSHOT_V10.len() as u64
    );
    drop(store);
    let store = Store::open(&path).unwrap();
    let restored = store.load().unwrap().unwrap().document;
    assert_eq!(restored, document);
    assert_eq!(restored.composite(), document.composite());
    assert!(restored.layers[0].pixels.is_empty());
    assert!(restored.layers[2].pixels.is_empty());
    let payload: Vec<u8> = store
        .connection
        .query_row(
            "SELECT payload FROM snapshots ORDER BY revision DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let mut raw = Vec::new();
    ZlibDecoder::new(payload.as_slice())
        .read_to_end(&mut raw)
        .unwrap();
    assert!(raw.starts_with(SNAPSHOT_V10));
    assert_eq!(snapshot_size(&restored).unwrap(), raw.len() as u64);
}

#[test]
fn mislabeled_new_kinds_and_invalid_paper_heads_fall_back_to_committed_artwork() {
    for version in [7, 8, 9, 10] {
        let directory = tempfile::tempdir().unwrap();
        let mut store = Store::open(&directory.path().join("fallback.parfait")).unwrap();
        let original = Document::new_with_paper(8, 5);
        store.save(&original).unwrap();
        let mut damaged = original.clone();
        damaged.touch();
        if version == 10 {
            damaged.layers[0].opacity = 0.5;
        }
        let mut raw = match version {
            7 => SNAPSHOT_V7,
            8 => SNAPSHOT_V8,
            9 => SNAPSHOT_V9,
            _ => SNAPSHOT_V10,
        }
        .to_vec();
        let options = bincode::DefaultOptions::new().with_fixint_encoding();
        if version < 9 {
            options
                .serialize_into(
                    &mut raw,
                    &(
                        damaged.width,
                        damaged.height,
                        &damaged.name,
                        &damaged.layers,
                        damaged.active,
                        damaged.revision,
                        damaged.next_layer_id,
                    ),
                )
                .unwrap();
        } else {
            options.serialize_into(&mut raw, &damaged).unwrap();
        }
        inject(&store, damaged.revision, &raw);
        let loaded = store.load().unwrap().unwrap();
        assert!(loaded.fell_back);
        let mut expected = original;
        expected.revision = loaded.document.revision;
        assert_eq!(loaded.document, expected);
        assert!(loaded.document.revision > damaged.revision);
    }
}

#[test]
fn solid_growth_and_rasterization_budget_guards_count_color_payload_and_actual_mask_shape() {
    let mut document = Document::new(1, 1);
    document.width = 4096;
    document.height = 4096;
    document.layers.resize(7, document.layers[1].clone());
    document.next_layer_id = 8;
    let pixels = Arc::new(vec![0; 4096 * 4096 * 4]);
    let mask = Arc::new(vec![128; 4096 * 4096]);
    for (index, layer) in document.layers.iter_mut().enumerate() {
        layer.id = LayerId(index as u64 + 1);
        layer.pixels = pixels.clone();
        if index < 2 {
            layer.mask = Some(LayerMask {
                enabled: true,
                coverage: MaskCoverage::Dense(mask.clone()),
            });
        }
    }
    document.selection = Some(MaskCoverage::Dense(mask));
    assert!(document.validate());
    assert!(
        document
            .solid_layer_creation_reason(false, "Fill")
            .is_none()
    );
    assert!(solid_layer_growth_error(&document, false, "Fill").is_some());
    document.selection = Some(MaskCoverage::Uniform(128));
    assert!(solid_layer_growth_error(&document, false, "Fill").is_none());
    document
        .create_solid_layer([50; 3], false, "Fill".into())
        .unwrap();
    assert!(solid_layer_growth_error(&document, false, "Extra").is_some());
    assert!(solid_layer_growth_error(&document, true, "Paper").is_none());
    document
        .create_solid_layer([240; 3], true, "Paper".into())
        .unwrap();
    assert!(solid_rasterization_error(&document, 0).is_some());
    assert!(document.validate());
    let small = Document::new_with_paper(8, 5);
    assert!(solid_rasterization_error(&small, 0).is_none());
}
