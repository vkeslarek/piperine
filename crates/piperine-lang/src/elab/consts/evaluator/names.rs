//! Names in a const expression: a path read as a param, a `const` item, a `let` of the body or a variant, and the trait items it reaches (§3.9).

use super::{Bindings, ConstEval, ConstFault};
use piperine_ir::Constant;
use crate::refusal::forms::{ConstantForm, ValueForm};
use crate::pom::{Design, DeclId, DeclKind, ExprId, ExprKind, PatKind, Resolution};
use crate::types::{ConstTerm, GenericValue};
use crate::elab::value::compound::EnumLayout;

impl ConstEval<'_, '_> {
    /// A name as the constant it is bound to (a `const`, a param, a binding of the call), or the
    /// level count of a `T::WIDTH`; a built-in name is dynamic.
    pub(super) fn path(&mut self, expr: ExprId, bindings: &Bindings) -> Result<Constant, ConstFault> {
        // `T::WIDTH`: the level count of `T`, which the checker recorded (§2, §22.1)
        if let Some(levels) = self.types.width(expr).or_else(|| self.parameter_levels(expr)) {
            return Ok(Constant::Int(i128::from(levels)));
        }
        let ExprKind::Path(path) = &self.design.expr_node(expr).kind else {
            return Err(ConstFault::refused(expr, ConstantForm::Name));
        };
        let res = self.design.path_node(*path).segments.last().map_or(Resolution::Unresolved, |s| s.res);
        let Resolution::Decl(decl) = res else {
            return Err(ConstFault::Dynamic { expr, what: "a built-in name".to_string() });
        };
        match &self.design.decl_data(decl).kind {
            DeclKind::Const(_) if self.open_constants.contains(&decl) => Err(ConstFault::Cyclic { expr, constant: decl }),
            DeclKind::Const(_) => self.fold_constant(decl).map_err(|fault| match fault {
                // A trait's `const` without a value is outside the subset; anywhere else it is `E0188`.
                ConstFault::NoValue { decl } if self.is_trait_item(decl) => ConstFault::refused(expr, ConstantForm::ConstWithoutValue),
                other => other,
            }),
            // a const parameter of the generic item being instantiated is bound to its argument (generics T33)
            // a const parameter of the instance being evaluated is the argument the call gave it (§3.8.3)
            DeclKind::GenericParam(_) => match (bindings.get(decl), self.substitution.get(decl)) {
                (Some(bound), _) => Ok(bound),
                (None, Some(GenericValue::Const(ConstTerm::Value(count)))) => Ok(Constant::Int(*count)),
                (None, Some(GenericValue::Const(ConstTerm::Bool(flag)))) => Ok(Constant::Bool(*flag)),
                _ => Err(ConstFault::UnboundParam { expr, param: decl }),
            },
            DeclKind::Param(_) => match bindings.get(decl) {
                // a composite param is an IR constant, which this evaluator's composites are not (`composite.rs`)
                Some(Constant::Aggregate(_)) => match bindings.tree(decl).cloned() {
                    Some(tree) => Ok(self.adopt(&tree)),
                    None => Err(ConstFault::refused(expr, ConstantForm::CompositeParamInConstExpression)),
                },
                Some(value) => Ok(value),
                None => Err(ConstFault::UnboundParam { expr, param: decl }),
            },
            // a module's bare name is the template `Name {}`, every param at its default (§8.4.1 amendment)
            DeclKind::Module(_) => self.template_literal(expr, decl, &[], bindings),
            // A unit struct, and a unit variant of an enum with fields, are held with their parts (`composite.rs`).
            DeclKind::Struct(_) | DeclKind::Variant(_) if let Some(value) = self.unit_value(decl) => Ok(value),
            // A unit variant is its tag (§2 rule 5).
            DeclKind::Variant(_) => EnumLayout::variant_tag(self.design, decl).map(|tag| Constant::Int(tag)).ok_or(ConstFault::refused(expr, ValueForm::VariantOutsideItsEnum)),
            DeclKind::FnParam(_) | DeclKind::Binding(_) => match (self.locals.get(&decl).copied(), bindings.get(decl)) {
                (Some(value), _) => Ok(value),
                // the loop variable of an unrolled analog loop, or the parameter of an unrolled
                // recursion, bound to its constant for this iteration or call (`unroll.rs`)
                (None, Some(Constant::Aggregate(_))) => Err(ConstFault::refused(expr, ConstantForm::CompositeValueBoundFromOutsideEvaluation)),
                (None, Some(value)) => Ok(value),
                (None, None) => self.body_let(expr, decl, bindings),
            },
            // A value input the caller binds to a constant (`structural.rs`, a selector through an input).
            DeclKind::Port(_) if let Some(Constant::Aggregate(_)) = bindings.get(decl) => Err(ConstFault::refused(expr, ConstantForm::CompositeValueInputInConstExpression)),
            DeclKind::Port(_) if let Some(value) = bindings.get(decl) => Ok(value),
            DeclKind::Port(_) | DeclKind::Signal(_) if self.types.carrier(decl).is_some() => Err(ConstFault::Dynamic { expr, what: "a carrier".to_string() }),
            DeclKind::Port(_) | DeclKind::Signal(_) => Err(ConstFault::Dynamic { expr, what: "a node".to_string() }),
            _ => Err(ConstFault::refused(expr, ConstantForm::NameInConstExpression)),
        }
    }

    /// The value of the module-body `let` name `decl` read at `expr`: its value expression evaluated
    /// over the same params (§8.1.19), so a param value may name a `let` that computes it. A `let`
    /// that reads a carrier, a probe, the environment or the result of an instance has no constant
    /// value: the fault names the `let`. A `let` that reads itself is one too.
    pub(super) fn body_let(&mut self, expr: ExprId, decl: DeclId, bindings: &Bindings) -> Result<Constant, ConstFault> {
        let Some(value) = Self::body_let_value(self.design, decl) else {
            return Err(ConstFault::Dynamic { expr, what: "a name with no value yet".to_string() });
        };
        let name = self.design.decl_data(decl).name.as_ref().map_or_else(String::new, |n| n.text.clone());
        if self.open_lets.contains(&decl) {
            return Err(ConstFault::Dynamic { expr, what: format!("the `let` `{name}`, which reads itself") });
        }
        self.open_lets.push(decl);
        let outer = std::mem::take(&mut self.locals);
        // a `let` written in generation: the iteration's value, or the array of them outside its loops (`generated.rs`)
        let result = self.generated_let(expr, decl, value, bindings);
        self.locals = outer;
        self.open_lets.pop();
        result.map_err(|fault| match fault {
            ConstFault::Dynamic { what, .. } => ConstFault::Dynamic { expr, what: format!("the `let` `{name}`, which reads {what}") },
            other => other,
        })
    }

    /// The value a module-body `let` binding stands for: the whole value of an identifier pattern,
    /// the matching component of a tuple pattern over a tuple expression.
    pub(in crate::elab) fn body_let_value(design: &Design, binding: DeclId) -> Option<ExprId> {
        let data = design.decl_data(binding);
        let DeclKind::Binding(b) = &data.kind else { return None };
        if b.origin != crate::pom::BindingOrigin::BodyLet {
            return None;
        }
        let DeclKind::Let(l) = &design.decl_data(data.parent?).kind else { return None };
        match &design.pat_node(l.pat).kind {
            PatKind::Ident { binding: bound, sub: None, .. } if *bound == binding => Some(l.value),
            PatKind::Tuple(items) => {
                let at = items.iter().position(|p| matches!(&design.pat_node(*p).kind, PatKind::Ident { binding: bound, sub: None, .. } if *bound == binding))?;
                let mut whole = l.value;
                while let ExprKind::Paren(inner) = &design.expr_node(whole).kind {
                    whole = *inner;
                }
                match &design.expr_node(whole).kind {
                    ExprKind::Tuple(values) if values.len() == items.len() => Some(values[at]),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// Whether `decl` is declared in a `trait` (the one place a `const` has no value, §3.9.2).
    pub fn is_trait_item(&self, decl: DeclId) -> bool {
        self.design.decl_data(decl).parent.is_some_and(|parent| matches!(self.design.decl_data(parent).kind, DeclKind::Trait(_)))
    }
}
