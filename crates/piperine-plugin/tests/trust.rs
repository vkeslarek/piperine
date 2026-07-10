//! TOFU state machine (SPEC Part VI §3.2): accept records to the lockfile,
//! reject aborts, a recorded hash never re-prompts, a changed hash is P0007.

use std::path::Path;

use piperine_plugin::{artifact_hash, Abi, Manifest, Permissions, PluginError, TrustMode};

fn manifest(name: &str) -> Manifest {
    Manifest {
        name: name.into(),
        abi: Abi::Native,
        entry: "lib.so".into(),
        description: None,
        permissions: Permissions::default(),
    }
}

fn ensure(root: &Path, m: &Manifest, hash: &str, mode: TrustMode) -> Result<(), PluginError> {
    piperine_plugin::trust_check(root, m, "test-source", hash, mode)
}

#[test]
fn accept_records_and_then_never_reasks() {
    let dir = tempfile::tempdir().unwrap();
    let m = manifest("p1");
    ensure(dir.path(), &m, "sha256:aaaa", TrustMode::AcceptAll).expect("first accept");
    // Same hash, strictest mode: already trusted, no prompt needed.
    ensure(dir.path(), &m, "sha256:aaaa", TrustMode::RejectUntrusted).expect("recorded trust");
    // Lockfile round-trips the entry.
    let lock = std::fs::read_to_string(dir.path().join("Piperine.lock")).unwrap();
    assert!(lock.contains("p1") && lock.contains("sha256:aaaa"), "{lock}");
}

#[test]
fn changed_hash_is_a_hash_mismatch() {
    let dir = tempfile::tempdir().unwrap();
    let m = manifest("p1");
    ensure(dir.path(), &m, "sha256:aaaa", TrustMode::AcceptAll).unwrap();
    let err = ensure(dir.path(), &m, "sha256:bbbb", TrustMode::AcceptAll).unwrap_err();
    assert!(matches!(err, PluginError::HashMismatch { .. }), "{err}");
}

#[test]
fn reject_mode_rejects_unknown_plugins() {
    let dir = tempfile::tempdir().unwrap();
    let err = ensure(dir.path(), &manifest("p2"), "sha256:cccc", TrustMode::RejectUntrusted)
        .unwrap_err();
    assert!(matches!(err, PluginError::Untrusted(_)), "{err}");
}

#[test]
fn artifact_hash_is_stable_sha256() {
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("artifact.bin");
    std::fs::write(&f, b"piperine").unwrap();
    let h1 = artifact_hash(&f).unwrap();
    let h2 = artifact_hash(&f).unwrap();
    assert_eq!(h1, h2);
    assert!(h1.starts_with("sha256:"), "{h1}");
    std::fs::write(&f, b"tampered").unwrap();
    assert_ne!(artifact_hash(&f).unwrap(), h1);
}
