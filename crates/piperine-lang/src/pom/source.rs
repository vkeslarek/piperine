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
        self.files.insert(Self::normalize(path.as_ref()), text.into());
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
}

impl SourceProvider for MemorySources {
    fn read(&self, path: &Path) -> Option<Vec<u8>> {
        self.files.get(&Self::normalize(path)).map(|t| t.as_bytes().to_vec())
    }

    fn canonical(&self, path: &Path) -> PathBuf {
        MemorySources::normalize(path)
    }
}

/// The filesystem. `canonical` is `std::fs::canonicalize`, falling back to
/// the path itself when it cannot be resolved.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DiskSources;

impl SourceProvider for DiskSources {
    fn read(&self, path: &Path) -> Option<Vec<u8>> {
        std::fs::read(path).ok()
    }

    fn canonical(&self, path: &Path) -> PathBuf {
        std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
    }
}

/// Where the children of one declaring file live: the file, whether it is a root file, and the
/// inline modules around the `mod` item.
pub(crate) struct ModLayout<'layout> {
    declaring_file: &'layout Path,
    is_root: bool,
    inline_modules: &'layout [String],
}

impl<'layout> ModLayout<'layout> {
    /// The layout of the children of `declaring_file`.
    pub(crate) fn new(declaring_file: &'layout Path, is_root: bool, inline_modules: &'layout [String]) -> Self {
        ModLayout { declaring_file, is_root, inline_modules }
    }

    /// The two files `mod name;` may load, in the order they are reported:
    /// `dir/name.msl` and `dir/name/mod.msl`. `dir` holds the children of the
    /// declaring file: its own directory when it is a root file or is named
    /// `mod.msl`, otherwise `<its directory>/<its stem>`; each enclosing inline
    /// module adds one more level.
    pub(crate) fn candidates(&self, name: &str) -> [PathBuf; 2] {
        let (declaring_file, is_root, inline_modules) = (self.declaring_file, self.is_root, self.inline_modules);
        let mut dir = declaring_file.parent().map(Path::to_path_buf).unwrap_or_default();
        let owns_dir = is_root || declaring_file.file_name().is_some_and(|n| n == "mod.msl");
        if !owns_dir {
            if let Some(stem) = declaring_file.file_stem() {
                dir.push(stem);
            }
        }
        for m in inline_modules {
            dir.push(m);
        }
        [dir.join(format!("{name}.msl")), dir.join(name).join("mod.msl")]
    }
}
