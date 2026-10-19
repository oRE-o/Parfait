use super::*;
use parfait_core::{BlendMode, LayerId};
use std::sync::Arc;

fn insert_raw(store: &Store, revision: u64, raw: &[u8]) {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::fast());
    encoder.write_all(raw).unwrap();
    store
        .connection
        .execute(
            "INSERT OR REPLACE INTO snapshots VALUES(?1,?2,?3)",
            params![
                revision as i64,
                Sha256::digest(raw).to_vec(),
                encoder.finish().unwrap()
            ],
        )
        .unwrap();
}

#[test]
fn exact_version_four_clipping_identity_alpha_migrate_without_masks_and_save_version_five() {
    for mode in BlendMode::ALL {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("v4.parfait");
        let mut store = Store::open(&path).unwrap();
        let old = legacy_v4::LegacyDocument {
            width: 2,
            height: 1,
            name: "기존 maskless 그림".into(),
            active: 1,
            revision: 12,
            next_layer_id: 81,
            layers: vec![
                legacy_v4::LegacyLayer {
                    id: LayerId(47),
                    name: "base".into(),
                    visible: true,
                    locked: false,
                    alpha_locked: false,
                    clipped: false,
                    opacity: 0.7,
                    blend_mode: mode,
                    pixels: Arc::new(vec![10, 20, 30, 128, 30, 40, 50, 255]),
                },
                legacy_v4::LegacyLayer {
                    id: LayerId(79),
                    name: "clip".into(),
                    visible: true,
                    locked: false,
                    alpha_locked: true,
                    clipped: true,
                    opacity: 0.9,
                    blend_mode: mode,
                    pixels: Arc::new(vec![90, 80, 70, 255, 40, 30, 20, 127]),
                },
            ],
        };
        let mut raw = SNAPSHOT_V4.to_vec();
        bincode::DefaultOptions::new()
            .with_fixint_encoding()
            .serialize_into(&mut raw, &old)
            .unwrap();
        insert_raw(&store, 12, &raw);
        store
            .connection
            .pragma_update(None, "user_version", 4)
            .unwrap();
        let mut document = store.load().unwrap().unwrap().document;
        assert_eq!(document.next_layer_id, 81);
        assert_eq!(document.active, 1);
        assert!(document.layers.iter().all(|layer| layer.mask.is_none()));
        assert_eq!(document.layers[0].id, LayerId(47));
        assert_eq!(document.layers[1].id, LayerId(79));
        assert!(document.layers[1].clipped && document.layers[1].alpha_locked);
        assert_eq!(document.layers[1].pixels, old.layers[1].pixels);
        let rendered = document.composite();
        assert!(document.create_layer_mask(1, 255));
        store.save(&document).unwrap();
        assert_eq!(
            store
                .connection
                .pragma_query_value::<i64, _>(None, "user_version", |row| row.get(0))
                .unwrap(),
            5
        );
        drop(store);
        let loaded = Store::open(&path).unwrap().load().unwrap().unwrap();
        assert_eq!(loaded.document, document);
        assert_eq!(loaded.document.composite(), rendered);
    }
}

#[test]
fn malformed_dense_masks_fall_back_to_valid_generation_without_changing_original_buffers() {
    for length in [0, 5, 7] {
        let directory = tempfile::tempdir().unwrap();
        let mut store = Store::open(&directory.path().join("mask-recovery.parfait")).unwrap();
        let mut document = Document::new(3, 2);
        assert!(document.create_layer_mask(1, 128));
        store.save(&document).unwrap();
        let before = document.clone();
        document.layers[1].mask = Some(parfait_core::LayerMask {
            enabled: false,
            coverage: parfait_core::MaskCoverage::Dense(Arc::new(vec![255; length])),
        });
        document.touch();
        assert!(!document.validate());
        assert!(store.save(&document).is_err());
        let mut raw = SNAPSHOT_V5.to_vec();
        bincode::DefaultOptions::new()
            .with_fixint_encoding()
            .serialize_into(&mut raw, &document)
            .unwrap();
        insert_raw(&store, document.revision, &raw);
        let recovered = store.load().unwrap().unwrap();
        assert!(recovered.fell_back);
        assert_eq!(recovered.document.layers, before.layers);
        assert!(recovered.document.revision > document.revision);
        store.save(&recovered.document).unwrap();
        assert!(!store.load().unwrap().unwrap().fell_back);
    }
}

fn version_two(mode: BlendMode) -> Vec<u8> {
    let old = legacy_v2::LegacyDocument {
        width: 2,
        height: 1,
        name: "기존 그림".into(),
        active: 1,
        revision: 1,
        layers: vec![
            legacy_v2::LegacyLayer {
                name: "Paper".into(),
                visible: true,
                locked: true,
                opacity: 1.,
                blend_mode: BlendMode::Normal,
                pixels: Arc::new(vec![255; 8]),
            },
            legacy_v2::LegacyLayer {
                name: "선화".into(),
                visible: true,
                locked: false,
                opacity: 0.75,
                blend_mode: mode,
                pixels: Arc::new(vec![30, 60, 90, 128, 0, 0, 0, 0]),
            },
        ],
    };
    let mut raw = SNAPSHOT_V2.to_vec();
    bincode::DefaultOptions::new()
        .with_fixint_encoding()
        .serialize_into(&mut raw, &old)
        .unwrap();
    raw
}

#[test]
fn exact_version_three_preserves_identity_high_water_alpha_and_pixels_then_saves_clipping() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("v3.parfait");
    let mut store = Store::open(&path).unwrap();
    let old = legacy_v3::LegacyDocument {
        width: 1,
        height: 1,
        name: "기존 그림".into(),
        active: 0,
        revision: 9,
        next_layer_id: 93,
        layers: vec![legacy_v3::LegacyLayer {
            id: LayerId(72),
            name: "투명 잠금".into(),
            visible: true,
            locked: false,
            alpha_locked: true,
            opacity: 0.75,
            blend_mode: BlendMode::Overlay,
            pixels: Arc::new(vec![30, 60, 90, 128]),
        }],
    };
    let mut raw = SNAPSHOT_V3.to_vec();
    bincode::DefaultOptions::new()
        .with_fixint_encoding()
        .serialize_into(&mut raw, &old)
        .unwrap();
    insert_raw(&store, 9, &raw);
    store
        .connection
        .pragma_update(None, "user_version", 3)
        .unwrap();
    let mut document = store.load().unwrap().unwrap().document;
    assert_eq!(document.layers[0].id, LayerId(72));
    assert_eq!(document.next_layer_id, 93);
    assert!(document.layers[0].alpha_locked);
    assert!(!document.layers[0].clipped);
    assert_eq!(document.layers[0].blend_mode, BlendMode::Overlay);
    assert_eq!(document.layers[0].opacity, 0.75);
    assert_eq!(*document.layers[0].pixels, [30, 60, 90, 128]);
    document.add_layer();
    document.layers[1].clipped = true;
    store.save(&document).unwrap();
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

#[test]
fn exact_version_two_layout_upgrades_all_blends_without_losing_pixels_or_opacity() {
    for mode in BlendMode::ALL {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("upgrade.parfait");
        let mut store = Store::open(&path).unwrap();
        insert_raw(&store, 1, &version_two(mode));
        store
            .connection
            .pragma_update(None, "user_version", 2)
            .unwrap();
        let mut document = store.load().unwrap().unwrap().document;
        assert_eq!(
            document
                .layers
                .iter()
                .map(|layer| layer.id)
                .collect::<Vec<_>>(),
            [LayerId(1), LayerId(2)]
        );
        assert_eq!(document.next_layer_id, 3);
        assert!(!document.layers[1].alpha_locked);
        assert_eq!(document.layers[1].blend_mode, mode);
        assert_eq!(document.layers[1].opacity, 0.75);
        assert_eq!(
            document.layers[1].pixels.as_slice(),
            &[30, 60, 90, 128, 0, 0, 0, 0]
        );
        let composite = document.composite();
        document.layers[1].alpha_locked = true;
        document.touch();
        store.save(&document).unwrap();
        assert_eq!(
            store
                .connection
                .pragma_query_value::<i64, _>(None, "user_version", |row| row.get(0))
                .unwrap(),
            5
        );
        drop(store);
        let loaded = Store::open(&path).unwrap().load().unwrap().unwrap();
        assert_eq!(loaded.document, document);
        assert_eq!(loaded.document.composite(), composite);
        assert!(!loaded.fell_back);
    }
}

#[test]
fn malformed_current_ids_fall_back_to_v2_and_resave_a_newer_valid_generation() {
    for invalid in 0..3 {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(&dir.path().join("recovery.parfait")).unwrap();
        insert_raw(&store, 1, &version_two(BlendMode::Screen));
        let mut document = store.load().unwrap().unwrap().document;
        document.touch();
        document.layers[1].alpha_locked = true;
        match invalid {
            0 => document.layers[1].id = document.layers[0].id,
            1 => document.layers[1].id = LayerId(0),
            _ => document.next_layer_id = 2,
        }
        assert!(!document.validate());
        assert!(store.save(&document).is_err());
        let mut raw = SNAPSHOT_V5.to_vec();
        bincode::DefaultOptions::new()
            .with_fixint_encoding()
            .serialize_into(&mut raw, &document)
            .unwrap();
        insert_raw(&store, 2, &raw);
        store
            .connection
            .pragma_update(None, "user_version", 3)
            .unwrap();
        let recovered = store.load().unwrap().unwrap();
        assert!(recovered.fell_back);
        assert_eq!(recovered.document.revision, 3);
        assert!(recovered.document.validate());
        assert!(!recovered.document.layers[1].alpha_locked);
        assert_eq!(recovered.document.layers[1].blend_mode, BlendMode::Screen);
        store.save(&recovered.document).unwrap();
        assert!(!store.load().unwrap().unwrap().fell_back);
    }
}

#[test]
fn unsupported_database_and_snapshot_versions_are_preserved_and_rejected() {
    for version in [-1, 6] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("future.parfait");
        let connection = Connection::open(&path).unwrap();
        connection
            .pragma_update(None, "user_version", version)
            .unwrap();
        drop(connection);
        assert!(Store::open(&path).is_err());
        let connection = Connection::open(&path).unwrap();
        assert_eq!(
            connection
                .pragma_query_value::<i64, _>(None, "user_version", |row| row.get(0))
                .unwrap(),
            version
        );
        assert_eq!(
            connection
                .query_row::<i64, _, _>(
                    "SELECT COUNT(*) FROM sqlite_master WHERE name='snapshots'",
                    [],
                    |row| row.get(0)
                )
                .unwrap(),
            0
        );
    }
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("unknown.parfait")).unwrap();
    let raw = b"PARFAIT\0SNAP9\0unknown";
    insert_raw(&store, 1, raw);
    assert!(store.load().is_err());
    assert_eq!(
        store
            .connection
            .query_row::<i64, _, _>("SELECT COUNT(*) FROM snapshots", [], |row| row.get(0))
            .unwrap(),
        1
    );
}

#[test]
fn identity_exhaustion_and_alpha_flags_roundtrip_and_size_matches_exact_serialization() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = Store::open(&dir.path().join("identity.parfait")).unwrap();
    let mut document = Document::new(3, 2);
    document.layers[1].id = LayerId(u64::MAX - 1);
    document.next_layer_id = u64::MAX;
    document.layers[1].alpha_locked = true;
    assert!(document.validate());
    assert!(!document.can_allocate_layers(1));
    assert!(layer_growth_error(&document, 5).is_some());
    assert_eq!(
        snapshot_size(&document).unwrap(),
        SNAPSHOT_V5.len() as u64
            + bincode::DefaultOptions::new()
                .with_fixint_encoding()
                .serialized_size(&document)
                .unwrap()
    );
    store.save(&document).unwrap();
    assert_eq!(store.load().unwrap().unwrap().document, document);
}
