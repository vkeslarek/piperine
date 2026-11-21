//! `IncludedFiles`: the files `include_str!` reads, each read once at elaboration (§20 rules 2 to
//! 4). A path is resolved against the directory of the source file that holds the
//! `include_str!` (an absolute path as it is), the file is read the first time its resolved path
//! is named and its text, or the reason it has none, is remembered: a later reading of the same
//! path never touches the disk, so a file that changes afterwards changes nothing until the host
//! elaborates again (§20 rule 3).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::elaborator::Elaboration;
use crate::pom::{ConstData, DeclId, DeclKind, ExprId, ExprKind, Resolution};

/// Why a file has no text: `E0612` or `E0613` at elaboration (§20).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileFault {
    /// The file cannot be read (`E0612`); `path` is the resolved path.
    Unreadable { path: PathBuf, reason: String },
    /// A relative path in a source that has no directory to resolve it against (`E0612`, D14).
    NoDirectory { path: PathBuf },
    /// The file is not valid UTF-8 (`E0613`); `offset` is the byte offset of the first invalid
    /// sequence.
    NotUtf8 { path: PathBuf, offset: usize },
}

impl FileFault {
    /// The code of the diagnostic this fault is (§20 errors table).
    pub fn code(&self) -> &'static str {
        match self {
            FileFault::Unreadable { .. } | FileFault::NoDirectory { .. } => "E0612",
            FileFault::NotUtf8 { .. } => "E0613",
        }
    }

    /// The message of the diagnostic: it names the resolved path (and the offset for `E0613`).
    pub fn message(&self) -> String {
        match self {
            FileFault::Unreadable { path, reason } => format!("the file `{}` cannot be read: {reason}", path.display()),
            FileFault::NoDirectory { path } => format!("the file `{}` cannot be found: the source that names it has no directory to resolve it against", path.display()),
            FileFault::NotUtf8 { path, offset } => format!("the file `{}` is not valid UTF-8: the first invalid sequence is at byte {offset}", path.display()),
        }
    }
}

/// The text of every file read so far, by resolved path.
#[derive(Debug, Default)]
pub struct IncludedFiles {
    texts: BTreeMap<PathBuf, Result<Arc<str>, FileFault>>,
    reads: usize,
}

impl IncludedFiles {
    /// No file read yet.
    pub fn new() -> Self {
        IncludedFiles::default()
    }

    /// How many files were opened: one per distinct resolved path, however often it was named.
    pub fn reads(&self) -> usize {
        self.reads
    }

    /// The text of the file `literal` names from the source at `source` (`None` for a source with
    /// no path): read the first time, remembered after.
    pub fn text(&mut self, source: Option<&Path>, literal: &str) -> Result<Arc<str>, FileFault> {
        let resolved = Self::resolve(source, literal)?;
        if let Some(known) = self.texts.get(&resolved) {
            return known.clone();
        }
        self.reads += 1;
        let read = Self::read(&resolved);
        self.texts.insert(resolved, read.clone());
        read
    }

    /// `literal` against the directory of `source`; an absolute path is taken as it is (§20
    /// Resolution). A source whose path has no directory, an in-memory one, resolves nothing
    /// (D14): the process's working directory is never consulted.
    fn resolve(source: Option<&Path>, literal: &str) -> Result<PathBuf, FileFault> {
        let path = Path::new(literal);
        if path.is_absolute() {
            return Ok(path.to_path_buf());
        }
        match source.and_then(Path::parent).filter(|directory| !directory.as_os_str().is_empty()) {
            Some(directory) => Ok(directory.join(path)),
            None => Err(FileFault::NoDirectory { path: path.to_path_buf() }),
        }
    }

    /// The text of the file at `path`, or why it has none.
    fn read(path: &Path) -> Result<Arc<str>, FileFault> {
        let bytes = std::fs::read(path).map_err(|error| FileFault::Unreadable { path: path.to_path_buf(), reason: error.to_string() })?;
        match String::from_utf8(bytes) {
            Ok(text) => Ok(Arc::from(text)),
            Err(error) => Err(FileFault::NotUtf8 { path: path.to_path_buf(), offset: error.utf8_error().valid_up_to() }),
        }
    }
}

impl Elaboration<'_, '_> {
    /// Read the file of every reached `const` whose initializer is `include_str!` (§20 rule 3: the
    /// text is part of the elaborated design, read now and never again). A file with no text is
    /// `E0612` or `E0613` at the `include_str!` (`const_fault`).
    pub(super) fn read_included_files(&mut self) {
        let consts: Vec<DeclId> = self.types.reached().iter().copied().filter(|decl| self.is_text_const(*decl)).collect();
        for decl in consts {
            if let Err(fault) = self.consts.text(decl) {
                self.const_fault(&fault, "an `include_str!`");
            }
        }
    }

    /// Whether `expr` reads a `const` whose `include_str!` file has no text: its fault is already
    /// reported (`read_included_files`), so the reader adds none.
    pub(super) fn names_unreadable_file(&mut self, expr: ExprId) -> bool {
        let ExprKind::Path(path) = &self.design.expr_node(expr).kind else { return false };
        match self.design.path_node(*path).segments.last().map(|segment| segment.res) {
            Some(Resolution::Decl(decl)) if self.is_text_const(decl) => self.consts.text(decl).is_err(),
            _ => false,
        }
    }

    /// Whether `decl` is a `const` item whose initializer is an `include_str!`.
    fn is_text_const(&self, decl: DeclId) -> bool {
        match &self.design.decl_data(decl).kind {
            DeclKind::Const(ConstData { value: Some(value), .. }) => matches!(self.design.expr_node(*value).kind, ExprKind::IncludeStr(_)),
            _ => false,
        }
    }
}
