//! `constraint` block elaboration (dv-core T8, DVC-04/07): a
//! [`ConstraintDecl`] (parse AST) → [`ConstraintBlock`] (POM), resolved
//! against the module the block attaches to. Like behavior elaboration,
//! the stored statements stay the authored surface AST; validation is the
//! pass's job, and what it proves lives in side tables
//! ([`ConstraintBlock::var_types`]).
//!
//! The resolution scope inside `constraint M { … }`:
//!
//! - the block's own `var`s (in declaration order), `M`'s params,
//!   module-level `var`s, ports, and wires — read by bare name;
//! - a child instance's **port** net or param — `m1.d` resolves through
//!   the authored instance tree (DVC-07);
//! - global constants (enum variants, `const` decls);
//! - calls: the `V`/`I` probes, the constraint helpers of
//!   `headers/constraints.phdl`, and any declared callable.
//!
//! Everything else is a loud elaboration error naming the reference and
//! its use site: unknown names, a child's **internal** node (it crosses
//! encapsulation), an undeclared helper, a non-Boolean `require`
//! predicate, a non-`Real` `target` operand, and a duplicate statement
//! label.

use std::collections::{BTreeSet, HashMap, HashSet};

use crate::parse::ast::{BinaryOp, ConstraintDecl, ConstraintStmt, Expr, Literal, UnaryOp};
use crate::elab::const_eval::ConstEnv;
use crate::pom::{
    ConstraintBlock, Design, ElabError, ElabErrorKind, Module, QuantityClass, StmtClass, StmtClassNode,
    ValueType,
};

use super::Elaborator;

/// What [`ConstraintScope::resolve_expr`] proves about one expression:
/// its value type, its pointwise/reduced class, and the analyses its
/// helpers belong to. T8 consumes the type; T10 consumes the class and
/// the analysis set.
pub(crate) struct ExprInfo {
    pub ty: ValueType,
    pub class: QuantityClass,
    pub analyses: BTreeSet<String>,
}

impl ExprInfo {
    fn pointwise(ty: ValueType) -> Self {
        Self { ty, class: QuantityClass::Pointwise, analyses: BTreeSet::new() }
    }

    /// Combine sibling subexpression results under a caller-chosen type:
    /// a reduced part makes the whole reduced (design §C2), and the
    /// analysis sets union.
    fn combine(ty: ValueType, parts: &[&ExprInfo]) -> Self {
        let class = if parts.iter().any(|p| p.class == QuantityClass::Reduced) {
            QuantityClass::Reduced
        } else {
            QuantityClass::Pointwise
        };
        let analyses = parts.iter().flat_map(|p| p.analyses.iter().cloned()).collect();
        Self { ty, class, analyses }
    }
}

/// The resolution scope one `constraint` block elaborates against: the
/// attached module, the design (child modules and disciplines), the
/// elaborator's registries (constraint helpers, callables, impl methods),
/// the global constants, and the block's own `var`s declared so far.
pub(crate) struct ConstraintScope<'a> {
    block: &'a str,
    module: &'a Module,
    design: &'a Design,
    ctx: &'a crate::elab::registry::ElabContext,
    globals: &'a HashMap<String, crate::value::Value>,
    /// Block `var`s declared so far, in declaration order.
    vars: HashMap<String, ExprInfo>,
}

impl<'a> ConstraintScope<'a> {
    pub(crate) fn new(
        block: &'a str,
        module: &'a Module,
        design: &'a Design,
        ctx: &'a crate::elab::registry::ElabContext,
        globals: &'a HashMap<String, crate::value::Value>,
    ) -> Self {
        Self { block, module, design, ctx, globals, vars: HashMap::new() }
    }

    /// Record a block `var` once its annotation and initializer resolve.
    pub(crate) fn define_var(&mut self, name: &str, info: ExprInfo) {
        self.vars.insert(name.to_string(), info);
    }

    fn fail(&self, site: &str, msg: String) -> ElabError {
        ElabError::from(ElabErrorKind::Other(format!(
            "constraint `{}` on module `{}` ({site}): {msg}",
            self.block,
            self.module.name()
        )))
    }

    /// The value type a net (port, wire, or child port) carries: its
    /// discipline's storage type for storage disciplines, `Real` (the
    /// potential) for conservative ones (SPEC §6.2).
    fn net_value_type(&self, discipline: &str) -> ValueType {
        crate::elab::typecheck::discipline_value_type(discipline, self.design)
    }

    /// Resolve a net reference — the argument form of a `V`/`I` probe:
    /// a bare net (`out`), an indexed net (`bus[3]`), or a child
    /// instance's port (`m1.d`). Anything else is loud.
    fn resolve_net_ref(&self, expr: &Expr, site: &str) -> Result<ExprInfo, ElabError> {
        match expr {
            Expr::Ident(name) => self.resolve_net_ident(name, site),
            Expr::Index(base, _idx) => match base.as_ref() {
                Expr::Ident(name) => self.resolve_net_ident(name, site),
                Expr::Field(_, _) => self.resolve_net_ref(base, site),
                other => Err(self.fail(site, format!(
                    "expected a net reference (a net, an indexed net, or an instance port), got `{other:?}`"
                ))),
            },
            Expr::Field(base, field) => {
                let Expr::Ident(inst) = base.as_ref() else {
                    return Err(self.fail(site, format!(
                        "expected a net reference (a net, an indexed net, or an instance port), got `{expr:?}`"
                    )));
                };
                self.resolve_instance_field(inst, field, site).and_then(|info| match info.kind {
                    FieldKind::Port => Ok(info.info),
                    FieldKind::Param => Err(self.fail(site, format!(
                        "`{inst}.{field}` is a param of instance `{inst}`, not a net — probe args are net references"
                    ))),
                })
            }
            other => Err(self.fail(site, format!(
                "expected a net reference (a net, an indexed net, or an instance port), got `{other:?}`"
            ))),
        }
    }

    /// A bare name used as a net: a port or wire of the attached module.
    fn resolve_net_ident(&self, name: &str, site: &str) -> Result<ExprInfo, ElabError> {
        if let Some(p) = self.module.port(name) {
            return Ok(ExprInfo::pointwise(self.net_value_type(p.net_type().discipline_name())));
        }
        if let Some(w) = self.module.wire(name) {
            return Ok(ExprInfo::pointwise(self.net_value_type(w.net_type().discipline_name())));
        }
        Err(self.fail(site, format!(
            "unknown net `{name}` — not a port or wire of module `{}`",
            self.module.name()
        )))
    }

    /// `inst.field` through the authored instance tree (DVC-07): a port
    /// of the child resolves; the child's internal nodes are loud (they
    /// cross encapsulation); an unknown field is loud naming it.
    fn resolve_instance_field(&self, inst: &str, field: &str, site: &str) -> Result<FieldInfo, ElabError> {
        let Some(instance) = self.module.instance(inst) else {
            return Err(self.fail(site, format!(
                "unknown instance `{inst}` — not an instance of module `{}`",
                self.module.name()
            )));
        };
        let child_name = instance.module_name();
        let Some(child) = self.design.module(child_name) else {
            return Err(self.fail(site, format!(
                "instance `{inst}` instantiates unknown module `{child_name}`"
            )));
        };
        if let Some(p) = child.port(field) {
            return Ok(FieldInfo {
                kind: FieldKind::Port,
                info: ExprInfo::pointwise(self.net_value_type(p.net_type().discipline_name())),
            });
        }
        if let Some(p) = child.param(field) {
            return Ok(FieldInfo { kind: FieldKind::Param, info: ExprInfo::pointwise(p.value_type().clone()) });
        }
        if child.wire(field).is_some() {
            return Err(self.fail(site, format!(
                "`{inst}.{field}` is an internal node of `{child_name}` — a constraint reads a child \
                 instance's ports, never its internals (it crosses encapsulation and depends on optional parameters)"
            )));
        }
        Err(self.fail(site, format!(
            "instance `{inst}` of `{child_name}` has no port or param named `{field}`"
        )))
    }

    /// Resolve and type every name in an expression, returning what the
    /// expression proves ([`ExprInfo`]). Unsupported forms and unknown
    /// references are loud, naming the reference and the use site.
    pub(crate) fn resolve_expr(&self, expr: &Expr, site: &str) -> Result<ExprInfo, ElabError> {
        match expr {
            Expr::Literal(lit) => match lit {
                Literal::Real(_) => Ok(ExprInfo::pointwise(ValueType::Real)),
                Literal::Int(_) => Ok(ExprInfo::pointwise(ValueType::Integer)),
                Literal::Bool(_) => Ok(ExprInfo::pointwise(ValueType::Boolean)),
                Literal::Quad(_) => Ok(ExprInfo::pointwise(ValueType::Quad)),
                Literal::String(_) => Ok(ExprInfo::pointwise(ValueType::Str)),
                Literal::None => Err(self.fail(site, "`none` has no value type in a constraint".into())),
            },

            Expr::Ident(name) => {
                if let Some(info) = self.vars.get(name) {
                    return Ok(ExprInfo {
                        ty: info.ty.clone(),
                        class: info.class,
                        analyses: info.analyses.clone(),
                    });
                }
                if let Some(p) = self.module.param(name) {
                    return Ok(ExprInfo::pointwise(p.value_type().clone()));
                }
                if let Some(v) = self.module.vars().iter().find(|v| v.name() == name) {
                    return Ok(ExprInfo::pointwise(v.value_type().clone()));
                }
                if self.module.port(name).is_some() || self.module.wire(name).is_some() {
                    return self.resolve_net_ident(name, site);
                }
                if let Some(val) = self.globals.get(name) {
                    return Ok(ExprInfo::pointwise(value_type_of(val)));
                }
                Err(self.fail(site, format!(
                    "unknown name `{name}` — not a net, port, param, `var`, or constant visible from module `{}`",
                    self.module.name()
                )))
            }

            Expr::Field(base, field) => {
                let Expr::Ident(inst) = base.as_ref() else {
                    return Err(self.fail(site, format!(
                        "unsupported instance path `{expr:?}` — a constraint reaches one level: `instance.port`"
                    )));
                };
                Ok(self.resolve_instance_field(inst, field, site)?.info)
            }

            Expr::Index(..) => self.resolve_net_ref(expr, site),

            Expr::Unary(UnaryOp::Not, inner) => {
                let i = self.resolve_expr(inner, site)?;
                Ok(ExprInfo::combine(ValueType::Boolean, &[&i]))
            }
            Expr::Unary(UnaryOp::Neg, inner) => {
                let i = self.resolve_expr(inner, site)?;
                let ty = i.ty.clone();
                Ok(ExprInfo::combine(ty, &[&i]))
            }

            Expr::Binary(lhs, op, rhs) => {
                let l = self.resolve_expr(lhs, site)?;
                let r = self.resolve_expr(rhs, site)?;
                use BinaryOp::*;
                let ty = match op {
                    Eq | Neq | Lt | Le | Gt | Ge | And | Or | BitAnd | BitOr | BitXor => ValueType::Boolean,
                    Add | Sub | Mul | Div | Rem => l.ty.clone(),
                };
                Ok(ExprInfo::combine(ty, &[&l, &r]))
            }

            Expr::Cast(target, inner) => {
                let i = self.resolve_expr(inner, site)?;
                let ty = match target.as_str() {
                    "real" => ValueType::Real,
                    "int" => ValueType::Integer,
                    "bit" | "Quad" => ValueType::Quad,
                    "Boolean" => ValueType::Boolean,
                    other => return Err(self.fail(site, format!("unsupported cast target `{other}` in a constraint"))),
                };
                Ok(ExprInfo::combine(ty, &[&i]))
            }

            Expr::Call(callee, args) => self.resolve_call(callee, args, site),

            other => Err(self.fail(site, format!(
                "unsupported expression form `{other:?}` in a constraint"
            ))),
        }
    }

    /// A call in a constraint: the `V`/`I` probes, a declared constraint
    /// helper (its class and analysis come from the registry — DVC-09),
    /// a `Type::method` cast form, or any declared callable (math fns).
    fn resolve_call(&self, callee: &Expr, args: &[Expr], site: &str) -> Result<ExprInfo, ElabError> {
        match callee {
            Expr::Ident(name) if name == "V" || name == "I" => {
                if args.is_empty() {
                    return Err(self.fail(site, format!("probe `{name}(..)` needs at least one net argument")));
                }
                let mut infos = Vec::new();
                for arg in args {
                    infos.push(self.resolve_net_ref(arg, site)?);
                }
                let ty = infos[0].ty.clone();
                let refs: Vec<&ExprInfo> = infos.iter().collect();
                Ok(ExprInfo::combine(ty, &refs))
            }
            Expr::Ident(name) => {
                if let Some(helper) = self.ctx.constraint_helpers.lookup(name) {
                    let (class, analysis) = (helper.class, helper.analysis.clone());
                    let mut infos = Vec::new();
                    for arg in args {
                        infos.push(self.resolve_expr(arg, site)?);
                    }
                    let refs: Vec<&ExprInfo> = infos.iter().collect();
                    let mut combined = ExprInfo::combine(ValueType::Real, &refs);
                    if class == QuantityClass::Reduced {
                        combined.class = QuantityClass::Reduced;
                    }
                    combined.analyses.insert(analysis);
                    return Ok(combined);
                }
                if !self.ctx.callables.candidates(name).is_empty() {
                    let mut infos = Vec::new();
                    for arg in args {
                        infos.push(self.resolve_expr(arg, site)?);
                    }
                    let refs: Vec<&ExprInfo> = infos.iter().collect();
                    return Ok(ExprInfo::combine(ValueType::Real, &refs));
                }
                Err(self.fail(site, format!(
                    "undeclared helper or function `{name}` — constraint helpers are declared in \
                     headers/constraints.phdl, math functions in headers/math.phdl"
                )))
            }
            Expr::Path(path) if path.segments.len() == 2 => {
                let (ty_name, method) = (&path.segments[0], &path.segments[1]);
                if self.ctx.impl_methods.candidates(ty_name, method).is_empty() {
                    return Err(self.fail(site, format!(
                        "no declaration found for `{ty_name}::{method}` in a constraint"
                    )));
                }
                let mut infos = Vec::new();
                for arg in args {
                    infos.push(self.resolve_expr(arg, site)?);
                }
                let refs: Vec<&ExprInfo> = infos.iter().collect();
                let ty = match ty_name.as_str() {
                    "Real" => ValueType::Real,
                    "Integer" => ValueType::Integer,
                    "Boolean" => ValueType::Boolean,
                    "Quad" => ValueType::Quad,
                    _ => ValueType::Real,
                };
                Ok(ExprInfo::combine(ty, &refs))
            }
            other => Err(self.fail(site, format!(
                "unsupported call form `{other:?}` in a constraint"
            ))),
        }
    }
}

/// What an instance field resolved to — used to keep probe args honest
/// (a param is a value, not a net).
enum FieldKind {
    Port,
    Param,
}

struct FieldInfo {
    kind: FieldKind,
    info: ExprInfo,
}

/// The [`ValueType`] of a folded global constant.
fn value_type_of(val: &crate::value::Value) -> ValueType {
    use crate::value::Value;
    match val {
        Value::Real(_) => ValueType::Real,
        Value::Nat(_) => ValueType::Natural,
        Value::Int(_) => ValueType::Integer,
        Value::Bool(_) => ValueType::Boolean,
        Value::Quad(_) => ValueType::Quad,
        Value::Str(_) => ValueType::Str,
        _ => ValueType::Real,
    }
}

impl Elaborator {
    /// Elaborate every registered `constraint` block and attach it to its
    /// module — the base module and every monomorphized variant
    /// (`constraint Dac { … }` reaches `Dac__8`, exactly like an `analog`
    /// block; D4). A block naming a module that exists neither as a base
    /// nor as any variant is a loud error.
    pub(crate) fn elab_constraints(&self, design: &mut Design) -> Result<(), ElabError> {
        for decl in &self.syms.constraint_decls.clone() {
            let base = &decl.name;
            let targets: Vec<String> = design
                .modules_map_mut()
                .keys()
                .filter(|name| {
                    *name == base
                        || name
                            .strip_prefix(&format!("{base}__"))
                            .is_some_and(|rest| !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit() || c == '_'))
                })
                .cloned()
                .collect();
            if targets.is_empty() {
                return Err(ElabError::from(ElabErrorKind::Other(format!(
                    "constraint `{base}` names a module that does not exist in the design"
                )))
                .with_span(decl.span));
            }
            // Resolve once against the reference module — the base when it
            // exists, else the first variant (variants share the authored
            // structure; const substitution changes widths and values, not
            // names).
            let reference = if targets.contains(base) { base.clone() } else { targets[0].clone() };
            let block = {
                let module = design.module(&reference).ok_or_else(|| {
                    ElabError::from(ElabErrorKind::UndefinedModule(reference.clone()))
                })?;
                self.elab_constraint(decl, module, design)?
            };
            for name in targets {
                if let Some(module) = design.modules_map_mut().get_mut(&name) {
                    module.constraints.push(block.clone());
                }
            }
        }
        Ok(())
    }

    /// Elaborate one `constraint` block against its module: duplicate
    /// labels, name resolution, and the `require`-Boolean / `target`-Real
    /// type checks (DVC-04 AC4/AC6, DVC-07). Event blocks (`@ dc { … }`,
    /// DVC-19) recurse with an augmented analyses scope.
    fn elab_constraint(
        &self,
        decl: &ConstraintDecl,
        module: &Module,
        design: &Design,
    ) -> Result<ConstraintBlock, ElabError> {
        // One namespace per block: a `require` and a `var` may not share a
        // label (DVC-04 AC6) — across the whole tree, including nested
        // event blocks.
        let mut seen: HashSet<String> = HashSet::new();
        collect_labels(&decl.body, &mut seen, decl, module)?;

        let env = ConstEnv::with_globals(self.syms.globals.clone());
        let mut scope = ConstraintScope::new(&decl.name, module, design, &self.ctx, &self.syms.globals);
        let mut var_types = HashMap::new();
        let stmt_classes = self.elab_stmts(&decl.body, &env, &mut scope, &mut var_types, &BTreeSet::new(), decl, module)?;

        Ok(ConstraintBlock {
            span: decl.span,
            doc: decl.doc.clone(),
            name: decl.name.clone(),
            stmts: decl.body.clone(),
            var_types,
            stmt_classes,
        })
    }

    /// Recursive statement elaboration: threads the enclosing event blocks'
    /// analyses (`scope_analyses`) so a leaf's `StmtClass.analyses` carries
    /// the union of its helpers' analyses and every enclosing `@ dc { … }`'s
    /// scope (DVC-19). `var_types` accumulates across the whole block — a
    /// `var` declared in an outer scope is visible to a `require` in an
    /// inner event block, exactly as it is to a later top-level statement.
    #[allow(clippy::too_many_arguments)]
    fn elab_stmts(
        &self,
        stmts: &[ConstraintStmt],
        env: &ConstEnv,
        scope: &mut ConstraintScope<'_>,
        var_types: &mut HashMap<String, ValueType>,
        scope_analyses: &BTreeSet<String>,
        decl: &ConstraintDecl,
        module: &Module,
    ) -> Result<Vec<StmtClassNode>, ElabError> {
        let mut out = Vec::with_capacity(stmts.len());
        for stmt in stmts {
            match stmt {
                ConstraintStmt::Var { name, ty, value, span } => {
                    let site = format!("var `{name}`");
                    let vt = self
                        .resolve_value_type(ty, env)
                        .map_err(|e| e.with_span(*span))?;
                    let info = scope.resolve_expr(value, &site).map_err(|e| e.with_span(*span))?;
                    var_types.insert(name.clone(), vt.clone());
                    // A `var` takes its expression's class: pointwise
                    // arithmetic over a reduced value is itself reduced
                    // (design §C2), so a reduced initializer marks the var
                    // reduced and any pointwise use of it fails below.
                    let analyses = union_analyses(scope_analyses, &info.analyses);
                    out.push(StmtClassNode::Leaf(StmtClass {
                        class: info.class,
                        analyses,
                    }));
                    scope.define_var(name, ExprInfo { ty: vt, class: info.class, analyses: info.analyses });
                }
                ConstraintStmt::Require { name, pred, span } => {
                    let site = format!("require `{name}`");
                    let info = scope.resolve_expr(pred, &site).map_err(|e| e.with_span(*span))?;
                    if info.ty != ValueType::Boolean {
                        return Err(scope
                            .fail(&site, format!(
                                "`require` predicate must be Boolean, found `{:?}`",
                                info.ty
                            ))
                            .with_span(*span));
                    }
                    // A `require` holds at every evaluation point (D2) — a
                    // reduced quantity does not exist per point (DVC-09).
                    if info.class == QuantityClass::Reduced {
                        return Err(scope
                            .fail(&site, "a reduced quantity is checkable once per analysis, not per point".into())
                            .with_span(*span));
                    }
                    out.push(StmtClassNode::Leaf(StmtClass {
                        class: QuantityClass::Pointwise,
                        analyses: union_analyses(scope_analyses, &info.analyses),
                    }));
                }
                ConstraintStmt::Target { expr, level, tol, span, .. } => {
                    let site = "`target` statement".to_string();
                    let expr_info = scope.resolve_expr(expr, &site).map_err(|e| e.with_span(*span))?;
                    if expr_info.ty != ValueType::Real {
                        return Err(scope
                            .fail(&site, format!("`target` operand must be Real, found `{:?}`", expr_info.ty))
                            .with_span(*span));
                    }
                    let mut analyses = expr_info.analyses.clone();
                    for (what, operand) in
                        [("level", level)].into_iter().chain(tol.iter().map(|t| ("tol scale", t)))
                    {
                        let info = scope.resolve_expr(operand, &site).map_err(|e| e.with_span(*span))?;
                        if info.ty != ValueType::Real {
                            return Err(scope
                                .fail(&site, format!("`target` {what} must be Real, found `{:?}`", info.ty))
                                .with_span(*span));
                        }
                        // A pointwise target is checked per point — its
                        // level and scale must exist per point too.
                        if expr_info.class == QuantityClass::Pointwise && info.class == QuantityClass::Reduced {
                            return Err(scope
                                .fail(&site, "a reduced quantity is checkable once per analysis, not per point".into())
                                .with_span(*span));
                        }
                        analyses.extend(info.analyses.iter().cloned());
                    }
                    out.push(StmtClassNode::Leaf(StmtClass {
                        class: expr_info.class,
                        analyses: union_analyses(scope_analyses, &analyses),
                    }));
                }
                ConstraintStmt::Event { spec, body, span } => {
                    // Validate the spec: every term must be an
                    // analysis-scope event (`dc`/`tran`/`ac`) registered
                    // in the existing `EventRegistry`. Other event kinds
                    // (cross/above/timer/posedge/…) belong to behavior
                    // bodies or T29's event windows; in a constraint event
                    // block they fail loud naming the term and the scope.
                    let analyses = analyses_of_spec(spec, &self.ctx.events).map_err(|e| {
                        ElabError::from(ElabErrorKind::Other(format!(
                            "constraint `{}` on module `{}` (@ event block): {e}",
                            decl.name,
                            module.name()
                        )))
                        .with_span(*span)
                    })?;
                    if analyses.is_empty() {
                        return Err(ElabError::from(ElabErrorKind::Other(format!(
                            "constraint `{}` on module `{}`: an `@` event block must name at least one \
                             analysis (`dc`, `tran`, `ac`) — none found",
                            decl.name,
                            module.name()
                        )))
                        .with_span(*span));
                    }
                    let mut child_scope = scope_analyses.clone();
                    child_scope.extend(analyses.iter().cloned());
                    let children = self.elab_stmts(body, env, scope, var_types, &child_scope, decl, module)?;
                    out.push(StmtClassNode::Event {
                        analyses: analyses.into_iter().collect(),
                        children,
                    });
                }
            }
        }
        Ok(out)
    }
}

/// Walk the authored statement tree (descending into event blocks) and
/// fail loud on the first duplicate `require`/`var` label — names are
/// block-scoped, not scope-scoped, so a top-level `var x` and an
/// `@ dc { var x : … }` shadow each other (DVC-04 AC6, DVC-19).
fn collect_labels(
    stmts: &[ConstraintStmt],
    seen: &mut HashSet<String>,
    decl: &ConstraintDecl,
    module: &Module,
) -> Result<(), ElabError> {
    for stmt in stmts {
        match stmt {
            ConstraintStmt::Require { name, .. } | ConstraintStmt::Var { name, .. } => {
                if !seen.insert(name.clone()) {
                    return Err(ElabError::from(ElabErrorKind::Other(format!(
                        "constraint `{}` on module `{}`: duplicate statement label `{name}`",
                        decl.name,
                        module.name()
                    )))
                    .with_span(stmt.span()));
                }
            }
            ConstraintStmt::Target { .. } => {}
            ConstraintStmt::Event { body, .. } => {
                collect_labels(body, seen, decl, module)?;
            }
        }
    }
    Ok(())
}

/// Union a leaf's helper analyses with the enclosing scope's analyses,
/// sorted and deduplicated — what the lowering / drivers read on
/// [`StmtClass`] (DVC-19).
fn union_analyses(scope: &BTreeSet<String>, helper: &BTreeSet<String>) -> Vec<String> {
    let mut v: Vec<String> = scope.iter().chain(helper.iter()).cloned().collect();
    v.sort();
    v.dedup();
    v
}

/// Resolve an `EventSpec` (dv-core T28, DVC-19) used inside a constraint
/// `@ … { … }` block to the set of analyses it scopes its body to. Every
/// named term must be an analysis-scope event (`dc`/`tran`/`ac`)
/// registered in the existing [`EventRegistry`]; `(a | b)` unions. Any
/// other term — an analog crossing (`cross`/`above`), a digital edge
/// (`posedge`), `initial`/`final`, or an unknown name — fails loud
/// naming it.
fn analyses_of_spec(
    spec: &crate::parse::ast::EventSpec,
    registry: &crate::elab::event::EventRegistry,
) -> Result<BTreeSet<String>, String> {
    let mut set = BTreeSet::new();
    collect_analyses(spec, registry, &mut set)?;
    Ok(set)
}

fn collect_analyses(
    spec: &crate::parse::ast::EventSpec,
    registry: &crate::elab::event::EventRegistry,
    set: &mut BTreeSet<String>,
) -> Result<(), String> {
    use crate::parse::ast::EventSpec;
    match spec {
        EventSpec::Named { name, args } => {
            let Some(ev) = registry.lookup(name) else {
                return Err(format!(
                    "unknown event `{name}` — analysis scoping accepts `dc`/`tran`/`ac` only"
                ));
            };
            if !ev.is_analysis_scope() {
                return Err(format!(
                    "event `{name}` is not an analysis-scope marker — `dc`/`tran`/`ac` only"
                ));
            }
            if !args.is_empty() {
                return Err(format!(
                    "the analysis-scope event `{name}` takes no arguments — wrote `{name}({args:?})`"
                ));
            }
            set.insert(name.clone());
        }
        EventSpec::Or(specs) => {
            for s in specs {
                collect_analyses(s, registry, set)?;
            }
        }
        EventSpec::Initial | EventSpec::Final => {
            return Err("`initial`/`final` are not analysis-scope markers — `dc`/`tran`/`ac` only".into());
        }
    }
    Ok(())
}
