use super::*;
use parfait_core::{BlendMode, LayerMask, MaskCoverage};
use std::sync::Arc;

fn version_seven(document: &Document) -> Vec<u8> {
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
                layer.kind,
                layer.parent,
            )
        })
        .collect();
    let old = (
        document.width,
        document.height,
        &document.name,
        layers,
        document.active,
        document.revision,
        document.next_layer_id,
    );
    let mut raw = SNAPSHOT_V7.to_vec();
    bincode::DefaultOptions::new()
        .with_fixint_encoding()
        .serialize_into(&mut raw, &old)
        .unwrap();
    raw
}

fn insert(store: &Store, revision: u64, raw: &[u8]) {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(raw).unwrap();
    store
        .connection
        .execute(
            "INSERT INTO snapshots(revision,checksum,payload) VALUES(?1,?2,?3)",
            params![
                i64::try_from(revision).unwrap(),
                Sha256::digest(raw).as_slice(),
                encoder.finish().unwrap()
            ],
        )
        .unwrap();
}

#[test]
fn exact_v7_tree_layout_upgrades_without_changing_masks_roles_ids_or_content() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("v7.parfait");
    let mut store = Store::open(&path).unwrap();
    let mut document = Document::new(4, 3);
    document.group_layers_in_folder(2, "Folder".into()).unwrap();
    document.create_layer_mask(2, 127);
    document.layers[1].reference = true;
    document.layers[1].draft = true;
    document.layers[2].opacity = 0.625;
    let original = document.clone();
    insert(&store, document.revision, &version_seven(&document));
    store
        .connection
        .pragma_update(None, "user_version", 7)
        .unwrap();
    assert_eq!(store.load().unwrap().unwrap().document, document);
    document.layers[2].blend_mode = BlendMode::Through;
    document.touch();
    store.save(&document).unwrap();
    assert_eq!(
        store
            .connection
            .pragma_query_value::<i64, _>(None, "user_version", |row| row.get(0))
            .unwrap(),
        10
    );
    drop(store);
    let store = Store::open(&path).unwrap();
    assert_eq!(store.load().unwrap().unwrap().document, document);
    assert_eq!(document.layers[1], original.layers[1]);
    assert_eq!(document.layers[2].mask, original.layers[2].mask);
}

#[test]
fn current_through_roundtrips_and_bad_current_or_mislabelled_legacy_heads_fall_back() {
    for legacy in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let mut store = Store::open(&directory.path().join("through.parfait")).unwrap();
        let mut document = Document::new(4, 3);
        document.group_layers_in_folder(2, "Pass".into()).unwrap();
        document.layers[2].blend_mode = BlendMode::Through;
        document.layers[2].mask = Some(LayerMask {
            enabled: true,
            coverage: MaskCoverage::Dense(Arc::new(
                (0..12).map(|index| (index * 19) as u8).collect(),
            )),
        });
        store.save(&document).unwrap();
        assert_eq!(store.load().unwrap().unwrap().document, document);
        document.touch();
        let raw = if legacy {
            version_seven(&document)
        } else {
            document.layers[1].blend_mode = BlendMode::Through;
            let mut raw = SNAPSHOT_V9.to_vec();
            bincode::DefaultOptions::new()
                .with_fixint_encoding()
                .serialize_into(&mut raw, &document)
                .unwrap();
            raw
        };
        insert(&store, document.revision, &raw);
        let loaded = store.load().unwrap().unwrap();
        assert!(loaded.fell_back);
        assert!(loaded.document.validate());
        assert_eq!(loaded.document.layers[2].blend_mode, BlendMode::Through);
        assert_eq!(loaded.document.layers[1].blend_mode, BlendMode::Normal);
        assert_eq!(
            snapshot_size(&loaded.document).unwrap(),
            bincode::DefaultOptions::new()
                .with_fixint_encoding()
                .serialized_size(&loaded.document)
                .unwrap()
                + SNAPSHOT_V10.len() as u64
        );
    }
}
