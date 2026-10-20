use super::*;
use parfait_core::{Affine, LayerContent, TextContent};
use std::sync::Arc;
fn native() -> Document {
    let mut d = Document::new(64, 48);
    d.create_content_layer(LayerContent::Text(TextContent{text:"복구".into(),font:Arc::new(include_bytes!("../../../apps/parfait/assets/pretendard/Pretendard-Regular.otf").to_vec()),size:20.,color:[30,40,50,255],matrix:Affine::translation(2.,2.)}),"Text".into()).unwrap();
    d
}
#[test]
fn native_v11_roundtrip_exact_size_read_only_and_corrupt_head_fallback() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("native.parfait");
    let mut store = Store::open(&path).unwrap();
    let d = native();
    let encoded = bincode::DefaultOptions::new()
        .with_fixint_encoding()
        .serialize(&d)
        .unwrap();
    assert_eq!(
        snapshot_size(&d),
        Some(encoded.len() as u64 + SNAPSHOT_V11.len() as u64)
    );
    store.save(&d).unwrap();
    let version: i64 = store
        .connection
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(version, 11);
    let read = Store::open_read_only(&path).unwrap().load().unwrap().unwrap();
    assert_eq!(read.document, d);
    assert_eq!(read.document.art_digest_v1(), d.art_digest_v1());
    let node = read.document.native_content(read.document.active).unwrap();
    assert_eq!(
        node.render(d.width, d.height).unwrap(),
        read.document.layers[read.document.active].pixels
    );
    store
        .connection
        .execute(
            "INSERT INTO snapshots(revision,checksum,payload) VALUES(?1,?2,?3)",
            params![(d.revision + 1) as i64, vec![0_u8; 32], vec![1_u8; 16]],
        )
        .unwrap();
    let loaded = store.load().unwrap().unwrap();
    assert!(loaded.fell_back);
    assert_eq!(loaded.document.contents, d.contents);
    let mut raster = loaded.document;
    assert!(raster.rasterize_content(raster.active));
    store.save(&raster).unwrap();
    let version: i64 = store
        .connection
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(version, 11);
    assert!(store.load().unwrap().unwrap().document.contents.is_empty());
}
#[test]
fn normal_drawings_continue_writing_exact_v10_payload() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("raster.parfait");
    let mut store = Store::open(&path).unwrap();
    let d = Document::new(5, 3);
    store.save(&d).unwrap();
    let payload: Vec<u8> = store
        .connection
        .query_row(
            "SELECT payload FROM snapshots ORDER BY revision DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let mut raw = Vec::new();
    ZlibDecoder::new(payload.as_slice()).read_to_end(&mut raw).unwrap();
    let mut expected = SNAPSHOT_V10.to_vec();
    expected.extend(
        bincode::DefaultOptions::new()
            .with_fixint_encoding()
            .serialize(&legacy_v10::Borrowed::from(&d))
            .unwrap(),
    );
    assert_eq!(raw, expected);
    assert_eq!(snapshot_size(&d), Some(raw.len() as u64));
    assert_eq!(store.load().unwrap().unwrap().document, d);
}
