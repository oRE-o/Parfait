//! Transactional project snapshots. No UI dependency; disk work belongs to the worker.
mod budget;
mod coordinator;
mod source;
pub use source::{SavedSource, SourceEvent, SourceFailure, SourceRequest};
mod legacy;
mod legacy_v10;
mod legacy_v11;
mod legacy_v2;
mod legacy_v3;
mod legacy_v4;
mod legacy_v5;
mod legacy_v6;
mod legacy_v8;
mod legacy_v9;
#[cfg(test)]
mod migration_tests;
#[cfg(test)]
mod native_tests;
#[cfg(test)]
mod procedural_tests;
mod recovery;
#[cfg(test)]
mod selection_tests;
mod session;
#[cfg(test)]
mod through_tests;
mod work_budget;
mod worker;
pub use coordinator::{
    JobKey, RecoveryEvent, RecoveryRequest, StorageCoordinator,
};
pub use recovery::{NativePath, Recovered, RecoveryMetadata, SnapshotHead};
pub use session::{
    SessionRecord, SessionStore, document_hex, migrate_legacy, recovery_path,
};
const SNAPSHOT_V2: &[u8] = b"PARFAIT\0SNAP2\0";
const SNAPSHOT_V3: &[u8] = b"PARFAIT\0SNAP3\0";
const SNAPSHOT_V4: &[u8] = b"PARFAIT\0SNAP4\0";
const SNAPSHOT_V5: &[u8] = b"PARFAIT\0SNAP5\0";
const SNAPSHOT_V6: &[u8] = b"PARFAIT\0SNAP6\0";
const SNAPSHOT_V7: &[u8] = b"PARFAIT\0SNAP7\0";
const SNAPSHOT_V8: &[u8] = b"PARFAIT\0SNAP8\0";
const SNAPSHOT_V9: &[u8] = b"PARFAIT\0SNAP9\0";
const SNAPSHOT_V12: &[u8] = b"PARFAIT\0SNAP12\0";
const SNAPSHOT_V11: &[u8] = b"PARFAIT\0SNAP11\0";
const SNAPSHOT_V10: &[u8] = b"PARFAIT\0SNAP10\0";
use anyhow::{Context, Result, bail, ensure};
use bincode::Options;
#[cfg(test)]
use flate2::read::ZlibDecoder;
use flate2::{Compression, write::ZlibEncoder};
use fs2::FileExt;
use parfait_core::Document;
use rusqlite::{Connection, params};
use sha2::{Digest, Sha256};
#[cfg(test)]
use std::io::Read;
use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::Path,
    time::Duration,
};
pub use worker::{Autosave, SaveEvent};

const MAX_BYTES: u64 = budget::SNAPSHOT_LIMIT;
pub use budget::{
    layer_duplication_error, layer_folder_growth_error,
    layer_growth_count_error, layer_growth_error, layer_mask_creation_error,
    layer_reparent_error, mask_edit_error, selection_update_error,
    snapshot_size, solid_layer_growth_error, solid_rasterization_error,
};

/// Workspace and brush preferences are separate from artwork and committed atomically.
pub fn load_preferences(path: &Path) -> Result<Option<String>> {
    use rusqlite::OptionalExtension;
    let c = Connection::open(path)?;
    c.execute_batch("CREATE TABLE IF NOT EXISTS preferences(id INTEGER PRIMARY KEY CHECK(id=1), json TEXT NOT NULL)")?;
    Ok(c.query_row("SELECT json FROM preferences WHERE id=1", [], |r| r.get(0))
        .optional()?)
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
    pub head: SnapshotHead,
}
impl Store {
    /// Reads an existing source without migrations, table creation, or write pragmas.
    pub fn open_read_only(path: &Path) -> Result<Self> {
        let connection = Connection::open_with_flags(
            path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )?;
        connection.busy_timeout(Duration::from_secs(3))?;
        let version: i64 =
            connection
                .pragma_query_value(None, "user_version", |r| r.get(0))?;
        ensure!(
            (0..=12).contains(&version),
            "This project uses an unsupported format ({version})"
        );
        Ok(Self { connection })
    }
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) =
            path.parent().filter(|p| !p.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent)?;
        }
        let connection = Connection::open(path)?;
        connection.busy_timeout(Duration::from_secs(3))?;
        let version: i64 =
            connection
                .pragma_query_value(None, "user_version", |r| r.get(0))?;
        ensure!(
            (0..=12).contains(&version),
            "This project uses an unsupported format ({version})"
        );
        connection.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;
            CREATE TABLE IF NOT EXISTS snapshots(revision INTEGER PRIMARY KEY, checksum BLOB NOT NULL, payload BLOB NOT NULL);
            CREATE TABLE IF NOT EXISTS session(id INTEGER PRIMARY KEY CHECK(id=1), clean INTEGER NOT NULL);
            INSERT OR IGNORE INTO session VALUES(1,1);")?;
        Ok(Self { connection })
    }
    pub fn unclean(&self) -> Result<bool> {
        Ok(self.connection.query_row(
            "SELECT clean=0 FROM session WHERE id=1",
            [],
            |r| r.get(0),
        )?)
    }
    pub fn mark_open(&self) -> Result<()> {
        self.connection.execute("UPDATE session SET clean=0 WHERE id=1", [])?;
        Ok(())
    }
    pub fn mark_clean(&self) -> Result<()> {
        self.connection.execute("UPDATE session SET clean=1 WHERE id=1", [])?;
        Ok(())
    }
    pub fn save(&mut self, doc: &Document) -> Result<()> {
        self.save_inner(doc, None, None).map(|_| ())
    }
    pub fn save_original(
        &mut self,
        doc: &Document,
        expected: Option<SnapshotHead>,
    ) -> Result<SnapshotHead> {
        ensure!(
            expected.as_ref().is_none_or(|head| doc.revision > head.generation),
            "Original generation must advance"
        );
        self.save_inner(doc, None, Some(expected))
    }
    fn save_inner(
        &mut self,
        doc: &Document,
        metadata: Option<Vec<u8>>,
        source_check: Option<Option<SnapshotHead>>,
    ) -> Result<SnapshotHead> {
        ensure!(doc.validate(), "Invalid document");
        ensure!(
            doc.logical_pixel_bytes() <= MAX_BYTES,
            "Drawing exceeds 512 MiB logical pixel budget"
        );
        ensure!(
            snapshot_size(doc).is_some_and(|bytes| bytes <= MAX_BYTES),
            "Snapshot exceeds 512 MiB before encoding"
        );
        let reservation = work_budget::EncodingReservation::acquire(
            snapshot_size(doc).unwrap(),
        )?;
        let mut raw = work_budget::WorkBuffer::new(reservation.raw)?;
        let header = if !doc.layer_colors.is_empty() {
            SNAPSHOT_V12
        } else if doc.contents.is_empty() {
            SNAPSHOT_V10
        } else {
            SNAPSHOT_V11
        };
        raw.write_all(header)?;
        let options = bincode::DefaultOptions::new()
            .with_fixint_encoding()
            .with_limit(MAX_BYTES - header.len() as u64);
        if !doc.layer_colors.is_empty() {
            options.serialize_into(&mut raw, doc)?;
        } else if doc.contents.is_empty() {
            options
                .serialize_into(&mut raw, &legacy_v10::Borrowed::from(doc))?;
        } else {
            options
                .serialize_into(&mut raw, &legacy_v11::Borrowed::from(doc))?;
        }
        let raw = raw.finish();
        ensure!(raw.len() as u64 <= MAX_BYTES, "Snapshot too large");
        let checksum = Sha256::digest(&raw).to_vec();
        let compressed = work_budget::WorkBuffer::new(reservation.compressed)?;
        let mut enc = ZlibEncoder::new(compressed, Compression::fast());
        enc.write_all(&raw)?;
        let payload = enc.finish()?.finish();
        let head = SnapshotHead {
            generation: doc.revision,
            checksum: checksum.clone().try_into().unwrap(),
        };
        let tx = self.connection.transaction_with_behavior(
            rusqlite::TransactionBehavior::Immediate,
        )?;
        if let Some(expected) = source_check {
            use rusqlite::OptionalExtension;
            let actual=tx.query_row("SELECT revision,checksum FROM snapshots ORDER BY revision DESC LIMIT 1",[],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,Vec<u8>>(1)?))).optional()?;
            let actual = actual
                .map(|(generation, checksum)| -> Result<SnapshotHead> {
                    Ok(SnapshotHead {
                        generation: u64::try_from(generation)?,
                        checksum: checksum.try_into().map_err(|_| {
                            anyhow::anyhow!("Invalid original head")
                        })?,
                    })
                })
                .transpose()?;
            if actual != expected {
                return Err(source::SourceConflict(actual).into());
            }
        }
        tx.execute(
            "INSERT OR REPLACE INTO snapshots VALUES(?1,?2,?3)",
            params![i64::try_from(doc.revision)?, &checksum, payload],
        )?;
        tx.execute("DELETE FROM snapshots WHERE revision NOT IN (SELECT revision FROM snapshots ORDER BY revision DESC LIMIT 3)",[])?;
        recovery::write_metadata(
            &tx,
            &SnapshotHead {
                generation: doc.revision,
                checksum: checksum.try_into().unwrap(),
            },
            metadata,
        )?;
        let previous: i64 =
            tx.pragma_query_value(None, "user_version", |r| r.get(0))?;
        tx.pragma_update(
            None,
            "user_version",
            previous.max(if !doc.layer_colors.is_empty() {
                12
            } else if doc.contents.is_empty() {
                10
            } else {
                11
            }),
        )?;
        tx.commit()?;
        Ok(head)
    }
    pub fn load(&self) -> Result<Option<Loaded>> {
        let _workspace = work_budget::EncodingReservation::acquire(MAX_BYTES)?;
        let mut stmt = self.connection.prepare(
            "SELECT CASE WHEN length(checksum)=32 THEN checksum ELSE NULL END, CASE WHEN length(payload) <= ?1 THEN payload ELSE NULL END, revision FROM snapshots ORDER BY revision DESC",
        )?;
        let rows = stmt.query_map(
            [work_budget::compressed_bound(MAX_BYTES).unwrap() as i64],
            |r| {
                Ok((
                    r.get::<_, Option<Vec<u8>>>(0)?,
                    r.get::<_, Option<Vec<u8>>>(1)?,
                    r.get::<_, i64>(2)?,
                ))
            },
        )?;
        let mut bad = false;
        for row in rows {
            let (hash, payload, generation) = row?;
            let decode = || -> Result<Document> {
                ensure!(generation >= 0, "Invalid snapshot generation");
                let payload = payload.as_ref().ok_or_else(|| {
                    anyhow::anyhow!("Compressed snapshot too large")
                })?;
                let hash = hash.as_ref().ok_or_else(|| {
                    anyhow::anyhow!("Invalid checksum length")
                })?;
                let raw = work_budget::decode_bounded(payload, MAX_BYTES)?;
                ensure!(
                    Sha256::digest(&raw).as_slice() == hash.as_slice(),
                    "Snapshot checksum mismatch"
                );
                let options = bincode::DefaultOptions::new()
                    .with_fixint_encoding()
                    .with_limit(MAX_BYTES)
                    .reject_trailing_bytes();
                let doc: Document = if let Some(payload) =
                    raw.strip_prefix(SNAPSHOT_V12)
                {
                    options.deserialize(payload)?
                } else if let Some(payload) = raw.strip_prefix(SNAPSHOT_V11) {
                    let old: legacy_v11::LegacyDocument =
                        options.deserialize(payload)?;
                    old.into()
                } else if let Some(payload) = raw.strip_prefix(SNAPSHOT_V10) {
                    let old: legacy_v10::LegacyDocument =
                        options.deserialize(payload)?;
                    old.into()
                } else if let Some(payload) = raw.strip_prefix(SNAPSHOT_V9) {
                    let old: legacy_v9::LegacyDocument =
                        options.deserialize(payload)?;
                    old.into()
                } else if let Some(payload) = raw.strip_prefix(SNAPSHOT_V8) {
                    let old: legacy_v8::LegacyDocument =
                        options.deserialize(payload)?;
                    old.into()
                } else if let Some(payload) = raw.strip_prefix(SNAPSHOT_V7) {
                    let old: legacy_v8::LegacyDocument =
                        options.deserialize(payload)?;
                    ensure!(
                        old.layers.iter().all(|layer| layer.blend_mode
                            != parfait_core::BlendMode::Through),
                        "Version 7 cannot contain a pass-through folder"
                    );
                    old.into()
                } else if let Some(payload) = raw.strip_prefix(SNAPSHOT_V6) {
                    let old: legacy_v6::LegacyDocument =
                        options.deserialize(payload)?;
                    old.into()
                } else if let Some(payload) = raw.strip_prefix(SNAPSHOT_V5) {
                    let old: legacy_v5::LegacyDocument =
                        options.deserialize(payload)?;
                    old.into()
                } else if let Some(payload) = raw.strip_prefix(SNAPSHOT_V4) {
                    let old: legacy_v4::LegacyDocument =
                        options.deserialize(payload)?;
                    old.into()
                } else if let Some(payload) = raw.strip_prefix(SNAPSHOT_V3) {
                    let old: legacy_v3::LegacyDocument =
                        options.deserialize(payload)?;
                    old.into()
                } else if let Some(payload) = raw.strip_prefix(SNAPSHOT_V2) {
                    let old: legacy_v2::LegacyDocument =
                        options.deserialize(payload)?;
                    old.into()
                } else {
                    ensure!(
                        !raw.starts_with(b"PARFAIT\0"),
                        "Unsupported snapshot version"
                    );
                    let old: legacy::LegacyDocument =
                        options.deserialize(&raw)?;
                    old.into()
                };
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
                        head: SnapshotHead {
                            generation: u64::try_from(generation)?,
                            checksum: hash.unwrap().try_into().unwrap(),
                        },
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
        let (busy, _, _): (i64, i64, i64) = self.connection.query_row(
            "PRAGMA wal_checkpoint(TRUNCATE)",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
        ensure!(busy == 0, "Checkpoint is busy; committed WAL is retained");
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
    file.try_lock_exclusive().context(
        "This recovery folder is already open in another Parfait window",
    )?;
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
        d.layers[d.active].blend_mode = parfait_core::BlendMode::Multiply;
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
    fn legacy_snapshots_upgrade_and_remain_recoverable() {
        use legacy::{LegacyDocument, LegacyLayer};
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("old.parfait");
        let mut store = Store::open(&path).unwrap();
        let old = LegacyDocument {
            width: 1,
            height: 1,
            name: "Old drawing".into(),
            active: 0,
            revision: 1,
            layers: vec![LegacyLayer {
                name: "Ink".into(),
                visible: true,
                locked: false,
                opacity: 0.75,
                pixels: std::sync::Arc::new(vec![30, 60, 90, 128]),
            }],
        };
        let raw = bincode::DefaultOptions::new()
            .with_fixint_encoding()
            .serialize(&old)
            .unwrap();
        let mut enc = ZlibEncoder::new(Vec::new(), Compression::fast());
        enc.write_all(&raw).unwrap();
        store
            .connection
            .execute(
                "INSERT INTO snapshots VALUES(1,?1,?2)",
                params![Sha256::digest(&raw).to_vec(), enc.finish().unwrap()],
            )
            .unwrap();
        store.connection.pragma_update(None, "user_version", 1).unwrap();
        let mut d = store.load().unwrap().unwrap().document;
        assert_eq!(d.layers[0].blend_mode, parfait_core::BlendMode::Normal);
        assert_eq!(d.layers[0].pixels.as_slice(), &[30, 60, 90, 128]);
        d.layers[0].blend_mode = parfait_core::BlendMode::Screen;
        d.touch();
        store.save(&d).unwrap();
        drop(store);
        let store = Store::open(&path).unwrap();
        assert_eq!(store.load().unwrap().unwrap().document, d);
        let version: i64 = store
            .connection
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(version, 10);
        store
            .connection
            .execute("UPDATE snapshots SET payload=x'00' WHERE revision=2", [])
            .unwrap();
        let recovered = store.load().unwrap().unwrap();
        assert!(recovered.fell_back);
        assert_eq!(
            recovered.document.layers[0].blend_mode,
            parfait_core::BlendMode::Normal
        );
        assert_eq!(recovered.document.name, "Old drawing");
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

#[cfg(test)]
mod read_only_tests;

#[cfg(test)]
mod stage3_tests;
