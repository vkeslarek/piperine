//! The cache (feature `ir-rust`, RS-04 AC 1 to 5 and 8, RS-Q3; checks C18, C19, C22): racing
//! builds of one key leave one complete entry and no staging; a directory that cannot be used is
//! named; the key changes with any part; a new cache directory is private. Entries here hold
//! stand-in files, no compiler runs.

use std::path::Path;
use std::sync::{Arc, Barrier};

use piperine_transpile::{Cache, Entry, Error, Key};

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
                    fill(staging, &format!("library built by thread {i}"))
                })
            })
        })
        .collect();
    let entries: Vec<Entry> = handles.into_iter().map(|h| h.join().unwrap().expect("every build completes")).collect();
    assert!(entries.windows(2).all(|w| w[0] == w[1]), "all builds return the one entry");

    assert_eq!(names(cache.rust_dir()), vec![key.as_str().to_string()], "one entry, no staging directory");
    let entry = cache.entry(&key).expect("the entry is complete");
    assert_eq!(names(entry.dir()).len(), 4, "design.rs, rt.rs, library, manifest");
    // the manifest belongs to the library that stayed (RS-04 AC 5)
    let library = std::fs::read(entry.library()).unwrap();
    let manifest = entry.manifest().unwrap();
    assert_eq!(manifest.library_sha256.len(), 64);
    assert_eq!(manifest.flags, "--edition 2024");
    assert_eq!(manifest.rustc, "rustc 1.98.0");
    assert!(String::from_utf8(library).unwrap().starts_with("library built by thread"));
    assert_eq!(manifest.library_sha256, piperine_transpile::Manifest::hash(&std::fs::read(entry.library()).unwrap()));
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
