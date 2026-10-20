//! Optional snapshot-bound metadata. A bad envelope never discards valid artwork.
use crate::{Loaded, Store};
use anyhow::{Result, ensure};
use bincode::Options;
use parfait_core::Document;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

pub const METADATA_LIMIT: u64 = 64 * 1024;
pub const PATH_LIMIT: usize = 32 * 1024;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NativePath {
    Unix(Vec<u8>),
    Windows(Vec<u16>),
}
impl NativePath {
    pub fn encode(path: &Path) -> Result<Self> {
        ensure!(path.is_absolute(), "Source path must be absolute");
        #[cfg(unix)]
        let value = {
            use std::os::unix::ffi::OsStrExt;
            Self::Unix(path.as_os_str().as_bytes().to_vec())
        };
        #[cfg(windows)]
        let value = {
            use std::os::windows::ffi::OsStrExt;
            Self::Windows(path.as_os_str().encode_wide().collect())
        };
        value.decode()?;
        Ok(value)
    }
    pub fn decode(&self) -> Result<PathBuf> {
        let path = match self {
            #[cfg(unix)]
            Self::Unix(bytes) => {
                use std::os::unix::ffi::OsStringExt;
                ensure!(
                    bytes.len() <= PATH_LIMIT && !bytes.contains(&0),
                    "Invalid source path"
                );
                PathBuf::from(std::ffi::OsString::from_vec(bytes.clone()))
            }
            #[cfg(windows)]
            Self::Windows(units) => {
                use std::os::windows::ffi::OsStringExt;
                ensure!(
                    units.len() <= PATH_LIMIT / 2 && !units.contains(&0),
                    "Invalid source path"
                );
                PathBuf::from(std::ffi::OsString::from_wide(units))
            }
            _ => anyhow::bail!("Source path belongs to a different OS"),
        };
        ensure!(path.is_absolute(), "Source path must be absolute");
        Ok(path)
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotHead {
    pub generation: u64,
    pub checksum: [u8; 32],
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryMetadata {
    pub document_id: [u8; 16],
    pub edit_state_id: [u8; 16],
    pub logical_digest_version: u32,
    pub logical_digest: [u8; 32],
    pub source_path: Option<NativePath>,
    pub source_saved_state_id: Option<[u8; 16]>,
    pub source_saved_digest: Option<[u8; 32]>,
    pub source_head: Option<SnapshotHead>,
}
impl RecoveryMetadata {
    pub fn validate(&self) -> Result<()> {
        ensure!(self.logical_digest_version == 1, "Unsupported logical digest");
        if let Some(path) = &self.source_path {
            path.decode()?;
        }
        Ok(())
    }
}
pub struct Recovered {
    pub loaded: Loaded,
    pub metadata: Option<RecoveryMetadata>,
}
fn envelope(head: &SnapshotHead, payload: &[u8]) -> Vec<u8> {
    let mut hash = Sha256::new();
    hash.update(b"Parfait.RecoveryMetadata\0");
    hash.update(1_u32.to_le_bytes());
    hash.update(head.generation.to_le_bytes());
    hash.update(head.checksum);
    hash.update((payload.len() as u64).to_le_bytes());
    hash.update(payload);
    hash.finalize().to_vec()
}
impl Store {
    pub fn save_recovery(
        &mut self,
        document: &Document,
        metadata: &RecoveryMetadata,
    ) -> Result<SnapshotHead> {
        metadata.validate()?;
        let payload = bincode::DefaultOptions::new()
            .with_fixint_encoding()
            .with_limit(METADATA_LIMIT)
            .serialize(metadata)?;
        ensure!(
            payload.len() as u64 <= METADATA_LIMIT,
            "Recovery metadata too large"
        );
        self.save_inner(document, Some(payload), None)
    }
    pub fn load_recovery(
        &self,
        expected: [u8; 16],
    ) -> Result<Option<Recovered>> {
        let Some(loaded) = self.load()? else {
            return Ok(None);
        };
        let read = || -> Result<Option<RecoveryMetadata>> {
            let table: bool = self.connection.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name='recovery_metadata' AND type='table')", [], |r| r.get(0))?;
            if !table {
                return Ok(None);
            }
            let row = self.connection.query_row(
                "SELECT CASE WHEN length(snapshot_checksum)=32 THEN snapshot_checksum ELSE NULL END,metadata_version,CASE WHEN length(payload)<=?2 THEN payload ELSE NULL END,CASE WHEN length(metadata_checksum)=32 THEN metadata_checksum ELSE NULL END FROM recovery_metadata WHERE generation=?1",
                params![i64::try_from(loaded.head.generation)?, METADATA_LIMIT as i64], |r| Ok((r.get::<_,Vec<u8>>(0)?,r.get::<_,i64>(1)?,r.get::<_,Option<Vec<u8>>>(2)?,r.get::<_,Vec<u8>>(3)?))).optional()?;
            let Some((snapshot, version, payload, checksum)) = row else {
                return Ok(None);
            };
            ensure!(
                version == 1 && snapshot == loaded.head.checksum,
                "Stale recovery metadata"
            );
            let payload =
                payload.ok_or_else(|| anyhow::anyhow!("Oversized metadata"))?;
            ensure!(
                envelope(&loaded.head, &payload) == checksum,
                "Recovery metadata checksum mismatch"
            );
            let value: RecoveryMetadata = bincode::DefaultOptions::new()
                .with_fixint_encoding()
                .with_limit(METADATA_LIMIT)
                .reject_trailing_bytes()
                .deserialize(&payload)?;
            value.validate()?;
            ensure!(value.document_id == expected, "Wrong recovery document");
            ensure!(
                loaded
                    .document
                    .art_digest_v1()
                    .is_some_and(|digest| digest.0 == value.logical_digest),
                "Recovery artwork mismatch"
            );
            Ok(Some(value))
        };
        let metadata = read().unwrap_or(None);
        Ok(Some(Recovered { loaded, metadata }))
    }
}

pub(crate) fn write_metadata(
    tx: &rusqlite::Transaction<'_>,
    head: &SnapshotHead,
    payload: Option<Vec<u8>>,
) -> Result<()> {
    let exists: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name='recovery_metadata' AND type='table')",[],|r|r.get(0))?;
    if payload.is_some() {
        tx.execute_batch("CREATE TABLE IF NOT EXISTS recovery_metadata(generation INTEGER PRIMARY KEY,snapshot_checksum BLOB NOT NULL,metadata_version INTEGER NOT NULL,payload BLOB NOT NULL,metadata_checksum BLOB NOT NULL)")?;
    }
    if exists || payload.is_some() {
        tx.execute(
            "DELETE FROM recovery_metadata WHERE generation=?1",
            [i64::try_from(head.generation)?],
        )?;
        if let Some(payload) = payload {
            let checksum = envelope(head, &payload);
            tx.execute(
                "INSERT INTO recovery_metadata VALUES(?1,?2,1,?3,?4)",
                params![
                    i64::try_from(head.generation)?,
                    head.checksum.as_slice(),
                    payload,
                    checksum
                ],
            )?;
        }
        tx.execute("DELETE FROM recovery_metadata WHERE generation NOT IN (SELECT revision FROM snapshots)",[])?;
    }
    Ok(())
}
