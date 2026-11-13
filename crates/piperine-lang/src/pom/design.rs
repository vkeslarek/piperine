//! The owning [`Design`] and the builder that makes it.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::body::{ExprNode, PatNode, PathNode, StmtNode, TypeNode};
use super::decl::{DeclData, DeclKind, NamespaceData, NamespaceOrigin};
use super::ids::{DeclId, ExprId, FileId, PatId, PathId, StmtId, TypeId};
use super::lower::lower_file;
use super::source::{DiskSources, MemorySources, SourceProvider};
use crate::msl::Span;
use super::meta::Meta;
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
    pub(super) files: Vec<SourceFileData>,
    pub(super) root_file: FileId,
    pub(super) decls: Vec<DeclData>,
    pub(super) exprs: Vec<ExprNode>,
    pub(super) stmts: Vec<StmtNode>,
    pub(super) pats: Vec<PatNode>,
    pub(super) types: Vec<TypeNode>,
    pub(super) paths: Vec<PathNode>,
    diagnostics: Vec<DesignDiagnostic>,
}

impl Design {
    pub fn builder() -> DesignBuilder {
        DesignBuilder::default()
    }

    /// Read the tree rooted at `root` from disk: `mod b;` follows Rust's
    /// layout. An unreadable root gives an empty root and one `E0161`.
    pub fn load(root: impl Into<PathBuf>) -> Design {
        Design::builder().root_file(root).build()
    }

    /// Every diagnostic of the build, parse diagnostics first, in file order.
    pub fn diagnostics(&self) -> &[DesignDiagnostic] {
        &self.diagnostics
    }

    pub(crate) fn file_data(&self, id: FileId) -> &SourceFileData {
        &self.files[id.index()]
    }

    /// The `Namespace` declaration of a file.
    pub(crate) fn file_root(&self, id: FileId) -> DeclId {
        self.files[id.0 as usize].root
    }

    pub(crate) fn push_file(&mut self, data: SourceFileData) -> FileId {
        self.files.push(data);
        FileId(self.files.len() as u32 - 1)
    }

    pub(crate) fn push_diagnostic(&mut self, file: FileId, diagnostic: Diagnostic) {
        self.diagnostics.push(DesignDiagnostic { file, diagnostic });
    }

    pub(crate) fn push_decl(&mut self, data: DeclData) -> DeclId {
        let id = DeclId(self.decls.len() as u32);
        self.decls.push(data);
        id
    }

    pub(crate) fn push_expr(&mut self, node: ExprNode) -> ExprId {
        self.exprs.push(node);
        ExprId(self.exprs.len() as u32 - 1)
    }

    pub(crate) fn push_stmt(&mut self, node: StmtNode) -> StmtId {
        self.stmts.push(node);
        StmtId(self.stmts.len() as u32 - 1)
    }

    pub(crate) fn push_pat(&mut self, node: PatNode) -> PatId {
        self.pats.push(node);
        PatId(self.pats.len() as u32 - 1)
    }

    pub(crate) fn push_type(&mut self, node: TypeNode) -> TypeId {
        self.types.push(node);
        TypeId(self.types.len() as u32 - 1)
    }

    pub(crate) fn push_path(&mut self, node: PathNode) -> PathId {
        self.paths.push(node);
        PathId(self.paths.len() as u32 - 1)
    }

    /// The expression with this id. Ids come from this design.
    pub fn expr_node(&self, id: ExprId) -> &ExprNode {
        &self.exprs[id.index()]
    }

    pub fn stmt_node(&self, id: StmtId) -> &StmtNode {
        &self.stmts[id.index()]
    }

    pub fn pat_node(&self, id: PatId) -> &PatNode {
        &self.pats[id.index()]
    }

    pub fn type_node(&self, id: TypeId) -> &TypeNode {
        &self.types[id.index()]
    }

    pub fn path_node(&self, id: PathId) -> &PathNode {
        &self.paths[id.index()]
    }

    pub fn expr_count(&self) -> usize {
        self.exprs.len()
    }

    pub fn stmt_count(&self) -> usize {
        self.stmts.len()
    }

    pub fn pat_count(&self) -> usize {
        self.pats.len()
    }

    pub fn type_count(&self) -> usize {
        self.types.len()
    }

    pub fn path_count(&self) -> usize {
        self.paths.len()
    }

    /// Store what segment `index` of `path` names.
    pub(super) fn set_segment_res(&mut self, path: PathId, index: usize, res: super::resolution::Resolution) {
        if let Some(segment) = self.paths[path.index()].segments.get_mut(index) {
            segment.res = res;
        }
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

    pub(crate) fn file_count(&self) -> usize {
        self.files.len()
    }
}

/// Collects sources, then builds the [`Design`]. `build` never fails.
#[derive(Debug, Clone, Default)]
pub struct DesignBuilder {
    prelude: Vec<(String, String)>,
    sources: Vec<(String, String)>,
    root_file: Option<PathBuf>,
}

impl DesignBuilder {
    /// A prelude source: its `pub` items are visible in every file.
    pub fn prelude(mut self, name: impl Into<String>, text: impl Into<String>) -> Self {
        self.prelude.push((name.into(), text.into()));
        self
    }

    /// A virtual source file. The first one is the root; the others are found
    /// by the `mod` items of the files that name them.
    pub fn source(mut self, path: impl Into<String>, text: impl Into<String>) -> Self {
        self.sources.push((path.into(), text.into()));
        self
    }

    /// The root file, read from disk with the files its `mod` items name. A
    /// root file replaces the virtual sources.
    pub fn root_file(mut self, path: impl Into<PathBuf>) -> Self {
        self.root_file = Some(path.into());
        self
    }

    /// Parse every file and assemble the design. Files are numbered prelude
    /// sources first, then the root. A parse error never stops the build: the
    /// recovered tree is kept and each diagnostic is listed with its file.
    pub fn build(self) -> Design {
        let mut design =
            Design { files: Vec::new(), root_file: FileId(0), decls: Vec::new(), exprs: Vec::new(), stmts: Vec::new(), pats: Vec::new(), types: Vec::new(), paths: Vec::new(), diagnostics: Vec::new() };
        if let Some(path) = &self.root_file {
            let name = path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned());
            let bytes = DiskSources.read(path);
            design.root_file = design.add_file(&name, path.clone(), bytes.as_deref().unwrap_or_default(), false, &DiskSources);
            if bytes.is_none() {
                let message = format!("cannot read the root file `{}`", path.display());
                design.push_diagnostic(design.root_file, Diagnostic::new("E0161", message, Span::new(0, 0), "unreadable file"));
            }
            super::resolve::run(&mut design);
            return design;
        }
        let mut memory = MemorySources::new();
        for (path, text) in &self.sources {
            memory.insert(path, text.as_str());
        }
        for (name, text) in &self.prelude {
            design.add_file(name, PathBuf::from(name), text.as_bytes(), true, &memory);
        }
        let empty = (String::new(), String::new());
        let (name, text) = self.sources.first().unwrap_or(&empty);
        design.root_file = design.add_file(name, PathBuf::from(name), text.as_bytes(), false, &memory);
        super::resolve::run(&mut design);
        design
    }
}

impl Design {
    fn add_file(&mut self, name: &str, path: PathBuf, bytes: &[u8], prelude: bool, provider: &dyn SourceProvider) -> FileId {
        let id = FileId(self.files.len() as u32);
        let parsed = msl::parser::parse_bytes(bytes);
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
            generics_span: None,
            public: true,
            meta: Meta::default(),
        });
        self.files.push(SourceFileData {
            name: name.to_string(),
            path: Some(path.clone()),
            prelude,
            text: String::from_utf8_lossy(bytes).into_owned(),
            ast: parsed.file.clone(),
            root,
        });
        lower_file(self, id, root, &parsed.file, provider, path);
        id
    }
}
