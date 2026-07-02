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
            out.push(ModBodyItem::Param(Param { attributes: Vec::new(),
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
        let mut local_types: HashMap<String, String> = HashMap::new();
        for port in &decl.ports {
            let resolved_name = type_subst.get(&port.ty.name).map(|s| s.as_str()).unwrap_or(&port.ty.name);
            local_types.insert(port.name.clone(), resolved_name.to_string());
            ports.extend(self.expand_port(port, env, type_subst)?);
        }

        for stmt in &decl.body {
            if let ModStmt::WireDecl { name, ty, attrs: _ } = stmt {
                let resolved_name = type_subst.get(&ty.name).map(|s| s.as_str()).unwrap_or(&ty.name);
                local_types.insert(name.clone(), resolved_name.to_string());
            }
        }

        let mut items: Vec<ModBodyItem> = Vec::new();
        let body = decl.body.clone();
        self.lower_mod_stmts(&body, env, type_subst, &local_types, &mut items)?;

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

        Ok(Module { attributes: Vec::new(), name: decl.name.clone(), ports, params, wires, vars, instances, connections, behaviors: vec![] })
    }

    /// Lowers a slice of `ModStmt`s, appending the resulting
    /// `ModBodyItem`s to `out`. Delegates each statement to
    /// [`lower_mod_stmt`](Elaborator::lower_mod_stmt).
    pub(crate) fn lower_mod_stmts(
        &mut self,
        stmts: &[ModStmt],
        env: &mut ConstEnv,
        type_subst: &HashMap<String, String>,
        local_types: &HashMap<String, String>,
        out: &mut Vec<ModBodyItem>,
    ) -> Result<(), ElabError> {
        for stmt in stmts {
            let stmt = stmt.clone();
            self.lower_mod_stmt(&stmt, env, type_subst, local_types, out)?;
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
        local_types: &HashMap<String, String>,
        out: &mut Vec<ModBodyItem>,
    ) -> Result<(), ElabError> {
        match stmt {
            ModStmt::ParamDecl { name, ty, default, attrs: _ } => {
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
                out.push(ModBodyItem::Param(Param { attributes: Vec::new(),
                    name: name.clone(),
                    ty: vt,
                    default: def,
                }));
            }

            ModStmt::WireDecl { name, ty, attrs: _ } => {
                let resolved_name = type_subst.get(&ty.name).map(|s| s.as_str()).unwrap_or(&ty.name);
                if let Some(bundle) = self.bundles.get(resolved_name).cloned() {
                    if self.is_net_capable_bundle(resolved_name) {
                        for field in &bundle.fields {
                            let field_ty = self.resolve_net_type(&field.ty, env, type_subst)?;
                            out.push(ModBodyItem::Wire(Wire { attributes: Vec::new(),
                                name: format!("{}_{}", name, field.name),
                                ty: field_ty,
                            }));
                        }
                        return Ok(());
                    }
                }
                let nt = self.resolve_net_type(ty, env, type_subst)?;
                out.push(ModBodyItem::Wire(Wire { attributes: Vec::new(), name: name.clone(), ty: nt }));
            }

            ModStmt::VarDecl { name, ty, default, attrs: _ } => {
                // §7.2 + §I.15 — a `var` declared directly in a `mod` body
                // (as opposed to inside `analog`/`digital`) is persistent
                // module-level state, e.g. `var sw_state : Real = 0.0;` in
                // a switch, or `var st : Bit = 0;` in a digital register.
                // Unlike behavior-local `var`s (inlined at lowering), this
                // must survive as real state — so it is elaborated here.
                //
                // A `var` is a value-typed binding. When the source writes
                // `var st : Bit = 0;`, `Bit` is a storage discipline — the
                // var's value type is the discipline's storage value type
                // (here `Boolean`). We resolve through the discipline to
                // recover that value type, so the var survives as
                // persistent digital state. A conservative discipline
                // (potential+flow) has no storage value type and is an
                // error — a `var` cannot be a conservative terminal.
                let resolved = self.resolve_type(ty, env, type_subst)?;
                let vt = match resolved {
                    crate::pom::TypeRef::Value(vt) => vt,
                    crate::pom::TypeRef::Net(crate::pom::NetType::Discipline(dname)) => {
                        self.storage_value_type(&dname)?
                            .ok_or_else(|| ElabError::Other(format!(
                                "module var `{}` has conservative discipline `{}` — a `var` needs a storage discipline or a value type",
                                name, dname
                            )))?
                    }
                    crate::pom::TypeRef::Net(other) => {
                        return Err(ElabError::Other(format!(
                            "module var `{}` has unsupported net type `{:?}`",
                            name, other
                        )));
                    }
                };
                let init = default
                    .as_ref()
                    .map(|e| {
                        env.eval(e).map_err(|e| ElabError::ConstEval {
                            context: format!("module var `{}` initializer", name),
                            source: e,
                        })
                    })
                    .transpose()?;
                out.push(ModBodyItem::ModVar(Var {
                    attributes: Vec::new(),
                    name: name.clone(),
                    ty: vt,
                    init,
                }));
            }

            ModStmt::StructuralFor { var, range, body, attrs: _ } => {
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
                    self.lower_mod_stmts(&body, env, type_subst, local_types, out)?;
                    env.pop();
                }
            }

            ModStmt::StructuralIf { cond, then_body, else_body, attrs: _ } => {
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
                self.lower_mod_stmts(&taken, env, type_subst, local_types, out)?;
            }

            ModStmt::Instance {
                name,
                array_index,
                module,
                const_args,
                type_args: _,
                ports,
                params,
             attrs: _ } => {
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
                let mut elab_ports = Vec::new();
                for p in ports {
                    let mut expanded = false;
                    if let crate::parse::ast::Expr::Ident(p_name) = p {
                        if let Some(ty_name) = local_types.get(p_name) {
                            if let Some(bundle) = self.bundles.get(ty_name).cloned() {
                                if self.is_net_capable_bundle(ty_name) {
                                    expanded = true;
                                    for field in &bundle.fields {
                                        elab_ports.push(crate::pom::net_type::NetRef::simple(format!("{}_{}", p_name, field.name)));
                                    }
                                }
                            }
                        }
                    }
                    if !expanded {
                        elab_ports.push(self.eval_net_ref(p, env)?);
                    }
                }

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

                out.push(ModBodyItem::Inst(Instance { attributes: Vec::new(),
                    label,
                    module: mono_name,
                    ports: elab_ports,
                    params: resolved_params,
                }));
            }

            ModStmt::Connection { lhs, rhs, attrs: _ } => {
                let mut is_bundle_conn = false;
                if let (crate::parse::ast::Expr::Ident(l_name), crate::parse::ast::Expr::Ident(r_name)) = (lhs, rhs) {
                    if let Some(l_ty_name) = local_types.get(l_name) {
                        if let Some(bundle) = self.bundles.get(l_ty_name).cloned() {
                            if self.is_net_capable_bundle(l_ty_name) {
                                is_bundle_conn = true;
                                for field in &bundle.fields {
                                    let l_ref = crate::pom::net_type::NetRef::simple(format!("{}_{}", l_name, field.name));
                                    let r_ref = crate::pom::net_type::NetRef::simple(format!("{}_{}", r_name, field.name));
                                    out.push(ModBodyItem::Conn(Connection { lhs: l_ref, rhs: r_ref }));
                                }
                            }
                        }
                    }
                }
                
                if !is_bundle_conn {
                    let lhs_ref = self.eval_net_ref(lhs, env)?;
                    let rhs_ref = self.eval_net_ref(rhs, env)?;
                    out.push(ModBodyItem::Conn(Connection { lhs: lhs_ref, rhs: rhs_ref }));
                }
            }
        }
        Ok(())
    }

}
