//! Stage 0: finding and reading the files of one source tree. The layout is
//! Rust's, with `.msl` for `.rs` and `mod.msl` for `mod.rs`.

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

/// Where the files of a tree come from.
pub trait SourceProvider {
    /// The bytes of the file at `path`; none when it cannot be read.
    fn read(&self, path: &Path) -> Option<Vec<u8>>;

    /// The path in a form that is equal for two spellings of the same file;
    /// the cycle check compares these.
    fn canonical(&self, path: &Path) -> PathBuf;
}

/// Files held in memory, keyed by their normalized path.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MemorySources {
    files: BTreeMap<PathBuf, String>,
}

impl MemorySources {
    pub fn new() -> Self {
        MemorySources::default()
    }

    /// Add or replace the file at `path`.
    pub fn insert(&mut self, path: impl AsRef<Path>, text: impl Into<String>) {
        self.files.insert(normalize(path.as_ref()), text.into());
    }
}

impl SourceProvider for MemorySources {
    fn read(&self, path: &Path) -> Option<Vec<u8>> {
        self.files.get(&normalize(path)).map(|t| t.as_bytes().to_vec())
    }

    fn canonical(&self, path: &Path) -> PathBuf {
        normalize(path)
    }
}

/// `path` without `.` components and with `..` folded into its parent.
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push("..");
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// The two files `mod name;` may load, in the order they are reported:
/// `dir/name.msl` and `dir/name/mod.msl`. `dir` holds the children of the
/// declaring file: its own directory when it is a root file or is named
/// `mod.msl`, otherwise `<its directory>/<its stem>`; each enclosing inline
/// module adds one more level.
// SPEC_DEVIATION: the design lists three parameters; `is_root` is the fourth.
// Reason: a root file (`top.msl`) keeps its children beside it like `mod.msl`
// does, and the path alone cannot say which file is the root.
pub(crate) fn mod_candidates(declaring_file: &Path, is_root: bool, inline_mods: &[String], name: &str) -> [PathBuf; 2] {
    let mut dir = declaring_file.parent().map(Path::to_path_buf).unwrap_or_default();
    let owns_dir = is_root || declaring_file.file_name().is_some_and(|n| n == "mod.msl");
    if !owns_dir {
        if let Some(stem) = declaring_file.file_stem() {
            dir.push(stem);
        }
    }
    for m in inline_mods {
        dir.push(m);
    }
    [dir.join(format!("{name}.msl")), dir.join(name).join("mod.msl")]
}
