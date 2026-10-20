use parfait_core::{
    Brush, BrushOperation, Document, History, OpacityMode, Point, Stroke,
};
use parfait_storage::Store;
use std::sync::Arc;

#[test]
fn capped_paint_and_erase_survive_history_reopen_and_snapshot_fallback() {
    for operation in [BrushOperation::Paint, BrushOperation::Erase] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("wash.parfait");
        let mut document = Document::new(65, 67);
        Arc::make_mut(&mut document.layers[1].pixels)
            .as_chunks_mut::<4>()
            .0
            .fill([190, 80, 20, 200]);
        let original = document.clone();
        let mut history = History::default();
        history.checkpoint(&document);
        let options = Brush {
            operation,
            opacity_mode: OpacityMode::PerStroke,
            size: 24.,
            opacity: 0.4,
            flow: 0.35,
            pressure: false,
            continuous_spray: true,
            spray_rate: 120.,
            ..Default::default()
        };
        let center = Point { x: 32.5, y: 32.5, pressure: 1. };
        let mut stroke = Stroke::begin(&mut document, options.clone(), center);
        for _ in 0..20 {
            stroke.spray(&mut document, 0.1, 1.);
        }
        drop(stroke);
        let painted = document.layers.clone();
        assert_ne!(painted, original.layers);
        let painted_composite = document.composite();
        history.undo(&mut document);
        assert_eq!(document.layers, original.layers);
        history.redo(&mut document);
        assert_eq!(document.layers, painted);
        assert_eq!(document.composite(), painted_composite);
        let expected = document.clone();
        let mut store = Store::open(&path).unwrap();
        store.mark_open().unwrap();
        store.save(&document).unwrap();
        drop(store);
        let mut store = Store::open(&path).unwrap();
        assert!(store.unclean().unwrap());
        let restored = store.load().unwrap().unwrap();
        assert!(!restored.fell_back);
        assert_eq!(restored.document, expected);
        document = restored.document;
        Stroke::begin(&mut document, options, center);
        assert_ne!(document.layers, expected.layers);
        store.save(&document).unwrap();
        drop(store);
        rusqlite::Connection::open(&path)
            .unwrap()
            .execute(
                "UPDATE snapshots SET payload=x'0001' WHERE revision=?1",
                [document.revision as i64],
            )
            .unwrap();
        let recovered = Store::open(&path).unwrap().load().unwrap().unwrap();
        assert!(recovered.fell_back);
        assert_eq!(recovered.document.layers, expected.layers);
        assert_eq!(recovered.document.composite(), painted_composite);
        assert!(recovered.document.revision > document.revision);
    }
}
