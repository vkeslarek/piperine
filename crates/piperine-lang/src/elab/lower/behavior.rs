//! `analog`/`digital` body elaboration: a `BehaviorDecl` (parse AST) →
//! `Behavior` (POM). The body stays the surface [`Stmt`] type
//! (SIMPLIFICATION.md P3) — elaboration folds elaboration-constant `if`s,
//! unrolls behavioral `for`s, and records resolved `var` types in the
//! [`Behavior::var_types`] side table. No parallel statement enum.

use std::collections::HashMap;

use crate::elab::const_eval::ConstEnv;
use crate::parse::ast::{BehaviorDecl, BehaviorKind, Expr, ForIter, Literal, Stmt};
use crate::pom::{Behavior, ElabError, ElabErrorKind, ValueType};
use crate::value::Value;

use super::Elaborator;

/// Infer a `ValueType` from an initializer expression. Supports literals
/// (Real, Int, Bool, Quad) and identifier references (looks up the
/// already-resolved `var_types` from preceding declarations).
fn infer_value_type(expr: &Expr, var_types: &HashMap<String, ValueType>) -> Option<ValueType> {
    match expr {
        Expr::Literal(Literal::Real(_)) => Some(ValueType::Real),
        Expr::Literal(Literal::Int(_)) => Some(ValueType::Natural),
        Expr::Literal(Literal::Bool(_)) => Some(ValueType::Boolean),
        Expr::Literal(Literal::Quad(_)) => Some(ValueType::Quad),
        Expr::Literal(Literal::String(_)) => Some(ValueType::Str),
        Expr::Ident(name) => var_types.get(name).cloned(),
        Expr::Cast(target, _) => match target.as_str() {
            "real" => Some(ValueType::Real),
            "int" => Some(ValueType::Integer),
            "bit" | "Quad" => Some(ValueType::Quad),
            _ => None,
        },
        // Binary arithmetic: result type follows the left operand.
        Expr::Binary(lhs, op, _) => {
            use crate::parse::ast::BinaryOp::*;
            if matches!(op, Add | Sub | Mul | Div | Rem) {
                infer_value_type(lhs, var_types)
            } else {
                // Comparisons and logic produce Boolean.
                Some(ValueType::Boolean)
            }
        }
        Expr::Unary(_, inner) => infer_value_type(inner, var_types),
        _ => None,
    }
}

impl Elaborator {
    // ────────────────────────── Behavior elaboration ──────────────────────────

    /// Elaborates a `BehaviorDecl` into a `Behavior`.
    pub(crate) fn elab_behavior(&self, beh: &BehaviorDecl) -> Result<Behavior, ElabError> {
        let mut env = ConstEnv::new();
        let mut var_types = HashMap::new();
        let body = self.lower_behavior_stmts(&beh.body, beh.kind.clone(), &mut env, &mut var_types)?;
        Ok(Behavior { span: None, name: beh.name.clone(), kind: beh.kind.clone(), body, var_types })
    }

    /// Fold/unroll a slice of behavior statements, recording resolved
    /// `var` types into `var_types`.
    pub(crate) fn lower_behavior_stmts(
        &self,
        stmts: &[Stmt],
        kind: BehaviorKind,
        env: &mut ConstEnv,
        var_types: &mut HashMap<String, ValueType>,
    ) -> Result<Vec<Stmt>, ElabError> {
        let mut out = Vec::new();
        for stmt in stmts {
            out.extend(self.lower_stmt_to_behavior(stmt, kind.clone(), env, var_types)?);
        }
        Ok(out)
    }

    /// Lower one statement into zero or more elaborated statements:
    /// `var` types resolve (into the side table), elaboration-constant
    /// `if`s fold to the taken branch, behavioral `for`s unroll with the
    /// loop variable substituted (SPEC §10 — the `for` is syntactic
    /// sugar), event bodies lower recursively.
    pub(crate) fn lower_stmt_to_behavior(
        &self,
        stmt: &Stmt,
        kind: BehaviorKind,
        env: &mut ConstEnv,
        var_types: &mut HashMap<String, ValueType>,
    ) -> Result<Vec<Stmt>, ElabError> {
        match stmt {
            Stmt::VarDecl { name, ty, default, .. } => {
                // Type inference: if `ty` is omitted, infer from the
                // initializer expression. Supports literal inference
                // (`var x = 0.0;` → Real) and identifier inference
                // (`var x = other_var;` → type of other_var from
                // already-resolved var_types).
                let vt = if let Some(ty) = ty.as_ref() {
                    self.resolve_value_type(ty, env)?
                } else if let Some(init) = default.as_ref() {
                    infer_value_type(init, var_types).ok_or_else(|| ElabError::from(
                        ElabErrorKind::Other(format!(
                            "cannot infer type of `var {name}` from initializer — add an explicit type annotation"
                        ))
                    ))?
                } else {
                    return Err(ElabError::from(ElabErrorKind::Other(format!(
                        "`var {name}` needs either an explicit type or an initializer to infer from"
                    ))));
                };
                var_types.insert(name.clone(), vt);
                Ok(vec![stmt.clone()])
            }

            Stmt::Bind { .. } | Stmt::Diagnostic { .. } | Stmt::Return(_) | Stmt::Expr(_) => {
                Ok(vec![stmt.clone()])
            }

            Stmt::If { cond, then_body, else_body } => {
                // An elaboration-constant condition folds to the taken
                // branch (structural `if`, SPEC §10); anything else stays
                // a runtime `If`.
                match env.eval(cond) {
                    Ok(Value::Bool(true)) | Ok(Value::Nat(1)) => {
                        self.lower_behavior_stmts(&then_body.stmts, kind, env, var_types)
                    }
                    Ok(Value::Bool(false)) | Ok(Value::Nat(0)) => match else_body {
                        Some(eb) => self.lower_behavior_stmts(&eb.stmts, kind, env, var_types),
                        None => Ok(vec![]),
                    },
                    _ => {
                        let then_elab =
                            self.lower_behavior_stmts(&then_body.stmts, kind.clone(), env, var_types)?;
                        let else_elab = else_body
                            .as_ref()
                            .map(|eb| self.lower_behavior_stmts(&eb.stmts, kind.clone(), env, var_types))
                            .transpose()?;
                        Ok(vec![Stmt::If {
                            cond: cond.clone(),
                            then_body: block(then_elab),
                            else_body: else_elab.map(block),
                        }])
                    }
                }
            }

            Stmt::Match { expr, arms } => {
                let elab_arms = arms
                    .iter()
                    .map(|arm| {
                        let body =
                            self.lower_behavior_stmts(&arm.body.stmts, kind.clone(), env, var_types)?;
                        Ok(crate::parse::ast::StmtMatchArm { pat: arm.pat.clone(), body: block(body) })
                    })
                    .collect::<Result<Vec<_>, ElabError>>()?;
                Ok(vec![Stmt::Match { expr: expr.clone(), arms: elab_arms }])
            }

            Stmt::For { var, iter, body } => {
                let range = match iter {
                    ForIter::Range(r) => r,
                    ForIter::Expr(_) => {
                        return Err(ElabErrorKind::Other(format!(
                            "behavioral for-loop (var `{var}`) must have an elaboration-constant range (`a..b`), not a runtime iterable"
                        ))
                        .into());
                    }
                };
                let iter = super::eval_range(range, env, &format!("behavioral for-loop (var `{var}`)"))?;
                let mut out = Vec::new();
                for i in iter {
                    env.push();
                    env.define(var.clone(), Value::Nat(i));
                    // Unroll with the loop variable substituted by its
                    // concrete value — `rseg[i]` becomes `rseg[0]`, etc.
                    for s in &body.stmts {
                        let mut s = s.clone();
                        s.subst_const(var, i);
                        out.extend(self.lower_stmt_to_behavior(&s, kind.clone(), env, var_types)?);
                    }
                    env.pop();
                }
                Ok(out)
            }

            Stmt::Event { spec, guard, body } => {
                let elab_body = self.lower_behavior_stmts(&body.stmts, kind, env, var_types)?;
                Ok(vec![Stmt::Event {
                    spec: spec.clone(),
                    guard: guard.clone(),
                    body: block(elab_body),
                }])
            }
        }
    }
}

/// Statements → a value-less block (behavior bodies have no trailing expr).
fn block(stmts: Vec<Stmt>) -> crate::parse::ast::Block {
    crate::parse::ast::Block { stmts, expr: None }
}
