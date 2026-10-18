//! Transactional project snapshots. No UI dependency; disk work belongs to the worker.
mod worker;
use anyhow::{Context, Result, bail, ensure};
use bincode::Options;
use flate2::{Compression, read::ZlibDecoder, write::ZlibEncoder};
use fs2::FileExt;
use parfait_core::Document;
use rusqlite::{Connection, params};
use sha2::{Digest, Sha256};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::Path,
    time::Duration,
};
pub use worker::{Autosave, SaveEvent};

const MAX_BYTES: u64 = 512 * 1024 * 1024;

/// Workspace and brush preferences are separate from artwork and committed atomically.
pub fn load_preferences(path: &Path) -> Result<Option<String>> {
    use rusqlite::OptionalExtension;
    let c = Connection::open(path)?;
    c.execute_batch("CREATE TABLE IF NOT EXISTS preferences(id INTEGER PRIMARY KEY CHECK(id=1), json TEXT NOT NULL)")?;
    Ok(
        c.query_row("SELECT json FROM preferences WHERE id=1", [], |r| r.get(0))
            .optional()?,
    )
}
pub fn save_preferences(path: &Path, json: &str) -> Result<()> {
    ensure!(json.len() <= 32 * 1024 * 1024, "Preferences exceed 32 MiB");
    let c = Connection::open(path)?;
    c.execute_batch("PRAGMA synchronous=FULL; CREATE TABLE IF NOT EXISTS preferences(id INTEGER PRIMARY KEY CHECK(id=1), json TEXT NOT NULL)")?;
    c.execute("INSERT OR REPLACE INTO preferences VALUES(1,?1)", [json])?;
    Ok(())
}
pub struct Store {
    connection: Connection,
}
pub struct Loaded {
    pub document: Document,
    pub fell_back: bool,
}
impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        let connection = Connection::open(path)?;
        connection.busy_timeout(Duration::from_secs(3))?;
        let version: i64 = connection.pragma_query_value(None, "user_version", |r| r.get(0))?;
        ensure!(version <= 1, "This project uses a newer format ({version})");
        connection.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;
            CREATE TABLE IF NOT EXISTS snapshots(revision INTEGER PRIMARY KEY, checksum BLOB NOT NULL, payload BLOB NOT NULL);
            CREATE TABLE IF NOT EXISTS session(id INTEGER PRIMARY KEY CHECK(id=1), clean INTEGER NOT NULL);
            INSERT OR IGNORE INTO session VALUES(1,1); PRAGMA user_version=1;")?;
        Ok(Self { connection })
    }
    pub fn unclean(&self) -> Result<bool> {
        Ok(self
            .connection
            .query_row("SELECT clean=0 FROM session WHERE id=1", [], |r| r.get(0))?)
    }
    pub fn mark_open(&self) -> Result<()> {
        self.connection
            .execute("UPDATE session SET clean=0 WHERE id=1", [])?;
        Ok(())
    }
    pub fn mark_clean(&self) -> Result<()> {
        self.connection
            .execute("UPDATE session SET clean=1 WHERE id=1", [])?;
        Ok(())
    }
    pub fn save(&mut self, doc: &Document) -> Result<()> {
        ensure!(doc.validate(), "Invalid document");
        let raw = bincode::DefaultOptions::new()
            .with_fixint_encoding()
            .with_limit(MAX_BYTES)
            .serialize(doc)?;
        let checksum = Sha256::digest(&raw).to_vec();
        let mut enc = ZlibEncoder::new(Vec::new(), Compression::fast());
        enc.write_all(&raw)?;
        let payload = enc.finish()?;
        let tx = self.connection.transaction()?;
        tx.execute(
            "INSERT OR REPLACE INTO snapshots VALUES(?1,?2,?3)",
            params![i64::try_from(doc.revision)?, checksum, payload],
        )?;
        tx.execute("DELETE FROM snapshots WHERE revision NOT IN (SELECT revision FROM snapshots ORDER BY revision DESC LIMIT 3)",[])?;
        tx.commit()?;
        Ok(())
    }
    pub fn load(&self) -> Result<Option<Loaded>> {
        let mut stmt = self
            .connection
            .prepare("SELECT checksum,payload FROM snapshots ORDER BY revision DESC")?;
        let rows = stmt.query_map([], |r| {
            Ok((r.get::<_, Vec<u8>>(0)?, r.get::<_, Vec<u8>>(1)?))
        })?;
        let mut bad = false;
        for row in rows {
            let (hash, payload) = row?;
            let decode = || -> Result<Document> {
                let mut raw = Vec::new();
                ZlibDecoder::new(payload.as_slice())
                    .take(MAX_BYTES + 1)
                    .read_to_end(&mut raw)?;
                ensure!(raw.len() as u64 <= MAX_BYTES, "Snapshot too large");
                ensure!(
                    Sha256::digest(&raw).as_slice() == hash,
                    "Snapshot checksum mismatch"
                );
                let doc: Document = bincode::DefaultOptions::new()
                    .with_fixint_encoding()
                    .with_limit(MAX_BYTES)
                    .reject_trailing_bytes()
                    .deserialize(&raw)?;
                ensure!(doc.validate(), "Invalid document layout");
                Ok(doc)
            };
            match decode() {
                Ok(mut document) => {
                    // A recovered older generation must supersede the broken generation on next save.
                    let max: i64 = self.connection.query_row(
                        "SELECT MAX(revision) FROM snapshots",
                        [],
                        |r| r.get(0),
                    )?;
                    if bad {
                        document.revision = u64::try_from(max)? + 1;
                    }
                    return Ok(Some(Loaded {
                        document,
                        fell_back: bad,
                    }));
                }
                Err(_) => bad = true,
            }
        }
        if bad {
            bail!("No valid recovery generation. Original database preserved.");
        }
        Ok(None)
    }
    pub fn checkpoint(&self) -> Result<()> {
        self.connection
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")?;
        Ok(())
    }
}

/// Held for the entire application lifetime, including final flush.
pub fn lock_session(path: &Path) -> Result<File> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path)?;
    file.try_lock_exclusive()
        .context("This recovery folder is already open in another Parfait window")?;
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roundtrip_and_corrupt_generation_fallback() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("doc.parfait");
        let mut s = Store::open(&path).unwrap();
        let mut d = Document::new(8, 8);
        s.save(&d).unwrap();
        d.add_layer();
        s.save(&d).unwrap();
        assert_eq!(s.load().unwrap().unwrap().document, d);
        s.connection
            .execute(
                "UPDATE snapshots SET payload=x'00' WHERE revision=?1",
                [d.revision as i64],
            )
            .unwrap();
        let loaded = s.load().unwrap().unwrap();
        assert!(loaded.fell_back);
        assert_eq!(loaded.document.layers.len(), 2);
        assert!(loaded.document.revision > d.revision);
        s.save(&loaded.document).unwrap();
        assert!(!s.load().unwrap().unwrap().fell_back);
    }
    #[test]
    fn session_lock_is_exclusive() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("lock");
        let a = lock_session(&path).unwrap();
        assert!(lock_session(&path).is_err());
        drop(a);
        assert!(lock_session(&path).is_ok());
    }
}
