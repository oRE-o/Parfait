//! Immutable original-save jobs, with a source-head check inside the write transaction.
use crate::JobKey;
use crate::{SnapshotHead, Store};
use parfait_core::{Document, EditStateId};
use std::{fmt, path::PathBuf};
#[derive(Clone)]
pub struct SourceRequest {
    pub key: JobKey,
    pub binding_epoch: u64,
    pub path: PathBuf,
    pub snapshot: Document,
    pub edit_state_id: EditStateId,
    pub expected_head: Option<SnapshotHead>,
}
pub struct SavedSource {
    pub head: SnapshotHead,
    pub digest: [u8; 32],
    pub post_commit_warning: Option<String>,
}
#[derive(Debug)]
pub enum SourceFailure {
    Changed(Option<SnapshotHead>),
    Failed(String),
}
pub struct SourceEvent {
    pub request: SourceRequest,
    pub result: Result<SavedSource, SourceFailure>,
}
#[derive(Debug)]
pub struct SourceConflict(pub Option<SnapshotHead>);
impl fmt::Display for SourceConflict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Original changed since inspection")
    }
}
impl std::error::Error for SourceConflict {}
pub(crate) fn write_source(
    request: &SourceRequest,
    digest: [u8; 32],
) -> Result<SavedSource, SourceFailure> {
    let run = || -> anyhow::Result<SavedSource> {
        let old = if request.path.exists() {
            Store::open_read_only(&request.path)?.load()?
        } else {
            None
        };
        if let Some(old) = &old {
            anyhow::ensure!(
                !old.fell_back,
                "Original contains a damaged latest snapshot; choose Save As"
            );
        }
        let head = old.as_ref().map(|old| old.head.clone());
        if head != request.expected_head {
            return Err(SourceConflict(head).into());
        }
        let mut snapshot = request.snapshot.clone();
        if let Some(old) = old {
            snapshot.revision = snapshot
                .revision
                .max(old.document.revision)
                .max(old.head.generation)
                .checked_add(1)
                .ok_or_else(|| {
                    anyhow::anyhow!("Original revision limit reached")
                })?;
        }
        anyhow::ensure!(
            i64::try_from(snapshot.revision).is_ok(),
            "Original revision limit reached"
        );
        let mut store = Store::open(&request.path)?;
        let head =
            store.save_original(&snapshot, request.expected_head.clone())?;
        let post_commit_warning = store
            .mark_clean()
            .and_then(|_| store.checkpoint())
            .err()
            .map(|e| {
                format!(
                    "Original committed; checkpoint/clean update failed: {e:#}"
                )
            });
        Ok(SavedSource { head, digest, post_commit_warning })
    };
    run().map_err(|error| {
        if let Some(conflict) = error.downcast_ref::<SourceConflict>() {
            SourceFailure::Changed(conflict.0.clone())
        } else {
            SourceFailure::Failed(format!("{error:#}"))
        }
    })
}
