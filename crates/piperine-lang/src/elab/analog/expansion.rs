//! A `fn` called where an expression is differentiated (Part IV §14.2.3, analog-operators AO-01): the call is
//! expanded where it stands, so the derivative of `ddx(diode_current(v(a, k)), v(a, k))` is the derivative of the
//! body of `diode_current` with its parameters read as the arguments.
//!
//! A `ddx` value is lowered standalone (`derivatives.rs`): it holds no local, and its derivative is built over one
//! expression. A `fn` called there is no IR function (its id would be handed out after the derivative is built,
//! `variants.rs`) and no block with locals: its body is *run*, symbolically (`symbolic.rs`), over an environment that
//! maps each parameter and each `let` of the body to the lowered expression it holds, and what the run returns is
//! the one expression the call stands for. The call binds its parameters first: a value to its lowered argument (and
//! to the constant it folds to, so the decisions of a recursion fold), an `Option` to the pair of its flag and its
//! payload, a `str` to its text, a handle to the caller's node. The methods of a type, a method through a bound and
//! a generic `fn` expand the same way, for the arguments their instance takes.

use super::AnalogLowering;
use std::collections::BTreeMap;

use piperine_ir::{Constant, ExprId as LoweredExpr};

use crate::elab::consts::Bindings;
use crate::elab::elaborator::Scope;
use super::unroll::{DEPTH, Expansion};
use crate::refusal::forms::StructureForm;
use crate::msl::Span;
use crate::pom::{DeclId, DeclKind, ExprId};
use crate::elab::functions::instances::ConcreteValue;
use crate::types::{Callee, Substitution, Ty};

/// What the names of the `fn` being expanded stand for, taken before a call rebinds them and put back after it.
#[derive(Clone)]
pub(in crate::elab) struct Environment {
    /// A parameter or a `let`: the expression it holds.
    values: BTreeMap<DeclId, LoweredExpr>,
    /// An `Option` parameter: its flag and its payload.
    options: BTreeMap<DeclId, (LoweredExpr, LoweredExpr)>,
    /// A `str` parameter: its text.
    texts: BTreeMap<DeclId, String>,
    /// A handle parameter: the caller's node.
    handles: BTreeMap<DeclId, ExprId>,
    /// A name that folds to a constant here: a loop variable, a parameter whose argument folds.
    unrolled: Bindings,
}

impl Environment {
    /// The environment `scope` has now.
    pub(in crate::elab) fn of(scope: &Scope) -> Environment {
        Environment { values: scope.analog.values.clone(), options: scope.analog.option_values.clone(), texts: scope.digital.texts.clone(), handles: scope.digital.handles.clone(), unrolled: scope.values.unrolled.clone() }
    }

    /// Puts this environment back in `scope`.
    pub(in crate::elab) fn restore(self, scope: &mut Scope) {
        scope.analog.values = self.values;
        scope.analog.option_values = self.options;
        scope.digital.texts = self.texts;
        scope.digital.handles = self.handles;
        scope.values.unrolled = self.unrolled;
    }
}

/// One parameter of an expanded `fn`, evaluated where the call stands, before any parameter is rebound.
pub(in crate::elab) enum Argument {
    /// A value, and the constant it folds to when it does.
    Value { param: DeclId, value: LoweredExpr, folded: Option<Constant> },
    /// An `Option`: its flag and its payload.
    Option { param: DeclId, flag: LoweredExpr, payload: LoweredExpr },
    /// A `str`: its text.
    Text { param: DeclId, text: String },
    /// A handle: the caller's node.
    Handle { param: DeclId, target: ExprId },
}

impl Argument {
    /// Binds the parameter in `scope`.
    pub(in crate::elab) fn bind(self, scope: &mut Scope) {
        match self {
            Argument::Value { param, value, folded } => {
                scope.analog.values.insert(param, value);
                match folded {
                    Some(constant) => scope.values.unrolled.bind(param, constant),
                    None => {
                        scope.values.unrolled.unbind(param);
                    }
                }
            }
            Argument::Option { param, flag, payload } => {
                scope.analog.option_values.insert(param, (flag, payload));
            }
            Argument::Text { param, text } => {
                scope.digital.texts.insert(param, text);
            }
            Argument::Handle { param, target } => {
                scope.digital.handles.insert(param, target);
            }
        }
    }
}

impl AnalogLowering<'_, '_, '_> {
    /// A call of a plain or generic `fn` inside a differentiated value, expanded; `None` for a call that is no `fn` (a
    /// probe, a core primitive, an `extern` function, a module) and is lowered as it always is. `Some(None)` after a
    /// diagnostic.
    pub(in crate::elab) fn expand_function_call(&mut self, scope: &mut Scope, call: ExprId) -> Option<Option<LoweredExpr>> {
        let Some(Callee::Function(function)) = self.state.types.callee(call) else { return None };
        if matches!(&self.state.design.decl_data(function).kind, DeclKind::Fn(f) if f.abi.is_some()) {
            return None;
        }
        let Some(binding) = self.state.types.binding(call).map(<[_]>::to_vec) else {
            self.elaboration.refuse(scope.values.file, self.state.design.expr_node(call).span, StructureForm::CallWhoseArgumentsTypeCheckerDidNot);
            return Some(None);
        };
        // a generic `fn` runs for the arguments the checker read from the call
        if self.state.design.decl_data(function).generics_span.is_some() {
            let Some(instance) = self.functions().concrete_instance(call) else { return Some(None) };
            let substitution = self.instance_substitution(instance.item, &instance.arguments);
            return Some(self.expand_under(scope, call, instance.item, binding, Some(substitution)));
        }
        Some(self.expand_under(scope, call, function, binding, None))
    }

    /// The call `receiver.f(arguments)` of the method `function` inside a differentiated value, expanded for the
    /// `impl` arguments `instance` the receiver's type gives. `None` after a diagnostic.
    pub(in crate::elab) fn expand_method_call(&mut self, scope: &mut Scope, call: ExprId, function: DeclId, receiver: ExprId, arguments: &[ExprId], instance: Vec<ConcreteValue>) -> Option<LoweredExpr> {
        let slots: Vec<Option<ExprId>> = std::iter::once(receiver).chain(arguments.iter().copied()).map(Some).collect();
        let substitution = (!instance.is_empty()).then(|| self.instance_substitution(function, &instance));
        self.expand_under(scope, call, function, slots, substitution)
    }

    /// `expand` under the substitution of the instance, when there is one.
    fn expand_under(&mut self, scope: &mut Scope, call: ExprId, function: DeclId, slots: Vec<Option<ExprId>>, substitution: Option<Substitution>) -> Option<LoweredExpr> {
        let outer = substitution.map(|substitution| self.state.consts.set_substitution(substitution));
        let expanded = self.expand(scope, call, function, &slots);
        if let Some(outer) = outer {
            self.state.consts.set_substitution(outer);
        }
        expanded
    }

    /// The call `call` of `function` with the argument `slots` (one per parameter, `None` for one left out),
    /// expanded: its body run over the environment its parameters make (`symbolic.rs`). `None` after a diagnostic.
    fn expand(&mut self, scope: &mut Scope, call: ExprId, function: DeclId, slots: &[Option<ExprId>]) -> Option<LoweredExpr> {
        let span = self.state.design.expr_node(call).span;
        let data = self.state.design.decl_data(function).clone();
        let DeclKind::Fn(f) = &data.kind else { return None };
        let Some(body) = f.body.clone() else {
            self.refuse(scope.values.file, span, StructureForm::CallOfFunctionWithNoBodyInside);
            return None;
        };
        if self.state.types.emits(function).is_some() {
            self.refuse(scope.values.file, span, StructureForm::CallOfFunctionThatEmitsInsideDdx);
            return None;
        }
        if !self.may_expand(scope, function, span) {
            return None;
        }
        let arguments = self.expanded_arguments(scope, call, &data.children, slots)?;
        let before = Environment::of(scope);
        for argument in arguments {
            argument.bind(scope);
        }
        if scope.analog.expanding.is_empty() {
            scope.analog.forks = 0;
        }
        scope.analog.expanding.push(Expansion::begin(function, scope.values.decisions));
        let value = crate::stack::Stack::deep(|| self.run_body(scope, &body, span));
        scope.analog.expanding.pop();
        before.restore(scope);
        value
    }

    /// Whether the call of `function` may be expanded one level deeper: a recursion is followed to 1 024 calls and
    /// while the decisions it hangs on fold (`E0126` naming the function otherwise, §3 rule 12).
    pub(in crate::elab) fn may_expand(&mut self, scope: &Scope, function: DeclId, span: Span) -> bool {
        if scope.analog.expanding.len() >= DEPTH {
            self.unfoldable_recursion(scope, function, span, "it is deeper than 1 024 calls");
            return false;
        }
        if scope.analog.expanding.iter().any(|open| open.hangs_on_unfolded(function, scope.values.decisions)) {
            self.unfoldable_recursion(scope, function, span, "a decision it hangs on does not fold");
            return false;
        }
        true
    }

    /// Each parameter of `function` with its argument evaluated where the call stands. `None` after a diagnostic.
    pub(in crate::elab) fn expanded_arguments(&mut self, scope: &mut Scope, call: ExprId, children: &[DeclId], slots: &[Option<ExprId>]) -> Option<Vec<Argument>> {
        let span = self.state.design.expr_node(call).span;
        let params: Vec<DeclId> = children.iter().copied().filter(|c| matches!(self.state.design.decl_data(*c).kind, DeclKind::FnParam(_))).collect();
        let mut arguments = Vec::new();
        for (param, slot) in params.iter().zip(slots) {
            let argument = match (self.typed_local(*param), *slot) {
                (Some(Ty::Option(payload)), slot) => {
                    let [flag, payload] = self.values().lower_option_slot(scope, slot, &payload, call)?;
                    Argument::Option { param: *param, flag, payload }
                }
                (_, None) => {
                    self.refuse(scope.values.file, span, StructureForm::CallThatLeavesOutArgumentInsideDdx);
                    return None;
                }
                (_, Some(arg)) if self.state.types.handle(*param) => Argument::Handle { param: *param, target: self.functions().handle_argument_target(scope, arg) },
                (Some(Ty::Str), Some(arg)) => match self.text_of(scope, arg) {
                    Some(text) => Argument::Text { param: *param, text },
                    None => {
                        self.elaboration.refuse(scope.values.file, self.state.design.expr_node(arg).span, StructureForm::StrThatIsNotTextAfterExpansion);
                        return None;
                    }
                },
                (_, Some(arg)) => {
                    let folded = self.fold_scalar(scope, arg);
                    Argument::Value { param: *param, value: self.values().lower_expr(scope, arg)?, folded }
                }
            };
            arguments.push(argument);
        }
        Some(arguments)
    }

    /// The constant `expr` folds to over the frame's params and the names an expansion has bound; `None` when it is no
    /// scalar constant here.
    pub(super) fn fold_scalar(&mut self, scope: &Scope, expr: ExprId) -> Option<Constant> {
        let bindings = self.frame_bindings(scope.values.frame).overlaid(&scope.values.unrolled);
        match self.state.consts.value(expr, &bindings) {
            Ok(constant @ (Constant::Int(_) | Constant::Bool(_) | Constant::Real(_))) => Some(constant),
            _ => None,
        }
    }
}
