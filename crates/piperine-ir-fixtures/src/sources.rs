//! The Rust sources of a crate under test, found through every directory of `src/`: the
//! static checks over the source text (no mutating handle method, no parser of the printed
//! text) cover a module that lives in a subdirectory of its parent as they do a flat one.

use std::path::{Path, PathBuf};

/// The source files of a crate, read from the manifest directory its test passes (`env!("CARGO_MANIFEST_DIR")`).
pub struct RustSources;

impl RustSources {
    /// Every `.rs` file under `src/` of the crate at `manifest_directory`, each with its path and text, in path order.
    pub fn files(manifest_directory: &str) -> Vec<(String, String)> {
        let mut pending: Vec<PathBuf> = vec![Path::new(manifest_directory).join("src")];
        let mut files: Vec<PathBuf> = Vec::new();
        while let Some(dir) = pending.pop() {
            for entry in std::fs::read_dir(&dir).expect("read a source directory") {
                let path = entry.expect("dir entry").path();
                if path.is_dir() {
                    pending.push(path);
                } else {
                    files.push(path);
                }
            }
        }
        files.sort();
        files.into_iter().map(|path| (path.display().to_string(), std::fs::read_to_string(&path).expect("read a source"))).collect()
    }
}
