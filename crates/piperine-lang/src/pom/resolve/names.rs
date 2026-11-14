//! Stage 5: every path with a lexical scope stack. A walk visits each
//! declaration once, keeps the generics and `Self` of the declarations around
//! it, and resolves each path it meets; the answers go to the [`Output`].
//!
//! Lookup order for a single-segment name: the scope stack (innermost first),
//! then the items and imports of the enclosing namespace, the prelude
//! sources and last the built-in table. A multi-segment path resolves segment
//! by segment through `Ctx::walk`.

use super::super::builtin::{BuiltinKind, builtin_implicit};
use std::collections::BTreeSet;

use super::super::body::{BehaviorKind, BlockNode, ExprKind, GenericArgNode, GenericArgsNode, PatKind, StmtKind, TypeKind};
use super::super::decl::{DeclKind, FieldsForm, FnParamData, GenerationData, GenericParamData, NatureFieldNode};
use super::super::decl::BehaviorData;
use super::super::ids::{DeclId, ExprId, PatId, PathId, StmtId, TypeId};
use super::super::resolution::Resolution;
use super::imports::{Prefer, Visit, WalkError, path_diagnostic};
use super::{Ctx, Output};

/// One level of the scope stack.
#[derive(Debug, Clone)]
enum Frame {
    /// The generic parameters that are children of this declaration.
    Generics(DeclId),
    /// What `Self` names here.
    SelfTy(Resolution),
    /// The members of this module: its ports, params, signals and body `let`s.
    Module(DeclId),
    /// Names bound one after another (a block's `let`s, a binder's pattern,
    /// a function's parameters): the latest of a repeated name wins.
    Locals(Vec<(String, DeclId)>),
    /// The items written in one block: visible in the whole block.
    Items(Vec<(String, DeclId)>),
    /// The names the `use` items of one block bring in: visible in the whole
    /// block like its other items. A glob keeps the declaration it names.
    Imports { named: Vec<(String, Resolution)>, globs: Vec<DeclId> },
    /// An item nested in a block does not see the locals and generics around
    /// it, only the items of the blocks it sits in.
    Barrier,
}

struct Names<'c, 'a> {
    ctx: &'c Ctx<'a>,
    out: &'c mut Output,
    frames: Vec<Frame>,
    /// The namespace the walk is inside.
    ns: DeclId,
    /// Inside this many behavior blocks bound to no module: a name read
    /// there that finds nothing is not reported (the block already is).
    quiet: u32,
    /// The identifier patterns that are paths (they bind nothing).
    path_pats: BTreeSet<PatId>,
}

impl Ctx<'_> {
    /// Resolve every path of every declaration.
    pub(super) fn resolve_names(&self, out: &mut Output) {
        for i in 0..self.design.decl_count() {
            let id = DeclId(i as u32);
            let data = self.design.decl_data(id);
            if data.parent.is_none() {
                let mut names = Names { ctx: self, out: &mut *out, frames: Vec::new(), ns: id, quiet: 0, path_pats: BTreeSet::new() };
                names.decl(id);
            }
        }
    }
}

impl Names<'_, '_> {
    fn decl(&mut self, id: DeclId) {
        let design = self.ctx.design;
        let data = design.decl_data(id);
        self.frames.push(Frame::Generics(id));
        self.derives(&data.meta, data.file);
        match &data.kind {
            DeclKind::Namespace(_) => {
                let saved = std::mem::replace(&mut self.ns, id);
                self.children(id);
                self.ns = saved;
            }
            DeclKind::Const(c) => {
                self.ty(c.ty);
                if let Some(v) = c.value {
                    self.expr(v);
                }
            }
            DeclKind::Fn(f) => {
                let mut params = Vec::new();
                for child in &data.children {
                    let d = design.decl_data(*child);
                    if matches!(d.kind, DeclKind::GenericParam(_) | DeclKind::FnParam(_)) {
                        self.decl(*child);
                    }
                    if let (DeclKind::FnParam(_), Some(name)) = (&d.kind, &d.name) {
                        params.push((name.text.clone(), *child));
                    }
                }
                if let Some(ret) = f.ret {
                    self.ty(ret);
                }
                self.frames.push(Frame::Locals(params));
                if let Some(w) = f.where_ {
                    self.expr(w);
                }
                if let Some(body) = &f.body {
                    self.block(body);
                }
                self.frames.pop();
            }
            DeclKind::FnParam(FnParamData::Named { ty }) => self.ty(*ty),
            DeclKind::Field(f) => self.ty(f.ty),
            DeclKind::Trait(t) => {
                for p in &t.supertraits {
                    self.path(*p, Prefer::Types);
                }
                self.frames.push(Frame::SelfTy(Resolution::Decl(id)));
                self.children(id);
                self.frames.pop();
            }
            DeclKind::Impl(i) => {
                if let Some(t) = i.trait_ {
                    self.path(t, Prefer::Types);
                }
                let self_res = match &design.type_node(i.self_ty).kind {
                    TypeKind::Path(p) => self.path(*p, Prefer::Types),
                    _ => {
                        self.ty(i.self_ty);
                        Resolution::Unresolved
                    }
                };
                self.frames.push(Frame::SelfTy(self_res));
                self.children(id);
                self.frames.pop();
            }
            DeclKind::GenericParam(GenericParamData::Type { bounds, default }) => {
                for b in bounds {
                    self.path(*b, Prefer::Types);
                }
                if let Some(d) = default {
                    self.ty(*d);
                }
            }
            DeclKind::GenericParam(GenericParamData::Const { ty, default }) => {
                self.ty(*ty);
                if let Some(d) = default {
                    self.expr(*d);
                }
            }
            DeclKind::Nature(n) => {
                self.children(id);
                for field in &n.fields {
                    match field {
                        NatureFieldNode::Value { ty, .. } => self.ty(*ty),
                        NatureFieldNode::Resolve { expr, .. } => self.expr(*expr),
                        NatureFieldNode::Quantity(_) => {}
                    }
                }
            }
            DeclKind::Quantity(q) => {
                // `access: v` introduces the accessor name: its path names the
                // quantity, and is not a reference to look up.
                for attr in &q.attrs {
                    if attr.name.text != "access" {
                        self.expr(attr.value);
                    } else if let ExprKind::Path(p) = &self.ctx.design.expr_node(attr.value).kind {
                        self.out.segments.push((*p, 0, Resolution::Decl(id)));
                    }
                }
            }
            DeclKind::Variant(v) => {
                self.children(id);
                if let Some(d) = v.discriminant {
                    self.expr(d);
                }
            }
            DeclKind::Module(m) => {
                self.frames.push(Frame::Module(id));
                self.children(id);
                if let Some(ret) = m.ret {
                    self.ty(ret);
                }
                self.opt_expr(m.where_);
                self.opt_expr(m.result);
                self.frames.pop();
            }
            DeclKind::Port(p) => {
                self.ty(p.ty);
                self.opt_expr(p.default);
            }
            DeclKind::Param(p) => {
                self.ty(p.ty);
                self.opt_expr(p.default);
                self.opt_expr(p.where_);
            }
            DeclKind::Signal(s) => {
                self.ty(s.ty);
                self.opt_expr(s.init);
            }
            DeclKind::Let(l) => {
                self.pat(l.pat);
                self.expr(l.value);
            }
            DeclKind::Definition(d) => {
                self.expr(d.place);
                self.expr(d.value);
                self.opt_expr(d.after);
            }
            DeclKind::Generation(GenerationData::For { pat, iter, .. }) => {
                self.expr(*iter);
                self.pat(*pat);
                let mut names = Vec::new();
                self.bindings(*pat, &mut names);
                self.frames.push(Frame::Locals(names));
                self.children(id);
                self.frames.pop();
            }
            DeclKind::Generation(GenerationData::If { cond, .. }) => {
                self.expr(*cond);
                self.children(id);
            }
            DeclKind::Behavior(b) => self.behavior(id, b),
            DeclKind::Reaction(r) => {
                self.expr(r.event);
                self.block(&r.body);
            }
            DeclKind::Struct(_) | DeclKind::Enum(_) => self.children(id),
            DeclKind::Use(_)
            | DeclKind::FnParam(_)
            | DeclKind::Binding(_)
            | DeclKind::Error => {}
        }
        self.frames.pop();
    }

    /// The names of `#[derive(..)]` resolve like a bound (App. H): to a
    /// built-in or a user trait; any other name is `E0161` at the name.
    fn derives(&mut self, meta: &crate::pom::meta::Meta, file: crate::pom::FileId) {
        use crate::pom::meta::{AttrArgNode, AttrInputNode};
        let design = self.ctx.design;
        for attr in &meta.attrs {
            let AttrInputNode::List(args) = &attr.input else { continue };
            if !matches!(design.path_node(attr.path).segments.as_slice(), [only] if only.name.text == "derive") {
                continue;
            }
            for arg in args {
                let AttrArgNode::Path(p) = arg else { continue };
                let node = design.path_node(*p);
                let found = self.path(*p, Prefer::Types);
                let is_trait = match found {
                    Resolution::Builtin(b) => b.info().kind == BuiltinKind::Trait,
                    Resolution::Decl(d) => matches!(design.decl_data(d).kind, DeclKind::Trait(_)),
                    _ => continue,
                };
                if !is_trait {
                    let name = node.segments.last().map_or("", |s| s.name.text.as_str());
                    self.out.diagnostics.push((
                        file,
                        crate::msl::Diagnostic::new("E0161", format!("`{name}` is not a trait that can be derived"), node.span, "not a trait")
                            .with_help("derive one of the traits of Appendix H, `Repr`, or a trait you declare"),
                    ));
                }
            }
        }
    }

    fn children(&mut self, id: DeclId) {
        for child in &self.ctx.design.decl_data(id).children {
            self.decl(*child);
        }
    }

    fn ty(&mut self, id: TypeId) {
        match &self.ctx.design.type_node(id).kind {
            TypeKind::Path(p) => {
                self.path(*p, Prefer::Types);
            }
            TypeKind::Array { elem, len } => {
                self.ty(*elem);
                self.expr(*len);
            }
            TypeKind::Slice(inner) | TypeKind::Ref { inner, .. } | TypeKind::Flip(inner) => self.ty(*inner),
            TypeKind::Tuple(items) => {
                for t in items {
                    self.ty(*t);
                }
            }
            TypeKind::FnPtr { params, ret } => {
                for t in params.iter().chain(ret) {
                    self.ty(*t);
                }
            }
            TypeKind::Infer => {}
        }
    }

    fn generic_args(&mut self, args: &GenericArgsNode) {
        match args {
            GenericArgsNode::Angle { args, .. } => {
                for a in args {
                    match a {
                        GenericArgNode::Type(t) => self.ty(*t),
                        GenericArgNode::Const(e) => self.expr(*e),
                    }
                }
            }
            GenericArgsNode::Paren { inputs, output, .. } => {
                for t in inputs.iter().chain(output) {
                    self.ty(*t);
                }
            }
        }
    }

    /// A single name seen from here.
    fn lookup(&self, name: &str, prefer: Prefer, visiting: &mut Vec<Visit>) -> Option<Resolution> {
        let mut hidden = false;
        for frame in self.frames.iter().rev() {
            match frame {
                Frame::Barrier => hidden = true,
                Frame::Items(items) => {
                    if let Some((_, d)) = items.iter().find(|(n, _)| n == name) {
                        return Some(Resolution::Decl(*d));
                    }
                }
                Frame::Imports { named, globs } => {
                    if let Some((_, res)) = named.iter().find(|(n, res)| n == name && *res != Resolution::Unresolved) {
                        return Some(*res);
                    }
                    let mut found = None;
                    for target in globs {
                        match (self.ctx.glob_member(*target, name, prefer, self.ns, visiting), found) {
                            (Some(hit), None) => found = Some(hit),
                            (Some(hit), Some(first)) if hit != first => {
                                self.ctx.ambiguous.set(true);
                                return None;
                            }
                            _ => {}
                        }
                    }
                    if let Some(found) = found {
                        return Some(Resolution::Decl(found));
                    }
                }
                _ if hidden => {}
                Frame::Locals(names) => {
                    if let Some((_, d)) = names.iter().rev().find(|(n, _)| n == name) {
                        return Some(Resolution::Decl(*d));
                    }
                }
                Frame::Module(module) => {
                    let members = self.ctx.scopes.members.get(module);
                    if let Some(d) = members.and_then(|m| m.get(name)) {
                        return Some(Resolution::Decl(d));
                    }
                    // `m` and `dtemp` read the instance's effective values (§8.5.2).
                    if let Some(implicit) = builtin_implicit(name) {
                        return Some(Resolution::Builtin(implicit));
                    }
                }
                Frame::Generics(owner) => {
                    if let Some(c) = self.ctx.scopes.generics.get(owner).and_then(|g| g.get(name)) {
                        return Some(Resolution::Decl(c));
                    }
                }
                Frame::SelfTy(res) if name == "Self" => {
                    return (*res != Resolution::Unresolved).then_some(*res);
                }
                Frame::SelfTy(_) => {}
            }
        }
        if name == "Self" {
            return None;
        }
        self.ctx.scope_lookup(self.ns, name, prefer, visiting)
    }

    /// Resolve one path: record each segment, report the first problem. The
    /// answer is the last segment's.
    fn path(&mut self, id: PathId, prefer: Prefer) -> Resolution {
        let ctx = self.ctx;
        let node = ctx.design.path_node(id);
        let walked = {
            let this = &*self;
            ctx.walk(&node.segments, this.ns, prefer, &mut Vec::new(), &mut |_, name, visiting| this.lookup(name, prefer, visiting))
        };
        for (i, res) in walked.res.iter().enumerate() {
            self.out.segments.push((id, i, *res));
        }
        if let (Some(error), 0) = (walked.error, self.quiet) {
            let file = ctx.design.decl_data(node.owner).file;
            self.out.diagnostics.push((file, path_diagnostic(&node.segments, node.span, error)));
        }
        for segment in &node.segments {
            if let Some(args) = &segment.args {
                self.generic_args(args);
            }
        }
        walked.res.last().copied().unwrap_or(Resolution::Unresolved)
    }
}

// ---- bodies: blocks, statements, expressions, patterns -----------------------------------------

impl Names<'_, '_> {
    /// Add names to the innermost `Locals` frame.
    fn declare(&mut self, names: Vec<(String, DeclId)>) {
        let frame = self.frames.iter_mut().rev().take_while(|f| !matches!(f, Frame::Barrier)).find_map(|f| match f {
            Frame::Locals(v) => Some(v),
            _ => None,
        });
        if let Some(v) = frame {
            v.extend(names);
        }
    }

    /// The names a pattern binds.
    fn bindings(&self, id: PatId, out: &mut Vec<(String, DeclId)>) {
        match &self.ctx.design.pat_node(id).kind {
            PatKind::Ident { name, sub, binding, .. } => {
                if !self.path_pats.contains(&id) {
                    out.push((name.text.clone(), *binding));
                }
                if let Some(s) = sub {
                    self.bindings(*s, out);
                }
            }
            PatKind::Tuple(items) | PatKind::Or(items) | PatKind::Slice(items) | PatKind::TupleStruct { pats: items, .. } => {
                for p in items {
                    self.bindings(*p, out);
                }
            }
            PatKind::Struct { fields, .. } => {
                for f in fields {
                    if let Some(b) = f.binding {
                        out.push((f.name.text.clone(), b));
                    }
                    if let Some(p) = f.pat {
                        self.bindings(p, out);
                    }
                }
            }
            PatKind::Ref(inner) => self.bindings(*inner, out),
            PatKind::Wild | PatKind::Rest | PatKind::Lit(_) | PatKind::Path(_) | PatKind::Range { .. } => {}
        }
    }

    /// What a plain identifier pattern (no `mut`, `ref` or `@`) names when it
    /// names a constant, a unit variant, a unit struct or a built-in variant
    /// but `Some`: it is then a path, not a binder (§1). A name that resolves
    /// to nothing, to a local, to a function or to a variant with fields is a
    /// binder.
    fn constant_named(&self, id: PatId) -> Option<Resolution> {
        let design = self.ctx.design;
        let PatKind::Ident { by_ref: false, mutable: false, name, sub: None, .. } = &design.pat_node(id).kind else { return None };
        let res = self.lookup(&name.text, Prefer::Values, &mut Vec::new())?;
        let names_one = match res {
            Resolution::Decl(d) => match &design.decl_data(d).kind {
                DeclKind::Const(_) => true,
                DeclKind::Variant(v) => v.form == FieldsForm::Unit,
                DeclKind::Struct(s) => s.form == FieldsForm::Unit,
                _ => false,
            },
            Resolution::Builtin(b) => b.info().kind == BuiltinKind::Variant && b.info().name != "Some",
            _ => false,
        };
        names_one.then_some(res)
    }

    /// Resolve the paths a pattern mentions (constants, variants, struct
    /// names) and the names of plain identifier patterns that are paths.
    fn pat(&mut self, id: PatId) {
        match &self.ctx.design.pat_node(id).kind {
            PatKind::Wild | PatKind::Rest => {}
            PatKind::Ident { sub, .. } => {
                if let Some(res) = self.constant_named(id) {
                    self.path_pats.insert(id);
                    self.out.pats.push((id, res));
                }
                if let Some(s) = sub {
                    self.pat(*s);
                }
            }
            PatKind::Lit(e) => self.expr(*e),
            PatKind::Path(p) => {
                self.path(*p, Prefer::Values);
            }
            PatKind::Tuple(items) | PatKind::Or(items) | PatKind::Slice(items) => {
                for p in items {
                    self.pat(*p);
                }
            }
            PatKind::TupleStruct { path, pats } => {
                self.path(*path, Prefer::Values);
                for p in pats {
                    self.pat(*p);
                }
            }
            PatKind::Struct { path, fields, .. } => {
                self.path(*path, Prefer::Values);
                for f in fields {
                    if let Some(p) = f.pat {
                        self.pat(p);
                    }
                }
            }
            PatKind::Range { lo, hi, .. } => {
                for e in lo.iter().chain(hi) {
                    self.expr(*e);
                }
            }
            PatKind::Ref(inner) => self.pat(*inner),
        }
    }

    /// A binder: its pattern's paths, then its names enter the innermost
    /// `Locals` frame.
    fn binder(&mut self, id: PatId) {
        self.pat(id);
        let mut names = Vec::new();
        self.bindings(id, &mut names);
        self.declare(names);
    }

    /// `{ stmts }`: a scope of its own, names bound one after another, the
    /// items it holds visible throughout.
    fn block(&mut self, block: &BlockNode) {
        self.stmt_list(&block.stmts);
    }

    /// `E0162` at each item of a block that repeats a name of its table (D5): the
    /// block holds items as a namespace does.
    fn block_duplicates(&mut self, items: &[(String, DeclId)]) {
        if self.quiet > 0 {
            return;
        }
        let design = self.ctx.design;
        let span_of = |d: DeclId| {
            let data = design.decl_data(d);
            data.name.as_ref().map_or(data.span, |n| n.span)
        };
        for (i, (name, decl)) in items.iter().enumerate() {
            let (types, values) = super::imports::occupies(design, *decl);
            let first = items[..i].iter().find(|(n, d)| {
                let (t, v) = super::imports::occupies(design, *d);
                n == name && ((t && types) || (v && values))
            });
            if let Some((_, first)) = first {
                let message = format!("`{name}` is defined more than once in this block");
                let diagnostic = crate::msl::Diagnostic::new("E0162", message, span_of(*decl), "defined again here").with_label(span_of(*first), "first defined here");
                self.out.diagnostics.push((design.decl_data(*decl).file, diagnostic));
            }
        }
    }

    /// Statements as one scope.
    fn stmt_list(&mut self, stmts: &[StmtId]) {
        let design = self.ctx.design;
        let items = stmts
            .iter()
            .filter_map(|s| match &design.stmt_node(*s).kind {
                StmtKind::Item(d) => design.decl_data(*d).name.as_ref().map(|n| (n.text.clone(), *d)),
                _ => None,
            })
            .collect::<Vec<(String, DeclId)>>();
        self.block_duplicates(&items);
        self.frames.push(Frame::Items(items));
        let imports_at = self.frames.len();
        self.frames.push(Frame::Imports { named: Vec::new(), globs: Vec::new() });
        self.block_uses(stmts, imports_at);
        self.frames.push(Frame::Locals(Vec::new()));
        for s in stmts {
            match &design.stmt_node(*s).kind {
                StmtKind::Let { pat, ty, init, else_ } => {
                    if let Some(t) = ty {
                        self.ty(*t);
                    }
                    if let Some(e) = init {
                        self.expr(*e);
                    }
                    if let Some(b) = else_ {
                        self.block(b);
                    }
                    self.binder(*pat);
                }
                StmtKind::Expr { expr, .. } => self.expr(*expr),
                StmtKind::Item(d) => {
                    self.frames.push(Frame::Barrier);
                    self.decl(*d);
                    self.frames.pop();
                }
                StmtKind::Contribution { lhs, rhs } => {
                    self.expr(*lhs);
                    self.expr(*rhs);
                }
                StmtKind::Commit { place, value, after } => {
                    self.expr(*place);
                    self.expr(*value);
                    if let Some(a) = after {
                        self.expr(*a);
                    }
                }
                // A `signal` statement is only ever misplaced (the parser reported it):
                // the declaration stays in the tree, but its name is in no scope.
                StmtKind::Signal(d) => self.decl(*d),
                StmtKind::Reaction(d) => self.decl(*d),
                StmtKind::Empty => {}
            }
        }
        self.frames.pop();
        self.frames.pop();
        self.frames.pop();
    }

    /// Does the first segment of a block-level `use` path name both a dependency and an item or
    /// import of the block or of the enclosing namespace (§3.5 rule 5)?
    fn block_clash(&self, segments: &[crate::pom::PathSegment]) -> bool {
        let Some(head) = segments.first() else { return false };
        let name = head.name.text.as_str();
        if matches!(name, "crate" | "self" | "super") || !self.ctx.is_dependency_name(self.ns, name) {
            return false;
        }
        let in_block = self.frames.iter().any(|f| match f {
            Frame::Items(items) => items.iter().any(|(n, _)| n == name),
            Frame::Imports { named, .. } => named.iter().any(|(n, _)| n == name),
            _ => false,
        });
        in_block || self.ctx.has_ns_item(self.ns, name)
    }

    /// The `use` items of one block, in source order: each path resolves
    /// from the namespace and the block items (never the locals), and its
    /// name joins the `Imports` frame at `at`, so a later `use` can build on
    /// an earlier one and every read in the block sees them all.
    fn block_uses(&mut self, stmts: &[StmtId], at: usize) {
        let design = self.ctx.design;
        for s in stmts {
            let StmtKind::Item(d) = &design.stmt_node(*s).kind else { continue };
            let DeclKind::Use(u) = &design.decl_data(*d).kind else { continue };
            for import in &u.imports {
                let node = design.path_node(import.path);
                if self.block_clash(&node.segments) {
                    let file = design.decl_data(node.owner).file;
                    self.out.diagnostics.push((file, path_diagnostic(&node.segments, node.span, WalkError::CrateClash { at: 0 })));
                    continue;
                }
                self.frames.push(Frame::Barrier);
                let res = self.path(import.path, Prefer::Types);
                self.frames.pop();
                let Frame::Imports { named, globs } = &mut self.frames[at] else { continue };
                match (import.glob, res) {
                    (true, Resolution::Decl(target)) => globs.push(target),
                    (true, _) => {}
                    (false, res) => {
                        let last = design.path_node(import.path).segments.last().map(|seg| seg.name.text.clone()).unwrap_or_default();
                        named.push((import.alias.as_ref().map_or(last, |a| a.text.clone()), res));
                    }
                }
            }
        }
    }

    fn opt_expr(&mut self, e: Option<ExprId>) {
        if let Some(e) = e {
            self.expr(e);
        }
    }

    fn expr(&mut self, id: ExprId) {
        crate::stack::deep(|| self.expr_inner(id))
    }

    fn expr_inner(&mut self, id: ExprId) {
        let design = self.ctx.design;
        match &design.expr_node(id).kind {
            ExprKind::Lit(_) | ExprKind::Continue | ExprKind::IncludeStr(_) | ExprKind::Error => {}
            ExprKind::Path(p) => {
                self.path(*p, Prefer::Values);
            }
            ExprKind::Unary { expr, .. } | ExprKind::Paren(expr) | ExprKind::Try(expr) | ExprKind::Field { expr, .. } => {
                self.expr(*expr)
            }
            ExprKind::Binary { lhs, rhs, .. } | ExprKind::Assign { lhs, rhs, .. } => {
                self.expr(*lhs);
                self.expr(*rhs);
            }
            ExprKind::Cast { expr, ty } => {
                self.expr(*expr);
                self.ty(*ty);
            }
            ExprKind::Call { callee, args, params, .. } => {
                // The access name of a nature is no declaration; classification links it.
                let alias = matches!(&design.expr_node(*callee).kind, ExprKind::Path(p)
                    if matches!(design.path_node(*p).segments.as_slice(), [one] if one.args.is_none() && self.ctx.access.contains_key(&one.name.text)));
                self.quiet += u32::from(alias);
                self.expr(*callee);
                self.quiet -= u32::from(alias);
                for a in args {
                    self.expr(a.value);
                }
                for f in params.iter().flatten() {
                    self.expr(f.value);
                }
            }
            ExprKind::MethodCall { recv, generics, args, .. } => {
                self.expr(*recv);
                if let Some(g) = generics {
                    self.generic_args(g);
                }
                for a in args {
                    self.expr(a.value);
                }
            }
            ExprKind::Index { expr, index } => {
                self.expr(*expr);
                self.expr(*index);
            }
            ExprKind::Range { lo, hi, .. } => {
                self.opt_expr(*lo);
                self.opt_expr(*hi);
            }
            ExprKind::Tuple(items) | ExprKind::Array(items) => {
                for e in items {
                    self.expr(*e);
                }
            }
            ExprKind::ArrayRepeat { elem, len } => {
                self.expr(*elem);
                self.expr(*len);
            }
            ExprKind::Struct { path, fields, .. } => {
                self.path(*path, Prefer::Values);
                for f in fields {
                    self.expr(f.value);
                }
            }
            ExprKind::Block(b) | ExprKind::Loop { body: b } => self.block(b),
            ExprKind::If { cond, then, else_ } => {
                self.frames.push(Frame::Locals(Vec::new()));
                self.expr(*cond);
                self.block(then);
                self.frames.pop();
                self.opt_expr(*else_);
            }
            ExprKind::Let { pat, expr } => {
                self.expr(*expr);
                self.binder(*pat);
            }
            ExprKind::Match { scrutinee, arms } => {
                self.expr(*scrutinee);
                for arm in arms {
                    self.frames.push(Frame::Locals(Vec::new()));
                    self.binder(arm.pat);
                    self.opt_expr(arm.guard);
                    self.expr(arm.body);
                    self.frames.pop();
                }
            }
            ExprKind::While { cond, body } => {
                self.frames.push(Frame::Locals(Vec::new()));
                self.expr(*cond);
                self.block(body);
                self.frames.pop();
            }
            ExprKind::For { pat, iter, body } => {
                self.expr(*iter);
                self.frames.push(Frame::Locals(Vec::new()));
                self.binder(*pat);
                self.block(body);
                self.frames.pop();
            }
            ExprKind::Break(v) | ExprKind::Return(v) => self.opt_expr(*v),
            ExprKind::Closure { params, ret, body, .. } => {
                self.frames.push(Frame::Locals(Vec::new()));
                for p in params {
                    self.binder(p.pat);
                    if let Some(t) = p.ty {
                        self.ty(t);
                    }
                }
                if let Some(t) = ret {
                    self.ty(*t);
                }
                self.expr(*body);
                self.frames.pop();
            }
            ExprKind::Emit { fields, .. } => {
                for f in fields {
                    self.expr(f.value);
                }
            }
        }
    }
}

// ---- behavior blocks ---------------------------------------------------------------------------

impl Names<'_, '_> {
    /// A block reads the members of its module; the generic names it lists
    /// stand for the module's generics of the same name. A block bound to no
    /// module, or with the wrong names, reports no name error: it has one.
    fn behavior(&mut self, id: DeclId, block: &BehaviorData) {
        let design = self.ctx.design;
        let module = self.ctx.links.get(&id).copied();
        let mut pushed = 0;
        if let Some(m) = module {
            self.frames.push(Frame::Module(m));
            // Each name stands for the module's generic of that name, never the one
            // at its position; a name the module has no generic of stays unresolved.
            let names = block
                .names
                .iter()
                .filter_map(|n| {
                    let generic = design.decl_data(m).children.iter().copied().find(|c| {
                        let d = design.decl_data(*c);
                        matches!(d.kind, DeclKind::GenericParam(_)) && d.name.as_ref().is_some_and(|g| g.text == n.text)
                    });
                    generic.map(|g| (n.text.clone(), g))
                })
                .collect();
            self.frames.push(Frame::Locals(names));
            pushed = 2;
        }
        // A block with no module, or with names that are not the module's, has its error already.
        let quiet = module.is_none_or(|m| !self.ctx.names_match(block, m));
        if quiet {
            self.quiet += 1;
        }
        // The target is resolved by stage 4; its generic arguments are types of this scope.
        for segment in &design.path_node(block.target).segments {
            if let Some(args) = &segment.args {
                self.generic_args(args);
            }
        }
        if block.kind == BehaviorKind::Analog {
            self.stmt_list(&block.body);
        } else {
            for child in &design.decl_data(id).children {
                if matches!(design.decl_data(*child).kind, DeclKind::Reaction(_) | DeclKind::Generation(_)) {
                    self.decl(*child);
                }
            }
        }
        if quiet {
            self.quiet -= 1;
        }
        for _ in 0..pushed {
            self.frames.pop();
        }
    }
}
