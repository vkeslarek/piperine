//! `simparam(name)` (Part IV §14.4, analog-operators AO-07, design mechanism 5): the session
//! parameter `name`, an `Option<f64>` constant during an analysis: `Some` of what the run gave it,
//! else `None` (user decision 2026-11-23: no default argument, no `E0456`; a model writes
//! `simparam("x").unwrap_or(1.0)`). It lowers to `Expr::SessionParameter`, one `SessionParameter`
//! per name per body, wherever a run-time value is read, definitions and selectors included
//! (§14.4.1). `name` is a literal found after inlining; anything else is `P0001` naming it.
//! `lower_session_pair` gives the two halves (`is`, `value`) an `Option` operation reads.

use piperine_ir::{Expr, ExprId as LoweredExpr, LevelPart, Origin, SessionParameter, ValueType};

use super::compound::Pair;
use super::elaborator::Elaboration;
use super::lower::Scope;
use crate::pom::ExprId;

impl Elaboration<'_, '_> {
    /// A `simparam` call read as a value: the packed `Option<f64>` read; `None` after a diagnostic.
    pub(super) fn lower_simparam(&mut self, scope: &mut Scope, args: &[ExprId], origin: Origin) -> Option<LoweredExpr> {
        let [name] = args else { return None };
        let span = self.design.expr_node(*name).span;
        let Some(text) = self.text_of(scope, *name) else {
            self.unsupported(scope.file, span, "a `simparam` name that is not a text literal");
            return None;
        };
        let parameter = match self.structure.session_parameters.iter().position(|p| p.name == text) {
            Some(at) => at,
            None => {
                self.structure.session_parameters.push(SessionParameter { name: text, origin: origin.clone() });
                self.structure.session_parameters.len() - 1
            }
        };
        Some(self.emit(Expr::SessionParameter { parameter: parameter as u32 }, ValueType::Option(Box::new(ValueType::F64)), origin))
    }

    /// The pair an `Option` operation reads of a `simparam` call: each half unpacked from the read.
    pub(super) fn lower_session_pair(&mut self, scope: &mut Scope, call: ExprId, origin: Origin) -> Option<Pair> {
        let crate::pom::ExprKind::Call { args, .. } = self.design.expr_node(call).kind.clone() else { return None };
        let args: Vec<ExprId> = args.iter().map(|a| a.value).collect();
        let packed = self.lower_simparam(scope, &args, origin.clone())?;
        let is = self.emit(Expr::OptionPart { option: packed, part: LevelPart::Is }, ValueType::Bool, origin.clone());
        let packed = self.lower_simparam(scope, &args, origin.clone())?;
        let val = self.emit(Expr::OptionPart { option: packed, part: LevelPart::Value }, ValueType::F64, origin);
        Some(Pair { is, val, literal: None })
    }
}
