//! The owning [`Design`] and the builder that makes it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use super::body::{ExprNode, PatNode, PathNode, StmtNode, TypeNode};
use super::decl::{DeclData, DeclKind, NamespaceData, NamespaceOrigin};
use super::crates::{CrateData, CrateGraph, DependencyData, is_crate_name};
use super::ids::{CrateId, DeclId, ExprId, FileId, PatId, PathId, StmtId, TypeId};
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
    /// The crate the file belongs to.
    pub krate: CrateId,
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
    pub krate: CrateId,
    pub text: String,
    pub root: DeclId,
}

impl From<SourceFileRepr> for SourceFileData {
    fn from(r: SourceFileRepr) -> Self {
        let ast = msl::parser::parse(&r.text).file;
        SourceFileData { name: r.name, path: r.path, krate: r.krate, text: r.text, ast, root: r.root }
    }
}

impl From<SourceFileData> for SourceFileRepr {
    fn from(d: SourceFileData) -> Self {
        SourceFileRepr { name: d.name, path: d.path, krate: d.krate, text: d.text, root: d.root }
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
    /// The crates in graph order, `std` last.
    pub(super) crates: Vec<CrateData>,
    pub(super) root_crate: CrateId,
    pub(super) std_crate: CrateId,
    pub(super) decls: Vec<DeclData>,
    pub(super) exprs: Vec<ExprNode>,
    pub(super) stmts: Vec<StmtNode>,
    pub(super) pats: Vec<PatNode>,
    pub(super) types: Vec<TypeNode>,
    pub(super) paths: Vec<PathNode>,
    diagnostics: Vec<DesignDiagnostic>,
    pub(super) index: super::index::Index,
    /// The expression that contains each expression (see `Expr::parent`), by `ExprId`; filled once
    /// after lowering.
    pub(super) expr_parent: Vec<Option<ExprId>>,
    /// The statement that holds each expression directly, by `ExprId`; filled with `expr_parent`.
    pub(super) expr_stmt: Vec<Option<StmtId>>,
    /// The by-name lookup tables, built on first use and never serialized.
    #[serde(skip)]
    names: NameTables,
}

/// The sorted by-name tables behind `Design::module` and `Module::signal`:
/// built from the finished arenas on first use, so a design read from JSON
/// has them too, and never part of the serialized form or of equality.
#[derive(Default)]
struct NameTables(OnceLock<Tables>);

/// Declaration ids in arena (source) order under each key.
pub(super) struct Tables {
    /// Every named declaration, by its name.
    pub by_name: BTreeMap<String, Vec<DeclId>>,
    /// Ports, params and signals, by the module they belong to (through
    /// generation) and their name.
    pub members: BTreeMap<(DeclId, String), Vec<DeclId>>,
}

impl Clone for NameTables {
    fn clone(&self) -> Self {
        NameTables::default()
    }
}

impl std::fmt::Debug for NameTables {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("NameTables")
    }
}

impl PartialEq for NameTables {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl Design {
    /// The by-name tables, built on first use.
    pub(super) fn tables(&self) -> &Tables {
        self.names.0.get_or_init(|| {
            let mut by_name: BTreeMap<String, Vec<DeclId>> = BTreeMap::new();
            let mut members: BTreeMap<(DeclId, String), Vec<DeclId>> = BTreeMap::new();
            for (i, d) in self.decls.iter().enumerate() {
                let id = DeclId(i as u32);
                let Some(name) = &d.name else { continue };
                by_name.entry(name.text.clone()).or_default().push(id);
                if matches!(d.kind, DeclKind::Port(_) | DeclKind::Param(_) | DeclKind::Signal(_)) {
                    let mut owner = d.parent;
                    while let Some(p) = owner.filter(|p| matches!(self.decls[p.index()].kind, DeclKind::Generation(_))) {
                        owner = self.decls[p.index()].parent;
                    }
                    if let Some(m) = owner.filter(|m| matches!(self.decls[m.index()].kind, DeclKind::Module(_))) {
                        members.entry((m, name.text.clone())).or_default().push(id);
                    }
                }
            }
            Tables { by_name, members }
        })
    }

    pub fn builder() -> DesignBuilder {
        DesignBuilder::default()
    }

    /// Read the tree rooted at `root` from disk: `mod b;` follows Rust's
    /// layout. An unreadable root gives an empty root and one `E0161`.
    pub fn load(root: impl Into<PathBuf>) -> Design {
        Design::builder().root_file(root).build()
    }

    /// Read every crate of `graph` from disk, `std` last: each crate root and the files its `mod`
    /// items name. An unreadable root gives an empty crate and one `E0161`.
    pub fn load_crates(graph: CrateGraph) -> Design {
        Design::builder().crates(graph).build()
    }

    /// Read a design back from its JSON and validate it. The derived
    /// `Deserialize` accepts any well-formed JSON of the right shape and
    /// checks nothing; this is the entry point for text that did not come
    /// from [`Design::build`] in this process. `Err` lists the problems:
    /// the text is not a design, or [`Design::check`] rejects what it holds.
    /// It never panics.
    pub fn from_json(text: &str) -> Result<Design, Vec<String>> {
        let design: Design = serde_json::from_str(text).map_err(|e| vec![format!("not a design: {e}")])?;
        let problems = design.check();
        if problems.is_empty() { Ok(design) } else { Err(problems) }
    }

    /// Every diagnostic of the build (parse diagnostics included), sorted by file, offset and code.
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

    /// Order the diagnostics by file, then start offset, then code (stable
    /// for the rest).
    pub(super) fn sort_diagnostics(&mut self) {
        self.diagnostics.sort_by(|a, b| (a.file, a.diagnostic.span().start, &a.diagnostic.code).cmp(&(b.file, b.diagnostic.span().start, &b.diagnostic.code)));
    }

    /// Fill the parent and statement tables from the nodes: the one child
    /// enumeration, inverted.
    pub(super) fn fill_parents(&mut self) {
        let n = self.exprs.len();
        let mut parent = vec![None; n];
        let mut stmt = vec![None; n];
        for (i, node) in self.exprs.iter().enumerate() {
            for c in super::body::expr_children(&node.kind, self) {
                parent[c.index()] = Some(ExprId(i as u32));
            }
        }
        for i in 0..self.stmts.len() {
            let id = StmtId(i as u32);
            for e in super::body::stmt_own_exprs(self, id) {
                stmt[e.index()] = Some(id);
            }
        }
        self.expr_parent = parent;
        self.expr_stmt = stmt;
    }

    /// Bind a behavior block to its module, both ways.
    pub(super) fn bind_behavior(&mut self, behavior: DeclId, module: DeclId) {
        if let DeclKind::Behavior(b) = &mut self.decls[behavior.index()].kind {
            b.module = Some(module);
        }
        if let DeclKind::Module(m) = &mut self.decls[module.index()].kind {
            m.behaviors.push(behavior);
        }
    }

    /// Store the kind of a port.
    pub(super) fn set_port_kind(&mut self, port: DeclId, kind: super::resolution::PortKind) {
        if let DeclKind::Port(p) = &mut self.decls[port.index()].kind {
            p.kind = kind;
        }
    }

    /// Store the bundle flag of a struct.
    pub(super) fn set_bundle(&mut self, decl: DeclId, bundle: bool) {
        if let DeclKind::Struct(s) = &mut self.decls[decl.index()].kind {
            s.bundle = Some(bundle);
        }
    }

    /// Store the class of a call expression and what each of its named
    /// arguments and param bindings names.
    pub(super) fn set_call(&mut self, expr: ExprId, class_: super::resolution::CallClass, args: &[(usize, super::resolution::Resolution)], params: &[(usize, super::resolution::Resolution)]) {
        if let super::body::ExprKind::Call { class, args: written, params: block, .. } = &mut self.exprs[expr.index()].kind {
            *class = class_;
            for (i, res) in args {
                if let Some(a) = written.get_mut(*i) {
                    a.res = *res;
                }
            }
            for (i, res) in params {
                if let Some(f) = block.as_mut().and_then(|b| b.get_mut(*i)) {
                    f.res = *res;
                }
            }
        }
    }

    /// Store the class of a `Name { .. }` expression and what each field names.
    pub(super) fn set_struct(&mut self, expr: ExprId, class_: super::resolution::StructClass, names: &[(usize, super::resolution::Resolution)]) {
        if let super::body::ExprKind::Struct { class, fields, .. } = &mut self.exprs[expr.index()].kind {
            *class = class_;
            for (i, res) in names {
                if let Some(f) = fields.get_mut(*i) {
                    f.res = *res;
                }
            }
        }
    }

    /// Store what the member of a field access or a method call names.
    pub(super) fn set_member(&mut self, expr: ExprId, res_: super::resolution::Resolution) {
        match &mut self.exprs[expr.index()].kind {
            super::body::ExprKind::Field { res, .. } | super::body::ExprKind::MethodCall { res, .. } => *res = res_,
            _ => {}
        }
    }

    /// Record an instance in the index of its module.
    pub(super) fn push_instance(&mut self, module: DeclId, expr: ExprId) {
        if let DeclKind::Module(m) = &mut self.decls[module.index()].kind {
            m.instances.push(expr);
        }
    }

    /// Store what segment `index` of `path` names.
    pub(super) fn set_segment_res(&mut self, path: PathId, index: usize, res: super::resolution::Resolution) {
        if let Some(segment) = self.paths[path.index()].segments.get_mut(index) {
            segment.res = res;
        }
    }

    /// Store what an identifier pattern names (it is a path, not a binder).
    /// Store the field a struct pattern's `index`th field names.
    pub(super) fn set_pattern_field(&mut self, pat: PatId, index: usize, res_: super::resolution::Resolution) {
        if let super::body::PatKind::Struct { fields, .. } = &mut self.pats[pat.index()].kind {
            if let Some(f) = fields.get_mut(index) {
                f.res = res_;
            }
        }
    }

    pub(super) fn set_pat_res(&mut self, pat: PatId, res: super::resolution::Resolution) {
        if let super::body::PatKind::Ident { res: slot, .. } = &mut self.pats[pat.index()].kind {
            *slot = res;
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

    /// The root crate's root namespace declaration.
    pub(crate) fn root_id(&self) -> DeclId {
        self.crates[self.root_crate.index()].root
    }

    pub(crate) fn crate_data(&self, id: CrateId) -> &CrateData {
        &self.crates[id.index()]
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
    graph: CrateGraph,
    std_sources: Vec<(String, String)>,
    sources: Vec<(String, String)>,
    root_file: Option<PathBuf>,
}

impl DesignBuilder {
    /// A virtual file of the standard library, replacing the embedded one: the first is its root,
    /// the others are found by the `mod` items of the files that name them. The `pub` items of the
    /// namespace `prelude` of the root are visible in every file.
    pub fn std_source(mut self, path: impl Into<String>, text: impl Into<String>) -> Self {
        self.std_sources.push((path.into(), text.into()));
        self
    }

    /// A virtual source file. The first one is the root; the others are found
    /// by the `mod` items of the files that name them.
    pub fn source(mut self, path: impl Into<String>, text: impl Into<String>) -> Self {
        self.sources.push((path.into(), text.into()));
        self
    }

    /// The crates to build, the first being the root crate. Each crate root is read from the
    /// virtual sources when any `source` was given, else from disk; a graph replaces `root_file`.
    /// An empty graph is no graph.
    pub fn crates(mut self, graph: CrateGraph) -> Self {
        self.graph = graph;
        self
    }

    /// The root file, read from disk with the files its `mod` items name. A
    /// root file replaces the virtual sources.
    pub fn root_file(mut self, path: impl Into<PathBuf>) -> Self {
        self.root_file = Some(path.into());
        self
    }

    /// Parse every file and assemble the design. Files are numbered the project's first (the root,
    /// then the files it loads), then the standard library's. A parse error never stops the build:
    /// the recovered tree is kept and each diagnostic is listed with its file.
    pub fn build(self) -> Design {
        let mut design = Design {
            files: Vec::new(),
            crates: Vec::new(),
            root_crate: CrateId(0),
            std_crate: CrateId(0),
            decls: Vec::new(),
            exprs: Vec::new(),
            stmts: Vec::new(),
            pats: Vec::new(),
            types: Vec::new(),
            paths: Vec::new(),
            diagnostics: Vec::new(),
            index: Default::default(),
            expr_parent: Vec::new(),
            expr_stmt: Vec::new(),
            names: NameTables::default(),
        };
        if !self.graph.crates().is_empty() {
            let mut memory = MemorySources::new();
            for (path, text) in &self.sources {
                memory.insert(path, text.as_str());
            }
            let from_memory = !self.sources.is_empty();
            let provider: &dyn SourceProvider = if from_memory { &memory } else { &DiskSources };
            for node in self.graph.crates() {
                let file_name = if from_memory {
                    node.root.display().to_string()
                } else {
                    node.root.file_name().map_or_else(|| node.root.display().to_string(), |n| n.to_string_lossy().into_owned())
                };
                design.add_crate(&node.name, &file_name, node.root.clone(), provider.read(&node.root), provider, node.dependencies.clone());
            }
        } else if let Some(path) = &self.root_file {
            let bytes = DiskSources.read(path);
            let file_name = path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned());
            let (name, reserved) = crate_name(path);
            let id = design.add_crate(&name, &file_name, path.clone(), bytes, &DiskSources, Vec::new());
            design.report_reserved_stem(id, reserved);
        } else {
            let mut memory = MemorySources::new();
            for (path, text) in &self.sources {
                memory.insert(path, text.as_str());
            }
            let empty = (String::new(), String::new());
            let (name, text) = self.sources.first().unwrap_or(&empty);
            let (crate_name, reserved) = crate_name(Path::new(name));
            let id = design.add_crate(&crate_name, name, PathBuf::from(name), Some(text.clone().into_bytes()), &memory, Vec::new());
            design.report_reserved_stem(id, reserved);
        }
        design.add_std(&self.std_sources);
        super::resolve::run(&mut design);
        design
    }
}

/// The name of the crate whose root file is `path`: the file stem, with every character that is
/// no ASCII letter, digit or `_` replaced by `_` and a `_` before a leading digit. A stem that is
/// then a keyword or `std` gets a `_` appended, and is returned as the second value so that the
/// design can report it (`E0171`).
fn crate_name(path: &Path) -> (String, Option<String>) {
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let mut name: String = stem.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' }).collect();
    if name.is_empty() || name.starts_with(|c: char| c.is_ascii_digit()) {
        name.insert(0, '_');
    }
    if name == "_" || is_crate_name(&name) {
        return (name, None);
    }
    let reserved = name.clone();
    name.push('_');
    (name, Some(reserved))
}

impl Design {
    /// `E0171` at the root file of `id` when its stem was reserved (see `crate_name`).
    fn report_reserved_stem(&mut self, id: CrateId, reserved: Option<String>) {
        let Some(stem) = reserved else { return };
        let file = self.crates[id.index()].root_file;
        let name = self.crates[id.index()].name.clone();
        let message = format!("the root file stem `{stem}` is no crate name: it is a keyword or `std`; the crate is named `{name}`");
        self.push_diagnostic(file, Diagnostic::new("E0171", message, Span::new(0, 0), "rename the file"));
    }

    /// Lower the standard library after every project file: `given` when not empty, else the
    /// embedded tree. Its own provider keeps its `mod` lookups apart from the project's.
    fn add_std(&mut self, given: &[(String, String)]) {
        let files: Vec<(&str, &str)> =
            if given.is_empty() { super::stdlib::EMBEDDED.to_vec() } else { given.iter().map(|(p, t)| (p.as_str(), t.as_str())).collect() };
        let mut memory = MemorySources::new();
        for (path, text) in &files {
            memory.insert(path, *text);
        }
        let (path, text) = files[0];
        self.std_crate = self.add_crate("std", path, PathBuf::from(path), Some(text.as_bytes().to_vec()), &memory, Vec::new());
    }

    /// Lower the crate whose root file is `path`, registered as `file_name` (`bytes` is none when it cannot be read: the
    /// crate is then empty, with one `E0161`) and store it. Returns its id.
    fn add_crate(&mut self, name: &str, file_name: &str, path: PathBuf, bytes: Option<Vec<u8>>, provider: &dyn SourceProvider, dependencies: Vec<DependencyData>) -> CrateId {
        let id = CrateId(self.crates.len() as u32);
        let file = self.add_file(file_name, path.clone(), bytes.as_deref().unwrap_or_default(), id, provider);
        if bytes.is_none() {
            let message = format!("cannot read the root file `{}`", path.display());
            self.push_diagnostic(file, Diagnostic::new("E0161", message, Span::new(0, 0), "unreadable file"));
        }
        let root = self.file_root(file);
        self.crates.push(CrateData { name: name.to_string(), root_file: file, root, dependencies });
        id
    }

    fn add_file(&mut self, name: &str, path: PathBuf, bytes: &[u8], krate: CrateId, provider: &dyn SourceProvider) -> FileId {
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
            krate,
            text: String::from_utf8_lossy(bytes).into_owned(),
            ast: parsed.file.clone(),
            root,
        });
        lower_file(self, id, root, &parsed.file, provider, path);
        id
    }
}
