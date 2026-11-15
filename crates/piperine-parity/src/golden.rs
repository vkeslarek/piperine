//! What one ngspice run produced: its captured output and the exports it wrote.

use std::path::PathBuf;

use crate::scratch::ScratchDir;

/// The result of running a `.cir`: the captured streams, the first line of `ngspice --version`
/// and the scratch directory holding the exports, removed when this value is dropped.
#[derive(Debug)]
pub struct Golden {
    fixture: String,
    stdout: String,
    stderr: String,
    version: String,
    scratch: ScratchDir,
}

impl Golden {
    pub(crate) fn new(fixture: &str, stdout: String, stderr: String, version: String, scratch: ScratchDir) -> Self {
        Golden { fixture: fixture.to_string(), stdout, stderr, version, scratch }
    }

    pub fn fixture(&self) -> &str {
        &self.fixture
    }

    pub fn stdout(&self) -> &str {
        &self.stdout
    }

    /// The first line of `ngspice --version`, for the message of a failed comparison.
    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn scratch_path(&self) -> &std::path::Path {
        self.scratch.path()
    }

    /// The file `wrdata <name>` wrote. Fails naming the fixture and ngspice's captured output when
    /// the run wrote no such file.
    pub fn export_file(&self, name: &str) -> PathBuf {
        let exact = self.scratch.path().join(name);
        if exact.is_file() {
            return exact;
        }
        panic!("{}: ngspice wrote no export {name:?}\n{}", self.fixture, self.captured());
    }

    pub(crate) fn captured(&self) -> String {
        format!("--- ngspice stdout ---\n{}\n--- ngspice stderr ---\n{}", self.stdout, self.stderr)
    }
}
