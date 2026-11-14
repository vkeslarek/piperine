//! `Design::check`: the invariants the lowering promises, verified over the
//! arenas. Every id a node holds is in range, every owner exists, every
//! resolved link points at a declaration, parent and child links agree, and
//! each declaration is reachable from a file root exactly once.

use std::collections::BTreeSet;

use super::body::{Arg, ExprKind, FieldInit, GenericArgNode, GenericArgsNode, PatKind, PathNode, StmtKind, TypeKind};
use super::body::Block;
use super::decl::{
    DeclKind, GenElseNode, GenerationData, GenericParamData, NamespaceOrigin, NatureFieldNode,
};
use super::design::Design;
use super::ids::{DeclId, ExprId, PatId, PathId, StmtId, TypeId};
use super::meta::{AttrArgNode, AttrInputNode, Meta};
use super::resolution::{CallClass, Resolution, StructClass};

/// Every arena id and resolved link one node or declaration holds.
#[derive(Default)]
struct Refs {
    exprs: Vec<ExprId>,
    stmts: Vec<StmtId>,
    pats: Vec<PatId>,
    types: Vec<TypeId>,
    paths: Vec<PathId>,
    decls: Vec<DeclId>,
    /// Declarations that resolved names point at.
    resolved: Vec<DeclId>,
}

impl Refs {
    fn res(&mut self, r: &Resolution) {
        if let Resolution::Decl(d) = r {
            self.resolved.push(*d);
        }
    }

    fn block(&mut self, b: &Block) {
        self.stmts.extend(&b.stmts);
    }

    fn generic_args(&mut self, g: &GenericArgsNode) {
        match g {
            GenericArgsNode::Angle { args, .. } => {
                for a in args {
                    match a {
                        GenericArgNode::Type(t) => self.types.push(*t),
                        GenericArgNode::Const(e) => self.exprs.push(*e),
                    }
                }
            }
            GenericArgsNode::Paren { inputs, output, .. } => {
                self.types.extend(inputs);
                self.types.extend(output);
            }
        }
    }

    fn args(&mut self, args: &[Arg]) {
        for a in args {
            self.exprs.push(a.value);
            self.res(&a.res);
        }
    }

    fn inits(&mut self, fields: &[FieldInit]) {
        for f in fields {
            self.exprs.push(f.value);
            self.res(&f.res);
        }
    }

    fn meta(&mut self, meta: &Meta) {
        for a in &meta.attrs {
            self.paths.push(a.path);
            match &a.input {
                AttrInputNode::None => {}
                AttrInputNode::Eq(e) => self.exprs.push(*e),
                AttrInputNode::List(args) => {
                    for arg in args {
                        if let AttrArgNode::Path(p) = arg {
                            self.paths.push(*p);
                        }
                    }
                }
            }
        }
    }

    fn path(&mut self, p: &PathNode) {
        for s in &p.segments {
            if let Some(a) = &s.args {
                self.generic_args(a);
            }
            self.res(&s.res);
        }
    }

    fn expr(&mut self, kind: &ExprKind) {
        match kind {
            ExprKind::Lit(_) | ExprKind::Continue | ExprKind::Error | ExprKind::IncludeStr(_) => {}
            ExprKind::Path(p) => self.paths.push(*p),
            ExprKind::Unary { expr, .. } | ExprKind::Paren(expr) | ExprKind::Try(expr) => self.exprs.push(*expr),
            ExprKind::Binary { lhs, rhs, .. } | ExprKind::Assign { lhs, rhs, .. } => self.exprs.extend([*lhs, *rhs]),
            ExprKind::Cast { expr, ty } => {
                self.exprs.push(*expr);
                self.types.push(*ty);
            }
            ExprKind::Call { callee, args, params, class } => {
                self.exprs.push(*callee);
                self.args(args);
                if let Some(p) = params {
                    self.inits(p);
                }
                match class {
                    CallClass::Instance { module, via } => self.resolved.extend(module.iter().chain(via)),
                    CallClass::Unclassified | CallClass::Function | CallClass::Probe => {}
                }
            }
            ExprKind::MethodCall { recv, generics, args, res, .. } => {
                self.exprs.push(*recv);
                if let Some(g) = generics {
                    self.generic_args(g);
                }
                self.args(args);
                self.res(res);
            }
            ExprKind::Field { expr, res, .. } => {
                self.exprs.push(*expr);
                self.res(res);
            }
            ExprKind::Index { expr, index } => self.exprs.extend([*expr, *index]),
            ExprKind::Range { lo, hi, .. } => self.exprs.extend(lo.iter().chain(hi)),
            ExprKind::Tuple(items) | ExprKind::Array(items) => self.exprs.extend(items),
            ExprKind::ArrayRepeat { elem, len } => self.exprs.extend([*elem, *len]),
            ExprKind::Struct { path, fields, class } => {
                self.paths.push(*path);
                self.inits(fields);
                if let StructClass::Template(d) = class {
                    self.resolved.push(*d);
                }
            }
            ExprKind::Block(b) | ExprKind::Loop { body: b } => self.block(b),
            ExprKind::If { cond, then, else_ } => {
                self.exprs.push(*cond);
                self.block(then);
                self.exprs.extend(else_);
            }
            ExprKind::Let { pat, expr } => {
                self.pats.push(*pat);
                self.exprs.push(*expr);
            }
            ExprKind::Match { scrutinee, arms } => {
                self.exprs.push(*scrutinee);
                for a in arms {
                    self.meta(&a.meta);
                    self.pats.push(a.pat);
                    self.exprs.extend(a.guard.iter().chain([&a.body]));
                }
            }
            ExprKind::While { cond, body } => {
                self.exprs.push(*cond);
                self.block(body);
            }
            ExprKind::For { pat, iter, body } => {
                self.pats.push(*pat);
                self.exprs.push(*iter);
                self.block(body);
            }
            ExprKind::Break(v) | ExprKind::Return(v) => self.exprs.extend(v),
            ExprKind::Closure { params, ret, body, .. } => {
                for p in params {
                    self.pats.push(p.pat);
                    self.types.extend(p.ty);
                }
                self.types.extend(ret);
                self.exprs.push(*body);
            }
            ExprKind::Emit { fields, .. } => self.inits(fields),
        }
    }

    fn stmt(&mut self, kind: &StmtKind) {
        match kind {
            StmtKind::Let { pat, ty, init, else_ } => {
                self.pats.push(*pat);
                self.types.extend(ty);
                self.exprs.extend(init);
                if let Some(b) = else_ {
                    self.block(b);
                }
            }
            StmtKind::Expr { expr, .. } => self.exprs.push(*expr),
            StmtKind::Item(d) | StmtKind::Signal(d) | StmtKind::Reaction(d) => self.decls.push(*d),
            StmtKind::Contribution { lhs, rhs } => self.exprs.extend([*lhs, *rhs]),
            StmtKind::Commit { place, value, after } => {
                self.exprs.extend([*place, *value]);
                self.exprs.extend(after);
            }
            StmtKind::Empty => {}
        }
    }

    fn pat(&mut self, kind: &PatKind) {
        match kind {
            PatKind::Wild | PatKind::Rest => {}
            PatKind::Ident { sub, binding, .. } => {
                self.pats.extend(sub);
                self.decls.push(*binding);
            }
            PatKind::Lit(e) => self.exprs.push(*e),
            PatKind::Path(p) => self.paths.push(*p),
            PatKind::Tuple(items) | PatKind::Or(items) | PatKind::Slice(items) => self.pats.extend(items),
            PatKind::TupleStruct { path, pats } => {
                self.paths.push(*path);
                self.pats.extend(pats);
            }
            PatKind::Struct { path, fields, .. } => {
                self.paths.push(*path);
                for f in fields {
                    self.pats.extend(f.pat);
                    self.decls.extend(f.binding);
                }
            }
            PatKind::Range { lo, hi, .. } => self.exprs.extend(lo.iter().chain(hi)),
            PatKind::Ref(p) => self.pats.push(*p),
        }
    }

    fn ty(&mut self, kind: &TypeKind) {
        match kind {
            TypeKind::Path(p) => self.paths.push(*p),
            TypeKind::Array { elem, len } => {
                self.types.push(*elem);
                self.exprs.push(*len);
            }
            TypeKind::Slice(t) | TypeKind::Flip(t) | TypeKind::Ref { inner: t, .. } => self.types.push(*t),
            TypeKind::Tuple(items) => self.types.extend(items),
            TypeKind::FnPtr { params, ret } => {
                self.types.extend(params);
                self.types.extend(ret);
            }
            TypeKind::Infer => {}
        }
    }

    fn decl(&mut self, kind: &DeclKind) {
        match kind {
            DeclKind::Namespace(_) | DeclKind::Struct(_) | DeclKind::Enum(_) | DeclKind::Error => {}
            DeclKind::Use(u) => self.paths.extend(u.imports.iter().map(|i| i.path)),
            DeclKind::Const(c) => {
                self.types.push(c.ty);
                self.exprs.extend(c.value);
            }
            DeclKind::Fn(f) => {
                self.types.extend(f.ret);
                self.exprs.extend(f.where_);
                if let Some(b) = &f.body {
                    self.block(b);
                }
            }
            DeclKind::FnParam(p) => {
                if let super::decl::FnParamData::Named { ty } = p {
                    self.types.push(*ty);
                }
            }
            DeclKind::Field(f) => self.types.push(f.ty),
            DeclKind::Variant(v) => self.exprs.extend(v.discriminant),
            DeclKind::Trait(t) => self.paths.extend(&t.supertraits),
            DeclKind::Impl(i) => {
                self.paths.extend(i.trait_);
                self.types.push(i.self_ty);
            }
            DeclKind::GenericParam(g) => match g {
                GenericParamData::Type { bounds, default } => {
                    self.paths.extend(bounds);
                    self.types.extend(default);
                }
                GenericParamData::Const { ty, default } => {
                    self.types.push(*ty);
                    self.exprs.extend(default);
                }
            },
            DeclKind::Nature(n) => {
                for f in &n.fields {
                    match f {
                        NatureFieldNode::Quantity(d) => self.decls.push(*d),
                        NatureFieldNode::Value { ty, .. } => self.types.push(*ty),
                        NatureFieldNode::Resolve { expr, .. } => self.exprs.push(*expr),
                    }
                }
            }
            DeclKind::Quantity(q) => self.inits(&q.attrs),
            DeclKind::Module(m) => {
                self.types.extend(m.ret);
                self.exprs.extend(m.where_.iter().chain(&m.result));
                self.exprs.extend(&m.instances);
                self.resolved.extend(&m.behaviors);
            }
            DeclKind::Port(p) => {
                self.types.push(p.ty);
                self.exprs.extend(p.default);
            }
            DeclKind::Param(p) => {
                self.types.push(p.ty);
                self.exprs.extend(p.default.iter().chain(&p.where_));
            }
            DeclKind::Signal(s) => {
                self.types.push(s.ty);
                self.exprs.extend(s.init);
            }
            DeclKind::Let(l) => {
                self.pats.push(l.pat);
                self.exprs.push(l.value);
            }
            DeclKind::Binding(b) => self.pats.push(b.pat),
            DeclKind::Definition(d) => {
                self.exprs.extend([d.place, d.value]);
                self.exprs.extend(d.after);
            }
            DeclKind::Generation(g) => match g {
                GenerationData::For { pat, iter, body } => {
                    self.pats.push(*pat);
                    self.exprs.push(*iter);
                    self.decls.extend(body);
                }
                GenerationData::If { cond, then, else_ } => {
                    self.exprs.push(*cond);
                    self.decls.extend(then);
                    match else_ {
                        Some(GenElseNode::Block(items)) => self.decls.extend(items),
                        Some(GenElseNode::If(d)) => self.decls.push(*d),
                        None => {}
                    }
                }
            },
            DeclKind::Behavior(b) => {
                self.paths.push(b.target);
                self.stmts.extend(&b.body);
                self.resolved.extend(b.module);
            }
            DeclKind::Reaction(r) => {
                self.exprs.push(r.event);
                self.block(&r.body);
            }
        }
    }
}

impl Design {
    /// The internal inconsistencies of the arenas, one message each; empty
    /// when the design is well formed.
    pub fn check(&self) -> Vec<String> {
        let mut problems = Vec::new();
        self.check_links(&mut problems);
        self.check_nodes(&mut problems);
        self.check_reachability(&mut problems);
        problems
    }

    /// Parent and child links, spans and files of the declarations.
    fn check_links(&self, problems: &mut Vec<String>) {
        for (i, d) in self.decls.iter().enumerate() {
            let id = DeclId(i as u32);
            if d.file.index() >= self.files.len() {
                problems.push(format!("{id:?}: file {:?} does not exist", d.file));
            } else if d.span.start > d.span.end || d.span.end > self.files[d.file.index()].text.len() {
                problems.push(format!("{id:?}: span {}..{} is outside its file", d.span.start, d.span.end));
            }
            if let DeclKind::Namespace(n) = &d.kind {
                if let NamespaceOrigin::External { file: Some(f) } = n.origin {
                    if f.index() >= self.files.len() {
                        problems.push(format!("{id:?}: loaded file {} does not exist", f.0));
                    }
                }
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
        for (i, f) in self.files.iter().enumerate() {
            if f.root.index() >= self.decls.len() {
                problems.push(format!("file {i}: root {:?} does not exist", f.root));
            }
        }
    }

    /// Every id and every resolved link of every node and declaration.
    fn check_nodes(&self, problems: &mut Vec<String>) {
        let mut each = |what: String, owner: Option<DeclId>, refs: Refs| {
            if let Some(o) = owner {
                if o.index() >= self.decls.len() {
                    problems.push(format!("{what}: owner decl {} does not exist", o.0));
                }
            }
            let mut bad = |kind: &str, id: u32, len: usize| {
                if id as usize >= len {
                    problems.push(format!("{what}: {kind} {id} is out of range"));
                }
            };
            for x in &refs.exprs {
                bad("expression", x.0, self.exprs.len());
            }
            for x in &refs.stmts {
                bad("statement", x.0, self.stmts.len());
            }
            for x in &refs.pats {
                bad("pattern", x.0, self.pats.len());
            }
            for x in &refs.types {
                bad("type", x.0, self.types.len());
            }
            for x in &refs.paths {
                bad("path", x.0, self.paths.len());
            }
            for x in &refs.decls {
                bad("decl", x.0, self.decls.len());
            }
            for x in &refs.resolved {
                if x.index() >= self.decls.len() {
                    problems.push(format!("{what}: resolution points at decl {}, which does not exist", x.0));
                }
            }
        };
        for (i, n) in self.exprs.iter().enumerate() {
            let mut r = Refs::default();
            r.expr(&n.kind);
            each(format!("expression {i}"), Some(n.owner), r);
        }
        for (i, n) in self.stmts.iter().enumerate() {
            let mut r = Refs::default();
            r.stmt(&n.kind);
            each(format!("statement {i}"), Some(n.owner), r);
        }
        for (i, n) in self.pats.iter().enumerate() {
            let mut r = Refs::default();
            r.pat(&n.kind);
            each(format!("pattern {i}"), Some(n.owner), r);
        }
        for (i, n) in self.types.iter().enumerate() {
            let mut r = Refs::default();
            r.ty(&n.kind);
            each(format!("type {i}"), Some(n.owner), r);
        }
        for (i, n) in self.paths.iter().enumerate() {
            let mut r = Refs::default();
            r.path(n);
            each(format!("path {i}"), Some(n.owner), r);
        }
        for (i, d) in self.decls.iter().enumerate() {
            let mut r = Refs::default();
            r.decl(&d.kind);
            r.meta(&d.meta);
            each(format!("decl {i}"), None, r);
        }
    }

    /// Each declaration is reached from a file root through `children`
    /// exactly once.
    fn check_reachability(&self, problems: &mut Vec<String>) {
        let mut seen = BTreeSet::new();
        let mut stack: Vec<DeclId> =
            (0..self.decls.len()).filter(|i| self.decls[*i].parent.is_none()).map(|i| DeclId(i as u32)).collect();
        for root in &stack {
            seen.insert(*root);
        }
        while let Some(d) = stack.pop() {
            for c in &self.decls[d.index()].children {
                if c.index() >= self.decls.len() {
                    continue;
                }
                if !seen.insert(*c) {
                    problems.push(format!("decl {} is reachable twice (again under decl {})", c.0, d.0));
                } else {
                    stack.push(*c);
                }
            }
        }
        for i in 0..self.decls.len() {
            if !seen.contains(&DeclId(i as u32)) {
                problems.push(format!("decl {i} is not reachable from any root"));
            }
        }
    }
}
