use parfait_core::{Document, LayerKind, LayerMask, MaskCoverage};
use parfait_storage::Store;
use std::sync::Arc;

#[test]
fn multi_document_digest_matches_an_independently_decoded_v10_snapshot() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("drawing.parfait");
    let mut document = Document::new(8, 6);
    document.layers[0].kind = LayerKind::Paper([240, 230, 220]);
    document.layers[0].pixels = Arc::new(Vec::new());
    document.layers[1].pixels = Arc::new([20, 30, 40, 128].repeat(48));
    document.layers[1].mask =
        Some(LayerMask { enabled: true, coverage: MaskCoverage::Uniform(127) });
    document.selection = Some(MaskCoverage::Dense(Arc::new(vec![128; 48])));
    document.group_layers_in_folder(2, "Group".into()).unwrap();
    let expected = document.art_digest_v1().unwrap();
    let mut store = Store::open(&path).unwrap();
    store.save(&document).unwrap();
    store.checkpoint().unwrap();
    drop(store);
    let mut decoded =
        Store::open(&path).unwrap().load().unwrap().unwrap().document;
    assert_eq!(decoded, document);
    assert!(!Arc::ptr_eq(
        &decoded.layers[1].pixels,
        &document.layers[1].pixels
    ));
    decoded.name = "Different file name".into();
    decoded.active = 0;
    decoded.touch();
    assert_eq!(decoded.art_digest_v1(), Some(expected));
    decoded.selection = Some(MaskCoverage::Uniform(127));
    assert_ne!(decoded.art_digest_v1(), Some(expected));
}
