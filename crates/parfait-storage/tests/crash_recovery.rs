use parfait_storage::Store;
use std::{
    process::{Command, Stdio},
    time::{Duration, Instant},
};
fn crash(mode: &str, expected_layers: usize) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("recovery.parfait");
    let ready = dir.path().join("ready");
    let mut child = Command::new(env!("CARGO_BIN_EXE_recovery_probe"))
        .args([path.to_str().unwrap(), ready.to_str().unwrap(), mode])
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    let start = Instant::now();
    while !ready.exists() {
        if let Some(status) = child.try_wait().unwrap() {
            panic!("Recovery probe exited: {status}");
        }
        if start.elapsed() > Duration::from_secs(30) {
            let _ = child.kill();
            let _ = child.wait();
            panic!("probe timed out");
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    child.kill().unwrap();
    child.wait().unwrap();
    let store = Store::open(&path).unwrap();
    assert!(store.unclean().unwrap());
    let loaded = store.load().unwrap().unwrap();
    assert!(!loaded.fell_back);
    assert_eq!(loaded.document.layers.len(), expected_layers);
    let mask = loaded.document.layers[1].mask.as_ref().unwrap();
    assert!(mask.enabled);
    match (&mask.coverage, mode) {
        (parfait_core::MaskCoverage::Uniform(128), "during") => {}
        (parfait_core::MaskCoverage::Dense(values), "after") => {
            assert_eq!(values.as_slice(), [127; 64 * 64])
        }
        _ => panic!("the last committed mask did not survive the process kill"),
    }
    assert_eq!(
        loaded.document.layers[1].blend_mode,
        parfait_core::BlendMode::Multiply
    );
    assert!(
        loaded.document.layers[1].pixels[(32 * 64 + 32) * 4 + 3] > 0,
        "stroke survived the process kill"
    );
}
#[test]
fn force_kill_during_transaction_retains_previous_drawing() {
    crash("during", 2);
}
#[test]
fn force_kill_after_commit_recovers_latest_layers_and_pixels() {
    crash("after", 3);
}
