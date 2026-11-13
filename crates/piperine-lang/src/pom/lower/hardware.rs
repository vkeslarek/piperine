//! Hardware items: natures, modules with their body items, behavior blocks
//! and the generation constructs that repeat module and digital items.

use super::Lowerer;
use super::body::name;
use crate::msl::ast;
use crate::pom::body::QuantityRole;
use crate::pom::decl::{
    BehaviorData, DeclKind, DefinitionData, GenElseNode, GenerationData, LetData, ModuleData, NatureData,
    NatureFieldNode, ParamData, PortData, QuantityData, SignalData,
};
use crate::pom::ids::DeclId;
use crate::pom::resolution::{NatureKind, PortKind};

/// The kind of a nature from the fields it declares: one `potential` is
/// `SignalFlow`, with one `flow` `Conservative`, one `value` `Discrete`, with
/// one `resolve` `Resolved`; anything else is `Invalid`.
fn nature_kind(fields: &[ast::NatureField]) -> NatureKind {
    let count = |f: fn(&ast::NatureFieldKind) -> bool| fields.iter().filter(|n| f(&n.kind)).count();
    let potentials = count(|k| matches!(k, ast::NatureFieldKind::Quantity { which: QuantityRole::Potential, .. }));
    let flows = count(|k| matches!(k, ast::NatureFieldKind::Quantity { which: QuantityRole::Flow, .. }));
    let values = count(|k| matches!(k, ast::NatureFieldKind::Value(_)));
    let resolves = count(|k| matches!(k, ast::NatureFieldKind::Resolve(_)));
    match (potentials, flows, values, resolves) {
        (1, 1, 0, 0) => NatureKind::Conservative,
        (1, 0, 0, 0) => NatureKind::SignalFlow,
        (0, 0, 1, 0) => NatureKind::Discrete,
        (0, 0, 1, 1) => NatureKind::Resolved,
        _ => NatureKind::Invalid,
    }
}

impl Lowerer<'_> {
    /// A nature: generics, then one `Quantity` per quantity field, as
    /// children of the nature (the current owner).
    pub(crate) fn nature_decl(&mut self, n: &ast::NatureDecl) -> DeclKind {
        self.generics(&n.generics);
        let fields = n
            .fields
            .iter()
            .map(|f| match &f.kind {
                ast::NatureFieldKind::Quantity { which, name: ident, attrs } => {
                    let id = self.new_decl(DeclKind::Error, Some(ident), f.span.0, false);
                    let attrs = self.within(id, |lw| attrs.iter().map(|a| lw.field_init(a)).collect());
                    self.design.decl_mut(id).kind = DeclKind::Quantity(QuantityData { role: *which, attrs });
                    NatureFieldNode::Quantity(id)
                }
                ast::NatureFieldKind::Value(t) => NatureFieldNode::Value { ty: self.ty(t), span: f.span.0 },
                ast::NatureFieldKind::Resolve(e) => NatureFieldNode::Resolve { expr: self.expr(e), span: f.span.0 },
            })
            .collect();
        DeclKind::Nature(NatureData { fields, kind: nature_kind(&n.fields) })
    }

    /// A module: generics, ports, then the body items, as children.
    pub(crate) fn module_decl(&mut self, m: &ast::ModuleDecl) -> DeclKind {
        self.generics(&m.generics);
        for p in &m.ports {
            let id = self.new_decl(DeclKind::Error, Some(&p.name), p.span.0, false);
            let (kind, meta) = self.within(id, |lw| {
                let data = PortData {
                    dir: p.dir,
                    ty: lw.ty(&p.ty),
                    default: p.default.as_ref().map(|e| lw.expr(e)),
                    kind: PortKind::Unknown,
                };
                (DeclKind::Port(data), lw.meta(&p.meta))
            });
            let decl = self.design.decl_mut(id);
            decl.kind = kind;
            decl.meta = meta;
        }
        let ret = m.ret.as_ref().map(|t| self.ty(t));
        let where_ = m.where_.as_ref().map(|e| self.expr(e));
        for item in &m.body {
            self.module_item(item);
        }
        let result = m.result.as_ref().map(|e| self.expr(e));
        DeclKind::Module(ModuleData { abi: m.abi.clone(), ret, where_, result, behaviors: Vec::new(), instances: Vec::new() })
    }

    /// One declaration per body item, as the last child of the owner.
    fn module_item(&mut self, item: &ast::ModuleItem) -> DeclId {
        let ident = match &item.kind {
            ast::ModuleItemKind::Param { name, .. } | ast::ModuleItemKind::Signal { name, .. } => Some(name),
            _ => None,
        };
        let id = self.new_decl(DeclKind::Error, ident, item.span.0, item.public);
        let (kind, meta) = self.within(id, |lw| (lw.module_item_kind(&item.kind), lw.meta(&item.meta)));
        let decl = self.design.decl_mut(id);
        decl.kind = kind;
        decl.meta = meta;
        id
    }

    fn module_item_kind(&mut self, kind: &ast::ModuleItemKind) -> DeclKind {
        match kind {
            ast::ModuleItemKind::Param { ty, default, where_, .. } => DeclKind::Param(ParamData {
                ty: self.ty(ty),
                default: default.as_ref().map(|e| self.expr(e)),
                where_: where_.as_ref().map(|e| self.expr(e)),
            }),
            ast::ModuleItemKind::Signal { ty, init, .. } => {
                DeclKind::Signal(SignalData { ty: self.ty(ty), init: init.as_ref().map(|e| self.expr(e)) })
            }
            ast::ModuleItemKind::Let { pat, value } => {
                let pat = self.pat(pat, crate::pom::decl::BindingOrigin::BodyLet);
                DeclKind::Let(LetData { pat, value: self.expr(value) })
            }
            ast::ModuleItemKind::Def { place, value, after } => DeclKind::Definition(DefinitionData {
                place: self.expr(place),
                value: self.expr(value),
                after: after.as_ref().map(|e| self.expr(e)),
            }),
            ast::ModuleItemKind::Gen(g) => self.generation_kind(g, Self::module_item),
            ast::ModuleItemKind::Error => DeclKind::Error,
        }
    }

    /// The payload of a generation: its items are lowered by `item` as
    /// children of the generation (the current owner), held exactly once.
    fn generation_kind<X>(&mut self, g: &ast::Gen<X>, item: fn(&mut Self, &X) -> DeclId) -> DeclKind {
        let data = match &g.kind {
            ast::GenKind::For { pat, iter, body } => {
                let pat = self.pat(pat, crate::pom::decl::BindingOrigin::GenFor);
                let iter = self.expr(iter);
                GenerationData::For { pat, iter, body: body.iter().map(|x| item(self, x)).collect() }
            }
            ast::GenKind::If { cond, then, else_ } => {
                let cond = self.expr(cond);
                let then = then.iter().map(|x| item(self, x)).collect();
                let else_ = else_.as_ref().map(|e| match &**e {
                    ast::GenElse::Block(items) => GenElseNode::Block(items.iter().map(|x| item(self, x)).collect()),
                    ast::GenElse::If(nested) => GenElseNode::If(self.generation(nested, item)),
                });
                GenerationData::If { cond, then, else_ }
            }
        };
        DeclKind::Generation(data)
    }

    /// A `Generation` declaration as the last child of the owner.
    fn generation<X>(&mut self, g: &ast::Gen<X>, item: fn(&mut Self, &X) -> DeclId) -> DeclId {
        let id = self.new_decl(DeclKind::Error, None, g.span.0, false);
        let kind = self.within(id, |lw| lw.generation_kind(g, item));
        self.design.decl_mut(id).kind = kind;
        id
    }

    /// A behavior block at file scope: kind, names and target, then either
    /// the analog statements or the digital items.
    pub(crate) fn behavior_decl(&mut self, b: &ast::BehaviorBlock) -> DeclKind {
        let names = b.names.iter().map(name).collect();
        let target = self.path(&b.target);
        let body = match &b.body {
            ast::BehaviorBody::Analog(stmts) => stmts.iter().map(|s| self.stmt(s)).collect(),
            ast::BehaviorBody::Digital(items) => {
                for i in items {
                    self.digital_item(i);
                }
                Vec::new()
            }
        };
        DeclKind::Behavior(BehaviorData { kind: b.kind, names, target, body, module: None })
    }

    fn digital_item(&mut self, item: &ast::DigitalItem) -> DeclId {
        match &item.kind {
            ast::DigitalItemKind::Reaction(r) => self.reaction(r, item.span.0),
            ast::DigitalItemKind::Gen(g) => self.generation(g, Self::digital_item),
            ast::DigitalItemKind::Error => self.new_decl(DeclKind::Error, None, item.span.0, false),
        }
    }
}
