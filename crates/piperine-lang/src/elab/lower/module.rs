//! `mod` body elaboration: a `ModDecl` (parse AST) → `Module` (POM) —
//! expanding ports, and lowering each `ModStmt` (param/wire/instance/
//! connection/`for`/`if`) into the structural pieces a `Module` owns.

use std::collections::HashMap;

use crate::parse::ast::{Expr, ModDecl};
use crate::parse::ast::ModStmt;
use crate::elab::const_eval::{ConstEnv, ConstVal};
use crate::pom::{Connection, ElabError, Instance, Module, Param, Var, Wire};

use super::Elaborator;

/// One item produced while lowering a `mod` body — sorted into the
/// `Module`'s `params`/`wires`/`instances`/`connections` once complete.
pub(crate) enum ModBodyItem {
    Param(Param),
    Wire(Wire),
    ModVar(Var),
    Inst(Instance),
    Conn(Connection),
}

impl Elaborator {
    // ─────────────────────────── Bundle-typed params ──────────────────────────

    /// Flattens a bundle-typed `param` declaration into one scalar
    /// [`Param`] per bundle field, named `{pname}_{field}` (GAPS §I.14).
    ///
    /// `default`, if present, must be a bundle literal of the same type
    /// (`DioModel {}` or `DioModel { .is = 1e-12, .. }`); each named field
    /// in the literal overrides that field's own default. A field with no
    /// override and no bundle-level default is a fail-loud error — there
    /// is nothing to flatten it to.
    fn flatten_bundle_param(
        &mut self,
        pname: &str,
        bundle_name: &str,
        default: Option<&Expr>,
        env: &ConstEnv,
        out: &mut Vec<ModBodyItem>,
    ) -> Result<(), ElabError> {
        let bundle = self
            .bundles
            .get(bundle_name)
            .cloned()
            .ok_or_else(|| ElabError::UnknownBundle(bundle_name.to_owned()))?;

        let overrides: HashMap<String, Expr> = match default {
            None => HashMap::new(),
            Some(Expr::BundleLit { ty, fields }) if ty.name == bundle_name => {
                fields.iter().cloned().collect()
            }
            Some(other) => {
                let found = match other {
                    Expr::BundleLit { ty, .. } => ty.name.clone(),
                    _ => "a non-bundle-literal expression".to_owned(),
                };
                return Err(ElabError::BundleParamDefault {
                    param: pname.to_owned(),
                    expected: bundle_name.to_owned(),
                    found,
                });
            }
        };

        // Every literal field must name a field the bundle actually has.
        for fname in overrides.keys() {
            if !bundle.fields.iter().any(|f| &f.name == fname) {
                return Err(ElabError::BundleFieldUnknown {
                    bundle: bundle_name.to_owned(),
                    field: fname.clone(),
                });
            }
        }

        for field in &bundle.fields {
            let value_expr = overrides
                .get(&field.name)
                .or(field.default.as_ref())
                .ok_or_else(|| ElabError::BundleFieldNoDefault {
                    param: pname.to_owned(),
                    bundle: bundle_name.to_owned(),
                    field: field.name.clone(),
                })?;
            let val = env.eval(value_expr).map_err(|e| ElabError::ConstEval {
                context: format!("bundle param `{pname}.{}` default", field.name),
                source: e,
            })?;
            let vt = self.resolve_value_type(&field.ty, env)?;
            out.push(ModBodyItem::Param(Param {
                name: format!("{pname}_{}", field.name),
                ty: vt,
                default: Some(val),
            }));
        }
        Ok(())
    }

    /// Flattens a `BundleLit` instance-param override (`.model = ResModel {
    /// .rsh = 50.0 }`) into one `({pname}_{field}, value)` pair per named
    /// field. Fields omitted from the literal are left to the child
    /// module's own flattened defaults — no pair is emitted for them.
    fn flatten_bundle_param_override(
        &self,
        pname: &str,
        fields: &[(String, Expr)],
        env: &ConstEnv,
    ) -> Result<Vec<(String, ConstVal)>, ElabError> {
        fields
            .iter()
            .map(|(fname, fexpr)| {
                let val = env.eval(fexpr).map_err(|e| ElabError::ConstEval {
                    context: format!("bundle param override `{pname}.{fname}`"),
                    source: e,
                })?;
                Ok((format!("{pname}_{fname}"), val))
            })
            .collect()
    }

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
        let mut vars = Vec::new();
        let mut instances = Vec::new();
        let mut connections = Vec::new();

        for item in items {
            match item {
                ModBodyItem::Param(p) => params.push(p),
                ModBodyItem::Wire(w) => wires.push(w),
                ModBodyItem::ModVar(v) => vars.push(v),
                ModBodyItem::Inst(i) => instances.push(i),
                ModBodyItem::Conn(c) => connections.push(c),
            }
        }

        Ok(Module { name: decl.name.clone(), ports, params, wires, vars, instances, connections, behaviors: vec![] })
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
                // GAPS §I.14 — a bundle-typed param (`param model : DioModel
                // = DioModel {};`) is flattened here into one scalar param
                // per bundle field, named `{name}_{field}`. This matches
                // the lowering side, which already turns `model.rsh` field
                // access into `IrExpr::Param("model_rsh")` — see
                // `lowering/expr.rs`'s `Expr::Field` arm.
                if ty.dimensions.is_empty() && self.bundles.contains_key(&ty.name)
                    && !self.is_net_capable_bundle(&ty.name)
                {
                    self.flatten_bundle_param(name, &ty.name, default.as_ref(), env, out)?;
                    return Ok(());
                }

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

            ModStmt::VarDecl { name, ty, default } => {
                // GAPS §I.15 — a `var` declared directly in a `mod` body
                // (as opposed to inside `analog`/`digital`) is persistent
                // module-level state, e.g. `var sw_state : Real = 0.0;` in
                // a switch. Unlike behavior-local `var`s (inlined at
                // lowering), this must survive as real state — so it is
                // elaborated here rather than silently dropped.
                //
                // Digital designs also write `var st : Bit = 0;`, where
                // `Bit` is a storage *discipline*, not a value type — that
                // resolves to `TypeRef::Net`. Digital persistent-var
                // lowering is a separate, still-open gap (tracked apart
                // from I.15, which is scoped to the analog/Real case), so
                // for a net-typed mod-body var we preserve the previous
                // behavior (skip at the structural level) rather than
                // half-implement digital state here.
                match self.resolve_type(ty, env, type_subst)? {
                    crate::pom::TypeRef::Value(vt) => {
                        let init = default
                            .as_ref()
                            .map(|e| {
                                env.eval(e).map_err(|e| ElabError::ConstEval {
                                    context: format!("module var `{}` initializer", name),
                                    source: e,
                                })
                            })
                            .transpose()?;
                        out.push(ModBodyItem::ModVar(Var { name: name.clone(), ty: vt, init }));
                    }
                    crate::pom::TypeRef::Net(_) => {
                        // Digital storage var — not yet lowered as persistent state.
                    }
                }
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

                // Resolve param overrides. A `BundleLit` override (GAPS
                // §I.14, e.g. `.model = ResModel { .rsh = 50.0 }`) flattens
                // to one `(param_field, value)` pair per named field,
                // matching the bundle-param flattening done for `param`
                // declarations above — fields the literal omits are left
                // to the child module's own flattened defaults.
                let mut resolved_params: Vec<(String, ConstVal)> = Vec::new();
                for pa in params {
                    match &pa.expr {
                        Expr::BundleLit { fields, .. } => {
                            resolved_params.extend(self.flatten_bundle_param_override(
                                &pa.name, fields, env,
                            )?);
                        }
                        other => {
                            let v = env.eval(other).map_err(|e| ElabError::ConstEval {
                                context: format!("param `{}` of instance `{}`", pa.name, module),
                                source: e,
                            })?;
                            resolved_params.push((pa.name.clone(), v));
                        }
                    }
                }

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
