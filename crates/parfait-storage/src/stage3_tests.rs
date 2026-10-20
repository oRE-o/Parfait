use super::*;
use parfait_core::{Document, EditStateId};
use rusqlite::Connection;
use std::{path::Path, sync::Arc};
fn document(value: u8) -> Document {
    let mut d = Document::new(8, 8);
    d.layers[1].pixels = Arc::new([value, 20, 40, 255].repeat(64));
    d
}
fn metadata(d: &Document, id: [u8; 16]) -> RecoveryMetadata {
    RecoveryMetadata {
        document_id: id,
        edit_state_id: EditStateId::try_new().unwrap().bytes(),
        logical_digest_version: 1,
        logical_digest: d.art_digest_v1().unwrap().0,
        source_path: None,
        source_saved_state_id: None,
        source_saved_digest: None,
        source_head: None,
    }
}
fn read(path: &Path, id: [u8; 16]) -> Recovered {
    Store::open_read_only(path).unwrap().load_recovery(id).unwrap().unwrap()
}
#[test]
fn stage3_metadata_tracks_selected_disk_generation_not_repaired_memory_revision()
 {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("drawing.parfait");
    let id = [1; 16];
    let mut store = Store::open(&path).unwrap();
    let d = document(10);
    let first = metadata(&d, id);
    store.save_recovery(&d, &first).unwrap();
    let mut next = document(20);
    next.revision = 2;
    store.save_recovery(&next, &metadata(&next, id)).unwrap();
    let c = Connection::open(&path).unwrap();
    c.execute(
        "UPDATE snapshots SET checksum=zeroblob(32) WHERE revision=2",
        [],
    )
    .unwrap();
    let recovered = read(&path, id);
    assert!(recovered.loaded.fell_back);
    assert_eq!(recovered.loaded.head.generation, 1);
    assert_eq!(recovered.loaded.document.revision, 3);
    assert_eq!(recovered.metadata, Some(first));
    assert_eq!(recovered.loaded.document.layers, d.layers);
}
#[test]
fn stage3_bad_optional_metadata_never_discards_valid_artwork() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("drawing.parfait");
    let id = [2; 16];
    let d = document(10);
    let m = metadata(&d, id);
    let mut store = Store::open(&path).unwrap();
    let c = Connection::open(&path).unwrap();
    for sql in [
        "UPDATE recovery_metadata SET metadata_checksum=zeroblob(32)",
        "UPDATE recovery_metadata SET metadata_version=99",
        "UPDATE recovery_metadata SET snapshot_checksum=zeroblob(32)",
        "UPDATE recovery_metadata SET payload=zeroblob(65537)",
    ] {
        store.save_recovery(&d, &m).unwrap();
        c.execute(sql, []).unwrap();
        let r = read(&path, id);
        assert!(r.metadata.is_none());
        assert_eq!(r.loaded.document, d);
    }
    store.save_recovery(&d, &m).unwrap();
    assert!(read(&path, [3; 16]).metadata.is_none());
    let mut wrong = m.clone();
    wrong.logical_digest = [9; 32];
    store.save_recovery(&d, &wrong).unwrap();
    assert!(read(&path, id).metadata.is_none());
    let mut invalid = m.clone();
    invalid.source_path = Some(NativePath::Unix(b"relative".to_vec()));
    assert!(store.save_recovery(&d, &invalid).is_err());
    assert_eq!(read(&path, id).loaded.document, d);
    store.save_recovery(&d, &m).unwrap();
    store.save(&d).unwrap();
    assert!(read(&path, id).metadata.is_none());
    for revision in 2..=7 {
        let mut d = d.clone();
        d.revision = revision;
        store.save_recovery(&d, &m).unwrap();
    }
    let count: i64 = c
        .query_row("SELECT COUNT(*) FROM recovery_metadata", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 3);
}
#[test]
fn stage3_session_registry_roundtrip_atomic_layout_closed_and_future_protection()
 {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("session.sqlite");
    let mut s = SessionStore::open(&path).unwrap();
    assert!(s.clean_shutdown().unwrap());
    s.begin_run().unwrap();
    s.register(
        [1; 16],
        Some(NativePath::encode(&dir.path().join("a.parfait")).unwrap()),
    )
    .unwrap();
    s.register([2; 16], None).unwrap();
    s.recovered([1; 16], 7).unwrap();
    s.layout(&[[2; 16], [1; 16]], Some([2; 16]), false).unwrap();
    assert!(!s.clean_shutdown().unwrap());
    assert_eq!(s.active().unwrap(), Some([2; 16]));
    assert!(s.layout(&[[2; 16], [2; 16]], None, false).is_err());
    assert_eq!(s.active().unwrap(), Some([2; 16]));
    s.close_document([2; 16]).unwrap();
    s.layout(&[[1; 16]], Some([1; 16]), true).unwrap();
    drop(s);
    let s = SessionStore::open(&path).unwrap();
    let rows = s.records().unwrap();
    assert_eq!(rows.iter().filter(|r| r.lifecycle == 0).count(), 1);
    assert_eq!(
        rows.iter().find(|r| r.document_id == [1; 16]).unwrap().last_generation,
        Some(7)
    );
    assert!(s.clean_shutdown().unwrap());
    drop(s);
    let c = Connection::open(&path).unwrap();
    c.pragma_update(None, "user_version", 99).unwrap();
    drop(c);
    let before = std::fs::read(&path).unwrap();
    assert!(SessionStore::open(&path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
}
#[test]
fn stage3_registry_rejects_bad_ids_paths_lifecycle_and_size_without_rewriting()
{
    for sql in [
        "UPDATE documents SET document_id=zeroblob(15)",
        "UPDATE documents SET lifecycle=99",
        "UPDATE documents SET tab_order=-1",
        "UPDATE documents SET source_path=zeroblob(8388609)",
        "UPDATE meta SET active_document_id=zeroblob(16)",
    ] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.sqlite");
        let mut s = SessionStore::open(&path).unwrap();
        s.register([1; 16], None).unwrap();
        drop(s);
        let c = Connection::open(&path).unwrap();
        c.execute(sql, []).unwrap();
        drop(c);
        let before = std::fs::read(&path).unwrap();
        assert!(SessionStore::open(&path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }
}
#[test]
fn stage3_migration_is_reentrant_preserves_legacy_bytes_and_ignores_backups() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("recovery.parfait");
    let d = document(40);
    let mut source = Store::open(&path).unwrap();
    source.save(&d).unwrap();
    source.checkpoint().unwrap();
    drop(source);
    let before = std::fs::read(&path).unwrap();
    let mut s = SessionStore::open(&dir.path().join("session.sqlite")).unwrap();
    let target = s.prepare_migration().unwrap();
    assert_eq!(s.prepare_migration().unwrap(), target);
    assert_eq!(migrate_legacy(dir.path(), &mut s).unwrap(), Some(target));
    assert!(migrate_legacy(dir.path(), &mut s).unwrap().is_none());
    assert_eq!(s.records().unwrap().len(), 1);
    let r = read(&recovery_path(dir.path(), target), target);
    assert_eq!(r.loaded.document, d);
    assert!(r.metadata.is_some());
    assert_eq!(std::fs::read(&path).unwrap(), before);
}
#[test]
fn stage3_failed_legacy_migration_preserves_unknown_or_corrupt_source() {
    for future in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("recovery.parfait");
        if future {
            let c = Connection::open(&path).unwrap();
            c.pragma_update(None, "user_version", 99).unwrap();
        } else {
            std::fs::write(&path, b"broken database").unwrap();
        }
        let before = std::fs::read(&path).unwrap();
        let mut s =
            SessionStore::open(&dir.path().join("session.sqlite")).unwrap();
        assert!(migrate_legacy(dir.path(), &mut s).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert!(s.records().unwrap().is_empty());
    }
}
fn request(id: u8, epoch: u8, number: u64, value: u8) -> RecoveryRequest {
    RecoveryRequest {
        key: JobKey {
            document_id: [id; 16],
            session_epoch: [epoch; 16],
            request_id: number,
        },
        edit_state_id: EditStateId::try_new().unwrap(),
        snapshot: document(value),
        source_path: None,
        source_saved_state_id: None,
        source_saved_digest: None,
        source_head: None,
    }
}
#[test]
fn stage3_worker_coalesces_per_document_routes_errors_and_cleans_owner_epochs()
{
    let dir = tempfile::tempdir().unwrap();
    let mut worker = StorageCoordinator::start(dir.path().to_owned()).unwrap();
    for n in 1..=30 {
        let mut r = request(1, 1, n, n as u8);
        r.snapshot.revision = n;
        worker.submit(r).unwrap();
    }
    let b = request(2, 1, 1, 90);
    worker.submit(b).unwrap();
    worker.barrier();
    let a = read(&recovery_path(dir.path(), [1; 16]), [1; 16]);
    assert_eq!(a.loaded.document.revision, 30);
    assert_eq!(a.loaded.document.layers[1].pixels[0], 30);
    assert_eq!(
        read(&recovery_path(dir.path(), [2; 16]), [2; 16])
            .loaded
            .document
            .layers[1]
            .pixels[0],
        90
    );
    assert!(worker.submit(request(1, 1, 29, 0)).is_err());
    assert!(worker.submit(request(1, 2, 31, 0)).is_err());
    let mut events = Vec::new();
    while let Some(e) = worker.poll() {
        events.push(e);
    }
    assert_eq!(events.len(), 2);
    assert!(events.iter().all(|e| e.result.is_ok()));
    worker.forget([1; 16], [1; 16]);
    worker.submit(request(1, 2, 1, 42)).unwrap();
    worker.barrier();
    assert_eq!(worker.poll().unwrap().key.session_epoch, [2; 16]);
    worker.forget([2; 16], [1; 16]);
    let path = recovery_path(dir.path(), [3; 16]);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, b"not sqlite").unwrap();
    worker.submit(request(3, 1, 1, 0)).unwrap();
    worker.submit(request(2, 2, 1, 50)).unwrap();
    worker.barrier();
    let mut events = Vec::new();
    while let Some(e) = worker.poll() {
        events.push(e);
    }
    assert!(
        events
            .iter()
            .any(|e| e.key.document_id == [3; 16] && e.result.is_err())
    );
    assert!(
        events.iter().any(|e| e.key.document_id == [2; 16] && e.result.is_ok())
    );
    assert!(worker.snapshots().is_empty());
    worker.finish();
    assert!(worker.submit(request(4, 1, 1, 0)).is_err());
}

#[test]
fn stage3_transaction_failures_keep_previous_artwork_metadata_layout_and_lifecycle()
 {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("drawing.parfait");
    let mut store = Store::open(&path).unwrap();
    let mut d = document(10);
    let m = metadata(&d, [1; 16]);
    store.save_recovery(&d, &m).unwrap();
    let c = Connection::open(&path).unwrap();
    c.execute_batch("CREATE TRIGGER refuse_metadata BEFORE INSERT ON recovery_metadata BEGIN SELECT RAISE(ABORT,'injected metadata failure'); END;").unwrap();
    d.revision += 1;
    assert!(store.save_recovery(&d, &metadata(&d, [1; 16])).is_err());
    let recovered = read(&path, [1; 16]);
    assert_eq!(recovered.loaded.head.generation, 1);
    assert_eq!(recovered.metadata, Some(m));
    let path = dir.path().join("session.sqlite");
    let mut session = SessionStore::open(&path).unwrap();
    session.register([1; 16], None).unwrap();
    session.register([2; 16], None).unwrap();
    session.layout(&[[1; 16], [2; 16]], Some([2; 16]), false).unwrap();
    let c = Connection::open(&path).unwrap();
    c.execute_batch("CREATE TRIGGER refuse_meta BEFORE UPDATE ON meta BEGIN SELECT RAISE(ABORT,'injected session failure'); END;").unwrap();
    assert!(session.layout(&[[2; 16], [1; 16]], Some([1; 16]), true).is_err());
    assert_eq!(session.active().unwrap(), Some([2; 16]));
    assert!(!session.clean_shutdown().unwrap());
    assert_eq!(session.records().unwrap()[0].document_id, [1; 16]);
    assert!(session.close_document([2; 16]).is_err());
    assert!(session.records().unwrap().iter().all(|r| r.lifecycle == 0));
}

#[test]
fn stage3_native_paths_are_lossless_bounded_and_never_relative() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("그림.parfait");
    assert_eq!(NativePath::encode(&path).unwrap().decode().unwrap(), path);
    assert!(NativePath::encode(Path::new("relative.parfait")).is_err());
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        let path =
            dir.path().join(std::ffi::OsString::from_vec(vec![b'f', 0xff]));
        assert_eq!(NativePath::encode(&path).unwrap().decode().unwrap(), path);
        assert!(NativePath::Unix(vec![b'/'; 32769]).decode().is_err());
        assert!(NativePath::Unix(vec![b'/', 0]).decode().is_err());
        assert!(NativePath::Windows(vec![1, 2]).decode().is_err());
    }
}

#[test]
fn stage4_original_head_is_rechecked_inside_the_commit_transaction() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("original.parfait");
    let mut store = Store::open(&path).unwrap();
    let mut d = document(10);
    let first = store.save_original(&d, None).unwrap();
    d.revision += 1;
    assert!(store.save_original(&d, None).is_err());
    assert_eq!(
        Store::open_read_only(&path).unwrap().load().unwrap().unwrap().head,
        first
    );
    let newer = store.save_original(&d, Some(first.clone())).unwrap();
    assert_eq!(newer.generation, 2);
    d.revision += 1;
    assert!(store.save_original(&d, Some(first)).is_err());
    assert_eq!(
        Store::open_read_only(&path).unwrap().load().unwrap().unwrap().head,
        newer
    );
}

#[test]
fn stage4_committed_original_is_distinct_from_a_busy_checkpoint_warning() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("original.parfait");
    let mut store = Store::open(&path).unwrap();
    let d = document(10);
    let head = store.save_original(&d, None).unwrap();
    store.checkpoint().unwrap();
    drop(store);
    let reader = Connection::open(&path).unwrap();
    reader.execute_batch("BEGIN;").unwrap();
    let _: Vec<u8> = reader
        .query_row("SELECT checksum FROM snapshots LIMIT 1", [], |r| r.get(0))
        .unwrap();
    let worker = StorageCoordinator::start(dir.path().to_owned()).unwrap();
    let mut next = document(20);
    next.revision = 2;
    worker
        .submit_source(SourceRequest {
            key: JobKey {
                document_id: [1; 16],
                session_epoch: [1; 16],
                request_id: 1,
            },
            binding_epoch: 0,
            path: path.clone(),
            snapshot: next,
            edit_state_id: EditStateId::try_new().unwrap(),
            expected_head: Some(head),
        })
        .unwrap();
    worker.barrier();
    let saved = worker.poll_source().unwrap().result.unwrap();
    assert!(saved.post_commit_warning.is_some());
    reader.execute_batch("COMMIT;").unwrap();
    assert_eq!(
        Store::open_read_only(&path)
            .unwrap()
            .load()
            .unwrap()
            .unwrap()
            .document
            .layers[1]
            .pixels[0],
        20
    );
}

#[test]
fn stage4_original_generation_advances_the_physical_head_not_just_payload_revision()
 {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("original.parfait");
    let mut store = Store::open(&path).unwrap();
    store.save(&document(10)).unwrap();
    drop(store);
    let c = Connection::open(&path).unwrap();
    c.execute("UPDATE snapshots SET revision=100", []).unwrap();
    drop(c);
    let head =
        Store::open_read_only(&path).unwrap().load().unwrap().unwrap().head;
    let worker = StorageCoordinator::start(dir.path().to_owned()).unwrap();
    worker
        .submit_source(SourceRequest {
            key: JobKey {
                document_id: [1; 16],
                session_epoch: [1; 16],
                request_id: 1,
            },
            binding_epoch: 0,
            path: path.clone(),
            snapshot: document(20),
            edit_state_id: EditStateId::try_new().unwrap(),
            expected_head: Some(head),
        })
        .unwrap();
    worker.barrier();
    assert_eq!(
        worker.poll_source().unwrap().result.unwrap().head.generation,
        101
    );
    assert_eq!(
        Store::open_read_only(&path)
            .unwrap()
            .load()
            .unwrap()
            .unwrap()
            .document
            .layers[1]
            .pixels[0],
        20
    );
}
