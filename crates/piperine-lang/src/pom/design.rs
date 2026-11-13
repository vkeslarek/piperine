//! The owning [`Design`] and the builder that makes it.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::ids::FileId;
use crate::msl::{self, Diagnostic, ast};

/// A source file as the design stores it: its text and the retained syntax
/// tree. The tree is not part of the serialized form; it is rebuilt from the
/// text (see [`SourceFileRepr`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(from = "SourceFileRepr", into = "SourceFileRepr")]
pub struct SourceFileData {
    /// As registered, or the file name on disk.
    pub name: String,
    /// The path it was read from (virtual or on disk).
    pub path: Option<PathBuf>,
    pub prelude: bool,
    pub text: String,
    pub ast: ast::File,
}

/// The serialized shape of a [`SourceFileData`]: every field but the tree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceFileRepr {
    pub name: String,
    pub path: Option<PathBuf>,
    pub prelude: bool,
    pub text: String,
}

impl From<SourceFileRepr> for SourceFileData {
    fn from(r: SourceFileRepr) -> Self {
        let ast = msl::parser::parse(&r.text).file;
        SourceFileData { name: r.name, path: r.path, prelude: r.prelude, text: r.text, ast }
    }
}

impl From<SourceFileData> for SourceFileRepr {
    fn from(d: SourceFileData) -> Self {
        SourceFileRepr { name: d.name, path: d.path, prelude: d.prelude, text: d.text }
    }
}

/// A diagnostic and the file it was found in.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DesignDiagnostic {
    pub file: FileId,
    pub diagnostic: Diagnostic,
}

/// The immutable object model of one set of sources. Build it with
/// [`Design::builder`]; nothing mutates it afterwards.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Design {
    files: Vec<SourceFileData>,
    diagnostics: Vec<DesignDiagnostic>,
}

impl Design {
    pub fn builder() -> DesignBuilder {
        DesignBuilder::default()
    }

    /// Every diagnostic of the build, parse diagnostics first, in file order.
    pub fn diagnostics(&self) -> &[DesignDiagnostic] {
        &self.diagnostics
    }

    pub(crate) fn file_data(&self, id: FileId) -> &SourceFileData {
        &self.files[id.index()]
    }

    pub(crate) fn file_count(&self) -> usize {
        self.files.len()
    }
}

/// Collects sources, then builds the [`Design`]. `build` never fails.
#[derive(Debug, Clone, Default)]
pub struct DesignBuilder {
    prelude: Vec<(String, String)>,
    sources: Vec<(String, String)>,
}

impl DesignBuilder {
    /// A prelude source: its `pub` items are visible in every file.
    pub fn prelude(mut self, name: impl Into<String>, text: impl Into<String>) -> Self {
        self.prelude.push((name.into(), text.into()));
        self
    }

    /// A virtual source file. The first one is the root.
    pub fn source(mut self, path: impl Into<String>, text: impl Into<String>) -> Self {
        self.sources.push((path.into(), text.into()));
        self
    }

    /// Parse every file and assemble the design. Files are numbered prelude
    /// sources first, then the root. A parse error never stops the build: the
    /// recovered tree is kept and each diagnostic is listed with its file.
    pub fn build(self) -> Design {
        let mut design = Design { files: Vec::new(), diagnostics: Vec::new() };
        for (name, text) in &self.prelude {
            design.add_file(name, text, true);
        }
        if let Some((name, text)) = self.sources.first() {
            design.add_file(name, text, false);
        }
        design
    }
}

impl Design {
    fn add_file(&mut self, name: &str, text: &str, prelude: bool) -> FileId {
        let id = FileId(self.files.len() as u32);
        let parsed = msl::parser::parse(text);
        for diagnostic in parsed.diagnostics {
            self.diagnostics.push(DesignDiagnostic { file: id, diagnostic });
        }
        self.files.push(SourceFileData {
            name: name.to_string(),
            path: Some(PathBuf::from(name)),
            prelude,
            text: text.to_string(),
            ast: parsed.file,
        });
        id
    }
}
