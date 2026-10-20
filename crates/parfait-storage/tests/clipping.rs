use parfait_core::{Document, History};
use parfait_storage::Store;
use std::sync::Arc;

#[test]
fn saved_clipping_reopens_with_hidden_pixels_and_history_roundtrips() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("clipping.parfait");
    let mut document = Document::new(3, 1);
    document.layers[0].visible = false;
    document.layers[1].pixels =
        Arc::new(vec![80, 120, 200, 128, 0, 0, 0, 0, 0, 0, 0, 0]);
    document.add_layer();
    document.layers[2].pixels = Arc::new([255, 32, 48, 255].repeat(3));
    let mut history = History::default();
    history.begin(&document, "Clip to layer below");
    document.layers[2].clipped = true;
    document.layers[2].alpha_locked = true;
    document.touch();
    history.commit(&document);
    let expected = document.clone();
    let mut store = Store::open(&path).unwrap();
    store.save(&document).unwrap();
    store.checkpoint().unwrap();
    drop(store);
    let mut document =
        Store::open(&path).unwrap().load().unwrap().unwrap().document;
    assert_eq!(document, expected);
    assert_eq!(
        document.composite(),
        [255, 32, 48, 128, 0, 0, 0, 0, 0, 0, 0, 0]
    );
    assert!(history.undo(&mut document));
    assert_eq!(&document.composite()[8..], &[255, 32, 48, 255]);
    assert!(history.redo(&mut document));
    assert_eq!(document.composite(), expected.composite());
}
