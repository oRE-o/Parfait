//! Small, independent session registry. Artwork never enters this database.
use crate::{NativePath, RecoveryMetadata, Store};
use anyhow::{Result, ensure};
use bincode::Options;
use parfait_core::EditStateId;
use rusqlite::{Connection, OptionalExtension, params};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    time::Duration,
};
const SESSION_LIMIT: i64 = 8 * 1024 * 1024;
#[derive(Clone, Debug)]
pub struct SessionRecord {
    pub document_id: [u8; 16],
    pub order: u64,
    pub lifecycle: u8,
    pub source_path: Option<NativePath>,
    pub last_generation: Option<u64>,
    pub recoverable: bool,
}
pub struct SessionStore {
    connection: Connection,
}
pub fn document_hex(id: [u8; 16]) -> String {
    id.iter().map(|v| format!("{v:02x}")).collect()
}
pub fn recovery_path(dir: &Path, id: [u8; 16]) -> PathBuf {
    dir.join("recovery").join(document_hex(id)).join("recovery.parfait")
}
fn id(value: Vec<u8>) -> Result<[u8; 16]> {
    value.try_into().map_err(|_| anyhow::anyhow!("Invalid document ID"))
}
fn path_encode(value: &Option<NativePath>) -> Result<Option<Vec<u8>>> {
    value
        .as_ref()
        .map(|path| {
            path.decode()?;
            Ok(bincode::serialize(path)?)
        })
        .transpose()
}
fn path_decode(value: Option<Vec<u8>>) -> Result<Option<NativePath>> {
    value
        .map(|bytes| {
            ensure!(
                bytes.len() <= crate::recovery::PATH_LIMIT + 32,
                "Oversized source path"
            );
            let path: NativePath = bincode::DefaultOptions::new()
                .with_fixint_encoding()
                .with_limit((crate::recovery::PATH_LIMIT + 32) as u64)
                .reject_trailing_bytes()
                .deserialize(&bytes)?;
            path.decode()?;
            Ok(path)
        })
        .transpose()
}
impl SessionStore {
    pub fn open(path: &Path) -> Result<Self> {
        let exists = path.exists();
        if exists {
            let probe = Connection::open_with_flags(
                path,
                rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
            )?;
            let version: i64 =
                probe.pragma_query_value(None, "user_version", |r| r.get(0))?;
            ensure!(
                version == 1,
                "Unsupported or uninitialized session registry ({version}); preserve this directory and choose another data location"
            );
            Self { connection: probe }.records()?;
        } else if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let connection = Connection::open(path)?;
        connection.busy_timeout(Duration::from_secs(3))?;
        if !exists {
            connection.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;
                BEGIN IMMEDIATE;
                CREATE TABLE meta(id INTEGER PRIMARY KEY CHECK(id=1),run_id BLOB NOT NULL,clean_shutdown INTEGER NOT NULL,active_document_id BLOB);
                INSERT INTO meta VALUES(1,zeroblob(16),1,NULL);
                CREATE TABLE documents(document_id BLOB PRIMARY KEY,tab_order INTEGER NOT NULL,lifecycle INTEGER NOT NULL,source_path BLOB,last_generation INTEGER,recoverable INTEGER NOT NULL);
                CREATE UNIQUE INDEX open_order ON documents(tab_order) WHERE lifecycle=0;
                CREATE TABLE migrations(migration_key TEXT PRIMARY KEY,target_document_id BLOB NOT NULL,state INTEGER NOT NULL);
                PRAGMA user_version=1; COMMIT;")?;
        } else {
            connection.execute_batch("PRAGMA synchronous=FULL;")?;
        }
        let store = Self { connection };
        store.records()?;
        Ok(store)
    }
    pub fn records(&self) -> Result<Vec<SessionRecord>> {
        let size:i64=self.connection.query_row("SELECT COALESCE(SUM(64+length(document_id)+COALESCE(length(source_path),0)),0) FROM documents",[],|r|r.get(0))?;
        let extra:i64=self.connection.query_row("SELECT COALESCE(SUM(64+length(migration_key)+length(target_document_id)),0) FROM migrations",[],|r|r.get(0))?;
        ensure!(
            size.checked_add(extra).is_some_and(|size| size <= SESSION_LIMIT),
            "Session registry exceeds 8 MiB"
        );
        let mut stmt=self.connection.prepare("SELECT document_id,tab_order,lifecycle,source_path,last_generation,recoverable FROM documents ORDER BY tab_order,document_id")?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, Vec<u8>>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, Option<Vec<u8>>>(3)?,
                r.get::<_, Option<i64>>(4)?,
                r.get::<_, i64>(5)?,
            ))
        })?;
        let mut result = Vec::new();
        let mut open_order = HashSet::new();
        for row in rows {
            let (key, order, life, path, generation, recoverable) = row?;
            ensure!(
                (0..=2).contains(&life) && (0..=1).contains(&recoverable),
                "Invalid session lifecycle"
            );
            ensure!(
                (0..=SESSION_LIMIT).contains(&order)
                    && generation.is_none_or(|g| g >= 0),
                "Invalid recovery generation/order"
            );
            if life == 0 {
                ensure!(open_order.insert(order), "Duplicate open tab order");
            }
            result.push(SessionRecord {
                document_id: id(key)?,
                order: order as u64,
                lifecycle: life as u8,
                source_path: path_decode(path)?,
                last_generation: generation.map(|g| g as u64),
                recoverable: recoverable != 0,
            });
        }
        let (run,clean,active):(Vec<u8>,i64,Option<Vec<u8>>)=self.connection.query_row("SELECT CASE WHEN length(run_id)=16 THEN run_id ELSE NULL END,clean_shutdown,CASE WHEN active_document_id IS NULL OR length(active_document_id)=16 THEN active_document_id ELSE zeroblob(15) END FROM meta WHERE id=1",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;
        id(run)?;
        ensure!((0..=1).contains(&clean), "Invalid shutdown marker");
        if let Some(active) = active {
            let active = id(active)?;
            ensure!(
                result
                    .iter()
                    .any(|r| r.document_id == active && r.lifecycle == 0),
                "Invalid active document"
            );
        }
        let mut stmt = self.connection.prepare(
            "SELECT migration_key,target_document_id,state FROM migrations",
        )?;
        let mut migrations = stmt.query([])?;
        let mut size = size;
        while let Some(row) = migrations.next()? {
            let key: String = row.get(0)?;
            let target: Vec<u8> = row.get(1)?;
            let state: i64 = row.get(2)?;
            ensure!(
                key == "legacy-recovery-v1" && (0..=1).contains(&state),
                "Invalid migration record"
            );
            id(target)?;
            size += 64 + key.len() as i64;
            ensure!(size <= SESSION_LIMIT, "Session registry exceeds 8 MiB");
        }
        Ok(result)
    }
    pub fn clean_shutdown(&self) -> Result<bool> {
        Ok(self.connection.query_row(
            "SELECT clean_shutdown FROM meta WHERE id=1",
            [],
            |r| r.get(0),
        )?)
    }
    pub fn active(&self) -> Result<Option<[u8; 16]>> {
        let value: Option<Vec<u8>> = self.connection.query_row(
            "SELECT active_document_id FROM meta WHERE id=1",
            [],
            |r| r.get(0),
        )?;
        value.map(id).transpose()
    }
    pub fn begin_run(&self) -> Result<()> {
        self.connection.execute(
            "UPDATE meta SET clean_shutdown=0,run_id=?1 WHERE id=1",
            [EditStateId::try_new()
                .map_err(|e| {
                    anyhow::anyhow!("Session entropy unavailable: {e}")
                })?
                .bytes()
                .as_slice()],
        )?;
        Ok(())
    }
    pub fn register(
        &mut self,
        key: [u8; 16],
        source: Option<NativePath>,
    ) -> Result<()> {
        let path = path_encode(&source)?;
        let size:i64=self.connection.query_row("SELECT COALESCE(SUM(128+COALESCE(length(source_path),0)),0) FROM documents",[],|r|r.get(0))?;
        ensure!(
            size + 128 + path.as_ref().map_or(0, |p| p.len() as i64)
                <= SESSION_LIMIT,
            "Session registry exceeds 8 MiB"
        );
        self.connection.execute("INSERT INTO documents VALUES(?1,(SELECT COALESCE(MAX(tab_order),-1)+1 FROM documents),0,?2,NULL,0) ON CONFLICT(document_id) DO UPDATE SET source_path=excluded.source_path WHERE documents.lifecycle=0",params![key.as_slice(),path])?;
        ensure!(
            self.records()?
                .iter()
                .any(|r| r.document_id == key && r.lifecycle == 0),
            "Document was closed"
        );
        Ok(())
    }
    pub fn recovered(&self, key: [u8; 16], generation: u64) -> Result<()> {
        ensure!(self.connection.execute("UPDATE documents SET recoverable=1,last_generation=?2 WHERE document_id=?1 AND lifecycle=0",params![key.as_slice(),i64::try_from(generation)?])?==1,"Recovery owner is no longer open");
        Ok(())
    }
    pub fn close_document(&mut self, key: [u8; 16]) -> Result<()> {
        let tx = self.connection.transaction()?;
        tx.execute("UPDATE documents SET lifecycle=1 WHERE document_id=?1 AND lifecycle=0",[key.as_slice()])?;
        tx.execute("UPDATE meta SET active_document_id=NULL WHERE active_document_id=?1",[key.as_slice()])?;
        tx.commit()?;
        Ok(())
    }
    pub fn layout(
        &mut self,
        order: &[[u8; 16]],
        active: Option<[u8; 16]>,
        clean: bool,
    ) -> Result<()> {
        let records = self.records()?;
        let open: HashSet<_> = records
            .iter()
            .filter(|r| r.lifecycle == 0)
            .map(|r| r.document_id)
            .collect();
        let mut ids = Vec::new();
        let mut seen = HashSet::new();
        for &id in order {
            if open.contains(&id) {
                ensure!(seen.insert(id), "Duplicate layout ID");
                ids.push(id);
            }
        }
        for record in &records {
            if record.lifecycle == 0 && seen.insert(record.document_id) {
                ids.push(record.document_id);
            }
        }
        let tx = self.connection.transaction()?;
        tx.execute(
            "UPDATE documents SET tab_order=-tab_order-1 WHERE lifecycle=0",
            [],
        )?;
        for (index, id) in ids.iter().enumerate() {
            tx.execute(
                "UPDATE documents SET tab_order=?2 WHERE document_id=?1",
                params![id.as_slice(), index as i64],
            )?;
        }
        let active =
            active.filter(|id| open.contains(id)).map(|id| id.to_vec());
        tx.execute("UPDATE meta SET active_document_id=?1,clean_shutdown=?2 WHERE id=1",params![active,clean])?;
        tx.commit()?;
        Ok(())
    }
    pub fn migration(&self) -> Result<Option<([u8; 16], bool)>> {
        let row=self.connection.query_row("SELECT target_document_id,state FROM migrations WHERE migration_key='legacy-recovery-v1'",[],|r|Ok((r.get::<_,Vec<u8>>(0)?,r.get::<_,bool>(1)?))).optional()?;
        row.map(|(key, state)| Ok((id(key)?, state))).transpose()
    }
    pub fn prepare_migration(&self) -> Result<[u8; 16]> {
        if let Some((id, _)) = self.migration()? {
            return Ok(id);
        }
        let existing = self.records()?;
        let key = loop {
            let key = EditStateId::try_new()
                .map_err(|e| {
                    anyhow::anyhow!("Migration entropy unavailable: {e}")
                })?
                .bytes();
            if !existing.iter().any(|r| r.document_id == key) {
                break key;
            }
        };
        self.connection.execute(
            "INSERT INTO migrations VALUES('legacy-recovery-v1',?1,0)",
            [key.as_slice()],
        )?;
        Ok(key)
    }
    pub fn commit_migration(
        &mut self,
        key: [u8; 16],
        generation: u64,
    ) -> Result<()> {
        let tx = self.connection.transaction()?;
        tx.execute("INSERT OR IGNORE INTO documents VALUES(?1,(SELECT COALESCE(MAX(tab_order),-1)+1 FROM documents),0,NULL,?2,1)",params![key.as_slice(),i64::try_from(generation)?])?;
        ensure!(tx.execute("UPDATE migrations SET state=1 WHERE migration_key='legacy-recovery-v1' AND target_document_id=?1",[key.as_slice()])?==1,"Missing prepared migration");
        tx.commit()?;
        Ok(())
    }
}
/// Reentrant and source-preserving. The caller holds the application directory lock.
pub fn migrate_legacy(
    dir: &Path,
    session: &mut SessionStore,
) -> Result<Option<[u8; 16]>> {
    if session.migration()?.is_some_and(|(_, committed)| committed) {
        return Ok(None);
    }
    let source = dir.join("recovery.parfait");
    if !source.exists() {
        return Ok(None);
    }
    let Some(old) = Store::open_read_only(&source)?.load()? else {
        return Ok(None);
    };
    let key = session.prepare_migration()?;
    let target = recovery_path(dir, key);
    let already = if target.exists() {
        Store::open_read_only(&target)?.load_recovery(key)?
    } else {
        None
    };
    let generation = if let Some(valid) =
        already.filter(|r| r.metadata.is_some())
    {
        let mut expected = old.document.clone();
        expected.revision = valid.loaded.document.revision;
        ensure!(
            valid.loaded.document == expected,
            "Prepared migration target differs from its preserved legacy source"
        );
        valid.loaded.head.generation
    } else {
        let digest = old
            .document
            .art_digest_v1()
            .ok_or_else(|| anyhow::anyhow!("Invalid legacy drawing"))?;
        let metadata = RecoveryMetadata {
            document_id: key,
            edit_state_id: EditStateId::try_new()
                .map_err(|e| {
                    anyhow::anyhow!("Migration entropy unavailable: {e}")
                })?
                .bytes(),
            logical_digest_version: 1,
            logical_digest: digest.0,
            source_path: None,
            source_saved_state_id: None,
            source_saved_digest: None,
            source_head: None,
        };
        Store::open(&target)?
            .save_recovery(&old.document, &metadata)?
            .generation
    };
    let verify = Store::open_read_only(&target)?
        .load_recovery(key)?
        .ok_or_else(|| anyhow::anyhow!("Migration verification failed"))?;
    ensure!(
        verify.metadata.is_some(),
        "Migration metadata verification failed"
    );
    let mut expected = old.document.clone();
    expected.revision = verify.loaded.document.revision;
    ensure!(
        verify.loaded.document == expected,
        "Migration artwork verification failed"
    );
    session.commit_migration(key, generation)?;
    Ok(Some(key))
}
