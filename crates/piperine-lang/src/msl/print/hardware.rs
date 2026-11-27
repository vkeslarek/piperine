//! The printer of hardware: modules, their items, behavior blocks, digital reactions and generation.

use super::{Kid, Printer};
use crate::msl::ast::*;
use crate::msl::span::Span;

impl Printer<'_> {
    pub(super) fn signal(&self, name: &Ident, t: &Type, init: &Option<Expr>) -> String {
        let init = init.as_ref().map_or(String::new(), |e| format!(" = {}", self.expr(e)));
        format!("signal {}: {}{init};", name.spelled(), self.ty(t))
    }

    pub(super) fn reaction(&self, r: &Reaction) -> String {
        format!("on {} {}", self.expr(&r.event), self.block(&r.body))
    }

    // ---- items --------------------------------------------------------------------------------

    pub(super) fn module(&self, m: &ModuleDecl, brace: Option<usize>) -> String {
        let mut s = String::new();
        if let Some(abi) = &m.abi {
            s.push_str(&format!("extern {} ", self.lit(abi)));
        }
        s.push_str(&format!("module {}{}", m.name.spelled(), self.generics(&m.generics)));
        let ports = m
            .ports
            .iter()
            .map(|p| {
                Kid::new(Self::extent(&p.meta, p.span), move || {
                    let default = p.default.as_ref().map_or(String::new(), |d| format!(" = {}", self.expr(d)));
                    format!("{}{}{}: {}{default}", self.meta(&p.meta), Self::direction(&p.dir), p.name.spelled(), self.ty(&p.ty))
                })
            })
            .collect();
        s.push_str(&self.list("(", ports, ")"));
        if let Some(r) = &m.ret {
            s.push_str(&format!(" -> {}", self.ty(r)));
        }
        if let Some(w) = &m.where_ {
            s.push_str(&format!(" where {}", self.expr(w)));
        }
        let mut kids: Vec<Kid<'_>> =
            m.body.iter().map(|i| Kid::new(Self::extent(&i.meta, i.span), move || self.module_item(i))).collect();
        if let Some(r) = &m.result {
            kids.push(Kid::new(r.span.0, move || self.expr(r)));
        }
        s.push_str(&format!(" {}", Self::braces(&self.lines(kids, "", brace))));
        s
    }

    pub(super) fn module_item(&self, i: &ModuleItem) -> String {
        let body = match &i.kind {
            ModuleItemKind::Param { name, ty: t, default, where_ } => {
                let mut s = format!("{}: {}", name.spelled(), self.ty(t));
                if let Some(d) = default {
                    s.push_str(&format!(" = {}", self.expr(d)));
                }
                if let Some(w) = where_ {
                    s.push_str(&format!(" where {}", self.expr(w)));
                }
                s.push(';');
                s
            }
            ModuleItemKind::Signal { name, ty: t, init } => self.signal(name, t, init),
            ModuleItemKind::Let { pat: p, value } => format!("let {} = {};", self.pat1(p), self.expr(value)),
            ModuleItemKind::Def { place, value, after } => {
                let after = after.as_ref().map_or(String::new(), |a| format!(" after {}", self.expr(a)));
                format!("{} = {}{after};", self.expr(place), self.expr(value))
            }
            ModuleItemKind::Gen(g) => {
                self.generation(g, &|i: &ModuleItem| Self::extent(&i.meta, i.span), &|i| self.module_item(i))
            }
            ModuleItemKind::Error => "/* error */".to_string(),
        };
        format!("{}{}{body}", self.meta(&i.meta), Self::visibility(i.public))
    }

    /// `for`/`if` generation over items with the extent `ext`, printed by `leaf`.
    pub(super) fn generation<X>(&self, g: &Gen<X>, ext: &dyn Fn(&X) -> Span, leaf: &dyn Fn(&X) -> String) -> String {
        let body = |items: &[X], brace: Option<usize>| {
            let kids = items.iter().map(|x| Kid::new(ext(x), move || leaf(x))).collect();
            Self::braces(&self.lines(kids, "", brace))
        };
        match &g.kind {
            GenKind::For { pat: p, iter, body: b } => {
                format!("for {} in {} {}", self.pat1(p), self.expr(iter), body(b, Self::last_brace(g.span)))
            }
            GenKind::If { cond, then, else_ } => {
                let then_brace = if else_.is_none() { Self::last_brace(g.span) } else { None };
                let mut s = format!("if {} {}", self.expr(cond), body(then, then_brace));
                match else_.as_deref() {
                    Some(GenElse::Block(b)) => s.push_str(&format!(" else {}", body(b, Self::last_brace(g.span)))),
                    Some(GenElse::If(inner)) => s.push_str(&format!(" else {}", self.generation(inner, ext, leaf))),
                    None => {}
                }
                s
            }
        }
    }

    pub(super) fn behavior(&self, b: &BehaviorBlock, brace: Option<usize>) -> String {
        let (word, kids) = match &b.body {
            BehaviorBody::Analog(stmts) => ("analog", self.stmts(stmts)),
            BehaviorBody::Digital(items) => (
                "digital",
                items.iter().map(|i| Kid::new(Self::extent(&i.meta, i.span), move || self.digital_item(i))).collect(),
            ),
        };
        let lines = self.lines(kids, "", brace);
        let names = if b.names.is_empty() {
            String::new()
        } else {
            format!("<{}>", b.names.iter().map(|i| self.ident(i)).collect::<Vec<_>>().join(", "))
        };
        format!("{word}{names} for {} {}", self.path(&b.target), Self::braces(&lines))
    }

    pub(super) fn digital_item(&self, i: &DigitalItem) -> String {
        let body = match &i.kind {
            DigitalItemKind::Reaction(r) => self.reaction(r),
            DigitalItemKind::Gen(g) => {
                self.generation(g, &|i: &DigitalItem| Self::extent(&i.meta, i.span), &|i| self.digital_item(i))
            }
            DigitalItemKind::Error => "/* error */".to_string(),
        };
        format!("{}{body}", self.meta(&i.meta))
    }
}
