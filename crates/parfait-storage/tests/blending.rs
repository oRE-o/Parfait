use parfait_core::{Brush, BrushOperation, Document, Point, Stroke};
use parfait_storage::Store;
use std::sync::Arc;

#[test]
fn blended_pixels_survive_reopen_and_corrupt_snapshot_fallback() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("blending.parfait");
    let mut document = Document::new(96, 64);
    for (index, pixel) in Arc::make_mut(&mut document.layers[1].pixels)
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .enumerate()
    {
        pixel.copy_from_slice(&[(index % 255) as u8, 90, 220, 180]);
    }
    for operation in [
        BrushOperation::Blend,
        BrushOperation::Blur,
        BrushOperation::Smudge,
    ] {
        let mut stroke = Stroke::begin(
            &mut document,
            Brush {
                operation,
                size: 20.,
                pressure: false,
                ..Default::default()
            },
            Point {
                x: 12.,
                y: 25.,
                pressure: 1.,
            },
        );
        stroke.extend(
            &mut document,
            Point {
                x: 80.,
                y: 30.,
                pressure: 1.,
            },
        );
    }
    let expected = document.clone();
    let mut store = Store::open(&path).unwrap();
    store.mark_open().unwrap();
    store.save(&document).unwrap();
    drop(store);
    let mut store = Store::open(&path).unwrap();
    assert!(store.unclean().unwrap());
    let loaded = store.load().unwrap().unwrap();
    assert!(!loaded.fell_back);
    assert_eq!(loaded.document, expected);
    document.touch();
    store.save(&document).unwrap();
    drop(store);
    rusqlite::Connection::open(&path)
        .unwrap()
        .execute(
            "UPDATE snapshots SET payload=x'0001' WHERE revision=?1",
            [document.revision as i64],
        )
        .unwrap();
    let loaded = Store::open(&path).unwrap().load().unwrap().unwrap();
    assert!(loaded.fell_back);
    assert!(loaded.document.layers == expected.layers);
    assert_eq!(loaded.document.active, expected.active);
    assert!(loaded.document.revision > document.revision);
}
