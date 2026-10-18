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
        Point {
            x: 32.,
            y: 32.,
            pressure: 1.,
        },
    );
    store.save(&doc)?;
    if args[3] == "during" {
        let conn = rusqlite::Connection::open(&path)?;
        conn.execute_batch("BEGIN IMMEDIATE; DELETE FROM snapshots; INSERT INTO snapshots VALUES(999,x'00',x'00');")?;
        std::fs::write(ready, "transaction-open")?;
        loop {
            std::thread::sleep(Duration::from_secs(1));
        }
    } else {
        doc.add_layer();
        store.save(&doc)?;
        std::fs::write(ready, "committed")?;
        loop {
            std::thread::sleep(Duration::from_secs(1));
        }
    }
}
