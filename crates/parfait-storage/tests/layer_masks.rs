use parfait_core::{Document, History, LayerMask, MaskCoverage};
use parfait_storage::{Store, snapshot_size};
use std::sync::Arc;

#[test]
fn dense_and_uniform_masks_preserve_raw_pixels_and_history_after_reopen() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("masks.parfait");
    let mut document = Document::new(11, 7);
    Arc::make_mut(&mut document.layers[1].pixels).fill(127);
    let original_pixels = document.layers[1].pixels.clone();
    document.layers[1].mask = Some(LayerMask {
        enabled: true,
        coverage: MaskCoverage::Dense(Arc::new((0..77).map(|index| (index * 29) as u8).collect())),
    });
    document.touch();
    let mut history = History::default();
    history.begin(&document, "Disable mask");
    assert!(document.set_layer_mask_enabled(1, false));
    assert!(history.commit(&document));
    history.undo(&mut document);
    let masked_pixels = document.composite();
    let mut store = Store::open(&path).unwrap();
    store.save(&document).unwrap();
    drop(store);
    let mut store = Store::open(&path).unwrap();
    let mut loaded = store.load().unwrap().unwrap().document;
    assert_eq!(loaded, document);
    assert_eq!(loaded.composite(), masked_pixels);
    assert_eq!(loaded.layers[1].pixels, original_pixels);
    assert!(snapshot_size(&loaded).unwrap() < 1024 * 1024);
    assert!(loaded.apply_layer_mask(1));
    assert!(loaded.layers[1].mask.is_none());
    assert_eq!(loaded.composite(), masked_pixels);
    store.save(&loaded).unwrap();
    assert_eq!(store.load().unwrap().unwrap().document, loaded);
    assert_eq!(document.layers[1].pixels, original_pixels);
}
