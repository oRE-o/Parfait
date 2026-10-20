//! Isolated process-kill fixture; never reads a user's application directory.
use parfait_core::{Document, EditStateId};
use parfait_storage::{RecoveryMetadata, SessionStore, Store, recovery_path};
use std::{path::PathBuf, sync::Arc, time::Duration};
fn main() -> anyhow::Result<()> {
    let args: Vec<_> = std::env::args().collect();
    let dir = PathBuf::from(&args[1]);
    let mode = &args[2];
    let mut session = SessionStore::open(&dir.join("session.sqlite"))?;
    session.begin_run()?;
    let mut doc = Document::new(16, 16);
    doc.layers[1].pixels = Arc::new([20, 40, 60, 255].repeat(256));
    let migration = mode.starts_with("migration-");
    let id = if migration {
        let mut old = Store::open(&dir.join("recovery.parfait"))?;
        old.save(&doc)?;
        old.checkpoint()?;
        drop(old);
        std::fs::copy(
            dir.join("recovery.parfait"),
            dir.join("original-bytes"),
        )?;
        session.prepare_migration()?
    } else {
        let id = [7; 16];
        session.register(id, None)?;
        id
    };
    if mode != "registered" && mode != "migration-prepared" {
        let metadata = RecoveryMetadata {
            document_id: id,
            edit_state_id: EditStateId::try_new()
                .map_err(|e| anyhow::anyhow!("{e}"))?
                .bytes(),
            logical_digest_version: 1,
            logical_digest: doc.art_digest_v1().unwrap().0,
            source_path: None,
            source_saved_state_id: None,
            source_saved_digest: None,
            source_head: None,
        };
        let mut store = Store::open(&recovery_path(&dir, id))?;
        store.mark_open()?;
        store.save_recovery(&doc, &metadata)?;
        if mode == "hint" {
            session.recovered(id, doc.revision)?;
        }
        if mode == "migration-committed" {
            session.commit_migration(id, doc.revision)?;
        }
        if mode == "transaction-open" {
            let connection =
                rusqlite::Connection::open(recovery_path(&dir, id))?;
            connection.execute_batch("BEGIN IMMEDIATE; DELETE FROM snapshots; DELETE FROM recovery_metadata;")?;
            std::fs::write(dir.join("ready"), "open transaction")?;
            loop {
                std::thread::sleep(Duration::from_secs(1));
            }
        }
    }
    if mode == "closed" || mode == "cleanup" {
        session.close_document(id)?;
    }
    if mode == "cleanup" {
        std::fs::remove_file(recovery_path(&dir, id))?;
    }
    if mode == "clean" {
        session.layout(&[id], Some(id), true)?;
    }
    std::fs::write(dir.join("ready"), "committed boundary")?;
    loop {
        std::thread::sleep(Duration::from_secs(1));
    }
}
