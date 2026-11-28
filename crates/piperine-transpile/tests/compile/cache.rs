//! The cache (feature `ir-rust`, RS-04 AC 1 to 5 and 8, RS-Q3; checks C18, C19, C22): racing
//! builds of one key leave one complete entry and no staging, through `Cache::build` and through
//! `Transpiler::compile` itself; a directory that cannot be used is named; the key changes with
//! any part; a new cache directory is private. Entries here hold stand-in files and no compiler
//! runs, except in the race of `Transpiler::compile`, which builds the resistor with `rustc`.

use std::path::Path;
use std::sync::{Arc, Barrier};
use std::time::Duration;

use piperine_ir::IrBuilder;
use piperine_transpile::{AbiSource, Cache, Entry, Error, Key, Settings, Transpiler};

use piperine_ir_fixtures::devices;

use devices::Devices;

/// A staging directory of the cache, filled and listed.
struct CacheStaging;

impl CacheStaging {
    /// Fills a staging directory the way a build does, with `tag` as the library's bytes.
    fn fill(staging: &Path, tag: &str) -> Result<(), Error> {
        let entry = Entry::at(staging);
        for (path, text) in [
            (entry.source(), "// design"),
            (entry.runtime(), "// rt"),
            (entry.library(), tag),
        ] {
            std::fs::write(&path, text).map_err(|e| Error::Cache { dir: staging.to_path_buf(), cause: e.to_string() })?;
        }
        Ok(())
    }

    fn names(dir: &Path) -> Vec<String> {
        let mut v: Vec<String> =
            std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        v.sort();
        v
    }
}

// RS-04 AC 4: two builds at the same time on an empty cache both complete and leave one
// complete entry (here 8, held at a barrier inside `make` so all stage before any renames).
#[test]
fn two_racing_builds_leave_one_entry_and_no_staging() {
    let root = tempfile::tempdir().unwrap();
    let cache = Arc::new(Cache::at(root.path()).unwrap());
    let key = Key::of(&[b"source", b"rt"]);
    let barrier = Arc::new(Barrier::new(8));
    let handles: Vec<_> = (0..8)
        .map(|i| {
            let (cache, key, barrier) = (cache.clone(), key.clone(), barrier.clone());
            std::thread::spawn(move || {
                cache.build(&key, "rustc 1.98.0", &["--edition", "2024"], |staging| {
                    barrier.wait();
                    CacheStaging::fill(staging, &format!("library built by thread {i}"))
                })
            })
        })
        .collect();
    let entries: Vec<Entry> = handles.into_iter().map(|h| h.join().unwrap().expect("every build completes")).collect();
    assert!(entries.windows(2).all(|w| w[0] == w[1]), "all builds return the one entry");

    assert_eq!(CacheStaging::names(cache.rust_dir()), vec![key.as_str().to_string()], "one entry, no staging directory");
    let entry = cache.entry(&key).expect("the entry is complete");
    assert_eq!(CacheStaging::names(entry.dir()).len(), 4, "design.rs, rt.rs, library, manifest");
    // the manifest belongs to the library that stayed (RS-04 AC 5)
    let library = std::fs::read(entry.library()).unwrap();
    let manifest = entry.manifest().unwrap();
    assert_eq!(manifest.library_sha256.len(), 64);
    assert_eq!(manifest.flags, "--edition 2024");
    assert_eq!(manifest.rustc, "rustc 1.98.0");
    assert!(String::from_utf8(library).unwrap().starts_with("library built by thread"));
    assert_eq!(manifest.library_sha256, piperine_transpile::Manifest::hash(&std::fs::read(entry.library()).unwrap()));

    // verification F5: the same race through `Transpiler::compile`, eight builds of one design on
    // one empty cache, their starts spread over the build so lookups fall before, during and after
    // another's rename; a build that finds no entry removes nothing, so each one completes and
    // loads, and one complete entry stays
    let root = tempfile::tempdir().unwrap();
    let settings = Settings { rustc: None, cache: root.path().to_path_buf() };
    let mut b = IrBuilder::new();
    let body = Devices::resistor(&mut b);
    let top = Devices::alone(&mut b, body, "R", 1, vec![1000.0]);
    let transpiler = Arc::new(Transpiler::new(Arc::new(b.finish(top))).unwrap_or_else(|e| panic!("{e}")));
    let handles: Vec<_> = (0..8u64)
        .map(|i| {
            let (transpiler, settings) = (transpiler.clone(), settings.clone());
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(150 * i));
                transpiler.compile(&settings).map(|compiled| compiled.report().cache_hit)
            })
        })
        .collect();
    for (i, h) in handles.into_iter().enumerate() {
        h.join().unwrap().unwrap_or_else(|e| panic!("compile {i} of 8 completes and loads: {e}"));
    }
    let rust = Cache::at(root.path()).unwrap();
    let left: Vec<String> = CacheStaging::names(rust.rust_dir()).into_iter().filter(|n| !n.starts_with("abi-")).collect();
    assert_eq!(left.len(), 1, "one design entry, no staging directory: {left:?}");
    assert!(CacheStaging::names(rust.rust_dir()).iter().all(|n| !n.starts_with('.')), "no staging directory left");
}

// RS-04 AC 8: a directory that cannot be created or written fails with an error naming it.
#[test]
fn an_unwritable_directory_is_named() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("a-file");
    std::fs::write(&file, "not a directory").unwrap();
    let under_a_file = file.join("cache");
    match Cache::at(&under_a_file) {
        Err(Error::Cache { dir, .. }) => assert_eq!(dir, under_a_file),
        other => panic!("{other:?}"),
    }

    // a directory that exists but cannot be written (a root user can write it anyway: then
    // there is nothing to refuse)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let locked = root.path().join("locked");
        std::fs::create_dir(&locked).unwrap();
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o500)).unwrap();
        let writable = std::fs::write(locked.join("probe"), "").is_ok();
        let result = Cache::at(&locked);
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o700)).unwrap();
        if !writable {
            match result {
                Err(Error::Cache { dir, .. }) => assert_eq!(dir, locked),
                other => panic!("{other:?}"),
            }
        }
    }
}

// RS-04 AC 2 and 3 (and RS-Q3): the key changes with the source, the runtime, the ABI sources,
// the compiler's version text and the flags - any one part.
#[test]
fn the_key_changes_with_any_part() {
    let base: [&[u8]; 5] = [b"design source", b"rt source", b"abi sources", b"rustc 1.98.0", b"--edition 2024"];
    let key = Key::of(&base);
    assert_eq!(key.as_str().len(), 32);
    assert!(key.as_str().bytes().all(|b| b.is_ascii_hexdigit()));
    assert_eq!(Key::of(&base), key, "the same parts, the same key");
    for i in 0..base.len() {
        let mut changed = base;
        changed[i] = b"changed";
        assert_ne!(Key::of(&changed), key, "part {i} must change the key");
    }
    // the parts are not run together
    assert_ne!(Key::of(&[b"ab", b"c"]), Key::of(&[b"a", b"bc"]));
}

// EU-16 AC 1 (T36): the contract is edited in place and stays major 1, so the cache key holds the
// contract's own source: an entry built against the earlier contract (a `Field` that still had
// `flags` and `known`) is keyed differently, is not found by the current key, and is rebuilt. The
// key is formed by `Transpiler::cache_keys`, the function `compile` calls, and the test finds the
// entry a real `compile` placed by that key: a `compile` that stopped hashing the ABI source would
// place it elsewhere and fail here.
#[test]
fn an_entry_of_the_earlier_contract_is_rebuilt() {
    let root = tempfile::tempdir().unwrap();
    let settings = Settings { rustc: None, cache: root.path().to_path_buf() };
    let mut b = IrBuilder::new();
    let body = Devices::resistor(&mut b);
    let top = Devices::alone(&mut b, body, "R", 1, vec![1000.0]);
    let transpiler = Transpiler::new(Arc::new(b.finish(top))).unwrap_or_else(|e| panic!("{e}"));
    let compiled = transpiler.compile(&settings).unwrap_or_else(|e| panic!("{e}"));
    assert!(!compiled.report().cache_hit, "the first compile builds");

    let source = transpiler.source().expect("the design prints");
    let runtime = piperine_transpile::rt::Runtime::files(transpiler.digital() || transpiler.loops());
    let version = piperine_transpile::Rustc::locate(&settings).expect("rustc").version_text().to_string();
    let current: Vec<(&str, &str)> = AbiSource::FILES.to_vec();
    let earlier: Vec<(String, String)> = current
        .iter()
        .map(|(name, text)| (name.to_string(), if *name == "raw/shape.rs" { text.replace("pub count: u32,\n}", "pub count: u32,\n    pub flags: u32,\n    pub known: u32,\n}") } else { text.to_string() }))
        .collect();
    let earlier: Vec<(&str, &str)> = earlier.iter().map(|(name, text)| (name.as_str(), text.as_str())).collect();
    assert_ne!(current, earlier, "the earlier contract's `Field` is found in `raw/shape.rs` to be replaced");

    let cache = Cache::at(root.path()).unwrap();
    let (current_key, _) = Transpiler::cache_keys(&source, &runtime, &version, &current);
    let (earlier_key, _) = Transpiler::cache_keys(&source, &runtime, &version, &earlier);
    assert_ne!(current_key, earlier_key, "a changed contract is a changed key");
    assert!(cache.entry(&current_key).is_some(), "`compile` placed its entry under the key that holds the ABI source");
    assert!(cache.entry(&earlier_key).is_none(), "an entry of the earlier contract is not there: the library is rebuilt, never loaded");
}

// RS-04 AC 8 / the design: a cache directory is created private, 0700 on Unix.
#[cfg(unix)]
#[test]
fn a_directory_is_created_private() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join("piperine");
    let cache = Cache::at(&dir).unwrap();
    for d in [dir.as_path(), cache.rust_dir()] {
        let mode = std::fs::metadata(d).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o700, "{}", d.display());
    }
}

/// Two threads of one process open the same cache directory at the same instant; both succeed
/// and no probe is left behind (orchestrator extra: the probe name is unique per call).
#[test]
fn concurrent_opens_of_one_directory_both_succeed() {
    let dir = tempfile::tempdir().expect("tmp");
    for _ in 0..200 {
        let barrier = Arc::new(Barrier::new(2));
        let opens: Vec<_> = (0..2)
            .map(|_| {
                let (barrier, path) = (barrier.clone(), dir.path().to_path_buf());
                std::thread::spawn(move || {
                    barrier.wait();
                    Cache::at(&path).map(|_| ())
                })
            })
            .collect();
        for open in opens {
            open.join().expect("thread").expect("a concurrent open succeeds");
        }
    }
    let left: Vec<_> = std::fs::read_dir(dir.path().join("rust")).expect("rust dir").map(|e| e.expect("entry").file_name()).collect();
    assert!(left.is_empty(), "no probe stays behind: {left:?}");
}
