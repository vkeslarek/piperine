use std::collections::HashMap;

use crate::parse::ast::{
    BehaviorDecl, BehaviorKind, BehaviorStmt, DisciplineDecl, EnumDecl, Expr, FnDecl, FnParam,
    ImplDecl, Item, ModDecl, ModStmt, Port, SourceFile, Type,
};
use crate::elab::const_eval::{ConstEnv, ConstVal};
use crate::elab::event::EventRegistry;
use crate::elab::ir::{
    ElabBehavior, ElabBehaviorStmt, ElabConn, ElabError, ElabFn, ElabImpl, ElabInstance,
    ElabMatchArm, ElabMod, ElabNetRef, ElabNetType, ElabParam, ElabPort, ElabProgram, ElabType,
    ElabValueType, ElabWire,
};
use crate::elab::validate::Validator;

// ─────────────────────────────── Elaborator ──────────────────────────────────

pub struct Elaborator {
    disciplines: HashMap<String, DisciplineDecl>,
    bundles: HashMap<String, crate::parse::ast::BundleDecl>,
    enums: HashMap<String, EnumDecl>,
    module_decls: HashMap<String, ModDecl>,
    behavior_decls: Vec<BehaviorDecl>,
    fn_decls: HashMap<String, FnDecl>,
    capability_decls: HashMap<String, crate::parse::ast::CapabilityDecl>,
    impl_decls: Vec<ImplDecl>,
    events: EventRegistry,
    /// Cache of monomorphized modules (mangled name → elaborated module).
    mono_cache: HashMap<String, ElabMod>,
}

impl Elaborator {
    pub fn new() -> Self {
        Self {
            disciplines: HashMap::new(),
            bundles: HashMap::new(),
            enums: HashMap::new(),
            module_decls: HashMap::new(),
            behavior_decls: Vec::new(),
            fn_decls: HashMap::new(),
            capability_decls: HashMap::new(),
            impl_decls: Vec::new(),
            events: EventRegistry::with_builtins(),
            mono_cache: HashMap::new(),
        }
    }

    pub fn elaborate(&mut self, source: SourceFile) -> Result<ElabProgram, ElabError> {
        self.register_items(source.items.iter())?;

        // Validation pass — borrows self.events immutably. Must complete before
        // any &mut self calls (elab_mod_inner, monomorphize).
        {
            let validator = Validator::new(&self.events);
            let mod_decls: Vec<_> = self.module_decls.values().cloned().collect();
            for decl in &mod_decls {
                if decl.const_params.is_empty() && decl.type_params.is_empty() {
                    validator.validate_mod_body(&decl.body)?;
                }
            }
            let beh_decls: Vec<_> = self.behavior_decls.clone();
            for beh in &beh_decls {
                validator.validate_behavior(beh.kind.clone(), &beh.body)?;
            }
        }

        let mut prog = ElabProgram::new();

        prog.disciplines = self.disciplines.clone();
        prog.enums = self.enums.clone();
        prog.capabilities = self.capability_decls.clone();

        for impl_decl in &self.impl_decls.clone() {
            prog.impls.push(self.elab_impl(impl_decl)?);
        }

        for fn_decl in self.fn_decls.values().cloned().collect::<Vec<_>>() {
            let f = self.elab_fn(&fn_decl)?;
            prog.functions.insert(f.name.clone(), f);
        }

        // Elaborate all non-generic modules. Monomorphization of generic
        // modules is triggered on demand inside lower_mod_stmt when an
        // instance with const args is encountered.
        let mod_names: Vec<String> = self.module_decls.keys().cloned().collect();
        for name in &mod_names {
            let decl = self.module_decls[name].clone();
            if decl.const_params.is_empty() && decl.type_params.is_empty() {
                let mut env = ConstEnv::new();
                let elab_mod = self.elab_mod_inner(&decl, &mut env, &HashMap::new())?;
                prog.modules.insert(name.clone(), elab_mod);
            }
        }

        for beh in &self.behavior_decls.clone() {
            prog.behaviors.push(self.elab_behavior(beh)?);
        }

        // Merge all on-demand monomorphized modules into the program.
        for (name, elab_mod) in self.mono_cache.drain() {
            prog.modules.entry(name).or_insert(elab_mod);
        }

        Ok(prog)
    }

    // ──────────────────────── Symbol registration ─────────────────────────────

    fn register_items<'a>(
        &mut self,
        items: impl Iterator<Item = &'a Item>,
    ) -> Result<(), ElabError> {
        for item in items {
            match item {
                Item::DisciplineDecl(d) => { self.disciplines.insert(d.name.clone(), d.clone()); }
                Item::BundleDecl(b)     => { self.bundles.insert(b.name.clone(), b.clone()); }
                Item::EnumDecl(e)       => { self.enums.insert(e.name.clone(), e.clone()); }
                Item::ModDecl(m)        => { self.module_decls.insert(m.name.clone(), m.clone()); }
                Item::BehaviorDecl(b)   => { self.behavior_decls.push(b.clone()); }
                Item::FnDecl(f)         => { self.fn_decls.insert(f.sig.name.clone(), f.clone()); }
                Item::CapabilityDecl(c) => { self.capability_decls.insert(c.name.clone(), c.clone()); }
                Item::ImplDecl(i)       => { self.impl_decls.push(i.clone()); }
                Item::UseDecl(_)        => {} // already expanded by Resolver
            }
        }
        Ok(())
    }

    // ─────────────────────────── Type resolution ──────────────────────────────

    fn resolve_type(
        &self,
        ty: &Type,
        env: &ConstEnv,
        type_subst: &HashMap<String, String>,
    ) -> Result<ElabType, ElabError> {
        let name = type_subst.get(&ty.name).map(|s| s.as_str()).unwrap_or(&ty.name);

        if !ty.dimensions.is_empty() {
            let inner_ty =
                Type { name: ty.name.clone(), args: ty.args.clone(), dimensions: vec![] };
            let inner = self.resolve_type(&inner_ty, env, type_subst)?;
            let mut result = inner;
            for dim_expr in &ty.dimensions {
                let n = env.eval_nat(dim_expr).map_err(|e| ElabError::ConstEval {
                    context: format!("array dimension of type `{}`", ty.name),
                    source: e,
                })?;
                result = match result {
                    ElabType::Net(nt) => ElabType::Net(ElabNetType::Array(Box::new(nt), n)),
                    ElabType::Value(vt) => ElabType::Value(ElabValueType::Array(Box::new(vt), n)),
                };
            }
            return Ok(result);
        }

        let value_prim = match name {
            "Real"    => Some(ElabValueType::Real),
            "Natural" => Some(ElabValueType::Natural),
            "Integer" => Some(ElabValueType::Integer),
            "Complex" => Some(ElabValueType::Complex),
            "Boolean" => Some(ElabValueType::Boolean),
            "Quad"    => Some(ElabValueType::Quad),
            "String"  => Some(ElabValueType::Str),
            _ => None,
        };
        if let Some(vt) = value_prim {
            return Ok(ElabType::Value(vt));
        }

        if self.disciplines.contains_key(name) {
            return Ok(ElabType::Net(ElabNetType::Discipline(name.to_owned())));
        }

        if self.enums.contains_key(name) {
            return Ok(ElabType::Value(ElabValueType::Enum(name.to_owned())));
        }

        if self.bundles.contains_key(name) {
            if self.is_net_capable_bundle(name) {
                return Ok(ElabType::Net(ElabNetType::Discipline(name.to_owned())));
            }
            return Err(ElabError::UndefinedType(format!(
                "`{}` is a value bundle — use field access, not as a bare type",
                name
            )));
        }

        if name == "fn" {
            let params_and_ret = ty
                .args
                .iter()
                .map(|a| self.resolve_type(a, env, type_subst))
                .collect::<Result<Vec<_>, _>>()?;
            let (ret, params) = params_and_ret
                .split_last()
                .ok_or_else(|| {
                    ElabError::UndefinedType("fn type requires a return type".to_owned())
                })?;
            return Ok(ElabType::Value(ElabValueType::FnPtr(
                params.to_vec(),
                Box::new(ret.clone()),
            )));
        }

        Err(ElabError::UndefinedType(name.to_owned()))
    }

    fn resolve_net_type(
        &self,
        ty: &Type,
        env: &ConstEnv,
        type_subst: &HashMap<String, String>,
    ) -> Result<ElabNetType, ElabError> {
        match self.resolve_type(ty, env, type_subst)? {
            ElabType::Net(nt) => Ok(nt),
            _ => Err(ElabError::NotNetCapable(ty.name.clone())),
        }
    }

    fn resolve_value_type(&self, ty: &Type, env: &ConstEnv) -> Result<ElabValueType, ElabError> {
        match self.resolve_type(ty, env, &HashMap::new())? {
            ElabType::Value(vt) => Ok(vt),
            ElabType::Net(nt) => Err(ElabError::Other(format!(
                "expected value type, found net type `{:?}`",
                nt
            ))),
        }
    }

    fn is_net_capable_bundle(&self, name: &str) -> bool {
        let Some(bundle) = self.bundles.get(name) else { return false };
        bundle.fields.iter().all(|f| self.is_net_type_name(&f.ty.name))
    }

    fn is_net_type_name(&self, name: &str) -> bool {
        self.disciplines.contains_key(name) || self.is_net_capable_bundle(name)
    }

    // ─────────────────────────── Net reference ────────────────────────────────

    /// Reduce a port-connection or net-connection expression to a concrete
    /// `ElabNetRef`. Supported forms:
    ///
    /// - `name` → `ElabNetRef::simple(name)`
    /// - `name[i]` — `i` evaluated via `env` → `ElabNetRef::indexed(name, i)`
    /// - `base.field` → `ElabNetRef::simple("{base}_{field}")` (bundle-field naming)
    fn eval_net_ref(&self, expr: &Expr, env: &ConstEnv) -> Result<ElabNetRef, ElabError> {
        match expr {
            Expr::Ident(name) => Ok(ElabNetRef::simple(name)),
            Expr::Index(base, idx) => {
                let base_name = match base.as_ref() {
                    Expr::Ident(n) => n.clone(),
                    other => {
                        return Err(ElabError::NotANetRef(format!(
                            "indexed net ref base must be an identifier, got `{:?}`",
                            other
                        )))
                    }
                };
                let i = env.eval_nat(idx).map_err(|e| ElabError::ConstEval {
                    context: format!("net ref index on `{}`", base_name),
                    source: e,
                })?;
                Ok(ElabNetRef::indexed(base_name, i))
            }
            Expr::Field(base, field) => {
                let base_name = match base.as_ref() {
                    Expr::Ident(n) => n.clone(),
                    other => {
                        return Err(ElabError::NotANetRef(format!(
                            "field net ref base must be an identifier, got `{:?}`",
                            other
                        )))
                    }
                };
                Ok(ElabNetRef::simple(format!("{}_{}", base_name, field)))
            }
            other => Err(ElabError::NotANetRef(format!(
                "expected net reference (identifier, index, or field), got `{:?}`",
                other
            ))),
        }
    }

    // ─────────────────────────── Port expansion ───────────────────────────────

    fn expand_port(
        &self,
        port: &Port,
        env: &ConstEnv,
        type_subst: &HashMap<String, String>,
    ) -> Result<Vec<ElabPort>, ElabError> {
        let resolved_name =
            type_subst.get(&port.ty.name).map(|s| s.as_str()).unwrap_or(&port.ty.name);

        if let Some(bundle) = self.bundles.get(resolved_name).cloned() {
            if !self.is_net_capable_bundle(resolved_name) {
                return Err(ElabError::NotNetCapable(resolved_name.to_owned()));
            }
            let mut out = Vec::new();
            for field in &bundle.fields {
                let field_ty = self.resolve_net_type(&field.ty, env, type_subst)?;
                out.push(ElabPort {
                    direction: port.direction.clone(),
                    name: format!("{}_{}", port.name, field.name),
                    ty: field_ty,
                });
            }
            return Ok(out);
        }

        let net_ty = self.resolve_net_type(&port.ty, env, type_subst)?;
        Ok(vec![ElabPort {
            direction: port.direction.clone(),
            name: port.name.clone(),
            ty: net_ty,
        }])
    }

    // ─────────────────────────── Module elaboration ───────────────────────────

    fn elab_mod_inner(
        &mut self,
        decl: &ModDecl,
        env: &mut ConstEnv,
        type_subst: &HashMap<String, String>,
    ) -> Result<ElabMod, ElabError> {
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

        Ok(ElabMod { name: decl.name.clone(), ports, params, wires, instances, connections })
    }

    fn lower_mod_stmts(
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

    fn lower_mod_stmt(
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
                out.push(ModBodyItem::Param(ElabParam {
                    name: name.clone(),
                    ty: vt,
                    default: def,
                }));
            }

            ModStmt::WireDecl { name, ty } => {
                let nt = self.resolve_net_type(ty, env, type_subst)?;
                out.push(ModBodyItem::Wire(ElabWire { name: name.clone(), ty: nt }));
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

                out.push(ModBodyItem::Inst(ElabInstance {
                    label,
                    module: mono_name,
                    ports: elab_ports,
                    params: resolved_params,
                }));
            }

            ModStmt::Connection { lhs, rhs } => {
                let lhs_ref = self.eval_net_ref(lhs, env)?;
                let rhs_ref = self.eval_net_ref(rhs, env)?;
                out.push(ModBodyItem::Conn(ElabConn { lhs: lhs_ref, rhs: rhs_ref }));
            }
        }
        Ok(())
    }

    // ────────────────────────── Behavior elaboration ──────────────────────────

    fn elab_behavior(&self, beh: &BehaviorDecl) -> Result<ElabBehavior, ElabError> {
        let mut env = ConstEnv::new();
        let body = self.lower_behavior_stmts(&beh.body, beh.kind.clone(), &mut env)?;
        Ok(ElabBehavior { name: beh.name.clone(), kind: beh.kind.clone(), body })
    }

    fn lower_behavior_stmts(
        &self,
        stmts: &[BehaviorStmt],
        kind: BehaviorKind,
        env: &mut ConstEnv,
    ) -> Result<Vec<ElabBehaviorStmt>, ElabError> {
        let mut out = Vec::new();
        for stmt in stmts {
            self.lower_behavior_stmt(stmt, kind.clone(), env, &mut out)?;
        }
        Ok(out)
    }

    fn lower_behavior_stmt(
        &self,
        stmt: &BehaviorStmt,
        kind: BehaviorKind,
        env: &mut ConstEnv,
        out: &mut Vec<ElabBehaviorStmt>,
    ) -> Result<(), ElabError> {
        match stmt {
            BehaviorStmt::VarDecl { name, ty, default } => {
                let vt = self.resolve_value_type(ty, env)?;
                out.push(ElabBehaviorStmt::VarDecl {
                    name: name.clone(),
                    ty: vt,
                    default: default.clone(),
                });
            }

            BehaviorStmt::Bind { dest, op, src } => {
                out.push(ElabBehaviorStmt::Bind {
                    dest: dest.clone(),
                    op: op.clone(),
                    src: src.clone(),
                });
            }

            BehaviorStmt::If { cond, then_body, else_body } => {
                let folded = match env.eval(cond) {
                    Ok(ConstVal::Bool(true)) | Ok(ConstVal::Nat(1)) => {
                        self.lower_behavior_stmts(then_body, kind.clone(), env)?
                    }
                    Ok(ConstVal::Bool(false)) | Ok(ConstVal::Nat(0)) => {
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
                        out.push(ElabBehaviorStmt::If {
                            cond: cond.clone(),
                            then_body: then_elab,
                            else_body: else_elab,
                        });
                        return Ok(());
                    }
                };
                out.extend(folded);
            }

            BehaviorStmt::Match { expr, arms } => {
                let elab_arms = arms
                    .iter()
                    .map(|arm| {
                        let body =
                            self.lower_behavior_stmts(&arm.body, kind.clone(), env)?;
                        Ok(ElabMatchArm { pat: arm.pat.clone(), body })
                    })
                    .collect::<Result<Vec<_>, ElabError>>()?;
                out.push(ElabBehaviorStmt::Match { expr: expr.clone(), arms: elab_arms });
            }

            BehaviorStmt::For { var, range, body } => {
                let start = env.eval_nat(&range.start).map_err(|e| ElabError::ConstEval {
                    context: format!("behavioral for-loop start (var `{}`)", var),
                    source: e,
                })?;
                let end_val = env.eval_nat(&range.end).map_err(|e| ElabError::ConstEval {
                    context: format!("behavioral for-loop end (var `{}`)", var),
                    source: e,
                })?;
                let end = if range.inclusive { end_val + 1 } else { end_val };
                for i in start..end {
                    env.push();
                    env.define(var.clone(), ConstVal::Nat(i));
                    let unrolled = self.lower_behavior_stmts(body, kind.clone(), env)?;
                    out.extend(unrolled);
                    env.pop();
                }
            }

            BehaviorStmt::Event { spec, guard, body } => {
                let elab_body: Vec<ElabBehaviorStmt> = body
                    .stmts
                    .iter()
                    .map(|s| self.lower_stmt_to_behavior(s, kind.clone(), env))
                    .collect::<Result<Vec<_>, _>>()?
                    .into_iter()
                    .flatten()
                    .collect();
                out.push(ElabBehaviorStmt::Event {
                    spec: spec.clone(),
                    guard: guard.clone(),
                    body: elab_body,
                });
            }

            BehaviorStmt::Diagnostic { sys, args } => {
                out.push(ElabBehaviorStmt::Diagnostic {
                    sys: sys.clone(),
                    args: args.clone(),
                });
            }

            BehaviorStmt::Expr(e) => {
                out.push(ElabBehaviorStmt::Expr(e.clone()));
            }
        }
        Ok(())
    }

    /// Lower a function-body `Stmt` to behavior statements (used inside event
    /// blocks and function bodies).
    fn lower_stmt_to_behavior(
        &self,
        stmt: &crate::parse::ast::Stmt,
        kind: BehaviorKind,
        env: &mut ConstEnv,
    ) -> Result<Vec<ElabBehaviorStmt>, ElabError> {
        use crate::parse::ast::Stmt;
        match stmt {
            Stmt::VarDecl { name, ty, default } => {
                let vt = self.resolve_value_type(ty, env)?;
                Ok(vec![ElabBehaviorStmt::VarDecl {
                    name: name.clone(),
                    ty: vt,
                    default: default.clone(),
                }])
            }
            Stmt::Bind { dest, op, src } => Ok(vec![ElabBehaviorStmt::Bind {
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
                Ok(vec![ElabBehaviorStmt::If {
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
                        Ok(ElabMatchArm { pat: arm.pat.clone(), body })
                    })
                    .collect::<Result<Vec<_>, ElabError>>()?;
                Ok(vec![ElabBehaviorStmt::Match { expr: expr.clone(), arms: elab_arms }])
            }
            Stmt::For { var, range, body } => {
                let start = env.eval_nat(&range.start).map_err(|e| ElabError::ConstEval {
                    context: format!("for-loop in event block (var `{}`)", var),
                    source: e,
                })?;
                let end_val = env.eval_nat(&range.end).map_err(|e| ElabError::ConstEval {
                    context: format!("for-loop end in event block (var `{}`)", var),
                    source: e,
                })?;
                let end = if range.inclusive { end_val + 1 } else { end_val };
                let mut unrolled = Vec::new();
                for i in start..end {
                    env.push();
                    env.define(var.clone(), ConstVal::Nat(i));
                    for s in &body.stmts {
                        unrolled.extend(self.lower_stmt_to_behavior(s, kind.clone(), env)?);
                    }
                    env.pop();
                }
                Ok(unrolled)
            }
            Stmt::Return(e) => Ok(vec![ElabBehaviorStmt::Expr(e.clone())]),
            Stmt::Expr(e) => Ok(vec![ElabBehaviorStmt::Expr(e.clone())]),
        }
    }

    // ─────────────────────────── Function elaboration ─────────────────────────

    fn elab_fn(&self, fn_decl: &FnDecl) -> Result<ElabFn, ElabError> {
        let mut env = ConstEnv::new();

        let params = fn_decl
            .sig
            .params
            .iter()
            .filter_map(|p| match p {
                FnParam::SelfParam => None,
                FnParam::Typed(name, ty) => {
                    let resolved = self.resolve_type(ty, &env, &HashMap::new()).ok()?;
                    Some(Ok((name.clone(), resolved)))
                }
            })
            .collect::<Result<Vec<_>, ElabError>>()?;

        let ret = self
            .resolve_type(&fn_decl.sig.ret, &env, &HashMap::new())
            .unwrap_or(ElabType::Value(ElabValueType::Real));

        // Lower the body from raw Stmt AST to ElabBehaviorStmt.
        // Functions use Analog as a placeholder kind (no behavior-specific ops allowed).
        let mut body: Vec<ElabBehaviorStmt> = fn_decl
            .body
            .stmts
            .iter()
            .map(|s| self.lower_stmt_to_behavior(s, BehaviorKind::Analog, &mut env))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten()
            .collect();

        // Trailing expression becomes an implicit return.
        if let Some(expr) = &fn_decl.body.expr {
            body.push(ElabBehaviorStmt::Expr(*expr.clone()));
        }

        Ok(ElabFn { name: fn_decl.sig.name.clone(), params, ret, body })
    }

    // ─────────────────────────── Impl elaboration ─────────────────────────────

    fn elab_impl(&self, impl_decl: &ImplDecl) -> Result<ElabImpl, ElabError> {
        let env = ConstEnv::new();
        let const_args = impl_decl
            .const_args
            .iter()
            .map(|e| {
                env.eval(e).map_err(|src| ElabError::ConstEval {
                    context: format!("impl const arg for `{}`", impl_decl.ty),
                    source: src,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        let methods = impl_decl
            .methods
            .iter()
            .map(|m| self.elab_fn(m))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(ElabImpl {
            capability: impl_decl.capability.clone(),
            ty: impl_decl.ty.clone(),
            const_args,
            methods,
        })
    }

    // ──────────────────── Generic module monomorphization ─────────────────────

    /// Elaborate a generic module on demand with specific const substitutions.
    /// Caches the result in `mono_cache`. Returns the monomorphized name.
    pub fn monomorphize(
        &mut self,
        module_name: &str,
        const_args: &[u64],
    ) -> Result<String, ElabError> {
        let mono_name = if const_args.is_empty() {
            module_name.to_owned()
        } else {
            let suffix: Vec<String> = const_args.iter().map(|n| n.to_string()).collect();
            format!("{}__{}", module_name, suffix.join("_"))
        };

        if self.mono_cache.contains_key(&mono_name) {
            return Ok(mono_name);
        }

        let decl = self
            .module_decls
            .get(module_name)
            .cloned()
            .ok_or_else(|| ElabError::UndefinedModule(module_name.to_owned()))?;

        if decl.const_params.len() != const_args.len() {
            return Err(ElabError::Other(format!(
                "module `{}` expects {} const params, got {}",
                module_name,
                decl.const_params.len(),
                const_args.len()
            )));
        }

        let mut env = ConstEnv::new();
        for (param_name, &val) in decl.const_params.iter().zip(const_args.iter()) {
            env.define(param_name.clone(), ConstVal::Nat(val));
        }

        let mut mono_decl = decl.clone();
        mono_decl.name = mono_name.clone();

        // Insert a placeholder to break potential recursion.
        self.mono_cache.insert(mono_name.clone(), ElabMod {
            name: mono_name.clone(),
            ports: vec![],
            params: vec![],
            wires: vec![],
            instances: vec![],
            connections: vec![],
        });

        let elab_mod = self.elab_mod_inner(&mono_decl, &mut env, &HashMap::new())?;
        self.mono_cache.insert(mono_name.clone(), elab_mod);
        Ok(mono_name)
    }
}

impl Default for Elaborator {
    fn default() -> Self {
        Self::new()
    }
}

// ─────────────────────────────── Internal ────────────────────────────────────

enum ModBodyItem {
    Param(ElabParam),
    Wire(ElabWire),
    Inst(ElabInstance),
    Conn(ElabConn),
}
