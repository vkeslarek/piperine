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

// ─── Reproducibility / offline-after-first fetch (PLG-20) ───────────────────

use piperine_project::lockfile::LockEntry;
use piperine_project::release::{PluginCache, ReleaseClient, ReleaseError, ReleaseRef};

const BYTES: &[u8] = b"fake device binary v1";

fn sha256(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("sha256:{:x}", Sha256::digest(bytes))
}

/// A second machine: the lockfile travels, the cache does not.
fn prewritten_lock(root: &std::path::Path, plugin: &str, content_hash: &str) {
    let mut lock = PiperineLock::new();
    lock.record_plugin(LockEntry {
        name: plugin.into(),
        source: COORD.into(),
        hash: content_hash.into(),
        kind: EntryKind::Plugin,
        content_hash: Some(content_hash.into()),
        abi: Some(TRIPLE.into()),
        trusted_at: Some("2026-07-25T00:00:00Z".into()),
    });
    lock.save(&root.join("Piperine.lock")).unwrap();
}

/// The release the lockfile was written against: serves the same bytes.
struct SameRelease;

impl ReleaseClient for SameRelease {
    fn list_assets(&self, _release: &ReleaseRef) -> Result<Vec<String>, ReleaseError> {
        Ok(vec![format!("libbjt-{TRIPLE}.so")])
    }
    fn download_asset(&self, _release: &ReleaseRef, _asset: &str) -> Result<Vec<u8>, ReleaseError> {
        Ok(BYTES.to_vec())
    }
}

/// No network at all: every call fails.
struct NoNetwork;

impl ReleaseClient for NoNetwork {
    fn list_assets(&self, release: &ReleaseRef) -> Result<Vec<String>, ReleaseError> {
        Err(ReleaseError::Fetch { release: release.coordinate().into(), reason: "offline".into() })
    }
    fn download_asset(&self, release: &ReleaseRef, _asset: &str) -> Result<Vec<u8>, ReleaseError> {
        Err(ReleaseError::Fetch { release: release.coordinate().into(), reason: "offline".into() })
    }
}

fn release_ref() -> ReleaseRef {
    ReleaseRef::parse(COORD).unwrap()
}

#[test]
fn a_second_machine_fetches_the_identical_asset_and_matches_the_pin_without_a_prompt() {
    // PLG-20: locked entry travels to a machine with an empty cache; the
    // fetch serves identical bytes, the hash matches the pin, and the
    // strictest trust mode needs no prompt.
    let root = tempfile::tempdir().unwrap();
    let cache_dir = tempfile::tempdir().unwrap();
    let hash = sha256(BYTES);
    prewritten_lock(root.path(), "bjt", &hash);

    let cache = PluginCache::new(cache_dir.path().to_path_buf());
    let fetched = cache
        .fetch(&SameRelease, &release_ref(), TRIPLE, Some(&hash))
        .expect("fetch succeeds");
    assert_eq!(fetched.content_hash, hash, "identical bytes, identical hash");
    ensure_release_trusted(
        root.path(), &manifest("bjt"), COORD, &fetched.content_hash, TRIPLE, None,
        TrustMode::RejectUntrusted,
    )
    .expect("pin match needs no prompt");
    let locked = lock(root.path());
    let entry = locked.plugin_entry("bjt").unwrap();
    assert_eq!(entry.content_hash.as_deref(), Some(hash.as_str()), "pin unchanged");
    assert_eq!(entry.abi.as_deref(), Some(TRIPLE));
}

#[test]
fn a_cached_and_pinned_binary_loads_without_network() {
    // Offline-after-first-fetch (spec edge case): cache + pin present,
    // network gone — the binary loads from cache, never re-downloads.
    let root = tempfile::tempdir().unwrap();
    let cache_dir = tempfile::tempdir().unwrap();
    let hash = sha256(BYTES);
    prewritten_lock(root.path(), "bjt", &hash);

    let cache = PluginCache::new(cache_dir.path().to_path_buf());
    std::fs::write(cache.pinned_path(&hash, TRIPLE), BYTES).unwrap();
    let fetched = cache
        .fetch(&NoNetwork, &release_ref(), TRIPLE, Some(&hash))
        .expect("loads from cache offline");
    assert_eq!(fetched.content_hash, hash);
    ensure_release_trusted(
        root.path(), &manifest("bjt"), COORD, &fetched.content_hash, TRIPLE, None,
        TrustMode::RejectUntrusted,
    )
    .expect("pinned entry needs no prompt");
}

#[test]
fn an_offline_second_machine_without_cache_fails_loud() {
    // The other half of the edge case: no cache AND no network is a loud
    // fetch error, never a silent skip.
    let root = tempfile::tempdir().unwrap();
    let cache_dir = tempfile::tempdir().unwrap();
    let hash = sha256(BYTES);
    prewritten_lock(root.path(), "bjt", &hash);

    let cache = PluginCache::new(cache_dir.path().to_path_buf());
    let err = cache.fetch(&NoNetwork, &release_ref(), TRIPLE, Some(&hash)).unwrap_err();
    assert!(err.to_string().contains("offline"), "{err}");
}
