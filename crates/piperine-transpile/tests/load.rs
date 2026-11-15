//! Loading a library through `piperine_abi_1` (feature `ir-rust`, RS-05 AC 3 and 4, RS-04 AC 5;
//! checks C29, C30, C31, C32, C46): a file that is no library, a library without the symbol and
//! one of a newer minor each fail with an error naming the file; a library whose bytes changed is
//! refused by the hash check before it is opened; and `unsafe` lives in `src/load.rs` only. The
//! test libraries are small sources compiled by `Rustc` into a temp dir.

use std::path::{Path, PathBuf};

use piperine_transpile::{AbiSource, Cache, Entry, Error, Key, Loaded, Rustc, Settings};

/// A compiler, the ABI rlib built from the embedded sources, and a private cache.
struct Workshop {
    dir: tempfile::TempDir,
    rustc: Rustc,
    abi: PathBuf,
    cache: Cache,
}

impl Workshop {
    fn new() -> Workshop {
        let dir = tempfile::tempdir().unwrap();
        let rustc = Rustc::locate(&Settings { rustc: None, cache: dir.path().to_path_buf() }).expect("rustc on PATH");
        let abi_src = dir.path().join("abi-src");
        AbiSource::write_to(&abi_src).unwrap();
        let abi = dir.path().join("libpiperine_abi.rlib");
        rustc.rlib(&abi_src.join("lib.rs"), &abi).expect("the ABI builds");
        let cache = Cache::at(&dir.path().join("cache")).unwrap();
        Workshop { dir, rustc, abi, cache }
    }

    /// An entry whose library is the `cdylib` of `source`; `key` keeps entries apart.
    fn library(&self, key: &str, source: &str) -> Entry {
        self.entry(key, |entry| {
            std::fs::write(entry.source(), source).unwrap();
            self.rustc.cdylib(&entry.source(), &self.abi, &entry.library()).expect("the test library builds");
        })
    }

    /// An entry whose library file holds `bytes`, whatever they are.
    fn file(&self, key: &str, bytes: &[u8]) -> Entry {
        self.entry(key, |entry| std::fs::write(entry.library(), bytes).unwrap())
    }

    fn entry(&self, key: &str, fill: impl FnOnce(&Entry)) -> Entry {
        self.cache
            .build(&Key::of(&[key.as_bytes()]), self.rustc.version_text(), Rustc::FLAGS, |staging| {
                let entry = Entry::at(staging);
                std::fs::write(entry.runtime(), "// no runtime").unwrap();
                std::fs::write(entry.source(), "// no source").unwrap();
                fill(&entry);
                Ok(())
            })
            .expect("the entry is built")
    }

    fn root(&self) -> &Path {
        self.dir.path()
    }
}

/// A library of the given minor with no module, written against the ABI crate.
fn library_source(minor: &str) -> String {
    format!(
        r#"use piperine_abi::raw::Library;
use std::sync::OnceLock;
#[unsafe(no_mangle)]
pub extern "C" fn piperine_abi_1() -> *const Library {{
    static LIBRARY: OnceLock<Library> = OnceLock::new();
    LIBRARY.get_or_init(|| {{
        let mut library = piperine_abi::thunk::library("test", "load.rs", &[]);
        library.minor = {minor};
        library
    }})
}}
"#
    )
}

fn must_name(err: &Error, entry: &Entry, what: &str) {
    match err {
        Error::Load { file, cause } => {
            assert_eq!(file, &entry.library(), "the error names the library file");
            assert!(cause.contains(what), "cause `{cause}` should mention `{what}`");
        }
        other => panic!("expected Error::Load, got {other:?}"),
    }
    assert!(err.to_string().contains(&*entry.library().to_string_lossy()), "Display names the file: {err}");
}

// RS-05 AC 3: a library that cannot be opened fails before any analysis, naming the file.
#[test]
fn a_file_that_is_no_library_is_named() {
    let shop = Workshop::new();
    let entry = shop.file("no-library", b"this is text, not a shared object");
    must_name(&Loaded::open(&entry).expect_err("not a library"), &entry, "cannot be opened");
}

// RS-05 AC 3: a library without `piperine_abi_1` fails, naming the file and the symbol.
#[test]
fn a_library_without_the_symbol_is_named() {
    let shop = Workshop::new();
    let entry = shop.library("no-symbol", "#[unsafe(no_mangle)]\npub extern \"C\" fn something_else() {}\n");
    must_name(&Loaded::open(&entry).expect_err("no symbol"), &entry, "piperine_abi_1");
}

// RS-05 AC 3 / abi.md §12: a `Library.minor` newer than the host's is refused, naming the file;
// the same source at the host's minor loads.
#[test]
fn a_library_of_a_newer_minor_is_refused() {
    let shop = Workshop::new();
    let current = shop.library("minor-now", &library_source("piperine_abi::raw::MINOR"));
    let loaded = Loaded::open(&current).expect("the host's own minor loads");
    assert_eq!(loaded.names(), Vec::<&str>::new());
    assert!(loaded.module("anything").is_none());

    let future = shop.library("minor-next", &library_source("piperine_abi::raw::MINOR + 1"));
    must_name(&Loaded::open(&future).expect_err("a newer minor"), &future, "minor");
}

// RS-04 AC 5: bytes that do not match the recorded hash are not loaded. The changed file is a
// library that would load (trailing bytes do not stop a shared object), so a refusal can only be
// the hash check, made before the open.
#[test]
fn a_library_whose_bytes_changed_is_not_loaded() {
    let shop = Workshop::new();
    let entry = shop.library("changed", &library_source("piperine_abi::raw::MINOR"));
    let mut bytes = std::fs::read(entry.library()).unwrap();
    bytes.extend_from_slice(b"tampered");
    std::fs::write(entry.library(), &bytes).unwrap();
    let err = Loaded::open(&entry).expect_err("a changed library");
    must_name(&err, &entry, "bytes changed");
    assert!(!err.to_string().contains("cannot be opened"), "the hash check came before the open: {err}");
    // the manifest still records the original hash: the entry did not adopt the change
    assert_ne!(entry.manifest().unwrap().library_sha256, piperine_transpile::Manifest::hash(&bytes));
    let _ = shop.root();
}

/// Source files under `dir`, recursively.
fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())) {
        let path = entry.unwrap().path();
        if path.is_dir() {
            sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// The lines of `text` that open an `unsafe` block, fn, impl, trait or extern, or that name an
/// `unsafe(...)` attribute other than `unsafe(no_mangle)`; comments are not code.
fn unsafe_lines(text: &str) -> Vec<(usize, &str)> {
    text.lines()
        .enumerate()
        .filter(|(_, line)| !line.trim_start().starts_with("//"))
        .filter(|(_, line)| {
            let mut rest = *line;
            while let Some(at) = rest.find("unsafe") {
                let before_ok = at == 0 || !rest[..at].ends_with(|c: char| c.is_alphanumeric() || c == '_' || c == '"');
                let after = rest[at + "unsafe".len()..].trim_start();
                let opens = ["{", "fn ", "impl", "trait", "extern"].iter().any(|k| after.starts_with(k));
                let attribute = after.starts_with('(') && !after.starts_with("(no_mangle)");
                if before_ok && (opens || attribute) {
                    return true;
                }
                rest = &rest[at + "unsafe".len()..];
            }
            false
        })
        .map(|(i, l)| (i + 1, l))
        .collect()
}

// RS-05 AC 4 and C32: every `unsafe` of the feature is in `src/load.rs`, each block under a
// `// SAFETY:` comment citing abi.md; no other source and no golden file has one, but the
// goldens each export the symbol with the one allowed attribute `#[unsafe(no_mangle)]`
// (orchestrator ruling 2026-11-15).
#[test]
fn unsafe_lives_in_load_only() {
    // the scan itself: it finds each form, and passes the one allowed attribute and a comment
    assert_eq!(unsafe_lines("let x = unsafe { f() };\nunsafe fn g() {}\nunsafe impl Send for T {}\n#[unsafe(export_name = \"x\")]").len(), 4);
    assert_eq!(unsafe_lines("#[unsafe(no_mangle)]\n// an unsafe { block } in a comment\nlet s = \"unsafe\";").len(), 0);

    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let load = root.join("src/load.rs");
    let mut files = Vec::new();
    sources(&root.join("src"), &mut files);
    let golden_dir = root.join("tests/golden");
    let mut goldens = Vec::new();
    sources(&golden_dir, &mut goldens);
    assert_eq!(goldens.len(), 5, "the five M1 goldens");

    for file in files.iter().chain(&goldens).filter(|f| **f != load) {
        let text = std::fs::read_to_string(file).unwrap();
        assert_eq!(unsafe_lines(&text), vec![], "{} holds unsafe code outside load.rs", file.display());
    }
    for golden in &goldens {
        let text = std::fs::read_to_string(golden).unwrap();
        let exports = text.lines().filter(|l| l.trim() == "#[unsafe(no_mangle)]").count();
        assert_eq!(exports, 1, "{} exports piperine_abi_1 with the one allowed attribute", golden.display());
    }

    let text = std::fs::read_to_string(&load).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    let blocks = unsafe_lines(&text);
    assert!(blocks.len() >= 3, "load.rs holds the unsafe of the feature: {blocks:?}");
    for (line, _) in blocks {
        let above: Vec<&str> =
            lines[..line - 1].iter().rev().take_while(|l| l.trim_start().starts_with("//")).copied().collect();
        assert!(above.iter().any(|l| l.contains("SAFETY:")), "load.rs:{line} has no `// SAFETY:` comment");
        assert!(above.iter().any(|l| l.contains("abi.md")), "load.rs:{line}: the SAFETY comment cites abi.md");
    }
}
