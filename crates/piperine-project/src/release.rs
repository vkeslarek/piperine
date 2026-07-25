//! Device-binary release resolution (plugin-interface v2, PLG-16/19,
//! design §4): a manifest's `device = { release = "github:owner/repo@tag" }`
//! resolves to the release asset named `lib<pkg>-<host-triple>.<ext>` —
//! no match is a loud `NoAssetForTriple`, never a silent skip and never a
//! build-from-source (D6). The HTTP layer is the [`ReleaseClient`] trait
//! so tests stub it; the fetch/cache/TOFU flow builds on this (T13+).

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ReleaseError {
    #[error("Bad release coordinate `{coord}`: {reason}")]
    BadCoordinate { coord: String, reason: String },
    #[error("No release asset for target triple `{triple}` in release `{release}` (expected an asset named `lib<pkg>-{triple}.<ext>`; prebuilt-binary only, no build-from-source)")]
    NoAssetForTriple { triple: String, release: String },
    #[error("Release fetch failed for `{release}`: {reason}")]
    Fetch { release: String, reason: String },
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// A parsed `github:owner/repo@tag` release coordinate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseRef {
    owner: String,
    repo: String,
    tag: String,
    coord: String,
}

impl ReleaseRef {
    /// Parse `github:owner/repo@tag`. Anything else fails loud — v1
    /// targets the `github:` scheme only (spec Out of Scope).
    pub fn parse(coord: &str) -> Result<Self, ReleaseError> {
        let bad = |reason: &str| ReleaseError::BadCoordinate {
            coord: coord.to_string(),
            reason: reason.to_string(),
        };
        let rest = coord
            .strip_prefix("github:")
            .ok_or_else(|| bad("expected the `github:owner/repo@tag` scheme"))?;
        let (path, tag) = rest
            .rsplit_once('@')
            .ok_or_else(|| bad("missing `@tag`"))?;
        let segments: Vec<&str> = path.split('/').collect();
        let [owner, repo] = segments.as_slice() else {
            return Err(bad("expected exactly `owner/repo`"));
        };
        if owner.is_empty() || repo.is_empty() || tag.is_empty() {
            return Err(bad("`owner`, `repo` and `tag` must be non-empty"));
        }
        Ok(Self {
            owner: owner.to_string(),
            repo: repo.to_string(),
            tag: tag.to_string(),
            coord: coord.to_string(),
        })
    }

    /// The original coordinate — the lockfile's `source` for the pin.
    pub fn coordinate(&self) -> &str {
        &self.coord
    }

    /// The package name used in asset names (`lib<pkg>-<triple>.<ext>`).
    pub fn package(&self) -> &str {
        &self.repo
    }

    /// The GitHub release API URL listing this release's assets.
    pub fn api_url(&self) -> String {
        format!(
            "https://api.github.com/repos/{}/{}/releases/tags/{}",
            self.owner, self.repo, self.tag
        )
    }

    /// The direct download URL of one asset of this release.
    pub fn download_url(&self, asset: &str) -> String {
        format!(
            "https://github.com/{}/{}/releases/download/{}/{}",
            self.owner, self.repo, self.tag, asset
        )
    }

    /// The asset name this release must carry for `triple`
    /// (`lib<pkg>-<triple>.<ext>` — spec assumption table, case-sensitive).
    pub fn asset_name(&self, triple: &str) -> String {
        format!("lib{}-{}.{}", self.repo, triple, Self::asset_extension(triple))
    }

    /// Pick this release's asset for `triple` out of a listing — a loud
    /// [`ReleaseError::NoAssetForTriple`] naming both when absent (PLG-19).
    pub fn select_asset(&self, assets: &[String], triple: &str) -> Result<String, ReleaseError> {
        let wanted = self.asset_name(triple);
        assets
            .iter()
            .find(|a| **a == wanted)
            .cloned()
            .ok_or_else(|| ReleaseError::NoAssetForTriple {
                triple: triple.to_string(),
                release: self.coord.clone(),
            })
    }

    /// The shared-library extension for a target triple.
    pub fn asset_extension(triple: &str) -> &'static str {
        if triple.contains("windows") {
            "dll"
        } else if triple.contains("apple") || triple.contains("darwin") {
            "dylib"
        } else {
            "so"
        }
    }

    /// The host's target triple — the exact `TARGET` this crate was built
    /// for (stamped by `build.rs`), with a platform-constants fallback.
    pub fn host_triple() -> String {
        let stamped = env!("PIPERINE_HOST_TRIPLE");
        if !stamped.is_empty() {
            return stamped.to_string();
        }
        format!("{}-unknown-{}", std::env::consts::ARCH, std::env::consts::OS)
    }
}

/// The release HTTP surface — stubbed in tests (no test hits the network).
pub trait ReleaseClient {
    /// The names of the release's assets.
    fn list_assets(&self, release: &ReleaseRef) -> Result<Vec<String>, ReleaseError>;
    /// The bytes of one asset.
    fn download_asset(&self, release: &ReleaseRef, asset: &str) -> Result<Vec<u8>, ReleaseError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_github_coordinate() {
        let r = ReleaseRef::parse("github:acme/bjt-models@v1.2.0").unwrap();
        assert_eq!(r.package(), "bjt-models");
        assert_eq!(r.coordinate(), "github:acme/bjt-models@v1.2.0");
        assert_eq!(
            r.api_url(),
            "https://api.github.com/repos/acme/bjt-models/releases/tags/v1.2.0"
        );
        assert_eq!(
            r.download_url("libbjt-models-x.so"),
            "https://github.com/acme/bjt-models/releases/download/v1.2.0/libbjt-models-x.so"
        );
    }

    #[test]
    fn malformed_coordinates_fail_loud() {
        for coord in [
            "acme/bjt@v1",
            "github:acme/bjt",
            "github:acme@v1",
            "github:/bjt@v1",
            "github:acme/@v1",
            "github:acme/bjt@",
            "github:a/b/c@v1",
        ] {
            let err = ReleaseRef::parse(coord).unwrap_err();
            assert!(matches!(err, ReleaseError::BadCoordinate { .. }), "{coord}: {err}");
        }
    }

    #[test]
    fn selects_the_host_triple_asset() {
        let r = ReleaseRef::parse("github:acme/bjt@v1").unwrap();
        let assets = vec![
            "libbjt-x86_64-unknown-linux-gnu.so".to_string(),
            "libbjt-x86_64-pc-windows-msvc.dll".to_string(),
            "libbjt-aarch64-apple-darwin.dylib".to_string(),
        ];
        assert_eq!(
            r.select_asset(&assets, "x86_64-pc-windows-msvc").unwrap(),
            "libbjt-x86_64-pc-windows-msvc.dll"
        );
        assert_eq!(
            r.select_asset(&assets, "aarch64-apple-darwin").unwrap(),
            "libbjt-aarch64-apple-darwin.dylib"
        );
    }

    #[test]
    fn asset_matching_is_case_sensitive() {
        // The convention is matched case-sensitively (spec assumption
        // table): `Lib…` does not satisfy `lib<pkg>-<triple>.<ext>`.
        let r = ReleaseRef::parse("github:acme/bjt@v1").unwrap();
        let assets = vec!["Libbjt-x86_64-unknown-linux-gnu.so".to_string()];
        let err = r.select_asset(&assets, "x86_64-unknown-linux-gnu").unwrap_err();
        assert!(matches!(err, ReleaseError::NoAssetForTriple { .. }), "{err}");
    }

    #[test]
    fn a_release_without_the_host_triple_fails_loud() {
        // PLG-19: the error names the triple AND the release — no silent
        // skip, no build-from-source.
        let r = ReleaseRef::parse("github:acme/bjt@v1").unwrap();
        let assets = vec!["libbjt-aarch64-apple-darwin.dylib".to_string()];
        let err = r.select_asset(&assets, "x86_64-unknown-linux-gnu").unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("x86_64-unknown-linux-gnu") && msg.contains("github:acme/bjt@v1"),
            "names triple + release: {msg}"
        );
    }

    #[test]
    fn asset_extension_follows_the_triple_family() {
        assert_eq!(ReleaseRef::asset_extension("x86_64-pc-windows-msvc"), "dll");
        assert_eq!(ReleaseRef::asset_extension("aarch64-apple-darwin"), "dylib");
        assert_eq!(ReleaseRef::asset_extension("x86_64-unknown-linux-gnu"), "so");
    }
}
