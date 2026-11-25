//! `inst.name` of a value item through a child of a child (§8.2.8, §8.4.8): `bank.c0.cs`, a param of the module `c0` places
//! (by name or through a template), or a `pub let` of it that reads only its params. The module of `bank` is fixed by the
//! body's keys and the template it binds, so the grandchild's params are constants of the body being lowered: the value is
//! folded, never reached; a grandchild placed in generation is read at the iteration its indexes name (`bank.cells[1][2].cs`).
//! A `pub let` that reads the solve and a carrier of a grandchild are not constants of the body and are refused by name
//! (`GAPS.md` L81).

use piperine_ir::{Constant, ExprId as LoweredExpr, Origin, ValueType};

use super::consts::Bindings;
use super::elaborator::Elaboration;
use super::lower::Scope;
use super::reach_through_templates::NestedDemand;
use crate::pom::{DeclId, DeclKind, ExprId, ExprKind};

impl Elaboration<'_, '_> {
    /// The value `bank.c0.x` reads when `x` is a param, or a `pub let` over params, of the module `c0` places in the module of
    /// `bank`. `None` when `expr` is no read through a child of a child; `Some(None)` after a diagnostic.
    pub(super) fn nested_value(&mut self, scope: &Scope, expr: ExprId, ty: ValueType, origin: Origin) -> Option<Option<LoweredExpr>> {
        let ExprKind::Field { expr: receiver, name, .. } = self.design.expr_node(expr).kind.clone() else { return None };
        let (_, child) = self.child_of_module(receiver)?;
        Some(self.nested_constant(scope, expr, receiver, child, &name.text).map(|value| self.constant_expr(value, ty, origin)))
    }

    /// The constant of the item `item` of the child `child` of the child `receiver` names (`bank.c0`): its param, or its `pub let`
    /// folded over its params, with the type the read was given checked. `None` after a diagnostic.
    fn nested_constant(&mut self, scope: &Scope, expr: ExprId, receiver: ExprId, child: DeclId, item: &str) -> Option<Constant> {
        let span = self.design.expr_node(expr).span;
        let mut owner = self.peel_parens(receiver);
        let mut indexes = Vec::new();
        while let ExprKind::Index { expr: inner, index } = self.design.expr_node(owner).kind.clone() {
            indexes.push(index);
            owner = self.peel_parens(inner);
        }
        indexes.reverse();
        let ExprKind::Field { expr: bank, .. } = self.design.expr_node(owner).kind.clone() else { return None };
        let (binding, key) = self.receiver_placement(scope, bank)?;
        let frame = self.placement_frame(scope, binding, key, span)?;
        let mut keys = self.frame_bindings(frame);
        // `bank.cells[r][c].cs`: the iteration the indexes name, its loop variables bound for the placement's values (§8.2.23)
        let call = self.peel_parens(self.body_let_value(child)?);
        if let Some(values) = self.named_iteration(scope, frame, call, child, &indexes)? {
            for (variable, value) in values {
                keys.bind(variable, Constant::Int(value));
            }
        }
        let demand = NestedDemand { child, item: item.to_string() };
        let Some(module) = self.concrete_module_of(&keys, &demand) else {
            self.unsupported(scope.file, span, "an item of a child whose module is not known");
            return None;
        };
        let decl = self.visible_item_of(scope, module, item, span)?;
        self.read_has_its_type(scope, expr, module, decl, &crate::types::Substitution::new())?;
        let params = self.grandchild_params(scope, &keys, call, module)?;
        let folded = match self.design.decl_data(decl).kind.clone() {
            DeclKind::Param(_) => params.get(decl).ok_or(()),
            DeclKind::Binding(_) => self.body_let_value(decl).ok_or(()).and_then(|value| self.consts.value(value, &params).map_err(|_| ())),
            _ => Err(()),
        };
        match folded {
            Ok(value) => Some(value),
            Err(()) => {
                self.unsupported(scope.file, span, "an item of a child's child that is not a param or a `let` over its params");
                None
            }
        }
    }

    /// The loop variables of the iteration `indexes` name of the placement `call` (bound to `child`), which the child of the body
    /// at `frame` places in generation: `Some(None)` for a placement outside generation, `E0342` for an iteration its loops do
    /// not make, and a named refusal for a read that does not index every loop. `None` after a diagnostic.
    fn named_iteration(&mut self, scope: &Scope, frame: usize, call: ExprId, child: DeclId, indexes: &[ExprId]) -> Option<Option<Vec<(DeclId, i128)>>> {
        let owner = self.design.expr_node(call).owner;
        let loops = self.generation_loops(owner).len();
        if loops == 0 && indexes.is_empty() {
            return Some(None);
        }
        let span = self.design.expr_node(call).span;
        if indexes.len() != loops {
            self.unsupported(scope.file, span, "an item of a child placed in generation, read without naming its iteration: index every loop");
            return None;
        }
        let bindings = self.frame_bindings(scope.frame).overlaid(&scope.unrolled);
        let mut ordinals = Vec::new();
        for index in indexes {
            match self.consts.value(*index, &bindings) {
                Ok(Constant::Int(ordinal)) if ordinal >= 0 => ordinals.push(ordinal as u64),
                _ => {
                    self.not_folded(scope, self.design.expr_node(*index).span, "the index of a generated instance");
                    return None;
                }
            }
        }
        let iterations = self.iterations_under(frame, owner);
        let held: Vec<Vec<u64>> = iterations.iter().map(|iteration| self.iteration_ordinals(owner, iteration)).collect();
        match held.iter().position(|each| *each == ordinals) {
            Some(at) => Some(Some(iterations[at].values.clone())),
            None => {
                let missing = Self::outermost_missing_ordinal(&held, &ordinals);
                self.refer_outside_generation(scope.file, span, child, Some(missing));
                None
            }
        }
    }

    /// The value of every param of the module `module` the placement `call` places, in the body of a child whose params hold
    /// `keys`: the template's (§8.4.8) or the placement's block, else the default over the params before it (§8.1.16).
    fn grandchild_params(&mut self, scope: &Scope, keys: &Bindings, call: ExprId, module: DeclId) -> Option<Bindings> {
        let mut within = Scope::new(scope.file);
        within.keys = keys.clone();
        let ExprKind::Call { class, .. } = self.design.expr_node(call).kind.clone() else { return None };
        if let Some(template) = self.template_through(&within, call, &class).ok()? {
            return match self.consts.template_bindings(&template) {
                Ok(bound) => Some(bound),
                Err(fault) => {
                    self.const_fault(&fault, "a param of a template");
                    None
                }
            };
        }
        let fields = self.placement_fields(call, module);
        let mut bound = Bindings::new();
        for param in self.params_of(module) {
            let DeclKind::Param(data) = self.design.decl_data(param).kind.clone() else { continue };
            let value = match fields.iter().find(|field| field.res == crate::pom::Resolution::Decl(param)) {
                Some(field) => self.fold_param_value(field.value, keys),
                None => match data.default {
                    Some(default) => self.fold_param_value(default, &bound),
                    None => continue,
                },
            };
            match value {
                Ok(value) => self.bind_param(&mut bound, param, value),
                Err(fault) => {
                    self.const_fault(&fault, "a param of a child's child");
                    return None;
                }
            }
        }
        Some(bound)
    }
}
