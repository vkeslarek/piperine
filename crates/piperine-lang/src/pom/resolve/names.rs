//! Stage 5: every path with a lexical scope stack. A walk visits each
//! declaration once, keeps the generics and `Self` of the declarations around
//! it, and resolves each path it meets; the answers go to the [`Output`].
//!
//! Lookup order for a single-segment name: the scope stack (innermost first),
//! then the items and imports of the enclosing namespace, the prelude
//! sources and last the built-in table. A multi-segment path resolves segment
//! by segment through `Ctx::walk`.

use super::super::body::{GenericArgNode, GenericArgsNode, TypeKind};
use super::super::decl::{DeclKind, FnParamData, GenericParamData, NatureFieldNode};
use super::super::ids::{DeclId, PathId, TypeId};
use super::super::resolution::Resolution;
use super::imports::{Prefer, Visit, path_diagnostic};
use super::{Ctx, Output};

/// One level of the scope stack.
#[derive(Debug, Clone, Copy)]
enum Frame {
    /// The generic parameters that are children of this declaration.
    Generics(DeclId),
    /// What `Self` names here.
    SelfTy(Resolution),
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
            DeclKind::Const(c) => self.ty(c.ty),
            DeclKind::Fn(f) => {
                for child in &data.children {
                    if matches!(design.decl_data(*child).kind, DeclKind::GenericParam(_) | DeclKind::FnParam(_)) {
                        self.decl(*child);
                    }
                }
                if let Some(ret) = f.ret {
                    self.ty(ret);
                }
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
            DeclKind::GenericParam(GenericParamData::Const { ty, .. }) => self.ty(*ty),
            DeclKind::Nature(n) => {
                self.children(id);
                for field in &n.fields {
                    if let NatureFieldNode::Value { ty, .. } = field {
                        self.ty(*ty);
                    }
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
            DeclKind::Struct(_) | DeclKind::Enum(_) | DeclKind::Variant(_) | DeclKind::Generation(_) => self.children(id),
            DeclKind::Use(_)
            | DeclKind::FnParam(_)
            | DeclKind::Quantity(_)
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
            TypeKind::Array { elem, .. } => self.ty(*elem),
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
                    if let GenericArgNode::Type(t) = a {
                        self.ty(*t);
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
        for frame in self.frames.iter().rev() {
            match frame {
                Frame::Generics(owner) => {
                    let design = self.ctx.design;
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
