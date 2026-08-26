//! Atomic-write and manifest-verification tests.

use omiai_checkpoint::{write_atomic, CheckpointError, verify_dir};

#[test]
fn atomic_write_leaves_no_tmp_on_success() {
    let dir = tempfile::tempdir().unwrap();
    write_atomic(dir.path(), "f.bin", b"data").unwrap();
    let entries: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| {
            e.unwrap()
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert_eq!(entries, vec!["f.bin".to_string()]);
}

#[test]
fn verify_detects_tampered_file() {
    let dir = tempfile::tempdir().unwrap();
    write_atomic(dir.path(), "f.bin", b"data").unwrap();
    // A manifest must exist for verification to mean anything.
    omiai_checkpoint::Manifest::write(
        dir.path(),
        &[omiai_checkpoint::FileRecord {
            path: "f.bin".into(),
            blake3: omiai_checkpoint::hash_file(&dir.path().join("f.bin")).unwrap(),
        }],
    )
    .unwrap();

    // Untampered: verify passes.
    verify_dir(dir.path()).unwrap();

    // Tamper, then verify must report Corrupt with the hash mismatch.
    std::fs::write(dir.path().join("f.bin"), b"datX").unwrap();
    let err = verify_dir(dir.path()).unwrap_err();
    assert!(matches!(err, CheckpointError::Corrupt { .. }), "got {err:?}");
}
