use parfait_storage::{SessionStore, Store, migrate_legacy, recovery_path};
use std::{
    process::{Command, Stdio},
    time::{Duration, Instant},
};
#[test]
fn killed_session_commits_and_migration_boundaries_remain_reentrant() {
    for mode in [
        "registered",
        "snapshot",
        "hint",
        "transaction-open",
        "migration-prepared",
        "migration-snapshot",
        "migration-committed",
        "closed",
        "cleanup",
        "clean",
    ] {
        let dir = tempfile::tempdir().unwrap();
        let mut child =
            Command::new(env!("CARGO_BIN_EXE_session_recovery_probe"))
                .arg(dir.path())
                .arg(mode)
                .stdout(Stdio::null())
                .spawn()
                .unwrap();
        let start = Instant::now();
        while !dir.path().join("ready").exists() {
            if let Some(status) = child.try_wait().unwrap() {
                panic!("probe {mode} exited: {status}");
            }
            if start.elapsed() > Duration::from_secs(30) {
                let _ = child.kill();
                let _ = child.wait();
                panic!("probe {mode} timed out");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        child.kill().unwrap();
        child.wait().unwrap();
        let mut session =
            SessionStore::open(&dir.path().join("session.sqlite")).unwrap();
        assert_eq!(session.clean_shutdown().unwrap(), mode == "clean");
        if mode.starts_with("migration-") {
            let target = session.migration().unwrap().unwrap().0;
            migrate_legacy(dir.path(), &mut session).unwrap();
            migrate_legacy(dir.path(), &mut session).unwrap();
            assert_eq!(session.records().unwrap().len(), 1);
            assert_eq!(session.records().unwrap()[0].document_id, target);
            assert_eq!(
                std::fs::read(dir.path().join("recovery.parfait")).unwrap(),
                std::fs::read(dir.path().join("original-bytes")).unwrap()
            );
            let recovered =
                Store::open_read_only(&recovery_path(dir.path(), target))
                    .unwrap()
                    .load_recovery(target)
                    .unwrap()
                    .unwrap();
            assert!(recovered.metadata.is_some());
            assert_eq!(
                recovered.loaded.document.layers[1].pixels.as_slice(),
                [20, 40, 60, 255].repeat(256)
            );
        } else {
            let record = &session.records().unwrap()[0];
            if mode == "closed" || mode == "cleanup" {
                assert_eq!(record.lifecycle, 1);
                if mode == "cleanup" {
                    assert!(
                        !recovery_path(dir.path(), record.document_id).exists()
                    );
                }
                continue;
            }
            if mode == "registered" {
                assert!(
                    !recovery_path(dir.path(), record.document_id).exists()
                );
                assert!(!record.recoverable);
            } else {
                let recovered = Store::open_read_only(&recovery_path(
                    dir.path(),
                    record.document_id,
                ))
                .unwrap()
                .load_recovery(record.document_id)
                .unwrap()
                .unwrap();
                assert!(recovered.metadata.is_some());
                assert_eq!(
                    recovered.loaded.document.layers[1].pixels.as_slice(),
                    [20, 40, 60, 255].repeat(256)
                );
                if mode == "snapshot" {
                    assert!(!record.recoverable);
                    assert!(record.last_generation.is_none());
                }
            }
        }
    }
}
