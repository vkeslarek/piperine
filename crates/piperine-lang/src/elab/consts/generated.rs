//! A module-body `let` written in generation, folded (part III §8.2.23, §8.2.24): read inside the generation `for`s that
//! declare it, whose loop variables the bindings hold, it is the value of that iteration; read outside a `for`, it is the
//! array of the values of the `for`'s iterations, one per ordinal, nested once for each such `for`. A `let` of a generation
//! `if` exists when its arm is taken: a read of one whose arm is not is `E0342`, and one that only some iterations of a
//! `for` elaborate would make an array with gaps, refused by name.

use piperine_ir::Constant;

use super::composite::CompositeKind;
use super::{Bindings, ConstEval, ConstFault};
use crate::refusal::forms::{ConstantForm, StructureForm};
use crate::pom::{DeclId, DeclKind, ExprId, ExprKind, GenElseNode, GenerationData, PatKind};

/// One generation around a `let`, outermost first, as the evaluation needs it.
#[derive(Clone, Copy)]
enum GenerationLevel {
    /// A `for` over `lo..hi` (`..=` when `inclusive`) whose variable is `variable`.
    For { variable: Option<DeclId>, lo: ExprId, hi: ExprId, inclusive: bool },
    /// An `if` whose condition is `cond`; the `let` is in its `then` arm when `then`, else in its `else` arm.
    If { cond: ExprId, then: bool },
}

impl ConstEval<'_, '_> {
    /// The value of the module-body `let` `binding` (its value `value`) read at `expr` over `bindings`.
    pub(super) fn generated_let(&mut self, expr: ExprId, binding: DeclId, value: ExprId, bindings: &Bindings) -> Result<Constant, ConstFault> {
        let levels = self.generation_levels(binding);
        self.fold_level(expr, binding, value, &levels, bindings, false)
    }

    /// The value of `value` under the generations `levels`, outermost first; `inside_for` when a `for` around them is being
    /// iterated here (an arm it skips is then a gap, not an item that does not exist).
    fn fold_level(&mut self, expr: ExprId, binding: DeclId, value: ExprId, levels: &[GenerationLevel], bindings: &Bindings, inside_for: bool) -> Result<Constant, ConstFault> {
        let Some((level, inner)) = levels.split_first() else { return self.fold(value, bindings) };
        match *level {
            // the reader is inside this `for`: its variable is the iteration's constant already
            GenerationLevel::For { variable: Some(variable), .. } if bindings.get(variable).is_some() => self.fold_level(expr, binding, value, inner, bindings, inside_for),
            GenerationLevel::For { variable, lo, hi, inclusive } => {
                let (first, last) = (self.integer_bound(expr, lo, bindings)?, self.integer_bound(expr, hi, bindings)?);
                let mut parts = Vec::new();
                for at in first..last + i128::from(inclusive) {
                    let mut iteration = bindings.clone();
                    if let Some(variable) = variable {
                        iteration.bind(variable, Constant::Int(at));
                    }
                    parts.push(self.fold_level(expr, binding, value, inner, &iteration, true)?);
                }
                Ok(self.make(CompositeKind::Array, parts))
            }
            GenerationLevel::If { cond, then } => match self.fold(cond, bindings)? {
                Constant::Bool(taken) if taken == then => self.fold_level(expr, binding, value, inner, bindings, inside_for),
                _ if inside_for => Err(ConstFault::refused(expr, StructureForm::LetDeclaredInGenerationIfThatNot)),
                _ => {
                    let name = self.design.decl_data(binding).name.as_ref().map_or_else(String::new, |n| n.text.clone());
                    Err(ConstFault::Ungenerated { expr, message: format!("`{name}` is declared in an arm of a generation that was not elaborated (§8.2.24)") })
                }
            },
        }
    }

    /// The integer a bound of a generation range folds to.
    fn integer_bound(&mut self, expr: ExprId, bound: ExprId, bindings: &Bindings) -> Result<i128, ConstFault> {
        match self.fold(bound, bindings)? {
            Constant::Int(value) => Ok(value),
            _ => Err(ConstFault::refused(expr, ConstantForm::GenerationRangeWhoseBoundIsNotInteger)),
        }
    }

    /// The generations around the `let` that binds `binding`, outermost first; empty outside generation.
    fn generation_levels(&self, binding: DeclId) -> Vec<GenerationLevel> {
        let mut levels = Vec::new();
        let Some(mut child) = self.design.decl_data(binding).parent else { return levels };
        while let Some(parent) = self.design.decl_data(child).parent {
            match &self.design.decl_data(parent).kind {
                DeclKind::Generation(GenerationData::For { pat, iter, .. }) => {
                    let ExprKind::Range { lo: Some(lo), hi: Some(hi), inclusive } = self.design.expr_node(*iter).kind else { break };
                    let variable = match self.design.pat_node(*pat).kind {
                        PatKind::Ident { binding, sub: None, .. } => Some(binding),
                        _ => None,
                    };
                    levels.push(GenerationLevel::For { variable, lo, hi, inclusive });
                }
                DeclKind::Generation(GenerationData::If { cond, then, else_ }) => {
                    let in_then = then.contains(&child);
                    let in_else = matches!(else_, Some(GenElseNode::Block(items)) if items.contains(&child)) || matches!(else_, Some(GenElseNode::If(next)) if *next == child);
                    if in_then || in_else {
                        levels.push(GenerationLevel::If { cond: *cond, then: in_then });
                    }
                }
                DeclKind::Module(_) => break,
                _ => {}
            }
            child = parent;
        }
        levels.reverse();
        levels
    }
}
