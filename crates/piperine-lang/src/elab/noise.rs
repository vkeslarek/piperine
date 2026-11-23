//! `noise(psd, name)` (Part IV §14.8, analog-operators AO-03 and AO-04, design mechanism 3): one
//! source per call per frame, the `NOISE` input `noise:<k>`; every read of the call is
//! `Expr::Noise(k)`, 0 in every solve and the source's input in a noise sweep. The stdlib's
//! `white_noise` and `flicker_noise` are modules with a result, hoisted into the body, so they
//! run through this lowering with no rule of their own.
//!
//! `psd` is lowered *standalone*, as a derivative state is (§14 rule 9): the assembly evaluates
//! it in a noise sweep at the operating point and the point's frequency. `name` groups the
//! sources of a report; it is a literal found after inlining (an `Option<str>` input of a hoisted
//! module is the caller's), anything else is `P0001` naming it. Two sources with one name stay
//! two inputs: their powers add.

use piperine_ir::{Expr, ExprId as LoweredExpr, NoiseSource, Origin, ValueType};

use super::elaborator::Elaboration;
use super::lower::Scope;
use crate::pom::{ExprId, ExprKind, Resolution};

impl Elaboration<'_, '_> {
    /// A `noise` call read as a value: `Expr::Noise(k)`. The source is made the first time the call
    /// is read in the frame; `None` after a diagnostic.
    pub(super) fn lower_noise(&mut self, scope: &mut Scope, call: ExprId, args: &[ExprId], origin: Origin) -> Option<LoweredExpr> {
        if let Some(source) = self.frames[scope.frame].noise_sources.get(&call).copied() {
            return Some(self.emit(Expr::Noise(source), ValueType::F64, origin));
        }
        let span = self.design.expr_node(call).span;
        let [density, rest @ ..] = args else { return None };
        let mut standalone = Self::standalone_scope(scope);
        standalone.at_solution = true;
        let density = self.lower_expr(&mut standalone, *density)?;
        if !standalone.locals.is_empty() {
            self.unsupported(scope.file, span, "a closure or an `Option` binding inside a `noise` density");
            return None;
        }
        let group = self.noise_group(scope, rest.first().copied())?;
        let source = self.structure.noise_sources.len() as u32;
        self.structure.noise_sources.push(NoiseSource { name: format!("noise{source}"), group, density, origin: origin.clone() });
        self.frames[scope.frame].noise_sources.insert(call, source);
        Some(self.emit(Expr::Noise(source), ValueType::F64, origin))
    }

    /// The group name `arg` of a `noise` call stands for: `Some(None)` for one left out or `None`,
    /// `Some(Some(text))` for a literal (through `Some(..)`, parentheses, a `const` or a hoisted
    /// module's input the caller gave one), `None` after a diagnostic.
    fn noise_group(&mut self, scope: &mut Scope, arg: Option<ExprId>) -> Option<Option<String>> {
        let Some(arg) = arg else { return Some(None) };
        let node = self.design.expr_node(arg).clone();
        match &node.kind {
            ExprKind::Paren(inner) => return self.noise_group(scope, Some(*inner)),
            ExprKind::Path(path) => match self.design.path_node(*path).segments.last().map(|s| s.res) {
                Some(Resolution::Builtin(id)) if id.info().name == "None" => return Some(None),
                Some(Resolution::Decl(decl)) => match self.input_of(scope, decl) {
                    Some(None) => return Some(None),
                    Some(Some(given)) => return self.in_caller(scope, |this, scope| this.noise_group(scope, Some(given))),
                    None => {}
                },
                _ => {}
            },
            ExprKind::Call { args, .. } if self.is_some_call(arg) => return self.noise_group(scope, args.first().map(|a| a.value)),
            _ => {}
        }
        match self.text_of(scope, arg) {
            Some(text) => Some(Some(text)),
            None => {
                self.unsupported(scope.file, node.span, "a `noise` name that is not a text literal");
                None
            }
        }
    }
}
