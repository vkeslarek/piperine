//! The printer of items: types, generics, functions, structs, enums, `use` trees and natures.

use super::{Kid, Printer, Shape};
use crate::msl::ast::*;

impl Printer<'_> {
    pub(super) fn generic_args(&self, g: &GenericArgs) -> String {
        match g {
            GenericArgs::Angle { args, turbofish, .. } => {
                let items: Vec<String> = args
                    .iter()
                    .map(|a| match a {
                        GenericArg::Type(t) => self.ty(t),
                        GenericArg::Const(e) => self.expr(e),
                    })
                    .collect();
                // `<-` is the commit operator: a leading minus needs a space.
                let gap = if items.first().is_some_and(|i| i.starts_with('-')) { " " } else { "" };
                format!("{}<{gap}{}>", if *turbofish { "::" } else { "" }, items.join(", "))
            }
            GenericArgs::Paren { inputs, output, .. } => {
                let items: Vec<String> = inputs.iter().map(|t| self.ty(t)).collect();
                let ret = output.as_ref().map_or(String::new(), |o| format!(" -> {}", self.ty(o)));
                format!("({}){ret}", items.join(", "))
            }
        }
    }

    pub(super) fn generics(&self, g: &Option<Generics>) -> String {
        let Some(g) = g else { return String::new() };
        let items: Vec<String> = g
            .params
            .iter()
            .map(|p| match &p.kind {
                GenericParamKind::Type { bounds, default } => {
                    let mut s = p.name.spelled();
                    if !bounds.is_empty() {
                        s.push_str(&format!(": {}", bounds.iter().map(|b| self.path(b)).collect::<Vec<_>>().join(" + ")));
                    }
                    if let Some(d) = default {
                        s.push_str(&format!(" = {}", self.ty(d)));
                    }
                    s
                }
                GenericParamKind::Const { ty: t, default } => {
                    let mut s = format!("const {}: {}", p.name.spelled(), self.ty(t));
                    if let Some(d) = default {
                        s.push_str(&format!(" = {}", self.expr(d)));
                    }
                    s
                }
            })
            .collect();
        format!("<{}>", items.join(", "))
    }

    pub(super) fn ty(&self, t: &Type) -> String {
        match &t.kind {
            TypeKind::Path(p) => self.path(p),
            TypeKind::Array { elem, len } => format!("[{}; {}]", self.ty(elem), self.expr(len)),
            TypeKind::Tuple(items) => match items.as_slice() {
                [one] => format!("({},)", self.ty(one)),
                _ => format!("({})", items.iter().map(|t| self.ty(t)).collect::<Vec<_>>().join(", ")),
            },
            TypeKind::Flip(inner) => format!("flip {}", self.ty(inner)),
            TypeKind::FnPtr { params, ret } => {
                let ret = ret.as_ref().map_or(String::new(), |r| format!(" -> {}", self.ty(r)));
                format!("fn({}){ret}", params.iter().map(|t| self.ty(t)).collect::<Vec<_>>().join(", "))
            }
            TypeKind::Infer => "_".to_string(),
        }
    }

    // ---- patterns -----------------------------------------------------------------------------

    pub(super) fn item(&self, i: &Item) -> String {
        format!("{}{}{}", self.meta(&i.meta), Self::visibility(i.public), self.item_kind(&i.kind, Self::last_brace(i.span)))
    }

    /// An item's own text; `brace` is where the `}` closing its body stands.
    pub(super) fn item_kind(&self, k: &ItemKind, brace: Option<usize>) -> String {
        match k {
            ItemKind::Fn(f) => self.fn_decl(f),
            ItemKind::Struct(s) => {
                format!("struct {}{}{}", s.name.spelled(), self.generics(&s.generics), self.fields(&s.fields, true, brace))
            }
            ItemKind::Enum(e) => {
                let kids = e.variants.iter().map(|v| Kid::new(Self::extent(&v.meta, v.span), move || self.variant(v))).collect();
                let lines = self.lines(kids, ",", brace);
                format!("enum {}{} {}", e.name.spelled(), self.generics(&e.generics), Self::braces(&lines))
            }
            ItemKind::Impl(i) => {
                let head = match &i.trait_ {
                    Some(t) => format!("{} for {}", self.path(t), self.ty(&i.self_ty)),
                    None => self.ty(&i.self_ty),
                };
                format!("impl{} {head} {}", self.generics(&i.generics), Self::braces(&self.impl_items(&i.items, brace)))
            }
            ItemKind::Trait(t) => {
                let supers = if t.supertraits.is_empty() {
                    String::new()
                } else {
                    format!(": {}", t.supertraits.iter().map(|p| self.path(p)).collect::<Vec<_>>().join(" + "))
                };
                format!(
                    "trait {}{}{supers} {}",
                    t.name.spelled(),
                    self.generics(&t.generics),
                    Self::braces(&self.impl_items(&t.items, brace))
                )
            }
            ItemKind::Const(c) => self.const_item(c),
            ItemKind::Use(u) => format!("use {};", self.use_tree(u)),
            ItemKind::Mod(m) => match &m.items {
                None => format!("mod {};", m.name.spelled()),
                Some(items) => {
                    let kids = items.iter().map(|i| Kid::new(Self::extent(&i.meta, i.span), move || self.item(i))).collect();
                    let (out, spans) = self.children(kids, "", Shape::Items);
                    let blocks: Vec<String> = out.into_iter().map(|(_, t)| t).collect();
                    let mut body = blocks.join("\n\n");
                    for line in self.hooks.inside(Self::close(brace, &spans), &spans, Shape::Items) {
                        body.push_str(&format!("\n{line}"));
                    }
                    if body.is_empty() {
                        format!("mod {} {{}}", m.name.spelled())
                    } else {
                        format!("mod {} {{\n{}\n}}", m.name.spelled(), Self::indent(body.trim_start_matches('\n')))
                    }
                }
            },
            ItemKind::Nature(n) => self.nature(n, brace),
            ItemKind::Module(m) => self.module(m, brace),
            ItemKind::Behavior(b) => self.behavior(b, brace),
            ItemKind::Error => "/* error */".to_string(),
        }
    }

    pub(super) fn fn_decl(&self, f: &FnDecl) -> String {
        let mut s = String::new();
        if f.is_const {
            s.push_str("const ");
        }
        if let Some(abi) = &f.abi {
            s.push_str(&format!("extern {} ", self.lit(abi)));
        }
        s.push_str(&format!("fn {}{}", f.name.spelled(), self.generics(&f.generics)));
        let params: Vec<Kid<'_>> = f
            .params
            .iter()
            .filter_map(|p| match p {
                FnParam::SelfValue(at) => Some(Kid::new(at.0, || "self".to_string())),
                FnParam::Named { name, ty: t, span } => {
                    Some(Kid::new(span.0, move || format!("{}: {}", name.spelled(), self.ty(t))))
                }
                FnParam::Error(_) => None,
            })
            .collect();
        s.push_str(&self.list("(", params, ")"));
        if let Some(r) = &f.ret {
            s.push_str(&format!(" -> {}", self.ty(r)));
        }
        if let Some(w) = &f.where_ {
            s.push_str(&format!(" where {}", self.expr(w)));
        }
        match &f.body {
            Some(b) => s.push_str(&format!(" {}", self.block(b))),
            None => s.push(';'),
        }
        s
    }

    pub(super) fn const_item(&self, c: &ConstItem) -> String {
        let value = c.value.as_ref().map_or(String::new(), |v| format!(" = {}", self.expr(v)));
        format!("const {}: {}{value};", c.name.spelled(), self.ty(&c.ty))
    }

    pub(super) fn impl_items(&self, items: &[ImplItem], brace: Option<usize>) -> Vec<String> {
        let kids = items
            .iter()
            .map(|i| {
                Kid::new(Self::extent(&i.meta, i.span), move || {
                    let body = match &i.kind {
                        ImplItemKind::Fn(f) => self.fn_decl(f),
                        ImplItemKind::Const(c) => self.const_item(c),
                    };
                    format!("{}{}{body}", self.meta(&i.meta), Self::visibility(i.public))
                })
            })
            .collect();
        self.lines(kids, "", brace)
    }

    pub(super) fn field(&self, f: &Field) -> String {
        let init = f.init.as_ref().map_or(String::new(), |init| format!(" = {}", self.expr(init)));
        let body = match &f.name {
            Some(n) => format!("{}: {}{init}", n.spelled(), self.ty(&f.ty)),
            None => self.ty(&f.ty),
        };
        format!("{}{}{}{body}", self.meta(&f.meta), Self::visibility(f.public), Self::direction(&f.dir))
    }

    /// The body of a struct (`unit_semi`: a unit struct or tuple struct ends in
    /// `;`) or of an enum variant; `brace` is where its `}` stands, when known.
    pub(super) fn fields(&self, f: &Fields, unit_semi: bool, brace: Option<usize>) -> String {
        match f {
            Fields::Named(fs) => {
                let kids = fs.iter().map(|f| Kid::new(Self::extent(&f.meta, f.span), move || self.field(f))).collect();
                format!(" {}", Self::braces(&self.lines(kids, ",", brace)))
            }
            Fields::Tuple(fs) => {
                let kids = fs.iter().map(|f| Kid::new(Self::extent(&f.meta, f.span), move || self.field(f))).collect();
                format!("{}{}", self.list("(", kids, ")"), if unit_semi { ";" } else { "" })
            }
            Fields::Unit => if unit_semi { ";".to_string() } else { String::new() },
        }
    }

    /// An enum variant, without the `,` the enum adds.
    pub(super) fn variant(&self, v: &Variant) -> String {
        let disc = v.discriminant.as_ref().map_or(String::new(), |d| format!(" = {}", self.expr(d)));
        format!("{}{}{}{disc}", self.meta(&v.meta), v.name.spelled(), self.fields(&v.fields, false, None))
    }

    pub(super) fn use_tree(&self, u: &UseTree) -> String {
        let prefix: Vec<String> = u.path.iter().map(|i| i.spelled()).collect();
        let joined = prefix.join("::");
        match &u.kind {
            UseKind::Simple { alias } => match alias {
                Some(a) => format!("{joined} as {}", a.spelled()),
                None => joined,
            },
            UseKind::Glob => if joined.is_empty() { "*".to_string() } else { format!("{joined}::*") },
            UseKind::Group(trees) => {
                let inner = trees.iter().map(|t| self.use_tree(t)).collect::<Vec<_>>().join(", ");
                if joined.is_empty() { format!("{{{inner}}}") } else { format!("{joined}::{{{inner}}}") }
            }
        }
    }

    pub(super) fn nature(&self, n: &NatureDecl, brace: Option<usize>) -> String {
        let kids = n
            .fields
            .iter()
            .map(|f| Kid::new(Self::extent(&f.meta, f.span), move || format!("{}{}", self.meta(&f.meta), self.nature_field(&f.kind))))
            .collect();
        let lines = self.lines(kids, ",", brace);
        format!("nature {}{} {}", n.name.spelled(), self.generics(&n.generics), Self::braces(&lines))
    }

    /// A nature field, without the `,` the nature adds.
    pub(super) fn nature_field(&self, k: &NatureFieldKind) -> String {
        match k {
            NatureFieldKind::Quantity { which, name, attrs } => {
                let w = match which {
                    Quantity::Potential => "potential",
                    Quantity::Flow => "flow",
                };
                let block = if attrs.is_empty() { String::new() } else { format!(" {}", self.field_inits(attrs)) };
                format!("{w}: {}{block}", name.spelled())
            }
            NatureFieldKind::Value(t) => format!("value: {}", self.ty(t)),
        }
    }
}
