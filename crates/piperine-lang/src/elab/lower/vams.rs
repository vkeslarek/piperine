//! VAMS → POM structural lowering: `module`/`macromodule`/`connectmodule`
//! declarations (LRM Annex A.1.2) into POM [`Module`]s — ports (both
//! declaration-list and name-list styles, §3.7), nets/wires with
//! disciplines (§3.6.3), parameters (§3.4), instantiation (named + ordered
//! connections, parameter overrides, §6.3) and behavior-block attachment
//! (`analog`/`analog initial`/`initial`/`always`, structurally nested
//! `ModuleItem`s — unlike PHDL, VAMS has no separate top-level `BehaviorDecl`
//! matched by name).
//!
//! **VAMS-04, MD-25**: the POM `Module`/`Instance`/`Port` shape mirrors the
//! LRM directly — a discipline name is carried as-is (`NetType::Discipline`),
//! never re-typed through a PHDL-only net-capability lattice; the
//! instance→submodule→sub-instance tree is exactly the authored nesting
//! (no flattening, no connect-module insertion — those are later passes,
//! T25/T52, writing only the derived side artifact).

use std::collections::{HashMap, HashSet};

use piperine_vams::ast::{
    BranchDecl, Declarator, Direction as AstDirection, Expr, ModuleDecl, ModuleItem, ParamType,
    PortConnection, PortDecl, Stmt,
};

use crate::elab::const_eval::ConstEnv;
use crate::pom::{
    Behavior, Branch, BranchKind, Design, Direction, ElabError, ElabErrorKind, Instance, Module,
    Nature, NetRef, NetType, Param, Port, Value, ValueType, Wire,
};

use super::Elaborator;

impl Elaborator {
    /// Elaborates one `ModuleDecl` into a POM `Module`. Structural
    /// (no cross-module resolution beyond looking up an instantiated
    /// module's port order in the already-registered symbol table).
    pub(crate) fn elab_module(&self, decl: &ModuleDecl) -> Result<Module, ElabError> {
        let env = ConstEnv::new();

        let mut ports = Vec::new();
        let mut wires = Vec::new();
        let mut params = Vec::new();
        let mut instances = Vec::new();
        let mut behaviors = Vec::new();
        let mut branches = Vec::new();

        // Declaration-list style port list (`module m(input electrical a, ...)`).
        for p in decl.ports.iter().flatten() {
            lower_port_decl(p, &mut ports)?;
        }

        for item in &decl.items {
            match item {
                // Name-list style: `module m(a, b); input electrical a; ...`.
                ModuleItem::Port(p) => lower_port_decl(p, &mut ports)?,
                ModuleItem::Net(n) => {
                    let discipline = n.discipline.as_deref().ok_or_else(|| {
                        ElabError::from(ElabErrorKind::Other(format!(
                            "net declaration has no discipline — undeclared-net discipline \
                             resolution (LRM Annex F) is not implemented until T26"
                        )))
                    })?;
                    for d in &n.names {
                        wires.push(declarator_to_wire(d, discipline, &env)?);
                    }
                }
                ModuleItem::Param(p) => {
                    let ty = param_value_type(p.ty);
                    for (name, expr) in &p.names {
                        let value = eval_param_default(expr, &env)?;
                        check_param_ranges(name, &value, &p.ranges, &env)?;
                        params.push(Param {
                            span: None,
                            attributes: Vec::new(),
                            doc: None,
                            name: name.clone(),
                            ty: ty.clone(),
                            default: Some(value),
                            bundle_origin: None,
                            distribution: None,
                            is_local: p.is_local,
                        });
                    }
                }
                // `aliasparam name = target;` (§3.4.7): a pure name alias —
                // lowered as a `Param` with no default of its own; its
                // value always mirrors `target`'s. Resolving `target` to a
                // concrete value happens where params are read (T30+,
                // codegen retarget) — recording the alias link here keeps
                // it visible on the POM (`bundle_origin` is reused as the
                // (alias, target) carrier since both are "this param's
                // value comes from elsewhere", not a new field).
                ModuleItem::AliasParam(a) => {
                    params.push(Param {
                        span: None,
                        attributes: Vec::new(),
                        doc: None,
                        name: a.name.clone(),
                        ty: ValueType::Real,
                        default: None,
                        bundle_origin: Some(("aliasparam".to_string(), a.target.clone())),
                        distribution: None,
                        is_local: false,
                    });
                }
                ModuleItem::Instantiation(inst_decl) => {
                    for ast_inst in &inst_decl.instances {
                        instances.push(self.lower_instance(
                            &inst_decl.module_name,
                            ast_inst,
                            &inst_decl.param_overrides,
                        )?);
                    }
                }
                ModuleItem::Behavior(b) => {
                    let stmts = match b.body.as_ref() {
                        piperine_vams::ast::Stmt::Block { stmts, .. } => stmts.clone(),
                        other => vec![other.clone()],
                    };
                    behaviors.push(Behavior::new(decl.name.clone(), b.kind, stmts));
                }
                ModuleItem::Branch(b) => lower_branch_decl(b, &mut branches),
                // genvar/generate (T25), ground/var/event declarations,
                // continuous assign, specify/gates: not yet lowered — later
                // tasks extend this match.
                ModuleItem::Ground(_)
                | ModuleItem::Var(_)
                | ModuleItem::Event(_)
                | ModuleItem::Genvar(_)
                | ModuleItem::Specparam(_)
                | ModuleItem::Defparam(_)
                | ModuleItem::Function(_)
                | ModuleItem::Task(_)
                | ModuleItem::ContinuousAssign(_)
                | ModuleItem::LoopGenerate(_)
                | ModuleItem::IfGenerate(_)
                | ModuleItem::CaseGenerate(_)
                | ModuleItem::GenerateRegion(_)
                | ModuleItem::Specify(_)
                | ModuleItem::Gate(_) => {}
            }
        }

        let mut module =
            Module::new(decl.name.clone(), ports, params, wires, instances, Vec::new(), behaviors);
        module.branches = branches;

        // Access-function call sites inside analog behaviors (§5.4.1) bind
        // to a branch, named or implicit, and drive its BranchKind —
        // resolve/create branches and record each contribution (T18's
        // conflict check fires here).
        for behavior in &module.behaviors {
            if !behavior.is_analog() {
                continue;
            }
            for stmt in behavior.body() {
                resolve_contribution_branches(stmt, &decl.name, &mut module.branches)?;
            }
        }

        Ok(module)
    }

    /// Lowers every top-level `nature` (LRM §3.6.1, incl. derived chains)
    /// and copies the registered `discipline` declarations into the design
    /// (T23, VAMS-01). Disciplines keep the surface AST shape verbatim
    /// (design F1 pattern: the POM re-exports the VAMS declaration rather
    /// than deep-copying it into a parallel type); natures resolve their
    /// `parent` inheritance chain into one flat, evaluated `Nature`.
    pub(crate) fn lower_declarations(&self, design: &mut Design) -> Result<(), ElabError> {
        *design.disciplines_map_mut() = self.syms.disciplines.clone();

        let names: Vec<String> = self.syms.natures.keys().cloned().collect();
        for name in names {
            let mut visiting = HashSet::new();
            let nature = self.effective_nature(&name, &mut visiting)?;
            design.natures_map_mut().insert(name, nature);
        }

        // §3.6.2.1: a discipline's potential/flow binding must name a
        // declared nature.
        for disc in self.syms.disciplines.values() {
            for (field, nature_name) in [("potential", &disc.potential), ("flow", &disc.flow)] {
                if let Some(n) = nature_name
                    && !self.syms.natures.contains_key(n)
                {
                    return Err(ElabError::from(ElabErrorKind::InvalidNatureBinding {
                        discipline: disc.name.clone(),
                        nature: n.clone(),
                        issue: format!("{field} nature `{n}` is undeclared"),
                        span: None,
                    }));
                }
            }
        }
        Ok(())
    }

    /// Resolves one nature's full attribute set by following its `parent`
    /// chain (§3.6.1.1) — a value present at this level wins; an absent one
    /// inherits from the nearest ancestor that sets it. `visiting` detects
    /// a parent cycle.
    fn effective_nature(&self, name: &str, visiting: &mut HashSet<String>) -> Result<Nature, ElabError> {
        if !visiting.insert(name.to_string()) {
            return Err(ElabError::from(ElabErrorKind::Other(format!(
                "nature `{name}`: derived-nature `parent` chain cycles back to itself"
            ))));
        }
        let decl = self
            .syms
            .natures
            .get(name)
            .ok_or_else(|| ElabError::from(ElabErrorKind::UndefinedType(name.to_string())))?;
        let parent = decl.parent.as_deref().map(|p| self.effective_nature(p, visiting)).transpose()?;

        let env = ConstEnv::new();
        let abstol = decl
            .abstol
            .as_ref()
            .map(|e| env.eval_f64(e))
            .transpose()
            .map_err(|e| ElabError::from(ElabErrorKind::ConstEval { context: format!("nature `{name}` abstol"), source: e }))?
            .or_else(|| parent.as_ref().and_then(|p| p.abstol));
        let units = decl
            .units
            .as_ref()
            .map(|e| expr_to_string(e))
            .transpose()?
            .or_else(|| parent.as_ref().and_then(|p| p.units.clone()));
        let access = decl.access.clone().or_else(|| parent.as_ref().and_then(|p| p.access.clone()));
        let idt_nature = decl.idt_nature.clone().or_else(|| parent.as_ref().and_then(|p| p.idt_nature.clone()));
        let ddt_nature = decl.ddt_nature.clone().or_else(|| parent.as_ref().and_then(|p| p.ddt_nature.clone()));

        let mut user_attrs = parent.map(|p| p.user_attrs).unwrap_or_default();
        for (k, e) in &decl.user_attrs {
            let v = env.eval(e).map_err(|e| {
                ElabError::from(ElabErrorKind::ConstEval { context: format!("nature `{name}` attribute `{k}`"), source: e })
            })?;
            user_attrs.insert(k.clone(), v);
        }

        Ok(Nature {
            span: None,
            doc: None,
            name: name.to_string(),
            parent: decl.parent.clone(),
            abstol,
            access,
            units,
            idt_nature,
            ddt_nature,
            user_attrs,
        })
    }

    /// Lowers one `Instance` (from a `module_identifier inst1(...), ...;`
    /// list) into a POM `Instance` — resolves ordered/named connections
    /// against the target module's registered port order (§6.3).
    fn lower_instance(
        &self,
        module_name: &str,
        ast_inst: &piperine_vams::ast::Instance,
        param_overrides: &[PortConnection],
    ) -> Result<Instance, ElabError> {
        let target = self.syms.module_decls.get(module_name).ok_or_else(|| {
            ElabError::from(ElabErrorKind::UndefinedModule(module_name.to_string()))
        })?;
        let target_port_names = target_port_order(target);

        let all_positional = ast_inst.connections.iter().all(|c| c.name.is_none());
        let mut ports = Vec::new();
        if all_positional {
            for c in &ast_inst.connections {
                let expr = c.value.as_ref().ok_or_else(|| {
                    ElabError::from(ElabErrorKind::Other(format!(
                        "instance `{}`: a port connection with no value is only legal for an unconnected port (`.p()`), \
                         which is not yet supported",
                        ast_inst.name
                    )))
                })?;
                ports.push(expr_to_net_ref(expr)?);
            }
        } else {
            let mut by_name: HashMap<&str, &Expr> = HashMap::new();
            for c in &ast_inst.connections {
                let name = c.name.as_deref().ok_or_else(|| {
                    ElabError::from(ElabErrorKind::Other(format!(
                        "instance `{}`: cannot mix ordered and named port connections",
                        ast_inst.name
                    )))
                })?;
                if let Some(v) = &c.value {
                    by_name.insert(name, v);
                }
            }
            for port_name in &target_port_names {
                let expr = by_name.get(port_name.as_str()).ok_or_else(|| {
                    ElabError::from(ElabErrorKind::Other(format!(
                        "instance `{}`: no connection given for port `{port_name}` of module `{module_name}`",
                        ast_inst.name
                    )))
                })?;
                ports.push(expr_to_net_ref(expr)?);
            }
        }

        let env = ConstEnv::new();
        let mut params = Vec::new();
        for ov in param_overrides {
            let name = ov.name.clone().ok_or_else(|| {
                ElabError::from(ElabErrorKind::Other(format!(
                    "instance `{}`: `#(...)` parameter overrides must be named",
                    ast_inst.name
                )))
            })?;
            let expr = ov.value.as_ref().ok_or_else(|| {
                ElabError::from(ElabErrorKind::Other(format!(
                    "instance `{}`: parameter override `{name}` has no value",
                    ast_inst.name
                )))
            })?;
            params.push((name, eval_param_default(expr, &env)?));
        }

        Ok(Instance {
            span: None,
            label_span: None,
            type_span: None,
            attributes: Vec::new(),
            doc: None,
            label: Some(ast_inst.name.clone()),
            module: module_name.to_string(),
            ports,
            params,
        })
    }
}

/// The target module's port order (declaration-list style first, else
/// name-list style items in declaration order) — needed to resolve
/// positional (ordered) instance connections (§6.3).
fn target_port_order(decl: &ModuleDecl) -> Vec<String> {
    if let Some(ports) = &decl.ports {
        ports.iter().flat_map(|p| p.names.iter().map(|d| d.name.clone())).collect()
    } else {
        decl.items
            .iter()
            .filter_map(|item| match item {
                ModuleItem::Port(p) => Some(p.names.iter().map(|d| d.name.clone())),
                _ => None,
            })
            .flatten()
            .collect()
    }
}

fn lower_port_decl(p: &PortDecl, out: &mut Vec<Port>) -> Result<(), ElabError> {
    let discipline = p.discipline.as_deref().ok_or_else(|| {
        ElabError::from(ElabErrorKind::Other(
            "port declaration has no discipline — undeclared-net discipline resolution \
             (LRM Annex F) is not implemented until T26"
                .to_string(),
        ))
    })?;
    let direction = match p.dir {
        AstDirection::Input => Direction::Input,
        AstDirection::Output => Direction::Output,
        AstDirection::Inout => Direction::Inout,
    };
    let env = ConstEnv::new();
    for d in &p.names {
        out.push(Port {
            span: None,
            attributes: Vec::new(),
            doc: None,
            direction,
            name: d.name.clone(),
            ty: declarator_net_type(d, discipline, &env)?,
        });
    }
    Ok(())
}

fn declarator_to_wire(d: &Declarator, discipline: &str, env: &ConstEnv) -> Result<Wire, ElabError> {
    Ok(Wire {
        span: None,
        attributes: Vec::new(),
        doc: None,
        name: d.name.clone(),
        ty: declarator_net_type(d, discipline, env)?,
    })
}

/// A `[msb:lsb]` range widens a scalar discipline into `NetType::Array`
/// (LRM §3.7/3.6.3 `range`) — width is `|msb - lsb| + 1`.
fn declarator_net_type(d: &Declarator, discipline: &str, env: &ConstEnv) -> Result<NetType, ElabError> {
    let base = NetType::Discipline(discipline.to_string());
    match &d.range {
        None => Ok(base),
        Some(r) => {
            let msb = env.eval_int(&r.msb).map_err(|e| {
                ElabError::from(ElabErrorKind::ConstEval { context: format!("net `{}` range msb", d.name), source: e })
            })?;
            let lsb = env.eval_int(&r.lsb).map_err(|e| {
                ElabError::from(ElabErrorKind::ConstEval { context: format!("net `{}` range lsb", d.name), source: e })
            })?;
            let width = (msb - lsb).unsigned_abs() + 1;
            Ok(NetType::Array(Box::new(base), width))
        }
    }
}

fn param_value_type(ty: Option<ParamType>) -> ValueType {
    match ty {
        Some(ParamType::Integer) => ValueType::Integer,
        Some(ParamType::Real) | Some(ParamType::Realtime) | Some(ParamType::Time) | None => ValueType::Real,
        Some(ParamType::String) => ValueType::Str,
    }
}

fn eval_param_default(expr: &Expr, env: &ConstEnv) -> Result<Value, ElabError> {
    env.eval(expr)
        .map_err(|e| ElabError::from(ElabErrorKind::ConstEval { context: "parameter default".into(), source: e }))
}

/// A port-connection value resolves to a `NetRef` — a bare identifier, or
/// `name[index]` for a bus tap.
fn expr_to_net_ref(expr: &Expr) -> Result<NetRef, ElabError> {
    match expr {
        Expr::Ident { name, .. } => Ok(NetRef::simple(name.clone())),
        Expr::Index { base, index, .. } => {
            let Expr::Ident { name, .. } = base.as_ref() else {
                return Err(ElabError::from(ElabErrorKind::Other(
                    "port connection: only `name[index]` indexing is supported".to_string(),
                )));
            };
            let idx = ConstEnv::new().eval_nat(index).map_err(|e| {
                ElabError::from(ElabErrorKind::ConstEval { context: "port connection index".into(), source: e })
            })?;
            Ok(NetRef::indexed(name.clone(), idx))
        }
        _ => Err(ElabError::from(ElabErrorKind::Other(
            "port connection must be a net name or `name[index]`".to_string(),
        ))),
    }
}

/// Evaluates a `nature`/attribute `Expr` that must be a string (e.g.
/// `units = "V"`) to its literal text.
fn expr_to_string(expr: &Expr) -> Result<String, ElabError> {
    match expr {
        Expr::Str { value, .. } => Ok(value.clone()),
        _ => Err(ElabError::from(ElabErrorKind::Other(
            "expected a string literal".to_string(),
        ))),
    }
}

/// `byte range` → `miette::SourceSpan`, mirroring
/// `From<piperine_vams::ParseError>`'s conversion (`pom/error.rs`).
fn to_miette_span(span: piperine_vams::Span) -> miette::SourceSpan {
    miette::SourceSpan::new(span.start.into(), span.end.saturating_sub(span.start))
}

/// `branch (pos[,neg]) name1, name2, ...;` (§5.2) — one POM `Branch` per
/// declared name, all sharing the same terminal pair.
fn lower_branch_decl(b: &BranchDecl, out: &mut Vec<Branch>) {
    for name in &b.names {
        out.push(Branch {
            span: Some(to_miette_span(b.span)),
            doc: None,
            name: Some(name.clone()),
            pos: NetRef::simple(b.pos.clone()),
            neg: b.neg.clone().map(NetRef::simple),
            kind: BranchKind::Probe,
        });
    }
}

/// Walks a statement tree looking for `V(...)`/`I(...)` access functions
/// used as a contribution target (`V(p,n) <+ expr;`, §5.4), resolving each
/// to a branch (a name lookup for `V(br)`, or find-or-create for the
/// two-terminal/ground-referenced implicit form, §5.4.1) and recording the
/// contribution's kind (`V` → potential, `I` → flow) — the T18
/// `record_contribution` conflict check fires here.
///
/// Scope note: only the conventional `V`/`I` access-function spellings are
/// recognized; a custom discipline's own `access` name (§3.6.1.2) is not
/// yet resolved to potential/flow here (needs the nature→discipline→access
/// lookup chain, deferred past T23's stated scope).
fn resolve_contribution_branches(stmt: &Stmt, module: &str, branches: &mut Vec<Branch>) -> Result<(), ElabError> {
    match stmt {
        Stmt::Contribution { target, span, .. } => {
            if let Expr::AccessFunction { function, pos, neg, .. } = target {
                let kind = match function.as_str() {
                    "V" => BranchKind::Potential,
                    "I" => BranchKind::Flow,
                    _ => return Ok(()),
                };
                let branch = resolve_or_create_branch(pos, neg.as_deref(), branches)?;
                branch.record_contribution(kind, Some(to_miette_span(*span)), module)?;
            }
            Ok(())
        }
        Stmt::Block { stmts, .. } => {
            stmts.iter().try_for_each(|s| resolve_contribution_branches(s, module, branches))
        }
        Stmt::If { then_branch, else_branch, .. } => {
            resolve_contribution_branches(then_branch, module, branches)?;
            if let Some(e) = else_branch {
                resolve_contribution_branches(e, module, branches)?;
            }
            Ok(())
        }
        Stmt::Case { arms, .. } => {
            arms.iter().try_for_each(|arm| resolve_contribution_branches(&arm.body, module, branches))
        }
        Stmt::For { body, .. } | Stmt::While { body, .. } | Stmt::Repeat { body, .. } | Stmt::Forever { body, .. }
        | Stmt::EventControl { body, .. } | Stmt::DelayControl { body, .. } => {
            resolve_contribution_branches(body, module, branches)
        }
        Stmt::Null { .. }
        | Stmt::Assign { .. }
        | Stmt::NonBlockingAssign { .. }
        | Stmt::Disable { .. }
        | Stmt::TaskCall { .. } => Ok(()),
    }
}

/// A single-arg access function (`V(br)`) names an already-declared branch;
/// otherwise (one or two nets) it's the implicit form — find an existing
/// implicit branch with the same terminal pair (order matters: `V(a,b)` and
/// `V(b,a)` are distinct branches, LRM §5.4.1) or create one.
fn resolve_or_create_branch<'a>(
    pos: &Expr,
    neg: Option<&Expr>,
    branches: &'a mut Vec<Branch>,
) -> Result<&'a mut Branch, ElabError> {
    if neg.is_none()
        && let Expr::Ident { name, .. } = pos
        && let Some(idx) = branches.iter().position(|b| b.name.as_deref() == Some(name.as_str()))
    {
        return Ok(&mut branches[idx]);
    }

    let pos_ref = expr_to_net_ref(pos)?;
    let neg_ref = neg.map(expr_to_net_ref).transpose()?;
    if let Some(idx) = branches
        .iter()
        .position(|b| b.is_implicit() && b.pos == pos_ref && b.neg == neg_ref)
    {
        return Ok(&mut branches[idx]);
    }

    branches.push(Branch { span: None, doc: None, name: None, pos: pos_ref, neg: neg_ref, kind: BranchKind::Probe });
    Ok(branches.last_mut().unwrap())
}

/// Validates a parameter's evaluated default against its declared
/// `from`/`exclude` ranges (§3.4.2): the value must fall in at least one
/// `from` range (when any are declared) and in none of the `exclude`
/// ranges/values. `exclude value;` (no `[lo:hi]` brackets) parses to a
/// range with `hi: None` — read as an exact-value exclusion; a `from`
/// range with `hi: None` is read as unbounded-above (the LRM's own
/// asymmetry: the bare-value shorthand is only meaningful for `exclude`,
/// §3.4.2).
fn check_param_ranges(
    name: &str,
    value: &Value,
    ranges: &[piperine_vams::ast::ParamRange],
    env: &ConstEnv,
) -> Result<(), ElabError> {
    if ranges.is_empty() {
        return Ok(());
    }
    let v = value.coerce_real().map_err(|_| {
        ElabError::from(ElabErrorKind::Other(format!(
            "parameter `{name}`: a `from`/`exclude` range requires a numeric default"
        )))
    })?;

    let eval_bound = |e: &Expr| -> Result<f64, ElabError> {
        env.eval_f64(e).map_err(|e| {
            ElabError::from(ElabErrorKind::ConstEval { context: format!("parameter `{name}` range bound"), source: e })
        })
    };

    let froms: Vec<_> = ranges.iter().filter(|r| !r.excluded).collect();
    if !froms.is_empty() {
        let mut in_any = false;
        for r in &froms {
            if in_from_range(v, r, &eval_bound)? {
                in_any = true;
                break;
            }
        }
        if !in_any {
            return Err(ElabError::from(ElabErrorKind::ParamRangeViolation {
                param: name.to_string(),
                value: v.to_string(),
                reason: "not within any declared `from` range".to_string(),
                span: None,
            }));
        }
    }
    for r in ranges.iter().filter(|r| r.excluded) {
        let excluded = match r.hi {
            None => {
                let point = r.lo.as_ref().map(|e| eval_bound(e)).transpose()?.unwrap_or(v);
                v == point
            }
            Some(_) => in_from_range(v, r, &eval_bound)?,
        };
        if excluded {
            return Err(ElabError::from(ElabErrorKind::ParamRangeViolation {
                param: name.to_string(),
                value: v.to_string(),
                reason: "falls in a declared `exclude` range".to_string(),
                span: None,
            }));
        }
    }
    Ok(())
}

fn in_from_range(
    v: f64,
    r: &piperine_vams::ast::ParamRange,
    eval_bound: &impl Fn(&Expr) -> Result<f64, ElabError>,
) -> Result<bool, ElabError> {
    let lo_ok = match &r.lo {
        None => true,
        Some(e) => {
            let lo = eval_bound(e)?;
            if r.lo_exclusive { v > lo } else { v >= lo }
        }
    };
    let hi_ok = match &r.hi {
        None => true,
        Some(e) => {
            let hi = eval_bound(e)?;
            if r.hi_exclusive { v < hi } else { v <= hi }
        }
    };
    Ok(lo_ok && hi_ok)
}
