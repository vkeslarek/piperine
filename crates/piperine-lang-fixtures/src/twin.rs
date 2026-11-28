//! The six twins of `tests/e2e/twins/`, loaded from disk.

use std::path::PathBuf;

use piperine_lang::pom::Design;

/// A twin of `tests/e2e/twins/`, the design of one `.msl` file the way a project would load it.
pub struct TwinSource;

impl TwinSource {
    /// The design of the twin `file` (its name without `.msl`).
    pub fn design(file: &str) -> Design {
        let path: PathBuf = [env!("CARGO_MANIFEST_DIR"), "..", "..", "tests", "e2e", "twins", &format!("{file}.msl")].iter().collect();
        Design::builder().root_file(path).build()
    }
}
