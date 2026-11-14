//! The reflection face: `Copy` handles that borrow a [`Design`], and the
//! [`Reflect`] trait every declaration handle shares.

mod body;
mod hardware;
mod index;
mod items;

use super::body::Lit;
use super::decl::{DeclData, DeclKind};
use super::design::Design;
use super::ids::{DeclId, ExprId};
use super::index::ReferenceId;
use super::meta::{AttrArgNode, AttrInputNode, AttrNode, Doc};
use crate::msl::Span;

pub use body::{Argument, BlockHandle, FieldBinding, Commit, Contribution, Expr, Path, Pat, SourceFile, Stmt, Type};
pub use hardware::{Behavior, Binding, Definition, GenArm, Generation, Instance, Let, Module, Nature, Param, Port, Quantity, Reaction, Signal};
pub use index::{Reference, Writer};
pub use items::{Const, Enum, Field, FnParam, Function, GenericParam, Impl, ImportRef, Namespace, Struct, Trait, Use, Variant};

/// The generic declaration handle: any kind, read through [`Decl::kind`].
#[derive(Clone, Copy)]
pub struct Decl<'d> {
    design: &'d Design,
    id: DeclId,
}

impl<'d> Decl<'d> {
    pub(crate) fn new(design: &'d Design, id: DeclId) -> Self {
        Decl { design, id }
    }

    pub fn id(self) -> DeclId {
        self.id
    }

    pub fn design(self) -> &'d Design {
        self.design
    }

    /// The raw storage of this declaration.
    pub fn data(self) -> &'d DeclData {
        self.design.decl_data(self.id)
    }

    pub fn kind(self) -> &'d DeclKind {
        &self.data().kind
    }

    /// The declarations lexically inside this one, at the first level, in
    /// source order.
    pub fn children(self) -> Vec<Decl<'d>> {
        self.data().children.iter().map(|c| Decl::new(self.design, *c)).collect()
    }
}

impl PartialEq for Decl<'_> {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.design, other.design) && self.id == other.id
    }
}

impl std::fmt::Debug for Decl<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Decl({:?}, {})", self.id, self.kind().label())
    }
}

/// What every declaration handle answers. Implementors give [`Reflect::decl`];
/// the rest are plain provided methods.
pub trait Reflect<'d>: Copy {
    fn decl(self) -> Decl<'d>;

    /// The declared name; none for an `impl`, a `use`, a behavior block, an
    /// error item or a file's root namespace.
    fn name(self) -> Option<&'d str> {
        self.decl().data().name.as_ref().map(|n| n.text.as_str())
    }

    /// The range of the whole declaration (an item's span includes its
    /// attributes and docs).
    fn span(self) -> Span {
        self.decl().data().span
    }

    fn file(self) -> SourceFile<'d> {
        let d = self.decl();
        SourceFile::new(d.design, d.data().file)
    }

    /// The authored text of the declaration.
    fn text(self) -> &'d str {
        self.file().text().get(self.span().range()).unwrap_or("")
    }

    /// The doc comments in order, one per comment. On a file's root
    /// namespace these are the file's `//!` comments.
    fn docs(self) -> &'d [Doc] {
        &self.decl().data().meta.docs
    }

    /// The attributes in order.
    fn attrs(self) -> Vec<Attr<'d>> {
        let d = self.decl();
        d.data().meta.attrs.iter().map(|node| Attr { owner: d, node }).collect()
    }

    /// The first attribute whose path is written `name`.
    fn attr(self, name: &str) -> Option<Attr<'d>> {
        self.attrs().into_iter().find(|a| a.name() == name)
    }

    fn is_pub(self) -> bool {
        self.decl().data().public
    }

    /// The module this declaration belongs to: itself for a module, else the
    /// nearest enclosing module (through generation). None outside any
    /// module. A `signal` written outside a module body (the parser reported
    /// it) belongs to no module.
    fn module(self) -> Option<Module<'d>> {
        let decl = self.decl();
        if decl.as_signal().is_some() && !decl.declared_in().is_some_and(|p| matches!(p.kind(), DeclKind::Module(_) | DeclKind::Generation(_))) {
            return None;
        }
        let mut at = Some(self.decl());
        while let Some(d) = at {
            if let Some(m) = d.as_module() {
                return Some(m);
            }
            // A behavior block is a file item: its module is the one it names.
            if let Some(b) = d.as_behavior() {
                return b.module();
            }
            at = d.declared_in();
        }
        None
    }

    /// The declaration that lexically contains this one; none for a file's
    /// root namespace.
    fn declared_in(self) -> Option<Decl<'d>> {
        let d = self.decl();
        d.data().parent.map(|p| Decl::new(d.design, p))
    }

    /// Every path with a segment, and every member expression, that resolved
    /// to this declaration, in source order.
    fn references(self) -> Vec<Reference<'d>> {
        let d = self.decl();
        d.design()
            .index
            .references(d.id())
            .iter()
            .map(|r| match *r {
                ReferenceId::Path(p) => Reference::Path(d.design().path(p)),
                ReferenceId::Member(e) => Reference::Member(d.design().expr(e)),
                ReferenceId::Pattern(p) => Reference::Pattern(d.design().pat(p)),
            })
            .collect()
    }

    /// Every expression of this declaration's own body in pre-order source
    /// order, each followed by its descendants; none that belongs to a nested
    /// item (a `fn`, a `const`, a reaction written inside a block). A function
    /// lists its body, a module its lets, signal initializers, definitions,
    /// generation conditions and result, a behavior block its statements, a
    /// reaction its event and body; a `const`, `signal`, `let` or definition
    /// its value. Every other kind lists nothing.
    fn exprs(self) -> Vec<Expr<'d>> {
        let d = self.decl();
        let design = d.design();
        let mut ids = Vec::new();
        for root in decl_roots(design, d.id()) {
            crate::pom::body::preorder(design, root, &mut ids);
        }
        ids.into_iter().map(|e| design.expr(e)).collect()
    }

    /// The namespace path joined by `::`, then the member chain joined by
    /// `.`: signal `w` of module `M` in `mod a` is `a::M.w`. An `impl` adds
    /// its header (`impl S`, `impl Tr for S`) and a behavior block its own
    /// (`analog for M`), so their items do not share a name with another
    /// impl's or with the module's; `Design::find` accepts every name this
    /// prints. A generation, a `use` and a file's root namespace add no
    /// segment.
    fn qualified_name(self) -> String {
        let mut chain = Vec::new();
        let mut at = Some(self.decl());
        while let Some(d) = at {
            chain.push(d);
            at = d.declared_in();
        }
        chain.reverse();
        let mut out = String::new();
        let mut in_members = false;
        for d in chain {
            let Some(segment) = qualified_segment(d) else { continue };
            if !out.is_empty() {
                out.push_str(if in_members { "." } else { "::" });
            }
            out.push_str(&segment);
            if !matches!(d.kind(), DeclKind::Namespace(_)) {
                in_members = true;
            }
        }
        out
    }
}

/// The root expressions of one declaration's own body, in source order (see
/// [`Reflect::exprs`]): each is walked pre-order by the caller.
fn decl_roots(design: &Design, id: DeclId) -> Vec<ExprId> {
    use crate::pom::body::{block_exprs, stmt_children};
    use crate::pom::decl::GenerationData;
    let data = design.decl_data(id);
    let body_items = |roots: &mut Vec<ExprId>| {
        for c in &data.children {
            if matches!(design.decl_data(*c).kind, DeclKind::Let(_) | DeclKind::Signal(_) | DeclKind::Definition(_) | DeclKind::Generation(_)) {
                roots.extend(decl_roots(design, *c));
            }
        }
    };
    let mut roots = Vec::new();
    match &data.kind {
        DeclKind::Fn(f) => roots.extend(f.body.iter().flat_map(|b| block_exprs(design, b))),
        DeclKind::Reaction(r) => {
            roots.push(r.event);
            roots.extend(block_exprs(design, &r.body));
        }
        DeclKind::Behavior(b) => {
            roots.extend(b.body.iter().flat_map(|s| stmt_children(design, *s)));
            body_items(&mut roots);
        }
        DeclKind::Module(m) => {
            body_items(&mut roots);
            roots.extend(m.result);
        }
        DeclKind::Generation(g) => {
            roots.push(match g {
                GenerationData::For { iter, .. } => *iter,
                GenerationData::If { cond, .. } => *cond,
            });
            body_items(&mut roots);
        }
        DeclKind::Const(c) => roots.extend(c.value),
        DeclKind::Signal(s) => roots.extend(s.init),
        DeclKind::Let(l) => roots.push(l.value),
        DeclKind::Definition(d) => {
            roots.extend([d.place, d.value]);
            roots.extend(d.after);
        }
        _ => {}
    }
    roots
}

/// What one declaration adds to a qualified name, if anything.
fn qualified_segment(d: Decl<'_>) -> Option<String> {
    let design = d.design();
    match d.kind() {
        DeclKind::Generation(_) => None,
        DeclKind::Impl(i) => {
            let self_ty = design.ty(i.self_ty).text();
            Some(match i.trait_ {
                Some(t) => format!("impl {} for {self_ty}", design.path(t).text()),
                None => format!("impl {self_ty}"),
            })
        }
        DeclKind::Behavior(b) => {
            let kind = match b.kind {
                crate::msl::ast::BehaviorKind::Analog => "analog",
                crate::msl::ast::BehaviorKind::Digital => "digital",
            };
            Some(format!("{kind} for {}", design.path(b.target).text()))
        }
        _ => d.name().map(str::to_string),
    }
}

/// An attribute: `#[path]`, `#[path(args)]` or `#[path = expr]`.
#[derive(Clone, Copy)]
pub struct Attr<'d> {
    owner: Decl<'d>,
    node: &'d AttrNode,
}

impl<'d> Attr<'d> {
    pub fn node(self) -> &'d AttrNode {
        self.node
    }

    /// The path as written, segments joined by `::`.
    pub fn name(self) -> String {
        let path = self.path();
        path.segments().iter().map(|s| s.name.text.as_str()).collect::<Vec<_>>().join("::")
    }

    /// The attribute's path node; its resolution stays `Unresolved`.
    pub fn path(self) -> Path<'d> {
        self.owner.design().path(self.node.path)
    }

    pub fn span(self) -> Span {
        self.node.span
    }

    /// The authored text, `#[...]` included.
    pub fn text(self) -> &'d str {
        self.owner.file().text().get(self.node.span.range()).unwrap_or("")
    }

    pub fn input(self) -> &'d AttrInputNode {
        &self.node.input
    }

    /// The `= expr` value of `#[path = expr]`.
    pub fn value(self) -> Option<Expr<'d>> {
        match &self.node.input {
            AttrInputNode::Eq(e) => Some(self.owner.design().expr(*e)),
            AttrInputNode::None | AttrInputNode::List(_) => None,
        }
    }

    /// The literal arguments written without a name, in order.
    pub fn positional(self) -> Vec<&'d Lit> {
        match &self.node.input {
            AttrInputNode::List(args) => args
                .iter()
                .filter_map(|a| match a {
                    AttrArgNode::Lit(l) => Some(l),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        }
    }

    /// The path arguments, as in `#[derive(Repr, Clone)]`, in order.
    pub fn paths(self) -> Vec<Path<'d>> {
        match &self.node.input {
            AttrInputNode::List(args) => args
                .iter()
                .filter_map(|a| match a {
                    AttrArgNode::Path(p) => Some(self.owner.design().path(*p)),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        }
    }

    /// The value of the argument `name = lit`.
    pub fn named(self, name: &str) -> Option<&'d Lit> {
        match &self.node.input {
            AttrInputNode::List(args) => args.iter().find_map(|a| match a {
                AttrArgNode::NameValue(n, l) if n.text == name => Some(l),
                _ => None,
            }),
            _ => None,
        }
    }
}

impl std::fmt::Debug for Attr<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Attr({})", self.name())
    }
}

impl<'d> Reflect<'d> for Decl<'d> {
    fn decl(self) -> Decl<'d> {
        self
    }
}

impl Design {
    /// The root file's namespace.
    pub fn root(&self) -> Decl<'_> {
        Decl::new(self, self.root_id())
    }

    /// The declaration with this id, as a handle: the way from a
    /// `Resolution::Decl(id)` to the declaration. Ids come from this design.
    pub fn decl(&self, id: DeclId) -> Decl<'_> {
        Decl::new(self, id)
    }

    /// Every declaration of the design in arena (source) order, prelude
    /// files first.
    pub fn decls(&self) -> Vec<Decl<'_>> {
        (0..self.decl_count()).map(|i| Decl::new(self, DeclId(i as u32))).collect()
    }

    /// The declarations of the files that are not prelude sources.
    fn authored(&self) -> impl Iterator<Item = Decl<'_>> {
        self.decls().into_iter().filter(|d| !d.file().is_prelude())
    }

    /// Every module of the non-prelude files, in source order.
    pub fn modules(&self) -> Vec<Module<'_>> {
        self.authored().filter_map(Decl::as_module).collect()
    }

    /// The first declaration `accept` takes: one of a non-prelude file, else
    /// one of a prelude source. The one precedence of every by-name lookup.
    fn first_project_first(&self, accept: impl Fn(Decl<'_>) -> bool) -> Option<Decl<'_>> {
        self.authored().find(|d| accept(*d)).or_else(|| self.decls().into_iter().find(|d| accept(*d)))
    }

    /// [`Design::first_project_first`] over the declarations named `name`,
    /// read from the sorted name table instead of a scan.
    fn first_named(&self, name: &str, accept: impl Fn(Decl<'_>) -> bool) -> Option<Decl<'_>> {
        let ids = self.tables().by_name.get(name)?;
        let mut candidates = ids.iter().map(|id| Decl::new(self, *id)).filter(|d| accept(*d));
        let mut first = None;
        for d in candidates.by_ref() {
            if !d.file().is_prelude() {
                return Some(d);
            }
            first.get_or_insert(d);
        }
        first
    }

    /// The module named `name`: the first match in a non-prelude file, else
    /// the first match in a prelude source.
    pub fn module(&self, name: &str) -> Option<Module<'_>> {
        self.first_named(name, |d| d.as_module().is_some()).and_then(Decl::as_module)
    }

    /// Every free function (a `fn` directly in a namespace) of the non-prelude
    /// files, in source order.
    pub fn functions(&self) -> Vec<Function<'_>> {
        self.authored().filter(Design::is_free).filter_map(Decl::as_function).collect()
    }

    fn is_free(d: &Decl<'_>) -> bool {
        d.declared_in().is_some_and(|p| p.as_namespace().is_some())
    }

    /// The free function named `name`, project files first.
    pub fn function(&self, name: &str) -> Option<Function<'_>> {
        self.first_named(name, |d| Design::is_free(&d) && d.as_function().is_some()).and_then(Decl::as_function)
    }

    /// Every struct of the non-prelude files, in source order.
    pub fn structs(&self) -> Vec<Struct<'_>> {
        self.authored().filter_map(Decl::as_struct).collect()
    }

    /// The struct named `name`, project files first.
    pub fn struct_named(&self, name: &str) -> Option<Struct<'_>> {
        self.first_named(name, |d| d.as_struct().is_some()).and_then(Decl::as_struct)
    }

    /// Every enum of the non-prelude files, in source order.
    pub fn enums(&self) -> Vec<Enum<'_>> {
        self.authored().filter_map(Decl::as_enum).collect()
    }

    /// Every trait of the non-prelude files, in source order.
    pub fn traits(&self) -> Vec<Trait<'_>> {
        self.authored().filter_map(Decl::as_trait).collect()
    }

    /// Every `const` item of the non-prelude files, in source order.
    pub fn consts(&self) -> Vec<Const<'_>> {
        self.authored().filter_map(Decl::as_const).collect()
    }

    /// Every `impl` block of the non-prelude files, in source order.
    pub fn impls(&self) -> Vec<Impl<'_>> {
        self.authored().filter_map(Decl::as_impl).collect()
    }

    /// Every named namespace (`mod a { .. }` or `mod a;`) of the non-prelude
    /// files; a file's unnamed root is not listed.
    pub fn namespaces(&self) -> Vec<Namespace<'_>> {
        self.authored().filter_map(Decl::as_namespace).filter(|n| n.name().is_some()).collect()
    }

    /// Every nature of the non-prelude files, in source order.
    pub fn natures(&self) -> Vec<Nature<'_>> {
        self.authored().filter_map(Decl::as_nature).collect()
    }

    /// The nature named `name`, project files first.
    pub fn nature(&self, name: &str) -> Option<Nature<'_>> {
        self.first_named(name, |d| d.as_nature().is_some()).and_then(Decl::as_nature)
    }

    /// The prelude sources, in registration order. The typed collections do
    /// not list their items; `decls()` does.
    pub fn prelude_files(&self) -> Vec<SourceFile<'_>> {
        self.files().into_iter().filter(|f| f.is_prelude()).collect()
    }

    /// The declaration at a `::` path from the root, each segment a child
    /// name (`a::M` is `M` inside namespace `a`), or at a name
    /// [`Reflect::qualified_name`] prints (`a::E.A`, `impl S.X`).
    pub fn find(&self, path: &str) -> Option<Decl<'_>> {
        let walked = path.split("::").try_fold(self.root(), |at, segment| at.children().into_iter().find(|c| c.name() == Some(segment)));
        // The other form is a name `qualified_name` printed: the first declaration with exactly it.
        walked.or_else(|| self.first_project_first(|d| d.qualified_name() == path))
    }
}
