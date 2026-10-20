use super::*;
use parfait_core::{BlendMode, MaskCoverage, SelectionMode};
use std::sync::Arc;

fn inject(store: &Store, revision: u64, raw: &[u8]) {
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

fn drawing() -> Document {
    let mut document = Document::new(4, 3);
    document.layers[1].pixels = Arc::new(
        (0..12).flat_map(|index| [30, 60, 90, (index * 21) as u8]).collect(),
    );
    document.group_layers_in_folder(2, "Folder".into()).unwrap();
    document.layers[2].blend_mode = BlendMode::Through;
    document.create_layer_mask(2, 193);
    document.layers[1].reference = true;
    document.layers[1].draft = true;
    document
}

#[test]
fn exact_v8_tree_snapshot_upgrades_without_selection_or_other_metadata_changes()
{
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("old.parfait");
    let mut store = Store::open(&path).unwrap();
    let document = drawing();
    let mut raw = SNAPSHOT_V8.to_vec();
    bincode::DefaultOptions::new()
        .with_fixint_encoding()
        .serialize_into(
            &mut raw,
            &(
                document.width,
                document.height,
                &document.name,
                &document.layers,
                document.active,
                document.revision,
                document.next_layer_id,
            ),
        )
        .unwrap();
    assert_eq!(snapshot_size(&document).unwrap(), raw.len() as u64 + 2);
    inject(&store, document.revision, &raw);
    store.connection.pragma_update(None, "user_version", 8).unwrap();
    let mut restored = store.load().unwrap().unwrap().document;
    assert_eq!(restored, document);
    assert!(restored.selection_from_layers(4, SelectionMode::Replace));
    store.save(&restored).unwrap();
    assert_eq!(
        store
            .connection
            .pragma_query_value::<i64, _>(None, "user_version", |row| row
                .get(0))
            .unwrap(),
        10
    );
    drop(store);
    assert_eq!(
        Store::open(&path).unwrap().load().unwrap().unwrap().document,
        restored
    );
}

#[test]
fn selection_shapes_and_fractional_masks_roundtrip_with_exact_snapshot_accounting()
 {
    for selection in [
        None,
        Some(MaskCoverage::Uniform(0)),
        Some(MaskCoverage::Uniform(127)),
        Some(MaskCoverage::Uniform(255)),
        Some(MaskCoverage::Dense(Arc::new(
            (0..12).map(|index| index * 21).collect(),
        ))),
    ] {
        let directory = tempfile::tempdir().unwrap();
        let mut store =
            Store::open(&directory.path().join("selection.parfait")).unwrap();
        let mut document = drawing();
        document.set_selection(selection);
        if document.selection.is_some() {
            document.create_mask_from_selection(1, true);
        }
        store.save(&document).unwrap();
        assert_eq!(store.load().unwrap().unwrap().document, document);
        let raw_len = store
            .connection
            .query_row(
                "SELECT payload FROM snapshots ORDER BY revision DESC LIMIT 1",
                [],
                |row| row.get::<_, Vec<u8>>(0),
            )
            .unwrap();
        let mut raw = Vec::new();
        ZlibDecoder::new(raw_len.as_slice()).read_to_end(&mut raw).unwrap();
        assert!(raw.starts_with(SNAPSHOT_V10));
        assert_eq!(snapshot_size(&document).unwrap(), raw.len() as u64);
    }
}

#[test]
fn malformed_selection_head_falls_back_and_invalid_saves_leave_committed_data_untouched()
 {
    let directory = tempfile::tempdir().unwrap();
    let mut store =
        Store::open(&directory.path().join("broken.parfait")).unwrap();
    let mut document = drawing();
    document.select_all_pixels();
    store.save(&document).unwrap();
    let original = document.clone();
    document.selection = Some(MaskCoverage::Dense(Arc::new(vec![127; 11])));
    document.touch();
    assert!(!document.validate());
    assert!(store.save(&document).is_err());
    assert_eq!(store.load().unwrap().unwrap().document, original);
    let mut raw = SNAPSHOT_V9.to_vec();
    bincode::DefaultOptions::new()
        .with_fixint_encoding()
        .serialize_into(&mut raw, &document)
        .unwrap();
    inject(&store, document.revision, &raw);
    let loaded = store.load().unwrap().unwrap();
    assert!(loaded.fell_back);
    assert!(loaded.document.revision > document.revision);
    let mut expected = original;
    expected.revision = loaded.document.revision;
    assert_eq!(loaded.document, expected);
}
