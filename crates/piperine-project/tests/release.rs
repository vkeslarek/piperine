//! Release fetch mechanics (plugin-interface v2, PLG-16/17, design §4):
//! the stubbed release serves a triple-named asset; the cache fetches,
//! hashes, and stores it content-addressed — and a pinned hash loads from
//! the cache with no network at all. No test hits the real network: the
//! HTTP layer is the `ReleaseClient` trait.

use piperine_project::release::{PluginCache, ReleaseClient, ReleaseError, ReleaseRef};

const TRIPLE: &str = "x86_64-unknown-linux-gnu";
const BYTES: &[u8] = b"fake device binary v1";

/// A stubbed GitHub release: one asset per listed triple, one fixed
/// payload.
struct Stub {
    assets: Vec<String>,
    bytes: Vec<u8>,
}

impl ReleaseClient for Stub {
    fn list_assets(&self, _release: &ReleaseRef) -> Result<Vec<String>, ReleaseError> {
        Ok(self.assets.clone())
    }
    fn download_asset(&self, _release: &ReleaseRef, _asset: &str) -> Result<Vec<u8>, ReleaseError> {
        Ok(self.bytes.clone())
    }
}

/// A client that stands in for "no network": every call fails.
struct Offline;

impl ReleaseClient for Offline {
    fn list_assets(&self, release: &ReleaseRef) -> Result<Vec<String>, ReleaseError> {
        Err(ReleaseError::Fetch { release: release.coordinate().into(), reason: "offline".into() })
    }
    fn download_asset(&self, release: &ReleaseRef, _asset: &str) -> Result<Vec<u8>, ReleaseError> {
        Err(ReleaseError::Fetch { release: release.coordinate().into(), reason: "offline".into() })
    }
}

fn release() -> ReleaseRef {
    ReleaseRef::parse("github:acme/bjt@v1").unwrap()
}

fn stub() -> Stub {
    Stub {
        assets: vec![
            format!("libbjt-{TRIPLE}.so"),
            "libbjt-aarch64-apple-darwin.dylib".to_string(),
        ],
        bytes: BYTES.to_vec(),
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}

#[test]
fn fetch_downloads_hashes_and_caches_the_triple_asset() {
    let dir = tempfile::tempdir().unwrap();
    let cache = PluginCache::new(dir.path().to_path_buf());
    let fetched = cache.fetch(&stub(), &release(), TRIPLE, None).expect("fetch");
    assert_eq!(fetched.content_hash, format!("sha256:{}", sha256_hex(BYTES)));
    assert_eq!(fetched.asset, format!("libbjt-{TRIPLE}.so"));
    assert_eq!(fetched.triple, TRIPLE);
    // Content-addressed: the cache file is exactly the hash + extension.
    assert_eq!(fetched.path, cache.pinned_path(&fetched.content_hash, TRIPLE));
    assert_eq!(std::fs::read(&fetched.path).unwrap(), BYTES);
}

#[test]
fn a_wrong_triple_release_errors_naming_triple_and_release() {
    let dir = tempfile::tempdir().unwrap();
    let cache = PluginCache::new(dir.path().to_path_buf());
    let err = cache
        .fetch(&stub(), &release(), "riscv64-unknown-linux-gnu", None)
        .unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("riscv64-unknown-linux-gnu") && msg.contains("github:acme/bjt@v1"),
        "{msg}"
    );
}

#[test]
fn a_pinned_hash_loads_from_cache_without_network() {
    let dir = tempfile::tempdir().unwrap();
    let cache = PluginCache::new(dir.path().to_path_buf());
    let hash = format!("sha256:{}", sha256_hex(BYTES));
    std::fs::write(cache.pinned_path(&hash, TRIPLE), BYTES).unwrap();
    // The offline client fails on any call — a successful fetch proves no
    // network was touched.
    let fetched = cache.fetch(&Offline, &release(), TRIPLE, Some(&hash)).expect("cached");
    assert_eq!(fetched.content_hash, hash);
    assert_eq!(std::fs::read(&fetched.path).unwrap(), BYTES);
}

#[test]
fn a_corrupt_cached_artifact_fails_loud() {
    let dir = tempfile::tempdir().unwrap();
    let cache = PluginCache::new(dir.path().to_path_buf());
    let hash = format!("sha256:{}", sha256_hex(BYTES));
    // Bytes under the pin's name that no longer match the pin.
    std::fs::write(cache.pinned_path(&hash, TRIPLE), b"tampered").unwrap();
    let err = cache.fetch(&stub(), &release(), TRIPLE, Some(&hash)).unwrap_err();
    assert!(err.to_string().contains("corrupt"), "{err}");
}
