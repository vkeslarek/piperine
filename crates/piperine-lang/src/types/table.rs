//! The result table of a check, and the entry point that makes it.

use std::collections::BTreeMap;

use crate::msl::{Diagnostic, Span};
use crate::pom::{BuiltinId, DeclId, Design, DesignDiagnostic, ExprId, ExprKind, FileId, LitKind, Reflect};

use super::check;
use super::Ty;

/// What a `Deferred` method call of the reachable set resolves to: a row of the Appendix H
/// catalogue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Exp,
    Ln,
    Sqrt,
    Abs,
}

/// What a call or a placement of the reachable set refers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Callee {
    Function(DeclId),
    Core(BuiltinId),
    Module(DeclId),
}

/// The types of everything one top module reaches.
///
/// Equality compares the tables, not the borrowed design.
#[derive(Debug, Clone)]
pub struct Types<'d> {
    pub(super) design: &'d Design,
    pub(super) exprs: Vec<Option<Ty>>,
    pub(super) locals: BTreeMap<DeclId, Ty>,
    pub(super) methods: BTreeMap<ExprId, Method>,
    pub(super) callees: BTreeMap<ExprId, Callee>,
    pub(super) reached: Vec<DeclId>,
}

impl PartialEq for Types<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.exprs == other.exprs && self.locals == other.locals && self.methods == other.methods && self.callees == other.callees && self.reached == other.reached
    }
}

impl<'d> Types<'d> {
    /// Check `top`, a module of the root crate, and everything it reaches.
    ///
    /// A design that already holds diagnostics gives them back and nothing is checked; so does
    /// a top that is not a module of the root crate (one diagnostic naming it).
    pub fn check(design: &'d Design, top: &str) -> Result<Types<'d>, Vec<DesignDiagnostic>> {
        if !design.diagnostics().is_empty() {
            return Err(design.diagnostics().to_vec());
        }
        let Some(module) = design.module(top).filter(|m| m.krate().is_root()) else {
            let diagnostic = Diagnostic::new("P0003", format!("`{top}` is not a module of the root crate"), Span::new(0, 0), "no such top module");
            return Err(vec![DesignDiagnostic { file: FileId(0), diagnostic }]);
        };
        check::Checker::new(design).run(module.decl().id())
    }

    /// The design the table was built over.
    pub fn design(&self) -> &'d Design {
        self.design
    }

    /// The type of an expression; `None` outside the reachable set.
    pub fn ty(&self, expr: ExprId) -> Option<&Ty> {
        self.exprs.get(expr.index())?.as_ref()
    }

    /// The integer type an integer literal of the reachable set was given: its suffix, the type
    /// its position expects, else `i32` (§1 rule 7). `None` for any other expression.
    pub fn literal(&self, expr: ExprId) -> Option<&Ty> {
        let is_integer_literal = matches!(&self.design.expr_node(expr).kind, ExprKind::Lit(lit) if matches!(&lit.kind, LitKind::Int(text) if !text.ends_with("f64")));
        self.ty(expr).filter(|ty| is_integer_literal && ty.is_integer())
    }

    /// The catalogue row a `Deferred` method call resolved to.
    pub fn method(&self, expr: ExprId) -> Option<Method> {
        self.methods.get(&expr).copied()
    }

    /// What a call or a placement refers to.
    pub fn callee(&self, expr: ExprId) -> Option<Callee> {
        self.callees.get(&expr).copied()
    }

    /// The type of a `Binding` declaration.
    pub fn local(&self, binding: DeclId) -> Option<&Ty> {
        self.locals.get(&binding)
    }

    /// The modules, functions and constants checked, in first-reach order.
    pub fn reached(&self) -> &[DeclId] {
        &self.reached
    }
}
