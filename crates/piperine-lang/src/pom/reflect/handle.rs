//! The generic declaration handle's methods, and the [`Reflect`] trait every declaration handle shares.

use super::{Attr, Commit, Crate, Decl, Expr, Impl, Module, Reference, SourceFile, Writer};
use crate::msl::Span;
use crate::pom::decl::{DeclData, DeclKind};
use crate::pom::design::Design;
use crate::pom::ids::{DeclId, ExprId};
use crate::pom::index::{ReferenceId, WriterId};
use crate::pom::meta::Doc;

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

    /// The root expressions of one declaration's own body, in source order (see
    /// [`Reflect::exprs`]): each is walked pre-order by the caller.
    fn body_roots(self) -> Vec<ExprId> {
        let (design, id) = (self.design, self.id);
        use crate::pom::BodyWalk;
        use crate::pom::decl::GenerationData;
        let data = design.decl_data(id);
        let body_items = |roots: &mut Vec<ExprId>| {
            for c in &data.children {
                if matches!(design.decl_data(*c).kind, DeclKind::Let(_) | DeclKind::Signal(_) | DeclKind::Definition(_) | DeclKind::Generation(_)) {
                    roots.extend(Decl::new(design, *c).body_roots());
                }
            }
        };
        let mut roots = Vec::new();
        match &data.kind {
            DeclKind::Fn(f) => roots.extend(f.body.iter().flat_map(|b| BodyWalk::new(design).block_expressions(b))),
            DeclKind::Reaction(r) => {
                roots.push(r.event);
                roots.extend(BodyWalk::new(design).block_expressions(&r.body));
            }
            DeclKind::Behavior(b) => {
                roots.extend(b.body.iter().flat_map(|s| BodyWalk::new(design).statement_children(*s)));
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
    fn qualified_segment(self) -> Option<String> {
        let (d, design) = (self, self.design);
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

    /// The declarations of a module-body or digital-block item list, in source
    /// order, with the items of every generation in place of the generation.
    pub(super) fn deep_items(self, out: &mut Vec<Decl<'d>>) {
        for child in self.children() {
            if matches!(child.kind(), DeclKind::Generation(_)) {
                child.deep_items(out);
            } else {
                out.push(child);
            }
        }
    }

    /// The authored text of `span` in the file of this declaration.
    pub(super) fn text_of(self, span: Span) -> &'d str {
        self.file().text().get(span.range()).unwrap_or("")
    }

    /// Every definition and commit whose place is rooted at this declaration, in source order.
    pub(super) fn writers_of(self) -> Vec<Writer<'d>> {
        self.design
            .index
            .writers(self.id)
            .iter()
            .filter_map(|w| match *w {
                WriterId::Definition(d) => Decl::new(self.design, d).as_definition().map(Writer::Definition),
                WriterId::Commit(st) => Some(Writer::Commit(Commit::new(self.design, st))),
            })
            .collect()
    }

    /// The `impl` blocks whose self type names this declaration, in source order.
    pub(super) fn impl_blocks(self) -> Vec<Impl<'d>> {
        self.design.impls_of(self.id).into_iter().filter_map(|i| Decl::new(self.design, i).as_impl()).collect()
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

impl<'d> Reflect<'d> for Decl<'d> {
    fn decl(self) -> Decl<'d> {
        self
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

    /// The crate the declaration is written in.
    fn krate(self) -> Crate<'d> {
        self.file().krate()
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
        d.data().meta.attrs.iter().map(|node| Attr::new(d, node)).collect()
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
        for root in d.body_roots() {
            crate::pom::BodyWalk::new(design).descendants_depth_first(root, &mut ids);
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
            let Some(segment) = d.qualified_segment() else { continue };
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
