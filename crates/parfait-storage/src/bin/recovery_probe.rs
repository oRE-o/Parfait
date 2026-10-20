//! Subprocess fixture for crash recovery tests. Not an end-user application.
use parfait_core::{Brush, Document, Point, Stroke};
use parfait_storage::Store;
use std::{path::PathBuf, time::Duration};
fn main() -> anyhow::Result<()> {
    let args: Vec<_> = std::env::args().collect();
    let path = PathBuf::from(&args[1]);
    let ready = PathBuf::from(&args[2]);
    let mut doc = Document::new(64, 64);
    let mut store = Store::open(&path)?;
    store.mark_open()?;
    Stroke::begin(
        &mut doc,
        Brush::default(),
        Point { x: 32., y: 32., pressure: 1. },
    );
    doc.layers[1].blend_mode = parfait_core::BlendMode::Multiply;
    doc.layers[1].reference = true;
    doc.layers[1].draft = true;
    assert!(doc.create_layer_mask(1, 128));
    if args[3].starts_with("native-") {
        use parfait_core::{Affine, LayerContent, VectorContent, VectorPath};
        use std::sync::Arc;
        doc.create_content_layer(
            LayerContent::Vector(VectorContent {
                matrix: Affine::translation(8., 8.),
                paths: vec![VectorPath {
                    points: Arc::new(vec![[0., 0.], [32., 24.]]),
                    color: [20, 40, 100, 200],
                    width: 5.,
                    closed: false,
                    fill: false,
                }],
            }),
            "Vector".into(),
        )
        .unwrap();
    }
    store.save(&doc)?;
    if args[3].ends_with("during") {
        let conn = rusqlite::Connection::open(&path)?;
        conn.execute_batch("BEGIN IMMEDIATE; DELETE FROM snapshots; INSERT INTO snapshots VALUES(999,x'00',x'00');")?;
        std::fs::write(ready, "transaction-open")?;
        loop {
            std::thread::sleep(Duration::from_secs(1));
        }
    } else {
        doc.add_layer();
        doc.layers[1].mask.as_mut().unwrap().coverage =
            parfait_core::MaskCoverage::Dense(std::sync::Arc::new(vec![
                127;
                64 * 64
            ]));
        doc.touch();
        store.save(&doc)?;
        std::fs::write(ready, "committed")?;
        loop {
            std::thread::sleep(Duration::from_secs(1));
        }
    }
}
