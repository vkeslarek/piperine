//! `analog`/`digital` body elaboration: a `BehaviorDecl` (parse AST) →
//! `Behavior` (POM), lowering each statement — including function-body
//! `Stmt`s inlined into a behavioral context — into `BehaviorStmt`.

use crate::parse::ast::{BehaviorDecl, BehaviorKind, BehaviorStmt as AstBehaviorStmt};
use crate::elab::const_eval::ConstEnv;
use crate::value::Value;
use crate::pom::{Behavior, BehaviorStmt, ElabError, ElabErrorKind, MatchArm};

use super::Elaborator;

impl Elaborator {
    // ────────────────────────── Behavior elaboration ──────────────────────────

    /// Elaborates a `BehaviorDecl` into a `Behavior`. Creates a fresh
    /// `ConstEnv` and lowers each body statement via
    /// [`lower_behavior_stmts`](Elaborator::lower_behavior_stmts).
    pub(crate) fn elab_behavior(&self, beh: &BehaviorDecl) -> Result<Behavior, ElabError> {
        let mut env = ConstEnv::new();
        let body = self.lower_behavior_stmts(&beh.body, beh.kind.clone(), &mut env)?;
        Ok(Behavior { name: beh.name.clone(), kind: beh.kind.clone(), body })
    }

    /// Lowers a slice of `AstBehaviorStmt`s into a `Vec<BehaviorStmt>`.
    /// Iterates over the statements and delegates each to
    /// [`lower_behavior_stmt`](Elaborator::lower_behavior_stmt).
    pub(crate) fn lower_behavior_stmts(
        &self,
        stmts: &[AstBehaviorStmt],
        kind: BehaviorKind,
        env: &mut ConstEnv,
    ) -> Result<Vec<BehaviorStmt>, ElabError> {
        let mut out = Vec::new();
        for stmt in stmts {
            self.lower_behavior_stmt(stmt, kind.clone(), env, &mut out)?;
        }
        Ok(out)
    }

    /// Lowers a single `AstBehaviorStmt` into one or more `BehaviorStmt`s
    /// appended to `out`. Handles `VarDecl`, `Bind` (contribution/assignment),
    /// `If` (const-folded when possible), `Match`, `For` (unrolled with
    /// const-evaluated range), `Event` (with body lowered via
    /// [`lower_stmt_to_behavior`](Elaborator::lower_stmt_to_behavior)),
    /// `Diagnostic`, and `Expr`.
    pub(crate) fn lower_behavior_stmt(
        &self,
        stmt: &AstBehaviorStmt,
        kind: BehaviorKind,
        env: &mut ConstEnv,
        out: &mut Vec<BehaviorStmt>,
    ) -> Result<(), ElabError> {
        match stmt {
            AstBehaviorStmt::VarDecl { name, ty, default } => {
                let vt = self.resolve_value_type(ty, env)?;
                out.push(BehaviorStmt::VarDecl {
                    name: name.clone(),
                    ty: vt,
                    default: default.clone(),
                });
            }

            AstBehaviorStmt::Bind { dest, op, src } => {
                out.push(BehaviorStmt::Bind {
                    dest: dest.clone(),
                    op: op.clone(),
                    src: src.clone(),
                });
            }

            AstBehaviorStmt::If { cond, then_body, else_body } => {
                let folded = match env.eval(cond) {
                    Ok(Value::Bool(true)) | Ok(Value::Nat(1)) => {
                        self.lower_behavior_stmts(then_body, kind.clone(), env)?
                    }
                    Ok(Value::Bool(false)) | Ok(Value::Nat(0)) => {
                        if let Some(eb) = else_body {
                            self.lower_behavior_stmts(eb, kind.clone(), env)?
                        } else {
                            vec![]
                        }
                    }
                    _ => {
                        let then_elab =
                            self.lower_behavior_stmts(then_body, kind.clone(), env)?;
                        let else_elab = if let Some(eb) = else_body {
                            Some(self.lower_behavior_stmts(eb, kind.clone(), env)?)
                        } else {
                            None
                        };
                        out.push(BehaviorStmt::If {
                            cond: cond.clone(),
                            then_body: then_elab,
                            else_body: else_elab,
                        });
                        return Ok(());
                    }
                };
                out.extend(folded);
            }

            AstBehaviorStmt::Match { expr, arms } => {
                let elab_arms = arms
                    .iter()
                    .map(|arm| {
                        let body =
                            self.lower_behavior_stmts(&arm.body, kind.clone(), env)?;
                        Ok(MatchArm { pat: arm.pat.clone(), body })
                    })
                    .collect::<Result<Vec<_>, ElabError>>()?;
                out.push(BehaviorStmt::Match { expr: expr.clone(), arms: elab_arms });
            }

            AstBehaviorStmt::For { var, range, body } => {
                let start = env.eval_nat(&range.start).map_err(|e| ElabErrorKind::ConstEval {
                    context: format!("behavioral for-loop start (var `{}`)", var),
                    source: e,
                })?;
                let end_val = env.eval_nat(&range.end).map_err(|e| ElabErrorKind::ConstEval {
                    context: format!("behavioral for-loop end (var `{}`)", var),
                    source: e,
                })?;
                let end = if range.inclusive { end_val + 1 } else { end_val };
                for i in start..end {
                    env.push();
                    env.define(var.clone(), Value::Nat(i));
                    // The `for` is syntactic sugar (SPEC §10): unroll with
                    // the loop variable substituted by its concrete value.
                    // Each iteration produces a fully-resolved copy of the
                    // body — `rseg[i]` becomes `rseg[0]`, `rseg[1]`, etc.
                    let mut unrolled_body: Vec<AstBehaviorStmt> = body
                        .iter()
                        .map(|s| {
                            let mut s = s.clone();
                            s.subst_const(var, i);
                            s
                        })
                        .collect();
                    // Also substitute in event block bodies (which use Block, not Vec<BehaviorStmt>).
                    for stmt in &mut unrolled_body {
                        if let AstBehaviorStmt::Event { body, .. } = stmt {
                            body.stmts.iter_mut().for_each(|s| s.subst_const(var, i));
                            if let Some(e) = &mut body.expr { e.subst_const(var, i); }
                        }
                    }
                    let lowered = self.lower_behavior_stmts(&unrolled_body, kind.clone(), env)?;
                    out.extend(lowered);
                    env.pop();
                }
            }

            AstBehaviorStmt::Event { spec, guard, body } => {
                let elab_body: Vec<BehaviorStmt> = body
                    .stmts
                    .iter()
                    .map(|s| self.lower_stmt_to_behavior(s, kind.clone(), env))
                    .collect::<Result<Vec<_>, _>>()?
                    .into_iter()
                    .flatten()
                    .collect();
                out.push(BehaviorStmt::Event {
                    spec: spec.clone(),
                    guard: guard.clone(),
                    body: elab_body,
                });
            }

            AstBehaviorStmt::Diagnostic { sys, args } => {
                out.push(BehaviorStmt::Diagnostic {
                    sys: sys.clone(),
                    args: args.clone(),
                });
            }

            AstBehaviorStmt::Expr(e) => {
                out.push(BehaviorStmt::Expr(e.clone()));
            }
        }
        Ok(())
    }

    /// Lower a function-body `Stmt` to behavior statements (used inside event
    /// blocks and function bodies).
    pub(crate) fn lower_stmt_to_behavior(
        &self,
        stmt: &crate::parse::ast::Stmt,
        kind: BehaviorKind,
        env: &mut ConstEnv,
    ) -> Result<Vec<BehaviorStmt>, ElabError> {
        use crate::parse::ast::Stmt;
        match stmt {
            Stmt::VarDecl { name, ty, default } => {
                // Type inference (an omitted `ty`) is only valid in an
                // interpreted `bench` body, which never reaches this
                // statically-elaborated path (SPEC Part I §9).
                let ty = ty.as_ref().ok_or_else(|| {
                    ElabError::from(ElabErrorKind::Other(format!(
                        "`var {name}` needs an explicit type outside a bench (type inference is bench-only)"
                    )))
                })?;
                let vt = self.resolve_value_type(ty, env)?;
                Ok(vec![BehaviorStmt::VarDecl {
                    name: name.clone(),
                    ty: vt,
                    default: default.clone(),
                }])
            }
            Stmt::Bind { dest, op, src } => Ok(vec![BehaviorStmt::Bind {
                dest: dest.clone(),
                op: op.clone(),
                src: src.clone(),
            }]),
            Stmt::If { cond, then_body, else_body } => {
                let then_elab = then_body
                    .stmts
                    .iter()
                    .map(|s| self.lower_stmt_to_behavior(s, kind.clone(), env))
                    .collect::<Result<Vec<_>, _>>()?
                    .into_iter()
                    .flatten()
                    .collect();
                let else_elab = if let Some(eb) = else_body {
                    Some(
                        eb.stmts
                            .iter()
                            .map(|s| self.lower_stmt_to_behavior(s, kind.clone(), env))
                            .collect::<Result<Vec<_>, _>>()?
                            .into_iter()
                            .flatten()
                            .collect(),
                    )
                } else {
                    None
                };
                Ok(vec![BehaviorStmt::If {
                    cond: cond.clone(),
                    then_body: then_elab,
                    else_body: else_elab,
                }])
            }
            Stmt::Match { expr, arms } => {
                let elab_arms = arms
                    .iter()
                    .map(|arm| {
                        let body = arm
                            .body
                            .stmts
                            .iter()
                            .map(|s| self.lower_stmt_to_behavior(s, kind.clone(), env))
                            .collect::<Result<Vec<_>, _>>()?
                            .into_iter()
                            .flatten()
                            .collect();
                        Ok(MatchArm { pat: arm.pat.clone(), body })
                    })
                    .collect::<Result<Vec<_>, ElabError>>()?;
                Ok(vec![BehaviorStmt::Match { expr: expr.clone(), arms: elab_arms }])
            }
            Stmt::For { var, iter, body } => {
                let range = match iter {
                    crate::parse::ast::ForIter::Range(r) => r,
                    crate::parse::ast::ForIter::Expr(_) => {
                        return Err(ElabErrorKind::Other(format!(
                            "for-loop in event block (var `{}`) must have an elaboration-constant range (`a..b`), not a runtime iterable",
                            var
                        ))
                        .into());
                    }
                };
                let start = env.eval_nat(&range.start).map_err(|e| ElabErrorKind::ConstEval {
                    context: format!("for-loop in event block (var `{}`)", var),
                    source: e,
                })?;
                let end_val = env.eval_nat(&range.end).map_err(|e| ElabErrorKind::ConstEval {
                    context: format!("for-loop end in event block (var `{}`)", var),
                    source: e,
                })?;
                let end = if range.inclusive { end_val + 1 } else { end_val };
                let mut unrolled = Vec::new();
                for i in start..end {
                    env.push();
                    env.define(var.clone(), Value::Nat(i));
                    for s in &body.stmts {
                        unrolled.extend(self.lower_stmt_to_behavior(s, kind.clone(), env)?);
                    }
                    env.pop();
                }
                Ok(unrolled)
            }
            // GAPS §D.5 — `Return(expr)` is preserved as a distinct
            // `Return` variant so the codegen can find the trailing
            // return value of a user `fn` body. The `Expr` arm keeps
            // bare expression statements (most common in behavior bodies).
            Stmt::Return(e) => Ok(vec![BehaviorStmt::Return(e.clone())]),
            Stmt::Expr(e) => Ok(vec![BehaviorStmt::Expr(e.clone())]),
        }
    }

}
