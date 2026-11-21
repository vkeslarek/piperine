//! Compound value types (DIG-03): unit enums (part I §2 rule 5), `Option<T>` with `Some`,
//! `None` and the implicit `Some` of an argument (§3.2.3), and tuple `let`s.
//!
//! An enum of the subset has unit variants only and derives `Repr`: its tag is its value. A
//! variant with a payload, an explicit discriminant or a `#[repr]` attribute is outside the
//! subset and refused by name. A tuple is the initializer of a tuple `let` and nothing else:
//! the elaborator gives each component its own local.

use crate::msl::{Diagnostic, Span};
use crate::pom::{Arg, AttrArgNode, AttrInputNode, DeclId, DeclKind, ExprId, ExprKind, FieldsForm, FileId, GenericArgNode, GenericArgsNode, PathId, Resolution};

use super::check::Checker;
use super::Ty;

impl Checker<'_> {
    /// The type of the enum `decl` named at `span` of `file`: `Ty::Enum` when it is in the
    /// subset, else one `P0001` naming what is not, and `None`.
    pub(super) fn enum_type(&mut self, decl: DeclId, file: FileId, span: Span) -> Option<Ty> {
        let data = self.design.decl_data(decl).clone();
        let name = data.name.as_ref().map_or_else(String::new, |n| n.text.clone());
        if data.generics_span.is_some() {
            self.unsupported(file, span, &format!("generic enum `{name}`"));
            return None;
        }
        if self.has_attr(decl, "repr") {
            self.unsupported(file, span, &format!("enum `{name}` with a `#[repr]` attribute"));
            return None;
        }
        for variant in data.children.iter().copied() {
            let v = self.design.decl_data(variant);
            let DeclKind::Variant(vd) = &v.kind else { continue };
            let vname = v.name.as_ref().map_or("", |n| n.text.as_str());
            if vd.form != FieldsForm::Unit {
                self.unsupported(file, span, &format!("enum `{name}` with a variant with a payload (`{vname}`)"));
                return None;
            }
            if vd.discriminant.is_some() {
                self.unsupported(file, span, &format!("enum `{name}` with an explicit discriminant (`{vname}`)"));
                return None;
            }
        }
        if !self.derives(decl, "Repr") {
            self.unsupported(file, span, &format!("enum `{name}` without `#[derive(Repr)]`"));
            return None;
        }
        Some(Ty::Enum(decl))
    }

    /// Whether `decl` carries `#[derive(.., trait_name, ..)]`.
    pub(super) fn derives(&self, decl: DeclId, trait_name: &str) -> bool {
        self.design.decl_data(decl).meta.attrs.iter().any(|attr| {
            let is_derive = matches!(self.design.path_node(attr.path).segments.as_slice(), [only] if only.name.text == "derive");
            is_derive
                && matches!(&attr.input, AttrInputNode::List(args) if args.iter().any(|arg| matches!(arg, AttrArgNode::Path(p) if self.design.path_node(*p).segments.last().is_some_and(|s| s.name.text == trait_name))))
        })
    }

    /// Whether `decl` carries an attribute whose path is the one word `name`.
    fn has_attr(&self, decl: DeclId, name: &str) -> bool {
        self.design.decl_data(decl).meta.attrs.iter().any(|attr| matches!(self.design.path_node(attr.path).segments.as_slice(), [only] if only.name.text == name))
    }

    /// `Option<T>` written with its one type argument.
    pub(super) fn option_type(&mut self, file: FileId, span: Span, path: PathId) -> Option<Ty> {
        let args = self.design.path_node(path).segments.last().and_then(|s| s.args.clone());
        match &args {
            Some(GenericArgsNode::Angle { args, .. }) if args.len() == 1 => match &args[0] {
                GenericArgNode::Type(t) => Some(Ty::Option(Box::new(self.lower_type(*t)?))),
                GenericArgNode::Const(_) => {
                    self.unsupported(file, span, "`Option` of a const argument");
                    None
                }
            },
            _ => {
                self.unsupported(file, span, "`Option` without one type argument");
                None
            }
        }
    }

    /// `Some(e)`: `Option` of the type of `e`, or of `inner` when the position expects
    /// `Option<inner>`.
    pub(super) fn some(&mut self, expr: ExprId, args: &[Arg], inner: Option<&Ty>) -> Option<Ty> {
        let file = self.file_of_expr(expr);
        let [arg] = args else {
            let span = self.design.expr_node(expr).span;
            let message = format!("`Some` takes 1 argument, found {}", args.len());
            self.report(file, Diagnostic::new("P0002", format!("mismatched types: {message}"), span, message));
            return None;
        };
        match inner {
            Some(inner) => {
                self.check(arg.value, inner);
                Some(Ty::Option(Box::new(inner.clone())))
            }
            None => Some(Ty::Option(Box::new(self.infer(arg.value)?))),
        }
    }

    /// Whether `expr` is the built-in `None`, through parens.
    pub(super) fn is_none_literal(&self, expr: ExprId) -> bool {
        match &self.design.expr_node(expr).kind {
            ExprKind::Paren(inner) => self.is_none_literal(*inner),
            ExprKind::Path(path) => matches!(self.design.path_node(*path).segments.last().map(|s| s.res), Some(Resolution::Builtin(id)) if id.info().name == "None"),
            _ => false,
        }
    }

    /// Whether `expr` is a call of the built-in `Some`, through parens.
    fn is_some_call(&self, expr: ExprId) -> bool {
        match &self.design.expr_node(expr).kind {
            ExprKind::Paren(inner) => self.is_some_call(*inner),
            ExprKind::Call { callee, .. } => matches!(self.callee_resolution(*callee), Some(Resolution::Builtin(id)) if id.info().name == "Some"),
            _ => false,
        }
    }

    /// An argument for a parameter of type `Option<inner>` (§3.2.3): an `Option` as it is, or a
    /// value of type `inner`, read as `Some` of it and recorded so.
    pub(super) fn option_argument(&mut self, expr: ExprId, expected: &Ty, inner: &Ty) {
        if self.is_none_literal(expr) || self.is_some_call(expr) {
            self.check(expr, expected);
            return;
        }
        if self.integer_literal_waits(expr) {
            self.check(expr, inner);
            self.somes.insert(expr);
            return;
        }
        let Some(found) = self.infer(expr) else { return };
        if found == *expected {
            return;
        }
        if found == *inner {
            self.somes.insert(expr);
            return;
        }
        let file = self.file_of_expr(expr);
        let span = self.design.expr_node(expr).span;
        self.mismatch(file, span, expected, &found);
    }
}
