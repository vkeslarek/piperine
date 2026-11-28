//! The embedded ABI sources (feature `ir-rust`, RS-05; checks C20, C21): the table holds exactly
//! the files of `crates/piperine-abi/src`, byte for byte, so a new ABI file cannot be left out;
//! and the table alone builds an `rlib` with `rustc`, no `cargo`, no checkout.

use std::collections::BTreeMap;
use std::path::PathBuf;

use piperine_transpile::{AbiSource, Rustc, Settings};

#[test]
fn the_embedded_sources_are_the_abi_crate() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../piperine-abi/src");
    let on_disk: BTreeMap<String, Vec<u8>> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|entry| {
            let path = entry.expect("dir entry").path();
            (path.file_name().unwrap().to_string_lossy().into_owned(), std::fs::read(&path).expect("read"))
        })
        .collect();
    let embedded: BTreeMap<String, Vec<u8>> =
        AbiSource::FILES.iter().map(|(name, text)| (name.to_string(), text.as_bytes().to_vec())).collect();
    assert_eq!(
        embedded.keys().collect::<Vec<_>>(),
        on_disk.keys().collect::<Vec<_>>(),
        "AbiSource::FILES must list every file of crates/piperine-abi/src"
    );
    for (name, bytes) in &on_disk {
        assert!(embedded[name] == *bytes, "{name} differs from the embedded copy");
    }
}

#[test]
fn the_abi_builds_with_rustc_alone() {
    let work = tempfile::tempdir().unwrap();
    let src = work.path().join("abi");
    AbiSource::write_to(&src).expect("write the sources");
    let written: Vec<String> = {
        let mut names: Vec<String> = std::fs::read_dir(&src)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    };
    let mut expected: Vec<String> = AbiSource::FILES.iter().map(|(n, _)| n.to_string()).collect();
    expected.sort();
    assert_eq!(written, expected);

    let rustc = Rustc::locate(&Settings { rustc: None, cache: work.path().to_path_buf() }).expect("rustc on PATH");
    let out = work.path().join("libpiperine_abi.rlib");
    let warnings = rustc.rlib(&src.join("lib.rs"), &out).expect("the ABI builds with rustc alone");
    assert!(out.is_file());
    assert_eq!(warnings, "", "the embedded ABI builds with no warning");
}
