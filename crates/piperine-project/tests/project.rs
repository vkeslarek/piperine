//! Scope: the entry surface of `piperine-project` (feature `msl-packages`): the project root is
//! the nearest `Piperine.toml` (PKG-14 AC 1), and the crate holds no release-fetch, plugin or
//! `SourceMap` surface (PKG-15 AC 1). The loading of a whole project joins this target with T25.

use std::fs;
use std::path::{Path, PathBuf};

use piperine_project::find_project_root;

fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

#[test]
fn the_project_root_is_the_nearest_manifest() {
    let tmp = tempfile::tempdir().unwrap();
    let base = tmp.path().canonicalize().unwrap();
    let app = base.join("ws/app");
    write(&app.join("Piperine.toml"), "");
    write(&app.join("src/deep/x.msl"), "");
    // The directory itself, and any directory below it.
    assert_eq!(find_project_root(&app), Some(app.clone()));
    assert_eq!(find_project_root(&app.join("src/deep")), Some(app.clone()));
    // The nearest of two manifests wins.
    let inner = app.join("vendor/inner");
    write(&inner.join("Piperine.toml"), "");
    write(&inner.join("src/x.msl"), "");
    assert_eq!(find_project_root(&inner.join("src")), Some(inner.clone()));
    assert_eq!(find_project_root(&app.join("vendor")), Some(app.clone()));
    // No manifest on the way up: none. (The temporary directory is searched up to the filesystem
    // root, so a manifest above it would make this fail; there is none.)
    let bare = base.join("bare/deeper");
    fs::create_dir_all(&bare).unwrap();
    assert_eq!(find_project_root(&bare), None);
}

fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn the_crate_holds_no_retired_surface() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest = fs::read_to_string(root.join("Cargo.toml")).unwrap();
    for name in ["ureq", "sha2", "serde_json", "thiserror", "serde"] {
        assert!(
            !manifest.lines().any(|l| l.trim_start().starts_with(&format!("{name} ")) || l.trim_start().starts_with(&format!("{name}="))),
            "Cargo.toml still depends on `{name}`"
        );
    }
    for gone in ["src/release.rs", "src/source_map.rs", "tests/release.rs"] {
        assert!(!root.join(gone).exists(), "{gone} still exists");
    }
    let mut files = Vec::new();
    sources(&root.join("src"), &mut files);
    assert!(files.len() >= 2, "{files:?}");
    // The crate's sources: the tests spell a retired table (`[plugins]`) to prove it is refused.
    for file in files {
        let text = fs::read_to_string(&file).unwrap().to_lowercase();
        for word in ["plugin", "source_map", "sourcemap", "release", "get_current_project_root"] {
            assert!(!text.contains(word), "{} mentions `{word}`", file.display());
        }
    }
}
