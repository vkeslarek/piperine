//! The cache of built libraries (design "Cache"; RS-04, RS-Q3): an entry per key, built in a
//! staging sibling and renamed into place, so a reader never sees a half-written entry and the
//! loser of a race discards its copy.
//!
//! ```text
//! <cache>/rust/abi-<key>/libpiperine_abi.rlib
//! <cache>/rust/<key>/{design.rs, rt.rs, <dll prefix>design<dll suffix>, manifest}
//! ```
//!
//! The cache directory is created private (`0700` on Unix): the library hash the manifest
//! records is checked before a load, and a directory only the user can write is what keeps the
//! file from changing between that check and the load. There is no eviction.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use sha2::{Digest, Sha256};

use crate::error::Error;

/// 32 hex characters of a SHA-256 over the parts that decide a build (RS-Q3).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Key(String);

impl Key {
    /// The key of `parts`. Each part is length-prefixed, so `["ab", "c"]` and `["a", "bc"]`
    /// differ.
    pub fn of(parts: &[&[u8]]) -> Key {
        let mut hash = Sha256::new();
        for part in parts {
            hash.update((part.len() as u64).to_le_bytes());
            hash.update(part);
        }
        let digest = hash.finalize();
        Key(digest[..16].iter().map(|b| format!("{b:02x}")).collect())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// What an entry records about its library (RS-04 AC 5): the library's hash, and the compiler
/// and flags that built it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Manifest {
    /// The SHA-256 of the library's bytes, 64 hex characters.
    pub library_sha256: String,
    /// The flags, joined by single spaces.
    pub flags: String,
    /// The output of `rustc -vV`.
    pub rustc: String,
}

impl Manifest {
    /// The SHA-256 of `bytes` as 64 hex characters.
    pub fn hash(bytes: &[u8]) -> String {
        Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
    }

    fn render(&self) -> String {
        format!("library-sha256: {}\nflags: {}\nrustc:\n{}", self.library_sha256, self.flags, self.rustc)
    }

    fn parse(text: &str) -> Option<Manifest> {
        let mut lines = text.splitn(4, '\n');
        let library_sha256 = lines.next()?.strip_prefix("library-sha256: ")?.to_string();
        let flags = lines.next()?.strip_prefix("flags: ")?.to_string();
        if lines.next()? != "rustc:" {
            return None;
        }
        Some(Manifest { library_sha256, flags, rustc: lines.next().unwrap_or("").to_string() })
    }
}

/// One built design: a directory holding its source, runtime source, library and manifest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    dir: PathBuf,
}

impl Entry {
    /// The entry layout rooted at `dir`: the staging directory a build writes into, or a built
    /// entry.
    pub fn at(dir: &Path) -> Entry {
        Entry { dir: dir.to_path_buf() }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// `design.rs`, the generated source (RS-02: kept beside the library).
    pub fn source(&self) -> PathBuf {
        self.dir.join("design.rs")
    }

    /// `rt.rs`, the runtime source `design.rs` declares as `mod rt`.
    pub fn runtime(&self) -> PathBuf {
        self.dir.join("rt.rs")
    }

    /// The library, named for the platform (`libdesign.so`, `design.dll`, `libdesign.dylib`).
    pub fn library(&self) -> PathBuf {
        self.dir.join(format!("{}design{}", std::env::consts::DLL_PREFIX, std::env::consts::DLL_SUFFIX))
    }

    pub fn manifest_path(&self) -> PathBuf {
        self.dir.join("manifest")
    }

    /// Reads the manifest; a missing or malformed one is `Error::Cache`.
    pub fn manifest(&self) -> Result<Manifest, Error> {
        let path = self.manifest_path();
        let fail = |cause: String| Error::Cache { dir: self.dir.clone(), cause };
        let text = std::fs::read_to_string(&path).map_err(|e| fail(format!("{}: {e}", path.display())))?;
        Manifest::parse(&text).ok_or_else(|| fail(format!("{} is malformed", path.display())))
    }

    /// Whether all four files are there.
    fn complete(&self) -> bool {
        [self.source(), self.runtime(), self.library(), self.manifest_path()].iter().all(|p| p.is_file())
    }
}

/// The cache directory.
#[derive(Clone, Debug)]
pub struct Cache {
    /// `<dir>/rust`, where entries, ABI libraries and staging directories live.
    rust: PathBuf,
}

/// Distinguishes the staging directories of one process.
static STAGING: AtomicU64 = AtomicU64::new(0);

impl Cache {
    /// The cache at `dir`, created (private on Unix) with its `rust` directory when absent. A
    /// directory that cannot be created or written is `Error::Cache` naming it.
    pub fn at(dir: &Path) -> Result<Cache, Error> {
        let rust = dir.join("rust");
        let fail = |e: std::io::Error| Error::Cache { dir: dir.to_path_buf(), cause: e.to_string() };
        Cache::create_private(&rust).map_err(fail)?;
        // Created is not writable: prove the entries can be staged.
        let probe = rust.join(format!(".probe-{}", std::process::id()));
        std::fs::create_dir(&probe).map_err(fail)?;
        std::fs::remove_dir(&probe).map_err(fail)?;
        Ok(Cache { rust })
    }

    fn create_private(dir: &Path) -> std::io::Result<()> {
        let mut builder = std::fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
        builder.create(dir)
    }

    /// The directory the entries live in.
    pub fn rust_dir(&self) -> &Path {
        &self.rust
    }

    /// The entry of `key`, when it is present and complete.
    pub fn entry(&self, key: &Key) -> Option<Entry> {
        let entry = Entry::at(&self.rust.join(key.as_str()));
        entry.complete().then_some(entry)
    }

    /// Builds the entry of `key`: `make` fills a staging directory (laid out by [`Entry::at`])
    /// with `design.rs`, `rt.rs` and the library; the manifest is written here from the
    /// library's hash, `rustc` (the output of `-vV`) and `flags`; the staging directory is
    /// renamed into place. When another build of the key finishes first, this one's copy is
    /// removed and the winner's entry is returned. When `make` fails, the staging directory
    /// stays on disk, for the error's source path to name (RS-03 AC 3).
    pub fn build(
        &self,
        key: &Key,
        rustc: &str,
        flags: &[&str],
        make: impl FnOnce(&Path) -> Result<(), Error>,
    ) -> Result<Entry, Error> {
        let staging = self.stage(key.as_str());
        let aside = Entry::at(&staging);
        let fail = |e: std::io::Error| Error::Cache { dir: self.rust.clone(), cause: e.to_string() };
        std::fs::create_dir(&staging).map_err(fail)?;
        make(&staging)?;
        let library = std::fs::read(aside.library())
            .map_err(|e| Error::Cache { dir: staging.clone(), cause: format!("no library was built: {e}") })?;
        let manifest = Manifest { library_sha256: Manifest::hash(&library), flags: flags.join(" "), rustc: rustc.into() };
        std::fs::write(aside.manifest_path(), manifest.render()).map_err(fail)?;
        let entry = Entry::at(&self.rust.join(key.as_str()));
        self.place(&staging, entry.dir(), |d| Entry::at(d).complete())?;
        Ok(entry)
    }

    /// The `rlib` of the embedded ABI under `key`, built by `make` when absent. `make` writes
    /// `libpiperine_abi.rlib` into the staging directory it is given.
    pub fn abi(&self, key: &Key, make: impl FnOnce(&Path) -> Result<(), Error>) -> Result<PathBuf, Error> {
        let final_dir = self.rust.join(format!("abi-{}", key.as_str()));
        let rlib = final_dir.join("libpiperine_abi.rlib");
        if rlib.is_file() {
            return Ok(rlib);
        }
        let staging = self.stage(&format!("abi-{}", key.as_str()));
        std::fs::create_dir(&staging).map_err(|e| Error::Cache { dir: self.rust.clone(), cause: e.to_string() })?;
        make(&staging)?;
        self.place(&staging, &final_dir, |d| d.join("libpiperine_abi.rlib").is_file())?;
        Ok(rlib)
    }

    /// Removes the entry of `key` (a library that failed its hash check is rebuilt).
    pub fn remove(&self, key: &Key) -> Result<(), Error> {
        let dir = self.rust.join(key.as_str());
        match std::fs::remove_dir_all(&dir) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                Err(Error::Cache { dir, cause: e.to_string() })
            }
            _ => Ok(()),
        }
    }

    /// Removes every entry; the cache stays usable.
    pub fn clear(&self) -> Result<(), Error> {
        let fail = |e: std::io::Error| Error::Cache { dir: self.rust.clone(), cause: e.to_string() };
        std::fs::remove_dir_all(&self.rust).map_err(fail)?;
        Cache::create_private(&self.rust).map_err(fail)
    }

    fn stage(&self, name: &str) -> PathBuf {
        let n = STAGING.fetch_add(1, Ordering::SeqCst);
        self.rust.join(format!(".staging-{name}-{}-{n}", std::process::id()))
    }

    /// Renames `staging` to `target`. If that fails and `done(target)` says another build got
    /// there first, `staging` is removed and that build's result stands; otherwise the failure
    /// is the error.
    fn place(&self, staging: &Path, target: &Path, done: impl Fn(&Path) -> bool) -> Result<(), Error> {
        match std::fs::rename(staging, target) {
            Ok(()) => Ok(()),
            Err(_) if done(target) => {
                // best effort: a leftover staging directory is harmless and never loaded
                let _ = std::fs::remove_dir_all(staging);
                Ok(())
            }
            Err(e) => Err(Error::Cache { dir: target.to_path_buf(), cause: e.to_string() }),
        }
    }
}
