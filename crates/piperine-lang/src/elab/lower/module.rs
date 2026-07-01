//! `mod` body elaboration: a `ModDecl` (parse AST) → `Module` (POM) —
//! expanding ports, and lowering each `ModStmt` (param/wire/instance/
//! connection/`for`/`if`) into the structural pieces a `Module` owns.

use std::collections::HashMap;

use crate::parse::ast::ModDecl;
use crate::parse::ast::ModStmt;
use crate::elab::const_eval::{ConstEnv, ConstVal};
use crate::pom::{Connection, ElabError, Instance, Module, Param, Wire};

use super::Elaborator;

/// One item produced while lowering a `mod` body — sorted into the
/// `Module`'s `params`/`wires`/`instances`/`connections` once complete.
pub(crate) enum ModBodyItem {
    Param(Param),
    Wire(Wire),
    Inst(Instance),
    Conn(Connection),
}

impl Elaborator {
    // ─────────────────────────── Module elaboration ───────────────────────────

    /// Elaborates a `ModDecl` into a `Module`. Expands ports through
    /// [`expand_port`](Elaborator::expand_port), lowers the body statements
    /// into `ModBodyItem`s, then partitions them into `params`, `wires`,
    /// `instances`, and `connections`.
    pub(crate) fn elab_mod_inner(
        &mut self,
        decl: &ModDecl,
        env: &mut ConstEnv,
        type_subst: &HashMap<String, String>,
    ) -> Result<Module, ElabError> {
        let mut ports = Vec::new();
        for port in &decl.ports.clone() {
            ports.extend(self.expand_port(port, env, type_subst)?);
        }

        let mut items: Vec<ModBodyItem> = Vec::new();
        let body = decl.body.clone();
        self.lower_mod_stmts(&body, env, type_subst, &mut items)?;

        let mut params = Vec::new();
        let mut wires = Vec::new();
        let mut instances = Vec::new();
        let mut connections = Vec::new();

        for item in items {
            match item {
                ModBodyItem::Param(p) => params.push(p),
                ModBodyItem::Wire(w) => wires.push(w),
                ModBodyItem::Inst(i) => instances.push(i),
                ModBodyItem::Conn(c) => connections.push(c),
            }
        }

        Ok(Module { name: decl.name.clone(), ports, params, wires, instances, connections, behaviors: vec![] })
    }

    /// Lowers a slice of `ModStmt`s, appending the resulting
    /// `ModBodyItem`s to `out`. Delegates each statement to
    /// [`lower_mod_stmt`](Elaborator::lower_mod_stmt).
    pub(crate) fn lower_mod_stmts(
        &mut self,
        stmts: &[ModStmt],
        env: &mut ConstEnv,
        type_subst: &HashMap<String, String>,
        out: &mut Vec<ModBodyItem>,
    ) -> Result<(), ElabError> {
        for stmt in stmts {
            let stmt = stmt.clone();
            self.lower_mod_stmt(&stmt, env, type_subst, out)?;
        }
        Ok(())
    }

    /// Lowers a single `ModStmt` into zero or more `ModBodyItem`s appended
    /// to `out`. Handles: `ParamDecl`, `WireDecl`, `StructuralFor` (unrolled
    /// with const-evaluated range), `StructuralIf` (const-folded),
    /// `Instance` (triggering on-demand monomorphization for generic
    /// modules), and `Connection`.
    pub(crate) fn lower_mod_stmt(
        &mut self,
        stmt: &ModStmt,
        env: &mut ConstEnv,
        type_subst: &HashMap<String, String>,
        out: &mut Vec<ModBodyItem>,
    ) -> Result<(), ElabError> {
        match stmt {
            ModStmt::ParamDecl { name, ty, default } => {
                let vt = self.resolve_value_type(ty, env)?;
                let def = if let Some(e) = default {
                    Some(env.eval(e).map_err(|e| ElabError::ConstEval {
                        context: format!("param `{}` default", name),
                        source: e,
                    })?)
                } else {
                    None
                };
                out.push(ModBodyItem::Param(Param {
                    name: name.clone(),
                    ty: vt,
                    default: def,
                }));
            }

            ModStmt::WireDecl { name, ty } => {
                let nt = self.resolve_net_type(ty, env, type_subst)?;
                out.push(ModBodyItem::Wire(Wire { name: name.clone(), ty: nt }));
            }

            ModStmt::VarDecl { .. } => {
                // Vars in mod body appear in behavior; skip at structural level.
            }

            ModStmt::StructuralFor { var, range, body } => {
                let start = env.eval_nat(&range.start).map_err(|e| ElabError::ConstEval {
                    context: "for-loop start in module body".to_owned(),
                    source: e,
                })?;
                let end_val = env.eval_nat(&range.end).map_err(|e| ElabError::ConstEval {
                    context: "for-loop end in module body".to_owned(),
                    source: e,
                })?;
                let end = if range.inclusive { end_val + 1 } else { end_val };
                for i in start..end {
                    env.push();
                    env.define(var.clone(), ConstVal::Nat(i));
                    let body = body.clone();
                    self.lower_mod_stmts(&body, env, type_subst, out)?;
                    env.pop();
                }
            }

            ModStmt::StructuralIf { cond, then_body, else_body } => {
                let val = env.eval(cond).map_err(|e| ElabError::ConstEval {
                    context: "structural if condition".to_owned(),
                    source: e,
                })?;
                let taken = match val {
                    ConstVal::Bool(true) | ConstVal::Nat(1) => then_body.as_slice(),
                    ConstVal::Nat(n) if n != 0 => then_body.as_slice(),
                    _ => else_body.as_deref().unwrap_or(&[]),
                };
                let taken = taken.to_vec();
                self.lower_mod_stmts(&taken, env, type_subst, out)?;
            }

            ModStmt::Instance {
                name,
                array_index,
                module,
                const_args,
                type_args: _,
                ports,
                params,
            } => {
                let label = if let Some(n) = name {
                    if let Some(idx_expr) = array_index {
                        let idx = env.eval_nat(idx_expr).map_err(|e| ElabError::ConstEval {
                            context: format!("instance array index for `{}`", n),
                            source: e,
                        })?;
                        Some(format!("{}_{}", n, idx))
                    } else {
                        Some(n.clone())
                    }
                } else {
                    None
                };

                // Resolve const args to concrete values.
                let mut resolved_const_args: Vec<u64> = Vec::new();
                for ce in const_args {
                    let v = env.eval_nat(ce).map_err(|e| ElabError::ConstEval {
                        context: format!("const arg for module `{}`", module),
                        source: e,
                    })?;
                    resolved_const_args.push(v);
                }

                // Mangle module name with const args.
                let mono_name = if resolved_const_args.is_empty() {
                    module.clone()
                } else {
                    let suffix: Vec<String> =
                        resolved_const_args.iter().map(|n| n.to_string()).collect();
                    format!("{}__{}", module, suffix.join("_"))
                };

                // Trigger on-demand monomorphization so the module exists in the program.
                if !resolved_const_args.is_empty() {
                    self.monomorphize(module, &resolved_const_args)?;
                }

                // Resolve port connections to concrete net references.
                let elab_ports = ports
                    .iter()
                    .map(|p| self.eval_net_ref(p, env))
                    .collect::<Result<Vec<_>, _>>()?;

                // Resolve param overrides.
                let resolved_params: Vec<(String, ConstVal)> = params
                    .iter()
                    .map(|pa| {
                        let v = env.eval(&pa.expr).map_err(|e| ElabError::ConstEval {
                            context: format!("param `{}` of instance `{}`", pa.name, module),
                            source: e,
                        })?;
                        Ok((pa.name.clone(), v))
                    })
                    .collect::<Result<_, ElabError>>()?;

                out.push(ModBodyItem::Inst(Instance {
                    label,
                    module: mono_name,
                    ports: elab_ports,
                    params: resolved_params,
                }));
            }

            ModStmt::Connection { lhs, rhs } => {
                let lhs_ref = self.eval_net_ref(lhs, env)?;
                let rhs_ref = self.eval_net_ref(rhs, env)?;
                out.push(ModBodyItem::Conn(Connection { lhs: lhs_ref, rhs: rhs_ref }));
            }
        }
        Ok(())
    }

}
