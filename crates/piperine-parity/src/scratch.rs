//! The scratch directory of one ngspice run.

use std::path::Path;

use tempfile::TempDir;

/// A directory created for one run and removed when dropped, so two runs (two tests on two
/// threads) never share an export file and nothing is left behind, a failing run included.
#[derive(Debug)]
pub struct ScratchDir {
    dir: TempDir,
}

impl ScratchDir {
    pub fn create(fixture: &str) -> Self {
        let dir = tempfile::Builder::new()
            .prefix(&format!("piperine-parity-{fixture}-"))
            .tempdir()
            .unwrap_or_else(|error| panic!("{fixture}: cannot create a scratch directory: {error}"));
        ScratchDir { dir }
    }

    pub fn path(&self) -> &Path {
        self.dir.path()
    }
}
