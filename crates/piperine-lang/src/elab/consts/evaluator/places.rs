//! Assignment into a composite constant: a field, an element or a tuple item written in a const function (§1.5.12).

use piperine_ir::{Constant};
use super::{Bindings, ConstEval, ConstFault};
use crate::refusal::forms::{ConstantForm};
use crate::pom::{BinOp, DeclId, ExprId, ExprKind, Resolution};

/// One step into a composite from a place: a field by position, or an element by value.
pub(super) enum Step {
    /// The field at this position of a struct or tuple.
    Field(usize),
    /// The element an index expression names.
    Element(ExprId),
}

impl ConstEval<'_, '_> {
    /// `place = value;` over a field or an element: the local the place starts at takes the
    /// composite built again with that part replaced (§1.5.12, §1.4.6). `None` when the target is
    /// a plain name.
    pub(super) fn assign_into(&mut self, expr: ExprId, target: ExprId, op: Option<BinOp>, rhs: ExprId, bindings: &Bindings) -> Option<Result<(), ConstFault>> {
        let (local, steps) = self.place_steps(target)?;
        Some(self.write_place(expr, local, &steps, target, op, rhs, bindings))
    }

    /// The local and the steps a place expression names, outermost step last; `None` for a name.
    pub(super) fn place_steps(&self, target: ExprId) -> Option<(DeclId, Vec<Step>)> {
        let mut steps = Vec::new();
        let mut at = target;
        loop {
            match &self.design.expr_node(at).kind {
                ExprKind::Paren(inner) => at = *inner,
                ExprKind::Field { expr: recv, name, .. } => {
                    steps.push(Step::Field(self.field_position(*recv, &name.text)?));
                    at = *recv;
                }
                ExprKind::Index { expr: base, index } => {
                    steps.push(Step::Element(*index));
                    at = *base;
                }
                ExprKind::Path(path) => {
                    let Some(Resolution::Decl(local)) = self.design.path_node(*path).segments.last().map(|s| s.res) else { return None };
                    if steps.is_empty() {
                        return None;
                    }
                    steps.reverse();
                    return Some((local, steps));
                }
                _ => return None,
            }
        }
    }

    /// Evaluates `rhs` and writes it into `local` at `steps`.
    pub(super) fn write_place(&mut self, expr: ExprId, local: DeclId, steps: &[Step], target: ExprId, op: Option<BinOp>, rhs: ExprId, bindings: &Bindings) -> Result<(), ConstFault> {
        let value = self.fold(rhs, bindings)?;
        let assigned = match op {
            None => value,
            Some(op) => {
                let current = self.fold(target, bindings)?;
                self.known_binary(target, op, (target, current), (rhs, value))?
            }
        };
        let Some(root) = self.locals.get(&local).copied() else {
            return Err(ConstFault::refused(expr, ConstantForm::AssignmentIntoLocalWithNoValue));
        };
        let mut positions = Vec::new();
        for step in steps {
            positions.push(match step {
                Step::Field(at) => *at,
                Step::Element(index) => match self.fold(*index, bindings)? {
                    Constant::Int(at) if at >= 0 => at as usize,
                    _ => return Err(ConstFault::refused(*index, ConstantForm::ElementIndexThatIsNotNonNegative)),
                },
            });
        }
        let rebuilt = self.replaced(expr, root, &positions, assigned)?;
        self.locals.insert(local, rebuilt);
        Ok(())
    }

    /// `whole` with the part at `positions` replaced by `value`.
    pub(super) fn replaced(&mut self, expr: ExprId, whole: Constant, positions: &[usize], value: Constant) -> Result<Constant, ConstFault> {
        let Some((&first, rest)) = positions.split_first() else { return Ok(value) };
        let (kind, mut parts) = self.parts_of(expr, whole, "an assignment into a value")?;
        let Some(part) = parts.get(first).copied() else {
            return Err(ConstFault::Panic { expr, what: format!("the index {first} is out of range for an array of {} elements (§1.5.25)", parts.len()) });
        };
        parts[first] = self.replaced(expr, part, rest, value)?;
        Ok(self.make(kind, parts))
    }
}
