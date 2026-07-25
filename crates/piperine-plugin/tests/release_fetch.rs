//! Release TOFU + pinning (plugin-interface v2, PLG-17/18, design §4): a
//! fetched asset's hash is TOFU-approved and pinned as
//! `(release-url, triple, content-hash)` in `Piperine.lock`
//! (`EntryKind::Plugin`); a changed asset re-prompts; a `verify` hash is
//! checked up front — mismatch is a hard fail with no prompt, a match
//! loads without one.

use piperine_plugin::{ensure_release_trusted, Manifest, Permissions, PluginError, TrustMode};
use piperine_project::lockfile::{EntryKind, PiperineLock};

const COORD: &str = "github:acme/bjt@v1";
const TRIPLE: &str = "x86_64-unknown-linux-gnu";

fn manifest(name: &str) -> Manifest {
    Manifest {
        name: name.into(),
        description: None,
        python: None,
        device: None,
        permissions: Permissions::default(),
    }
}

fn lock(root: &std::path::Path) -> PiperineLock {
    PiperineLock::load(&root.join("Piperine.lock")).unwrap().expect("lockfile written")
}

#[test]
fn an_approved_asset_is_pinned_as_release_triple_hash() {
    let dir = tempfile::tempdir().unwrap();
    ensure_release_trusted(dir.path(), &manifest("bjt"), COORD, "sha256:aaaa", TRIPLE, None, TrustMode::AcceptAll)
        .expect("first approval");
    let entry = lock(dir.path()).plugin_entry("bjt").expect("pinned").clone();
    assert_eq!(entry.kind, EntryKind::Plugin);
    assert_eq!(entry.source, COORD, "the release url is the pin's source");
    assert_eq!(entry.content_hash.as_deref(), Some("sha256:aaaa"));
    assert_eq!(entry.abi.as_deref(), Some(TRIPLE), "the triple is pinned");
    assert!(entry.trusted_at.is_some(), "approval is timestamped");
    // The pin short-circuits: strictest mode, same hash, no prompt.
    ensure_release_trusted(dir.path(), &manifest("bjt"), COORD, "sha256:aaaa", TRIPLE, None, TrustMode::RejectUntrusted)
        .expect("pinned entry needs no prompt");
}

#[test]
fn a_changed_asset_re_prompts_and_re_pins_on_accept() {
    // A mutable release tag serving new bytes (spec edge case): TOFU's
    // content-hash mismatch catches it — accept re-pins, reject aborts.
    let dir = tempfile::tempdir().unwrap();
    ensure_release_trusted(dir.path(), &manifest("bjt"), COORD, "sha256:aaaa", TRIPLE, None, TrustMode::AcceptAll)
        .unwrap();
    let err = ensure_release_trusted(dir.path(), &manifest("bjt"), COORD, "sha256:bbbb", TRIPLE, None, TrustMode::RejectUntrusted)
        .unwrap_err();
    assert!(matches!(err, PluginError::Untrusted(_)), "reject aborts: {err}");
    assert_eq!(
        lock(dir.path()).plugin_entry("bjt").unwrap().content_hash.as_deref(),
        Some("sha256:aaaa"),
        "a rejected re-prompt leaves the old pin"
    );
    ensure_release_trusted(dir.path(), &manifest("bjt"), COORD, "sha256:bbbb", TRIPLE, None, TrustMode::AcceptAll)
        .expect("accept re-pins");
    assert_eq!(
        lock(dir.path()).plugin_entry("bjt").unwrap().content_hash.as_deref(),
        Some("sha256:bbbb"),
        "the new hash replaces the pin"
    );
}

#[test]
fn a_verify_mismatch_hard_fails_without_a_prompt() {
    // Even in the most permissive mode a `verify` mismatch is a hard fail
    // — no prompt, no pin (PLG-18).
    let dir = tempfile::tempdir().unwrap();
    let err = ensure_release_trusted(
        dir.path(),
        &manifest("bjt"),
        COORD,
        "sha256:aaaa",
        TRIPLE,
        Some("sha256:deadbeef"),
        TrustMode::AcceptAll,
    )
    .unwrap_err();
    assert!(matches!(err, PluginError::VerifyMismatch { .. }), "{err}");
    assert!(lock_opt(dir.path()).is_none(), "no pin is written on a verify mismatch");
}

fn lock_opt(root: &std::path::Path) -> Option<PiperineLock> {
    PiperineLock::load(&root.join("Piperine.lock")).unwrap()
}

#[test]
fn a_verify_match_loads_without_a_tofu_prompt() {
    // The strictest TOFU mode would reject any unpinned plugin — the
    // matching `verify` hash is the consent instead (PLG-18).
    let dir = tempfile::tempdir().unwrap();
    ensure_release_trusted(
        dir.path(),
        &manifest("bjt"),
        COORD,
        "sha256:aaaa",
        TRIPLE,
        Some("sha256:aaaa"),
        TrustMode::RejectUntrusted,
    )
    .expect("verify match needs no prompt");
    let entry = lock(dir.path()).plugin_entry("bjt").expect("pinned").clone();
    assert_eq!(entry.content_hash.as_deref(), Some("sha256:aaaa"));
    assert_eq!(entry.abi.as_deref(), Some(TRIPLE));
}

#[test]
fn an_unknown_asset_is_rejected_in_reject_mode() {
    let dir = tempfile::tempdir().unwrap();
    let err = ensure_release_trusted(dir.path(), &manifest("bjt"), COORD, "sha256:cccc", TRIPLE, None, TrustMode::RejectUntrusted)
        .unwrap_err();
    assert!(matches!(err, PluginError::Untrusted(_)), "{err}");
}
