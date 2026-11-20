//! Continuity (part III §8.2): an expression is **continuous** when it reads a probe, `now()`,
//! `freq()`, `gmin()`, `source_scale()` or `analysis(Static)`, directly or through a `let`, an
//! argument of a function or the result of a placement; any other is **discrete**. A definition,
//! its delay, an expression at an `in` port and the argument of `at` must be discrete.
//!
//! The analysis is syntactic and runs over the POM: a name that stands for a value (`let`,
//! pattern binding) is followed to the expression that gave it its value.

use std::collections::BTreeMap;

use crate::pom::{CallClass, DeclId, DeclKind, Design, ExprId, ExprKind, PatId, PatKind, Resolution, StmtKind};

use super::check::Checker;

/// The `Resolution::Builtin` names whose read is continuous (§14.4, §8.2.5).
const CONTINUOUS_BUILTINS: [&str; 5] = ["now", "freq", "gmin", "source_scale", "analysis"];

/// What the checker knows of continuity, filled as it goes.
#[derive(Debug, Default)]
pub(super) struct Continuity {
    expressions: BTreeMap<ExprId, bool>,
    functions: BTreeMap<DeclId, bool>,
    /// The expression each binding stands for: a `let`, a pattern of an `if let`, a `match` arm.
    inits: BTreeMap<DeclId, ExprId>,
    /// What is being decided, so a cycle does not recurse for ever.
    open: Vec<ExprId>,
}

impl Continuity {
    pub(super) fn new(design: &Design) -> Self {
        let mut inits = BTreeMap::new();
        for i in 0..design.decl_count() {
            if let DeclKind::Let(l) = &design.decl_data(DeclId(i as u32)).kind {
                bind_pattern(design, l.pat, l.value, &mut inits);
            }
        }
        for i in 0..design.stmt_count() {
            if let StmtKind::Let { pat, init: Some(init), .. } = &design.stmt_node(crate::pom::StmtId(i as u32)).kind {
                bind_pattern(design, *pat, *init, &mut inits);
            }
        }
        for i in 0..design.expr_count() {
            match &design.expr_node(ExprId(i as u32)).kind {
                ExprKind::Let { pat, expr } => bind_pattern(design, *pat, *expr, &mut inits),
                ExprKind::Match { scrutinee, arms } => arms.iter().for_each(|a| bind_pattern(design, a.pat, *scrutinee, &mut inits)),
                _ => {}
            }
        }
        Continuity { expressions: BTreeMap::new(), functions: BTreeMap::new(), inits, open: Vec::new() }
    }

    /// Every expression decided continuous so far.
    pub(super) fn continuous_set(&self) -> impl Iterator<Item = ExprId> + '_ {
        self.expressions.iter().filter(|(_, c)| **c).map(|(e, _)| *e)
    }
}

/// Every binding of `pat` stands for `init`; the components of a tuple pattern for the matching
/// components of a tuple expression.
fn bind_pattern(design: &Design, pat: PatId, init: ExprId, out: &mut BTreeMap<DeclId, ExprId>) {
    match &design.pat_node(pat).kind {
        PatKind::Ident { binding, sub, .. } => {
            out.insert(*binding, init);
            if let Some(sub) = sub {
                bind_pattern(design, *sub, init, out);
            }
        }
        PatKind::Tuple(items) => match &design.expr_node(init).kind {
            ExprKind::Tuple(values) if values.len() == items.len() => items.iter().zip(values).for_each(|(p, v)| bind_pattern(design, *p, *v, out)),
            _ => items.iter().for_each(|p| bind_pattern(design, *p, init, out)),
        },
        PatKind::TupleStruct { pats: items, .. } | PatKind::Or(items) | PatKind::Slice(items) => items.iter().for_each(|p| bind_pattern(design, *p, init, out)),
        PatKind::Ref(inner) => bind_pattern(design, *inner, init, out),
        PatKind::Wild | PatKind::Rest | PatKind::Lit(_) | PatKind::Path(_) | PatKind::Struct { .. } | PatKind::Range { .. } => {}
    }
}

impl Checker<'_> {
    /// The expression a `let` or a pattern binding stands for.
    pub(super) fn binding_init(&self, binding: DeclId) -> Option<ExprId> {
        self.continuity.inits.get(&binding).copied()
    }

    /// Whether `expr` is a continuous expression.
    pub(super) fn continuous(&mut self, expr: ExprId) -> bool {
        if let Some(known) = self.continuity.expressions.get(&expr) {
            return *known;
        }
        if self.continuity.open.contains(&expr) {
            return false;
        }
        self.continuity.open.push(expr);
        let result = self.continuous_now(expr);
        self.continuity.open.pop();
        self.continuity.expressions.insert(expr, result);
        result
    }

    fn continuous_now(&mut self, expr: ExprId) -> bool {
        let node = self.design.expr_node(expr).clone();
        match &node.kind {
            ExprKind::Call { class: CallClass::Probe, .. } => return true,
            // `p.inflow()` is a probe of the ammeter branch (§6 rule 5).
            ExprKind::MethodCall { res: Resolution::Builtin(id), .. } if id.info().name == "inflow" => return true,
            ExprKind::Path(path) => {
                return match self.design.path_node(*path).segments.last().map(|s| s.res) {
                    Some(Resolution::Decl(d)) => match self.continuity.inits.get(&d).copied() {
                        Some(init) => self.continuous(init),
                        None => false,
                    },
                    _ => false,
                };
            }
            ExprKind::Call { callee, class, args, .. } => {
                if let ExprKind::Path(path) = &self.design.expr_node(*callee).kind {
                    match self.design.path_node(*path).segments.last().map(|s| s.res) {
                        // `analysis(Dc)` and the other kinds are constant during an analysis
                        // (§14.4.1): only `analysis(Static)` is continuous
                        Some(Resolution::Builtin(id)) if id.info().name == "analysis" && !self.names_static(args) => {}
                        Some(Resolution::Builtin(id)) if CONTINUOUS_BUILTINS.contains(&id.info().name) => return true,
                        Some(Resolution::Decl(f)) if matches!(self.design.decl_data(f).kind, DeclKind::Fn(_)) && self.function_continuous(f) => return true,
                        _ => {}
                    }
                }
                if let CallClass::Instance { module: Some(m), .. } = class
                    && self.result_continuous(*m)
                {
                    return true;
                }
            }
            _ => {}
        }
        crate::pom::expr_children(&node.kind, self.design).into_iter().any(|c| self.continuous(c))
    }

    /// Whether the argument of an `analysis` call is the kind `Static`.
    fn names_static(&self, args: &[crate::pom::Arg]) -> bool {
        let [arg] = args else { return true };
        match &self.design.expr_node(arg.value).kind {
            ExprKind::Path(path) => match self.design.path_node(*path).segments.last().map(|s| s.res) {
                Some(Resolution::Builtin(id)) => id.info().name == "Static",
                _ => true,
            },
            _ => true,
        }
    }

    /// Whether a function reads a continuous value in its own body.
    fn function_continuous(&mut self, function: DeclId) -> bool {
        if let Some(known) = self.continuity.functions.get(&function) {
            return *known;
        }
        self.continuity.functions.insert(function, false);
        let DeclKind::Fn(f) = &self.design.decl_data(function).kind else { return false };
        let Some(body) = f.body.clone() else { return false };
        let result = crate::pom::block_exprs(self.design, &body).into_iter().any(|e| self.continuous(e));
        self.continuity.functions.insert(function, result);
        result
    }

    /// Whether the result of a module reads a continuous value (`transition` reads `now()`).
    fn result_continuous(&mut self, module: DeclId) -> bool {
        let DeclKind::Module(m) = &self.design.decl_data(module).kind else { return false };
        match m.result {
            Some(result) => self.continuous(result),
            None => false,
        }
    }
}
