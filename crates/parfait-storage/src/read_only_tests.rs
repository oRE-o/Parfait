use super::*;
#[test]
fn multi_document_read_only_never_creates_file_parent_or_schema() {
    let dir = tempfile::tempdir().unwrap();
    let absent = dir.path().join("absent/file.parfait");
    assert!(Store::open_read_only(&absent).is_err());
    assert!(!dir.path().join("absent").exists());
    let path = dir.path().join("empty.sqlite");
    let conn = Connection::open(&path).unwrap();
    conn.execute_batch("CREATE TABLE unrelated(value TEXT);").unwrap();
    drop(conn);
    let bytes = std::fs::read(&path).unwrap();
    assert!(Store::open_read_only(&path).unwrap().load().is_err());
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    let conn = Connection::open(&path).unwrap();
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
    conn.pragma_update(None, "user_version", 999).unwrap();
    drop(conn);
    let bytes = std::fs::read(&path).unwrap();
    assert!(Store::open_read_only(&path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
}
#[test]
fn multi_document_read_only_preserves_source_and_older_generation_fallback() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("그림.parfait");
    let mut store = Store::open(&path).unwrap();
    let document = Document::new(8, 8);
    store.save(&document).unwrap();
    store
        .connection
        .execute("INSERT INTO snapshots VALUES(100, zeroblob(32), X'0012')", [])
        .unwrap();
    store.checkpoint().unwrap();
    drop(store);
    let bytes = std::fs::read(&path).unwrap();
    let source = Store::open_read_only(&path).unwrap();
    let recovered = source.load().unwrap().unwrap();
    assert!(recovered.fell_back);
    assert_eq!(recovered.document.layers, document.layers);
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
}

#[test]
fn multi_document_decoded_pixel_capacity_matches_non_power_of_two_payload() {
    use parfait_core::{BufferCensus, LayerMask, MaskCoverage};
    use std::sync::Arc;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("capacity.parfait");
    let mut doc = Document::new(513, 257);
    let scalar = 513 * 257;
    doc.layers[1].mask = Some(LayerMask {
        enabled: true,
        coverage: MaskCoverage::Dense(Arc::new(vec![73; scalar])),
    });
    doc.selection = Some(MaskCoverage::Dense(Arc::new(vec![191; scalar])));
    let mut store = Store::open(&path).unwrap();
    store.save(&doc).unwrap();
    let loaded = store.load().unwrap().unwrap().document;
    assert_eq!(loaded, doc);
    let mut census = BufferCensus::default();
    census.add_document(&loaded);
    assert_eq!(census.bytes(), (scalar * 10) as u64);
    assert_eq!(census.bytes(), loaded.logical_pixel_bytes());
}
