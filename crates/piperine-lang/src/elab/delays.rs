//! `delay(x, td, max)` (Part IV §14.6, analog-operators AO-02, design mechanism 2): one site per
//! call per frame, whose output is the private pin `delay<k>.value`; every read of the call is
//! `Expr::Delay(k)`, so `0.5 * delay(v(src), 1u)` needs no rule of its own.
//!
//! `x`, `td` and `max` are lowered *standalone*, as a derivative state is (§14 rule 9): a core
//! module's inputs are evaluated at every accepted solution whatever control flow surrounds the
//! call. Without `max`, `td` must be constant during an analysis (`E0452`, §14.6.2), judged on the
//! lowered `td` of each frame, after the value inputs are substituted; with `max`, `td` is free.
//! A `max` is a discrete pin written on the parameter cone: one that reads anything else is
//! `P0001` naming it (D6).

use piperine_ir::{DelaySite, Env, Expr, ExprId as LoweredExpr, Origin, ValueType};

use super::elaborator::Elaboration;
use super::lower::Scope;
use crate::msl::Diagnostic;
use crate::pom::ExprId;

impl Elaboration<'_, '_> {
    /// A `delay` call read as a value: `Expr::Delay(k)`. The site is made the first time the call
    /// is read in the frame; `None` after a diagnostic.
    pub(super) fn lower_delay(&mut self, scope: &mut Scope, call: ExprId, args: &[ExprId], origin: Origin) -> Option<LoweredExpr> {
        if let Some(site) = self.frames[scope.frame].delays.get(&call).copied() {
            return Some(self.emit(Expr::Delay(site), ValueType::F64, origin));
        }
        let span = self.design.expr_node(call).span;
        let [input, time, rest @ ..] = args else { return None };
        let mut standalone = Self::standalone_scope(scope);
        let input = self.lower_expr(&mut standalone, *input)?;
        let time_span = self.design.expr_node(*time).span;
        let time = self.lower_expr(&mut standalone, *time)?;
        let maximum = self.optional_constant(&mut standalone, rest.first().copied(), "a `delay` max")?;
        if !standalone.locals.is_empty() {
            self.unsupported(scope.file, span, "a `let` inside a `delay`");
            return None;
        }
        if maximum.is_none() && !self.constant_during_an_analysis(time) {
            let module = self.design.decl_data(self.frames[scope.frame].module).name.as_ref().map_or_else(String::new, |n| n.text.clone());
            let message = format!("`delay` in module `{module}` has no `max` and its `td` is not constant during an analysis (§14.6.2): give it a `max`, or write `td` over params and constants");
            self.report(scope.file, Diagnostic::new("E0452", message, time_span, "a `td` that varies, with no `max`"));
            return None;
        }
        let site = self.structure.delays.len() as u32;
        self.structure.delays.push(DelaySite { name: format!("delay{site}"), input, time, maximum, origin: origin.clone() });
        self.frames[scope.frame].delays.insert(call, site);
        Some(self.emit(Expr::Delay(site), ValueType::F64, origin))
    }

    /// Whether the lowered expression `expr` is constant during an analysis (§14.4.1): it reads
    /// only literals, params, the temperature, the kind of analysis and session parameters.
    pub(super) fn constant_during_an_analysis(&self, expr: LoweredExpr) -> bool {
        let mut open = vec![expr];
        let mut seen = std::collections::HashSet::new();
        while let Some(e) = open.pop() {
            if !seen.insert(e) {
                continue;
            }
            let Some(kind) = self.ir.expr_kind(e) else { return false };
            match kind {
                Expr::Real(_) | Expr::Bool(_) | Expr::Int(_) | Expr::Unit | Expr::Param(_) | Expr::SessionParameter { .. } | Expr::Env(Env::Temperature | Env::Analysis(_)) => {}
                Expr::Probe { .. } | Expr::Flow { .. } | Expr::Env(_) | Expr::Carrier(_) | Expr::Previous(_) | Expr::Fired(_) | Expr::Derivative(_) | Expr::Delay(_) | Expr::Noise(_) | Expr::Stim { .. } | Expr::Limit { .. } | Expr::Local(_) => return false,
                _ => {}
            }
            open.extend(kind.operands());
        }
        true
    }
}
