//! Stage 5: every path with a lexical scope stack. A walk visits each
//! declaration once, keeps the generics and `Self` of the declarations around
//! it, and resolves each path it meets; the answers go to the [`Output`].
//!
//! Lookup order for a single-segment name: the scope stack (innermost first),
//! then the items and imports of the enclosing namespace, the prelude
//! sources and last the built-in table. A multi-segment path resolves segment
//! by segment through `Ctx::walk`.

use super::super::body::{Block, ExprKind, GenericArgNode, GenericArgsNode, PatKind, StmtKind, TypeKind};
use super::super::decl::{DeclKind, FnParamData, GenericParamData, NatureFieldNode};
use super::super::ids::{DeclId, ExprId, PatId, PathId, TypeId};
use super::super::resolution::Resolution;
use super::imports::{Prefer, Visit, path_diagnostic};
use super::{Ctx, Output};

/// One level of the scope stack.
#[derive(Debug, Clone)]
enum Frame {
    /// The generic parameters that are children of this declaration.
    Generics(DeclId),
    /// What `Self` names here.
    SelfTy(Resolution),
    /// Names bound one after another (a block's `let`s, a binder's pattern,
    /// a function's parameters): the latest of a repeated name wins.
    Locals(Vec<(String, DeclId)>),
    /// The items written in one block: visible in the whole block.
    Items(Vec<(String, DeclId)>),
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
}

impl Ctx<'_> {
    /// Resolve every path of every declaration.
    pub(super) fn resolve_names(&self, out: &mut Output) {
        for i in 0..self.design.decl_count() {
            let id = DeclId(i as u32);
            let data = self.design.decl_data(id);
            if data.parent.is_none() {
                let mut names = Names { ctx: self, out: &mut *out, frames: Vec::new(), ns: id };
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
                // `access: v` names the accessor the nature introduces; it is not a reference.
                for attr in q.attrs.iter().filter(|a| a.name.text != "access") {
                    self.expr(attr.value);
                }
            }
            DeclKind::Variant(v) => {
                self.children(id);
                if let Some(d) = v.discriminant {
                    self.expr(d);
                }
            }
            DeclKind::Module(m) => {
                self.children(id);
                if let Some(ret) = m.ret {
                    self.ty(ret);
                }
            }
            DeclKind::Port(p) => self.ty(p.ty),
            DeclKind::Param(p) => self.ty(p.ty),
            DeclKind::Signal(s) => self.ty(s.ty),
            DeclKind::Struct(_) | DeclKind::Enum(_) | DeclKind::Generation(_) => self.children(id),
            DeclKind::Use(_)
            | DeclKind::FnParam(_)
            | DeclKind::Let(_)
            | DeclKind::Binding(_)
            | DeclKind::Definition(_)
            | DeclKind::Behavior(_)
            | DeclKind::Reaction(_)
            | DeclKind::Error => {}
        }
        self.frames.pop();
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
        let design = self.ctx.design;
        let mut hidden = false;
        for frame in self.frames.iter().rev() {
            match frame {
                Frame::Barrier => hidden = true,
                Frame::Items(items) => {
                    if let Some((_, d)) = items.iter().find(|(n, _)| n == name) {
                        return Some(Resolution::Decl(*d));
                    }
                }
                _ if hidden => {}
                Frame::Locals(names) => {
                    if let Some((_, d)) = names.iter().rev().find(|(n, _)| n == name) {
                        return Some(Resolution::Decl(*d));
                    }
                }
                Frame::Generics(owner) => {
                    let found = design.decl_data(*owner).children.iter().copied().find(|c| {
                        let d = design.decl_data(*c);
                        matches!(d.kind, DeclKind::GenericParam(_)) && d.name.as_ref().is_some_and(|n| n.text == name)
                    });
                    if let Some(c) = found {
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
        if let Some(error) = walked.error {
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
                out.push((name.text.clone(), *binding));
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

    /// Resolve the paths a pattern mentions (constants, variants, struct
    /// names); a plain identifier is a binding, never a reference.
    fn pat(&mut self, id: PatId) {
        match &self.ctx.design.pat_node(id).kind {
            PatKind::Wild | PatKind::Rest => {}
            PatKind::Ident { sub, .. } => {
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
    fn block(&mut self, block: &Block) {
        let design = self.ctx.design;
        let items = block
            .stmts
            .iter()
            .filter_map(|s| match &design.stmt_node(*s).kind {
                StmtKind::Item(d) => design.decl_data(*d).name.as_ref().map(|n| (n.text.clone(), *d)),
                _ => None,
            })
            .collect();
        self.frames.push(Frame::Items(items));
        self.frames.push(Frame::Locals(Vec::new()));
        for s in &block.stmts {
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
                StmtKind::Signal(d) => {
                    self.decl(*d);
                    if let Some(n) = &design.decl_data(*d).name {
                        self.declare(vec![(n.text.clone(), *d)]);
                    }
                }
                StmtKind::Reaction(d) => self.decl(*d),
                StmtKind::Empty => {}
            }
        }
        self.frames.pop();
        self.frames.pop();
    }

    fn opt_expr(&mut self, e: Option<ExprId>) {
        if let Some(e) = e {
            self.expr(e);
        }
    }

    fn expr(&mut self, id: ExprId) {
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
                self.expr(*callee);
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
