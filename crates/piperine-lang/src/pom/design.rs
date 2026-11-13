//! The owning [`Design`] and the builder that makes it.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::decl::{DeclData, DeclKind, NamespaceData, NamespaceOrigin};
use super::ids::{DeclId, FileId};
use super::lower::lower_file;
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
    /// The file's `Namespace` declaration.
    pub root: DeclId,
}

/// The serialized shape of a [`SourceFileData`]: every field but the tree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceFileRepr {
    pub name: String,
    pub path: Option<PathBuf>,
    pub prelude: bool,
    pub text: String,
    pub root: DeclId,
}

impl From<SourceFileRepr> for SourceFileData {
    fn from(r: SourceFileRepr) -> Self {
        let ast = msl::parser::parse(&r.text).file;
        SourceFileData { name: r.name, path: r.path, prelude: r.prelude, text: r.text, ast, root: r.root }
    }
}

impl From<SourceFileData> for SourceFileRepr {
    fn from(d: SourceFileData) -> Self {
        SourceFileRepr { name: d.name, path: d.path, prelude: d.prelude, text: d.text, root: d.root }
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
    root_file: FileId,
    decls: Vec<DeclData>,
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

    pub(crate) fn push_decl(&mut self, data: DeclData) -> DeclId {
        let id = DeclId(self.decls.len() as u32);
        self.decls.push(data);
        id
    }

    pub(crate) fn decl_mut(&mut self, id: DeclId) -> &mut DeclData {
        &mut self.decls[id.index()]
    }

    /// The declaration with this id. Ids come from this design.
    pub fn decl_data(&self, id: DeclId) -> &DeclData {
        &self.decls[id.index()]
    }

    pub fn decl_count(&self) -> usize {
        self.decls.len()
    }

    /// The root file's namespace declaration.
    pub(crate) fn root_id(&self) -> DeclId {
        self.files[self.root_file.index()].root
    }

    /// One line per declaration, depth first, roots in file order: indent
    /// (two spaces per level), kind, name (`-` when none), `start..end`.
    pub fn dump(&self) -> String {
        let mut out = String::new();
        for (i, d) in self.decls.iter().enumerate() {
            if d.parent.is_none() {
                self.dump_decl(DeclId(i as u32), 0, &mut out);
            }
        }
        out
    }

    fn dump_decl(&self, id: DeclId, depth: usize, out: &mut String) {
        let d = self.decl_data(id);
        let name = d.name.as_ref().map_or("-", |n| n.text.as_str());
        out.push_str(&format!(
            "{}{} {} {}..{}\n",
            "  ".repeat(depth),
            d.kind.label(),
            name,
            d.span.start,
            d.span.end
        ));
        for c in &d.children {
            self.dump_decl(*c, depth + 1, out);
        }
    }

    /// The internal inconsistencies of the arenas, one message each; empty
    /// when the design is well formed.
    pub fn check(&self) -> Vec<String> {
        let mut problems = Vec::new();
        for (i, d) in self.decls.iter().enumerate() {
            let id = DeclId(i as u32);
            if d.file.index() >= self.files.len() {
                problems.push(format!("{id:?}: file {:?} does not exist", d.file));
            } else if d.span.start > d.span.end || d.span.end > self.files[d.file.index()].text.len() {
                problems.push(format!("{id:?}: span {}..{} is outside its file", d.span.start, d.span.end));
            }
            match d.parent {
                Some(p) if p.index() >= self.decls.len() => problems.push(format!("{id:?}: parent {p:?} does not exist")),
                Some(p) if !self.decls[p.index()].children.contains(&id) => {
                    problems.push(format!("{id:?}: not listed among the children of its parent {p:?}"))
                }
                _ => {}
            }
            for c in &d.children {
                match self.decls.get(c.index()) {
                    None => problems.push(format!("{id:?}: child {c:?} does not exist")),
                    Some(child) if child.parent != Some(id) => {
                        problems.push(format!("{id:?}: child {c:?} names another parent"))
                    }
                    Some(_) => {}
                }
            }
        }
        problems
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
        let mut design =
            Design { files: Vec::new(), root_file: FileId(0), decls: Vec::new(), diagnostics: Vec::new() };
        for (name, text) in &self.prelude {
            design.add_file(name, text, true);
        }
        let empty = (String::new(), String::new());
        let (name, text) = self.sources.first().unwrap_or(&empty);
        design.root_file = design.add_file(name, text, false);
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
        let root = self.push_decl(DeclData {
            kind: DeclKind::Namespace(NamespaceData { origin: NamespaceOrigin::Root }),
            name: None,
            parent: None,
            children: Vec::new(),
            file: id,
            span: parsed.file.span.0,
            public: true,
        });
        self.files.push(SourceFileData {
            name: name.to_string(),
            path: Some(PathBuf::from(name)),
            prelude,
            text: text.to_string(),
            ast: parsed.file.clone(),
            root,
        });
        lower_file(self, id, root, &parsed.file);
        id
    }
}
