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

use crate::ast::{
    BehaviorKind, BlockItemDecl, BranchDecl, CaseArm, ContinuousAssign, Declarator,
    Direction as AstDirection, Domain as VamsDomain, EventExpr, Expr, FunctionDecl, ModuleDecl,
    ModuleItem, ParamType, PortConnection, PortDecl, Range, Stmt, UnOp, VarKind,
};

use crate::elab::const_eval::ConstEnv;
use crate::pom::module::Attribute;
use crate::pom::{
    Behavior, Branch, BranchKind, Design, Direction, ElabError, ElabErrorKind, Instance, Module,
    Nature, NetRef, NetType, Param, Port, Value, ValueType, Var, Wire,
};

use super::Elaborator;

/// Maps every discipline's own `access` name (§3.6.1.2, via its
/// `potential`/`flow` nature binding's `Nature::access()`) to the
/// [`BranchKind`] a contribution through that name records — LRMC-C1-09.
/// Built once per elaboration run from `Design::disciplines`/`natures`
/// (populated by the `LowerDeclarations` pass, which runs before
/// `ElabModules`, so this is always complete by the time module bodies are
/// lowered). `V`/`I`/`potential`/`flow` are NOT included here — they are
/// the LRM's own always-legal names, handled directly by
/// [`resolve_contribution_branches`] regardless of what any discipline
/// declares, exactly as before this task.
pub(super) fn build_access_kind_map(design: &Design) -> HashMap<String, BranchKind> {
    let mut map = HashMap::new();
    for (_, disc) in design.disciplines() {
        if let Some(nat_name) = &disc.potential {
            if let Some(access) = design.nature(nat_name).and_then(Nature::access) {
                map.entry(access.to_string()).or_insert(BranchKind::Potential);
            }
        }
        if let Some(nat_name) = &disc.flow {
            if let Some(access) = design.nature(nat_name).and_then(Nature::access) {
                map.entry(access.to_string()).or_insert(BranchKind::Flow);
            }
        }
    }
    map
}

impl Elaborator {
    /// Elaborates one `ModuleDecl` into a POM `Module`. Structural
    /// (no cross-module resolution beyond looking up an instantiated
    /// module's port order in the already-registered symbol table).
    pub(crate) fn elab_module(
        &self,
        decl: &ModuleDecl,
        access_kinds: &HashMap<String, BranchKind>,
        default_nettype: crate::directives::NetTypeDefault,
        timescale: Option<crate::directives::Timescale>,
    ) -> Result<Module, ElabError> {
        // Mutable so each parameter becomes visible to elaboration-constant
        // evaluation for everything declared after it in this same module
        // (a later param's default, a net's array-range bound, a
        // sub-instance param override) — mirrors how a later `parameter`
        // already sees an earlier one via the identical `env.define` below.
        // A param that references itself or a not-yet-declared later name
        // fails loud through `ConstEvalError::Undefined` (C6), never panics.
        let mut env = ConstEnv::new();

        let mut ports = Vec::new();
        let mut wires = Vec::new();
        let mut params = Vec::new();
        let mut instances = Vec::new();
        let mut behaviors = Vec::new();
        let mut branches = Vec::new();
        let mut vars = Vec::new();
        // §4.7 (LRMW2-44, T44): `analog function`/`function` declarations,
        // in declaration order.
        let mut functions: Vec<crate::pom::Function> = Vec::new();
        let mut events: Vec<String> = Vec::new();
        // §3.5 (LRMW2-13, T17): a declared `genvar` registers in the POM
        // (`Module::genvars`) — collected in a prepass (not the main item
        // loop below) so the full set is known before any parameter
        // default is evaluated, regardless of declaration order.
        let genvars: Vec<String> = decl
            .items
            .iter()
            .filter_map(|item| match item {
                ModuleItem::Genvar(g) => Some(g.names.iter().cloned()),
                _ => None,
            })
            .flatten()
            .collect();
        // A genvar has no runtime storage outside a loop-generate
        // construct's own static unrolling (`elab/lower/generate.rs`'s
        // per-iteration scope) — referencing its name in an ordinary
        // (non-generate) parameter default is a use outside a valid
        // generate context, §3.5's own restriction ("genvar variable can
        // only be assigned within the for-loop control"). Checked here,
        // before any param default is evaluated, so the error names the
        // clause instead of falling through to a generic "undefined
        // name".
        check_genvar_used_outside_generate(&decl.items, &genvars)?;
        let mut constraints: Vec<crate::pom::PomConstraintStmt> = Vec::new();
        // Labels seen so far (all kinds, all blocks, any scope depth) —
        // the E4008 duplicate check's accumulator; one namespace for the
        // whole module.
        let mut constraint_labels: Vec<(String, Option<miette::SourceSpan>)> = Vec::new();
        let mut gates: Vec<crate::pom::GateInstance> = Vec::new();
        // `assign target = expr;` (§8.5.3.1/§8.5.3.2, LRMW2-06): collected
        // here, lowered into a synthesized behavior in a second pass below
        // once `ports`/`wires` are final — a target's domain (digital vs.
        // the analog default, `domain_of`'s twin `is_digital_discipline`
        // below) can only be known once its declaration is on hand, and
        // `assign` may textually precede its target's declaration.
        let mut pending_assigns: Vec<&ContinuousAssign> = Vec::new();
        // `ground [discipline] [range] name;` (§3.6.4, LRMW2-07): collected
        // here, resolved in a second pass below once `ports`/`wires` are
        // final.
        let mut pending_grounds: Vec<&crate::ast::GroundDecl> = Vec::new();

        // Declaration-list style port list (`module m(input electrical a, ...)`).
        for p in decl.ports.iter().flatten() {
            lower_port_decl(p, &self.ctx.schemas, &mut ports)?;
        }

        for item in &decl.items {
            match item {
                // Name-list style: `module m(a, b); input electrical a; ...`.
                ModuleItem::Port(p) => lower_port_decl(p, &self.ctx.schemas, &mut ports)?,
                ModuleItem::Net(n) => {
                    let discipline = n.discipline.as_deref().unwrap_or(super::disciplines::UNRESOLVED);
                    let net_attrs = lower_net_attrs(&n.attrs, &self.ctx.schemas)?;
                    for d in &n.names {
                        // Two-step port declaration (LRM §3.6/Annex F worked
                        // examples, e.g. `input p; electrical p;`): a
                        // `discipline_type` net decl naming an already-known
                        // port merges its discipline into that port rather
                        // than shadowing it with a same-named wire — the
                        // module ends up with one `p`, not a discipline-less
                        // port plus an orphan wire ResolveDisciplines can
                        // never connect back to it.
                        if let Some(port) = ports.iter_mut().find(|p| p.name == d.name) {
                            port.ty = declarator_net_type(d, discipline, &env)?;
                            continue;
                        }
                        wires.push(declarator_to_wire(
                            d,
                            discipline,
                            n.span,
                            n.doc.clone(),
                            &env,
                            net_attrs.clone(),
                            n.nodeset.as_ref(),
                        )?);
                    }
                }
                ModuleItem::Param(p) => {
                    let base_ty = param_value_type(p.ty);
                    // Numeric `from` intervals become the draw-time legal
                    // domain (`Param::from_ranges`); `exclude` and the
                    // enumerated `{…}` forms constrain authored values
                    // only (draws are reported, not constrained, by the
                    // latter — sampling explores).
                    let from_ranges: Vec<crate::pom::FromRange> = p
                        .ranges
                        .iter()
                        .filter(|r| !r.excluded && r.hi.is_some())
                        .filter_map(|r| {
                            let lo = r.lo.as_ref().and_then(|e| env.eval_f64(e).ok())?;
                            let hi = r.hi.as_ref().and_then(|e| env.eval_f64(e).ok())?;
                            Some(crate::pom::FromRange {
                                lo,
                                lo_exclusive: r.lo_exclusive,
                                hi,
                                hi_exclusive: r.hi_exclusive,
                            })
                        })
                        .collect();
                    // The `tol` clause (one per declaration) applies to
                    // every name it lists — the same sharing rule as
                    // `ranges`. Keyed args const-eval in module scope
                    // (`sigma = 0.005 / sqrt(w*l)` is a legal module
                    // constant); an unknown kind fails loud naming it.
                    let distribution = p.tol.as_ref().map(|t| {
                        let kind = match t.kind.as_str() {
                            "gauss" | "uniform" => t.kind.clone(),
                            other => {
                                return Err(ElabError::from(ElabErrorKind::Other(format!(
                                    "parameter declaration's `tol` clause: unknown distribution \
                                     `{other}` (gauss and uniform are implemented)"
                                ))));
                            }
                        };
                        let mut args = Vec::new();
                        for (name, expr) in &t.args {
                            let value = env.eval(expr).map_err(|e| {
                                ElabError::from(ElabErrorKind::ConstEval {
                                    context: format!("`tol {kind}` argument `{name}`"),
                                    source: e,
                                })
                            })?;
                            args.push((name.clone(), value));
                        }
                        Ok(crate::pom::Distribution { kind, args, global: t.global })
                    }).transpose()?;
                    // §3.4.3 (LRMW2-11, T15): same schema-checked path
                    // ports/nets already use — a `parameter` declaration's
                    // attrs shared list-wide, same sharing rule as `ranges`
                    // and `tol`.
                    let param_attrs = lower_generic_attrs(&p.attrs, &self.ctx.schemas)?;
                    for pa in &p.names {
                        let (ty, value) = eval_and_define_param_assignment(pa, &base_ty, &p.ranges, &mut env)?;
                        params.push(Param {
                            span: Some(to_miette_span(p.span)),
                            name_span: Some(to_miette_span(pa.name_span)),
                            attributes: param_attrs.clone(),
                            doc: p.doc.clone(),
                            name: pa.name.clone(),
                            ty,
                            default: Some(value),
                            bundle_origin: None,
                            distribution: distribution.clone(),
                            from_ranges: from_ranges.clone(),
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
                // value comes from elsewhere", not a new field). `ty`
                // mirrors the aliased parameter's own declared type
                // (LRMC-C1-07) — an alias over an `integer`/`string`
                // parameter must carry that type, not be hardcoded to
                // `Real`; `target` must already be declared earlier in
                // this same module (§3.4.7 gives no forward-reference
                // rule), so an unknown target fails loud by name.
                ModuleItem::AliasParam(a) => {
                    let target_ty = params
                        .iter()
                        .find(|p| p.name == a.target)
                        .map(|p| p.ty.clone())
                        .ok_or_else(|| {
                            ElabError::from(ElabErrorKind::Other(format!(
                                "aliasparam `{}`: target parameter `{}` is not declared (§3.4.7)",
                                a.name, a.target
                            )))
                        })?;
                    params.push(Param {
                        span: Some(to_miette_span(a.span)),
                        name_span: None,
                        attributes: Vec::new(),
                        doc: None,
                        name: a.name.clone(),
                        ty: target_ty,
                        default: None,
                        bundle_origin: Some(("aliasparam".to_string(), a.target.clone())),
                        distribution: None,
                        from_ranges: Vec::new(),
                        is_local: false,
                    });
                }
                ModuleItem::Instantiation(inst_decl) => {
                    // LRMW2-11/T15: same schema-checked path ports/nets
                    // already use — an instantiation's attrs shared
                    // list-wide, same sharing rule `param_overrides` has.
                    let inst_attrs = lower_generic_attrs(&inst_decl.attrs, &self.ctx.schemas)?;
                    for ast_inst in &inst_decl.instances {
                        instances.push(self.lower_instance(
                            &inst_decl.module_name,
                            inst_decl.module_name_span,
                            ast_inst,
                            &inst_decl.param_overrides,
                            &env,
                            inst_attrs.clone(),
                        )?);
                    }
                }
                ModuleItem::Behavior(b) => {
                    let stmts = match b.body.as_ref() {
                        crate::ast::Stmt::Block { stmts, .. } => stmts.clone(),
                        other => vec![other.clone()],
                    };
                    behaviors.push(Behavior::new(decl.name.clone(), b.kind, stmts));
                }
                ModuleItem::Branch(b) => lower_branch_decl(b, &mut branches),
                // `integer`/`real`/`time`/`realtime` module-level
                // declarations (LRM Annex A.2.1.3 `module_or_generate_item_
                // declaration`) — module-level persistent state (GAPS
                // §I.15's `pom::Var`, reused as-is). An optional `=
                // constant_expression` initializer (A.2.2.1) is evaluated
                // by `var_declarator_to_var` into `Var::init`.
                ModuleItem::Var(v) => {
                    let ty = var_kind_value_type(v.kind);
                    // §3.2.1 (LRMW2-11, T15): same schema-checked path
                    // ports/nets already use — a var declaration's attrs
                    // shared list-wide, same sharing rule its discipline
                    // already has.
                    let var_attrs = lower_generic_attrs(&v.attrs, &self.ctx.schemas)?;
                    for d in &v.names {
                        vars.push(var_declarator_to_var(
                            d,
                            ty.clone(),
                            v.span,
                            v.doc.clone(),
                            &env,
                            var_attrs.clone(),
                        )?);
                    }
                }
                // `event name1, name2, ...;` (§5.10.4, LRMC-C5-04) — an
                // authored declaration list, same simplicity as `genvars`
                // (MD-25: navigable exactly as written).
                // LRMC-C5-04 runtime half (T26): each declared event also
                // gets a hidden `integer` var codegen increments on
                // `-> e;` and watches for any-change on `@(e)` — the
                // actual mechanism a trigger/sensitivity pair communicates
                // through, in either domain. `module.events` (above) stays
                // the authored declaration surface (MD-25); this hidden
                // var is the derived runtime plumbing.
                ModuleItem::Event(e) => {
                    events.extend(e.names.iter().cloned());
                    for name in &e.names {
                        vars.push(Var {
                            span: Some(to_miette_span(e.span)),
                            attributes: Vec::new(),
                            doc: None,
                            name: name.clone(),
                            ty: ValueType::Integer,
                            init: None,
                        });
                    }
                }
                // `assign` is buffered; lowered below once `ports`/`wires`
                // are final (see `pending_assigns`'s own doc comment).
                ModuleItem::ContinuousAssign(ca) => pending_assigns.push(ca),
                // `ground` is buffered too — same reason (`ground gnd;`
                // most commonly follows the net's own declaration, per the
                // LRM's own §3.6.4 example, but the combined-declare form
                // `ground electrical gnd;` doesn't require it, so ordering
                // must not matter either way).
                ModuleItem::Ground(g) => pending_grounds.push(g),
                // §3.5 (LRMW2-13, T17): already collected into `genvars`
                // by the prepass above, ahead of this loop.
                ModuleItem::Genvar(_) => {}
                // `defparam` is applied by its own later pass
                // (`elab::lower::defparam::ApplyDefparams`, LRMC-C5-06) —
                // a no-op here by design, not "not yet lowered".
                // §4.7 (LRMW2-44, T44): `analog function`/`function`
                // declarations lower into the module's `functions` list —
                // the codegen inliner (`resolve/pom/expr.rs::inline_user_fn`)
                // reads it back through `SymbolTable::fn_by_name`.
                ModuleItem::Function(f) => {
                    functions.push(lower_function_decl(f)?);
                }
                // generate (T25), specify/gates: not yet lowered —
                // later tasks extend this match.
                ModuleItem::Specparam(_)
                | ModuleItem::Defparam(_)
                | ModuleItem::Task(_)
                | ModuleItem::LoopGenerate(_)
                | ModuleItem::IfGenerate(_)
                | ModuleItem::CaseGenerate(_)
                | ModuleItem::GenerateRegion(_)
                | ModuleItem::Specify(_) => {}
                // `constraint begin … end` (sanctioned extension, MD-49 +
                // `constraint-statements`): the authored statements land
                // on the POM in authored order — multiple blocks compose
                // (all statements apply), a duplicate label fails loud
                // across kinds (E4008), and nothing is resolved here:
                // name resolution, classification, and lowering are the
                // codegen `resolve` stage's job, exactly as for the
                // analog body.
                ModuleItem::Constraint(c) => {
                    for stmt in &c.stmts {
                        lower_constraint_stmt(stmt, &decl.name, &mut constraints, &mut constraint_labels)?;
                    }
                }

                ModuleItem::Gate(g) => {
                    if is_switch_level_gate(g.kind) {
                        return Err(ElabError::from(ElabErrorKind::Other(format!(
                            "gate primitive `{:?}`: switch-level primitives are not supported \
                             (Annex A.4.2) — no truth-table model exists for device-physics-level \
                             MOS/pass-transistor gates",
                            g.kind
                        ))));
                    }
                    let delay = g
                        .delay
                        .as_ref()
                        .map(|e| {
                            env.eval(e).map_err(|err| {
                                ElabError::from(ElabErrorKind::ConstEval { context: "gate delay".into(), source: err })
                            })
                        })
                        .transpose()?
                        .map(|v| v.coerce_real().unwrap_or(0.0))
                        .map(|raw| {
                            // 1364-2005: a gate `#delay` number is in the
                            // module's `` `timescale `` time unit, and the
                            // resulting time is rounded to the precision
                            // grid. Absent a directive the number carries
                            // raw simulation-time units (seconds) — the
                            // pre-existing behavior.
                            match timescale {
                                Some(ts) => {
                                    let scaled = raw * 10f64.powi(ts.unit_exponent);
                                    let grid = 10f64.powi(ts.precision_exponent);
                                    (scaled / grid).round() * grid
                                }
                                None => raw,
                            }
                        });
                    for gi in &g.instances {
                        let mut terminals = Vec::new();
                        for t in &gi.terminals {
                            terminals.push(expr_to_net_ref(t, &env)?);
                        }
                        gates.push(crate::pom::GateInstance {
                            name: gi.name.clone(),
                            kind: g.kind,
                            terminals,
                            delay,
                        });
                    }
                }
            }
        }

        // 1364/§3.7's implicit net declaration: an identifier used as an
        // instance port connection that names neither a declared port nor
        // a declared net still refers to a real (1-bit, undeclared-
        // discipline) net — it just needs registering so `ResolveDisciplines`
        // (T26) has something to resolve. Declared-name collisions are
        // skipped (already a real port/wire).
        let declared: std::collections::HashSet<String> =
            ports.iter().map(|p| p.name.clone()).chain(wires.iter().map(|w| w.name.clone())).collect();
        let mut implicit_seen = std::collections::HashSet::new();
        for inst in &instances {
            for net_ref in &inst.ports {
                let n = net_ref.net();
                if !declared.contains(n) && implicit_seen.insert(n.to_string()) {
                    if default_nettype == crate::directives::NetTypeDefault::None && !net_ref.is_unconnected() {
                        return Err(ElabError::from(ElabErrorKind::UndeclaredNetDefaultNettypeNone {
                            name: n.to_string(),
                        }));
                    }
                    wires.push(Wire {
                        span: None,
                        attributes: Vec::new(),
                        doc: None,
                        name: n.to_string(),
                        ty: NetType::Discipline(super::disciplines::UNRESOLVED.to_string()),
                        nodeset: None,
                    });
                }
            }
        }

        // `ground [discipline] [range] name;` (§3.6.4, LRMW2-07): the
        // common form (LRM's own example, `electrical gnd; ground gnd;`)
        // designates an already-declared net as the global reference node
        // — just register the name. The combined form (`ground electrical
        // gnd;` / `ground [3:0] bus;`) declares the net in the same step,
        // reusing `declarator_net_type` (the same range→`NetType::Array`
        // logic an ordinary `electrical bus[3:0];` net decl uses) so a
        // ranged ground is a real bus, not a scalar with a discarded range.
        // A `ground` naming an ALREADY-declared port/wire never creates a
        // second entry (would silently double the net) — it only adds the
        // name to `ground_names`; `resolve_node` (codegen) checks that
        // registry before ever consulting the symbol table by name, so the
        // ordinary port/wire entry simply becomes unreachable-by-name,
        // exactly how the crate-global `gnd`/`GND`/etc. names already
        // coexist with a real declared net of the same name.
        let mut ground_names: Vec<String> = Vec::new();
        for g in &pending_grounds {
            let already_declared =
                ports.iter().any(|p| p.name == g.name) || wires.iter().any(|w| w.name == g.name);
            if !already_declared {
                let discipline = g.discipline.as_deref().unwrap_or(super::disciplines::UNRESOLVED);
                let synthetic =
                    Declarator { name: g.name.clone(), range: g.range.clone(), depth_range: None, init: None };
                let ty = declarator_net_type(&synthetic, discipline, &env)?;
                wires.push(Wire {
                    span: Some(to_miette_span(g.span)),
                    attributes: Vec::new(),
                    doc: None,
                    name: g.name.clone(),
                    ty,
                    nodeset: None,
                });
            }
            ground_names.push(g.name.clone());
        }

        // `assign target = expr;` (LRMW2-06, §8.5.3.1/§8.5.3.2): a
        // digital-domain target lowers to the `always @(*)` combinational
        // shape (same Active-region participation as a blocking assign,
        // §8.5.3.1); an analog-domain target (`wreal`'s own discipline is
        // never declared, so it hits the same "unknown discipline
        // defaults to analog" rule `resolve/pom/structure.rs::domain_of`
        // already applies at codegen time, §6.5.3) lowers to the
        // ideal-source contribution idiom this codebase already uses
        // pervasively (`V(net) <+ expr;`, implicit-ground branch). Only a
        // bare net-name target is handled — anything else (an indexed/
        // hierarchical LHS) fails loud rather than silently doing
        // nothing, same as the pre-existing no-op did NOT do.
        for ca in &pending_assigns {
            let Expr::Ident { name: target_name, .. } = &ca.target else {
                return Err(ElabError::from(ElabErrorKind::Other(format!(
                    "assign target must be a plain net name (§8.5.3.1) — \
                     indexed/hierarchical continuous-assign targets are not \
                     yet supported: {:?}",
                    ca.target
                ))));
            };
            let ty = ports
                .iter()
                .find(|p| &p.name == target_name)
                .map(|p| &p.ty)
                .or_else(|| wires.iter().find(|w| &w.name == target_name).map(|w| &w.ty))
                .ok_or_else(|| {
                    ElabError::from(ElabErrorKind::Other(format!(
                        "assign target `{target_name}` is not a declared net or port (§8.5.3.1)"
                    )))
                })?;
            let discipline = ty.discipline_name();
            let is_digital = self
                .syms
                .disciplines
                .get(discipline)
                .and_then(|d| d.domain)
                == Some(VamsDomain::Discrete);
            let stmt = if is_digital {
                Stmt::EventControl {
                    span: ca.span,
                    events: vec![EventExpr::ImplicitAll],
                    body: Box::new(Stmt::Assign {
                        span: ca.span,
                        target: ca.target.clone(),
                        value: ca.value.clone(),
                    }),
                }
            } else {
                Stmt::Contribution {
                    span: ca.span,
                    target: Expr::AccessFunction {
                        span: ca.span,
                        function: "V".to_string(),
                        pos: Box::new(ca.target.clone()),
                        neg: None,
                    },
                    value: ca.value.clone(),
                }
            };
            let kind = if is_digital { BehaviorKind::Always } else { BehaviorKind::Analog };
            behaviors.push(Behavior::new(decl.name.clone(), kind, vec![stmt]));
        }

        let mut module =
            Module::new(decl.name.clone(), ports, params, wires, instances, Vec::new(), behaviors);
        module.span = Some(to_miette_span(decl.span));
        module.doc = decl.doc.clone();
        module.branches = branches;
        module.vars = vars;
        module.functions = functions;
        module.events = events;
        module.genvars = genvars;
        module.constraints = constraints;
        // 1364-2005 `timescale: the last directive at or before the
        // module's start governs it (resolved by the `ElabModules` pass,
        // which owns the design's `DirectiveState`).
        module.timescale = timescale;
        module.gates = gates;
        module.ground_names = ground_names;
        module.attributes = lower_module_attrs(&decl.attrs, &self.ctx.schemas)?;

        // LRMC-C5-04: `-> event_name;` must name a declared `event` —
        // undeclared fails loud citing the trigger, not a silent no-op.
        for behavior in &module.behaviors {
            for stmt in behavior.body() {
                check_event_triggers(stmt, &module.events, &decl.name)?;
            }
        }

        // Access-function call sites inside analog behaviors (§5.4.1) bind
        // to a branch, named or implicit, and drive its BranchKind —
        // resolve/create branches and record each contribution (T18's
        // conflict check fires here).
        for behavior in &module.behaviors {
            if !behavior.is_analog() {
                continue;
            }
            for stmt in behavior.body() {
                resolve_contribution_branches(stmt, &decl.name, &mut module.branches, access_kinds, false)?;
            }
        }

        // LRMC-C1-10: `I(<port>)` port-access reads have no branch of
        // their own — rewrite each into the KCL-equivalent sum of this
        // module's own branch flows touching that port (§4.4/§5.4.3),
        // using the branches `resolve_contribution_branches` just
        // finished resolving above. Runs over every behavior (not only
        // analog — a `PortAccess` cannot occur in digital source, but the
        // rewrite is a no-op there) so `Expr::PortAccess` never survives
        // past this function.
        let port_branches = module.branches.clone();
        for behavior in &mut module.behaviors {
            behavior.body = std::mem::take(&mut behavior.body)
                .into_iter()
                .map(|stmt| rewrite_port_access_stmt(stmt, &port_branches))
                .collect();
        }

        // T6 (LRMW3-01): a port-connection index (`inst(bus[9])`,
        // `expr_to_net_ref`'s `name[index]` form) that names an
        // out-of-range bit or indexes a scalar net is a loud error here,
        // rather than surfacing later as a generic "unresolved net" once
        // flattening/circuit-build looks for a node that was never
        // registered.
        validate_indexed_net_refs(&module)?;

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
        *design.paramsets_map_mut() = self.syms.paramsets.clone();
        *design.udps_map_mut() = self.syms.primitives.clone();

        // `const <name> = <expr>;` (sanctioned extension, MD-50): every
        // top-level const is const-evaluated in source order (a later
        // const may reference an earlier one, mirroring the param rule)
        // into `Design::consts` — the surface `LowerCtx::const_irs`
        // reads into every module's scope. An unresolvable name fails
        // loud, never a silently-missing constant.
        {
            let mut env = ConstEnv::new();
            for c in &self.pending_consts {
                let value = env.eval(&c.value).map_err(|e| {
                    ElabError::from(ElabErrorKind::ConstEval { context: format!("const `{}`", c.name), source: e })
                })?;
                design.consts_map_mut().insert(c.name.clone(), value.clone());
                env.define(c.name.clone(), value);
            }
        }

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
            span: Some(to_miette_span(decl.span)),
            doc: decl.doc.clone(),
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
    /// against the target module's registered port order (§6.3). `env`
    /// carries whatever elaboration-constant scope is live at this call
    /// site — a fresh empty one for ordinary module-body instantiation, or
    /// a generate-loop's genvar-bound scope (`elab/lower/generate.rs`,
    /// T25) so `res r1(bus[i])` inside `for (i=...)` resolves `i`.
    pub(super) fn lower_instance(
        &self,
        module_name: &str,
        module_name_span: crate::error::PreprocessedSpan,
        ast_inst: &crate::ast::Instance,
        param_overrides: &[PortConnection],
        env: &ConstEnv,
        attributes: Vec<Attribute>,
    ) -> Result<Instance, ElabError> {
        // LRMC-C5-07: an instance may name a `paramset` instead of a
        // module — resolves to `(paramset.module_name, bound params)`,
        // §6.4's own indirection.
        if let Some(candidates) = self.syms.paramsets.get(module_name) {
            let ps = select_paramset(module_name, candidates, param_overrides, env)?;
            return self.lower_paramset_instance(
                module_name, ps, ast_inst, param_overrides, env, module_name_span, attributes,
            );
        }

        // LRMC-C7-13 (T39): a UDP instantiation is syntactically
        // identical to a module instantiation (Annex A.5's own
        // `udp_instance` production reuses `name_of_instance
        // (terminal...)`) — resolved the same way a paramset instance is,
        // against the UDP's own canonical port order instead of a
        // module's. UDPs take no `#(...)` parameter overrides (Annex
        // A.5.1 has no such production); a non-empty `param_overrides`
        // here fails loud rather than silently discarding it.
        if let Some(udp) = self.syms.primitives.get(module_name) {
            if !param_overrides.is_empty() {
                return Err(ElabError::from(ElabErrorKind::Other(format!(
                    "instance `{}`: UDP `{module_name}` does not accept `#(...)` parameter overrides",
                    ast_inst.name
                ))));
            }
            let target_port_names = udp_port_order(udp);
            let ports = resolve_instance_ports(ast_inst, module_name, &target_port_names, env)?;
            return Ok(Instance {
                span: Some(to_miette_span(ast_inst.span)),
                label_span: Some(to_miette_span(ast_inst.name_span)),
                type_span: Some(to_miette_span(module_name_span)),
                attributes,
                doc: None,
                label: Some(ast_inst.name.clone()),
                module: module_name.to_string(),
                ports,
                params: Vec::new(),
                range: instance_range(ast_inst, env)?,
            });
        }

        let target = self.syms.module_decls.get(module_name).ok_or_else(|| {
            ElabError::from(ElabErrorKind::UndefinedModule(module_name.to_string()))
        })?;
        let target_port_names = target_port_order(target);
        let ports = resolve_instance_ports(ast_inst, module_name, &target_port_names, env)?;

        // §3.4.2: a declared `from`/`exclude` range constrains every value
        // the parameter takes — the declared default *and* an
        // instantiation override. `xfmr #(.k(1.5))` against
        // `from (0:1)` must fail here, not slip through silent (fail
        // loud, CLAUDE.md). Range bounds evaluate against the target
        // module's own param defaults — the same environment the
        // declared default was checked against — built lazily because
        // only a ranged param needs it.
        let ranges_by_param = declared_param_ranges(target.items.iter().filter_map(|item| {
            match item { ModuleItem::Param(p) => Some(p), _ => None }
        }));
        let bounds_env_needed = param_overrides
            .iter()
            .any(|ov| ov.name.as_deref().is_some_and(|n| ranges_by_param.contains_key(n)));
        let bounds_env = if bounds_env_needed { Some(module_param_env(target)?) } else { None };

        // §6.3.2/§6.3.3: an instance's `#(...)` overrides are either all
        // positional (bound in declaration order) or all named — the LRM
        // gives no rule for mixing the two forms within one instance, so a
        // `#(...)` list with both kinds fails loud rather than guessing an
        // interpretation.
        let all_positional = !param_overrides.is_empty() && param_overrides.iter().all(|ov| ov.name.is_none());
        let all_named = param_overrides.iter().all(|ov| ov.name.is_some());
        if !param_overrides.is_empty() && !all_positional && !all_named {
            return Err(ElabError::from(ElabErrorKind::Other(format!(
                "instance `{}`: `#(...)` parameter overrides mix positional and named forms \
                 (§6.3.2/§6.3.3 define no combined form — use one style per instance)",
                ast_inst.name
            ))));
        }

        let mut params = Vec::new();
        if all_positional {
            // §6.3.2: bound in the order the module declares its
            // (non-local, non-alias) parameters. "the left-most parameter
            // assignment(s) can not be skipped" is naturally satisfied by
            // positional binding; supplying more overrides than declared
            // parameters fails loud rather than silently dropping the
            // extras.
            let ordered_names = target_param_order(target);
            if param_overrides.len() > ordered_names.len() {
                return Err(ElabError::from(ElabErrorKind::Other(format!(
                    "instance `{}`: {} positional parameter override(s) given but module `{module_name}` \
                     declares only {} parameter(s) (§6.3.2)",
                    ast_inst.name,
                    param_overrides.len(),
                    ordered_names.len(),
                ))));
            }
            for (ov, name) in param_overrides.iter().zip(ordered_names.iter()) {
                let expr = ov.value.as_ref().ok_or_else(|| {
                    ElabError::from(ElabErrorKind::Other(format!(
                        "instance `{}`: positional parameter override for `{name}` has no value",
                        ast_inst.name
                    )))
                })?;
                let value = eval_param_default(expr, env)?;
                if let (Some(ranges), Some(bounds)) =
                    (ranges_by_param.get(name.as_str()).copied(), &bounds_env)
                {
                    check_param_ranges(name, &value, ranges, bounds)?;
                }
                params.push((name.clone(), value));
            }
        } else {
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
                let value = eval_param_default(expr, env)?;
                if let (Some(ranges), Some(bounds)) =
                    (ranges_by_param.get(name.as_str()).copied(), &bounds_env)
                {
                    check_param_ranges(&name, &value, ranges, bounds)?;
                }
                params.push((name, value));
            }
        }

        Ok(Instance {
            span: Some(to_miette_span(ast_inst.span)),
            label_span: Some(to_miette_span(ast_inst.name_span)),
            type_span: Some(to_miette_span(module_name_span)),
            attributes,
            doc: None,
            label: Some(ast_inst.name.clone()),
            module: module_name.to_string(),
            ports,
            params,
            range: instance_range(ast_inst, env)?,
        })
    }

    /// An instance naming a `paramset` (§6.4) resolves to the paramset's
    /// associated module, with `params` computed from the paramset's own
    /// `.field = expr;` statements — evaluated against the paramset's own
    /// parameters (seeded with their defaults, then overridden by this
    /// instance's `#(...)`, exactly like an ordinary module instantiation,
    /// just against the paramset's param list instead of a module's).
    /// "An instance ... that attempts to override any of the other
    /// parameters of the underlying module ... would generate an error"
    /// (§6.4's own text) — an override naming anything other than one of
    /// the paramset's own declared parameters fails loud. Likewise, a
    /// `.field = expr;` statement naming a parameter the associated
    /// module doesn't declare fails loud (the paramset's own ".model
    /// style filtering/type-check").
    fn lower_paramset_instance(
        &self,
        paramset_name: &str,
        ps: &crate::ast::ParamsetDecl,
        ast_inst: &crate::ast::Instance,
        param_overrides: &[PortConnection],
        env: &ConstEnv,
        module_name_span: crate::error::PreprocessedSpan,
        attributes: Vec<Attribute>,
    ) -> Result<Instance, ElabError> {
        let target = self.syms.module_decls.get(&ps.module_name).ok_or_else(|| {
            ElabError::from(ElabErrorKind::Other(format!(
                "paramset `{paramset_name}`: associated module `{}` does not exist",
                ps.module_name
            )))
        })?;
        let target_port_names = target_port_order(target);
        let ports = resolve_instance_ports(ast_inst, paramset_name, &target_port_names, env)?;
        let target_param_names: HashSet<&str> =
            target.items.iter().filter_map(|item| match item {
                ModuleItem::Param(p) => Some(p.names.iter().map(|pa| pa.name.as_str())),
                _ => None,
            }).flatten().collect();

        // The paramset's own parameters, defaults first (range-checked by
        // `eval_and_define_param`, same as a module's own), then this
        // instance's overrides applied on top (in declaration order, so a
        // later default may reference an earlier override — same rule
        // `module_param_env` already follows for ordinary module params).
        // An override naming a ranged paramset parameter is checked
        // against the paramset's declared ranges (§3.4.2, fail loud).
        let ps_env = build_paramset_env(ps, param_overrides, env).map_err(|e| match e {
            ParamsetEnvError::UnknownOverride(name) => ElabError::from(ElabErrorKind::Other(format!(
                "instance `{}`: `{name}` is not a parameter of paramset `{paramset_name}` \
                 (only the paramset's own declared parameters may be overridden, §6.4)",
                ast_inst.name
            ))),
            ParamsetEnvError::UnnamedOverride => ElabError::from(ElabErrorKind::Other(format!(
                "instance `{}`: `#(...)` parameter overrides must be named",
                ast_inst.name
            ))),
            ParamsetEnvError::MissingOverrideValue(name) => ElabError::from(ElabErrorKind::Other(format!(
                "instance `{}`: parameter override `{name}` has no value",
                ast_inst.name
            ))),
            ParamsetEnvError::Elab(e) => e,
        })?;

        // A `.field = expr;` statement binds the associated module's
        // parameter — the final value that parameter takes, so the
        // module's own declared ranges apply to it too (§3.4.2/§6.4,
        // fail loud). Bounds evaluate against the module's own defaults.
        let target_ranges = declared_param_ranges(target.items.iter().filter_map(|item| {
            match item { ModuleItem::Param(p) => Some(p), _ => None }
        }));
        let target_bounds = if ps.statements.iter().any(|s| target_ranges.contains_key(s.target.as_str())) {
            Some(module_param_env(target)?)
        } else {
            None
        };
        let mut params = Vec::new();
        for stmt in &ps.statements {
            if !target_param_names.contains(stmt.target.as_str()) {
                return Err(ElabError::from(ElabErrorKind::Other(format!(
                    "paramset `{paramset_name}`: `.{}` names no parameter of module `{}`",
                    stmt.target, ps.module_name
                ))));
            }
            let value = ps_env.eval(&stmt.value).map_err(|e| {
                ElabError::from(ElabErrorKind::ConstEval {
                    context: format!("paramset `{paramset_name}` statement `.{}`", stmt.target),
                    source: e,
                })
            })?;
            if let (Some(ranges), Some(bounds)) =
                (target_ranges.get(stmt.target.as_str()).copied(), &target_bounds)
            {
                check_param_ranges(&stmt.target, &value, ranges, bounds)?;
            }
            params.push((stmt.target.clone(), value));
        }

        Ok(Instance {
            span: Some(to_miette_span(ast_inst.span)),
            label_span: Some(to_miette_span(ast_inst.name_span)),
            type_span: Some(to_miette_span(module_name_span)),
            attributes,
            doc: None,
            label: Some(ast_inst.name.clone()),
            module: ps.module_name.clone(),
            ports,
            params,
            // Array-of-instances via a paramset (`ps8 [7:0] (...)`) is out
            // of LRMC-C5-07/C5-08's own combined scope — not asserted by
            // either task's Done-when, disclosed rather than silently
            // dropped: `ast_inst.range` is simply not read on this branch.
            range: None,
        })
    }
}

/// A failure building a paramset's own parameter environment — kept
/// distinct from [`ElabError`] so [`select_paramset`] (LRMW2-22, T29,
/// §6.4.2 overloading) can treat "this candidate doesn't fit" (unknown
/// override, out-of-range value) as *exclusion from the candidate set*
/// rather than an immediate hard failure, while
/// [`lower_paramset_instance`]'s single-candidate call site still turns
/// every variant into the same loud, named `ElabError` it always raised.
enum ParamsetEnvError {
    UnnamedOverride,
    UnknownOverride(String),
    MissingOverrideValue(String),
    Elab(ElabError),
}

impl From<ElabError> for ParamsetEnvError {
    fn from(e: ElabError) -> Self {
        ParamsetEnvError::Elab(e)
    }
}

/// Builds one paramset's own parameter environment: declared defaults
/// first (range-checked, §3.4.2), then `param_overrides` applied on top
/// (checked against the paramset's own declared ranges, §6.4.2's first two
/// selection rules — "all parameters overridden on the instance shall be
/// parameters of the paramset" and "within the allowed ranges"). Shared by
/// [`Elaborator::lower_paramset_instance`] (the single-paramset path) and
/// [`select_paramset`] (the overloaded path, which tries this per
/// candidate and excludes any that error).
fn build_paramset_env(
    ps: &crate::ast::ParamsetDecl,
    param_overrides: &[PortConnection],
    env: &ConstEnv,
) -> Result<ConstEnv, ParamsetEnvError> {
    let ps_ranges = declared_param_ranges(ps.params.iter());
    let mut ps_env = ConstEnv::new();
    for p in &ps.params {
        for pa in &p.names {
            eval_and_define_param(&pa.name, &pa.value, &p.ranges, &mut ps_env)?;
        }
    }
    for ov in param_overrides {
        let name = ov.name.clone().ok_or(ParamsetEnvError::UnnamedOverride)?;
        if !ps.params.iter().any(|p| p.names.iter().any(|pa| pa.name == name)) {
            return Err(ParamsetEnvError::UnknownOverride(name));
        }
        let expr = ov.value.as_ref().ok_or_else(|| ParamsetEnvError::MissingOverrideValue(name.clone()))?;
        let value = eval_param_default(expr, env)?;
        if let Some(ranges) = ps_ranges.get(name.as_str()).copied() {
            check_param_ranges(&name, &value, ranges, &ps_env)?;
        }
        ps_env.define(name, value);
    }
    Ok(ps_env)
}

/// §6.4.2 paramset overloading: several `paramset` declarations may share
/// `paramset_name`. Picks the one candidate whose declared parameters
/// cover every override on this instance, and whose overridden-and-default
/// parameter values all fall within its own declared ranges — the two
/// selection rules this task implements (the LRM's further tie-break
/// cascade — fewest un-overridden parameters, most-ranged local
/// parameters, fewest unconnected ports — is not implemented; a genuine
/// tie after the first two rules is reported as an ambiguity error rather
/// than silently resolved by an unimplemented rule).
fn select_paramset<'a>(
    paramset_name: &str,
    candidates: &'a [crate::ast::ParamsetDecl],
    param_overrides: &[PortConnection],
    env: &ConstEnv,
) -> Result<&'a crate::ast::ParamsetDecl, ElabError> {
    if candidates.len() == 1 {
        // The common (non-overloaded) case: no candidate filtering needed
        // — a bad override still gets `lower_paramset_instance`'s own
        // named error, not a generic "no candidate matched".
        return Ok(&candidates[0]);
    }
    let matches: Vec<&crate::ast::ParamsetDecl> = candidates
        .iter()
        .filter(|ps| build_paramset_env(ps, param_overrides, env).is_ok())
        .collect();
    match matches.len() {
        1 => Ok(matches[0]),
        0 => Err(ElabError::from(ElabErrorKind::Other(format!(
            "no `{paramset_name}` paramset among its {} overloaded declarations accepts this instance's \
             parameter overrides within its declared ranges (LRM §6.4.2)",
            candidates.len()
        )))),
        n => Err(ElabError::from(ElabErrorKind::Other(format!(
            "ambiguous paramset selection for `{paramset_name}`: {n} overloaded declarations (targeting {}) \
             all accept this instance's parameter overrides — LRM §6.4.2 requires a unique match \
             (its further tie-break rules are not implemented; disambiguate the ranges or overrides)",
            matches.iter().map(|m| m.module_name.as_str()).collect::<Vec<_>>().join(", ")
        )))),
    }
}

/// The target module's port order (declaration-list style first, else
/// name-list style items in declaration order) — needed to resolve
/// positional (ordered) instance connections (§6.3).
/// Resolves an instance's port connections (ordered or named, never mixed)
/// against `target_port_names` — shared by ordinary module instantiation
/// and paramset instantiation (`lower_paramset_instance`), whose ports
/// resolve the same way against the paramset's *associated module*'s port
/// order.
/// Evaluates an instance's optional `[msb:lsb]` array range (§6.2.2,
/// LRMC-C5-08) into `(msb, lsb)` — `None` for an ordinary (non-array)
/// instance. Stored on the authored `pom::Instance` as-is; expansion into
/// per-element leaf instances is `FlattenHierarchy`'s job (MD-25).
fn instance_range(ast_inst: &crate::ast::Instance, env: &ConstEnv) -> Result<Option<(i64, i64)>, ElabError> {
    let Some(r) = &ast_inst.range else { return Ok(None) };
    let msb = env.eval_int(&r.msb).map_err(|e| {
        ElabError::from(ElabErrorKind::ConstEval {
            context: format!("instance `{}` array range msb", ast_inst.name),
            source: e,
        })
    })?;
    let lsb = env.eval_int(&r.lsb).map_err(|e| {
        ElabError::from(ElabErrorKind::ConstEval {
            context: format!("instance `{}` array range lsb", ast_inst.name),
            source: e,
        })
    })?;
    Ok(Some((msb, lsb)))
}

fn resolve_instance_ports(
    ast_inst: &crate::ast::Instance,
    _type_name: &str,
    target_port_names: &[String],
    env: &ConstEnv,
) -> Result<Vec<NetRef>, ElabError> {
    let all_positional = ast_inst.connections.iter().all(|c| c.name.is_none());
    let mut ports = Vec::new();
    if all_positional {
        // §9.19 (LRMC-C7-09, T50): `.p()` or a blank positional slot
        // (`mod inst(a, , c)`) leaves that port genuinely unconnected —
        // legal Verilog-AMS, not an error. `NetRef::unconnected()`'s
        // sentinel carries this through to `$port_connected()`.
        for (i, c) in ast_inst.connections.iter().enumerate() {
            match &c.value {
                Some(expr) => ports.push(expr_to_net_ref(expr, env)?),
                None => ports.push(NetRef::unconnected(&format!("{}.{i}", ast_inst.name))),
            }
        }
        // Fewer positional connections than declared ports, with no
        // trailing blank slots to mark it (`myclk c1 (q1);` against two
        // declared ports) — §9.19: the missing trailing ports are just
        // as unconnected as an explicit blank slot would be.
        while ports.len() < target_port_names.len() {
            let i = ports.len();
            ports.push(NetRef::unconnected(&format!("{}.{i}", ast_inst.name)));
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
        for port_name in target_port_names {
            match by_name.get(port_name.as_str()) {
                // A named connection with a value (`.p(net)`).
                Some(expr) => ports.push(expr_to_net_ref(expr, env)?),
                // Either `.p()` (present, blank) or the port's name
                // never appeared at all (§9.19: both are "unconnected",
                // not an error — a design may omit ports it doesn't
                // need, exactly the clock-module example the LRM's own
                // `$port_connected()` motivation text gives).
                None => ports.push(NetRef::unconnected(&format!("{}.{port_name}", ast_inst.name))),
            }
        }
    }
    Ok(ports)
}

/// Switch-level gate keywords (Annex A.4.2) still rejected — the
/// device-physics MOS primitives (`nmos`/`pmos`/`cmos`/`r{n,p,c}mos`) and
/// the resistive pull primitives (`pullup`/`pulldown`) have no
/// truth-table model, distinct from the 12 digital gate keywords that
/// reach the POM (LRMC-C7-13, T39). T67/LRMW2-64 (§8.5.3.5) moved the
/// bidirectional pass-switch family (`tran`/`rtran`/`tranif0/1`/
/// `rtranif0/1`) out of this rejected set — see
/// `piperine-codegen::kernel::digital::gates`'s own module doc for that
/// family's (disclosed, unidirectional-only) model.
fn is_switch_level_gate(kind: crate::ast::GateKind) -> bool {
    use crate::ast::GateKind::*;
    matches!(kind, Nmos | Pmos | Cmos | Rnmos | Rpmos | Rcmos | Pullup | Pulldown)
}

/// A UDP's canonical port order (Annex A.5.1: the first port is always
/// the output, followed by inputs in declaration order) — flattened from
/// `PrimitiveDecl::ports` the same way `target_port_order` flattens a
/// module's port list. `Reg` entries are the output's `reg` re-
/// declaration, not a distinct port, and are skipped.
fn udp_port_order(decl: &crate::ast::PrimitiveDecl) -> Vec<String> {
    decl.ports
        .iter()
        .flat_map(|p| -> Vec<String> {
            match p {
                crate::ast::UdpPortDecl::Output { name, .. } => vec![name.clone()],
                crate::ast::UdpPortDecl::Input { names, .. } => names.clone(),
                crate::ast::UdpPortDecl::Reg { .. } => vec![],
            }
        })
        .collect()
}

/// The module's declared, non-local parameters in source order — §6.3.2's
/// binding order for `#(pos0, pos1, ...)` positional overrides.
/// `localparam` declarations are excluded per §6.3.2 ("Local parameter
/// declarations are not considered when assigning by order"); `aliasparam`
/// is a distinct `ModuleItem` variant and is never yielded by the
/// `ModuleItem::Param` filter, satisfying the same sentence's "parameter
/// alias declarations are also skipped".
fn target_param_order(decl: &ModuleDecl) -> Vec<String> {
    decl.items
        .iter()
        .filter_map(|item| match item {
            ModuleItem::Param(p) if !p.is_local => {
                Some(p.names.iter().map(|pa| pa.name.clone()))
            }
            _ => None,
        })
        .flatten()
        .collect()
}

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

fn lower_port_decl(
    p: &PortDecl,
    known_schemas: &HashSet<String>,
    out: &mut Vec<Port>,
) -> Result<(), ElabError> {
    // An absent discipline (`inout p;` with no leading discipline name) is
    // left as the `ResolveDisciplines` pass's sentinel (T26, LRM Annex F) —
    // resolved from connectivity after every module is lowered, never
    // guessed here.
    let discipline = p.discipline.as_deref().unwrap_or(super::disciplines::UNRESOLVED);
    let direction = match p.dir {
        AstDirection::Input => Direction::Input,
        AstDirection::Output => Direction::Output,
        AstDirection::Inout => Direction::Inout,
    };
    let env = ConstEnv::new();
    let attributes = lower_port_attrs(&p.attrs, known_schemas)?;
    for d in &p.names {
        out.push(Port {
            span: Some(to_miette_span(p.span)),
            attributes: attributes.clone(),
            doc: p.doc.clone(),
            direction,
            name: d.name.clone(),
            ty: declarator_net_type(d, discipline, &env)?,
        });
    }
    Ok(())
}

/// Lower one module declaration's `(* ... *)` attribute instances into the
/// plugin system's `device` schema (VAMS-12/13, T35 — re-anchors PHDL's
/// `@device(plugin = …, type = …)` attribute, design P2). No `device` attr
/// present ⇒ no attributes. Any attribute name that is neither `device`,
/// its required `type` companion, nor a name `known_schemas` has itself
/// registered fails loud (`ElabErrorKind::UnknownAttrSchema`, MD-24's
/// "no silent fallback" ported to VAMS-13).
///
/// SPEC_DEVIATION: VAMS's `attr_spec` (LRM §2.9.1) is `attr_name [=
/// constant_expression]` — there is no nested-call form for a PHDL-style
/// `@device(plugin = …, type = …)` attribute. The schema's two fields
/// travel as two sibling attribute names in one `(* device = "plugin",
/// type = "type_id" *)` instance instead: `device`'s own value is the
/// `plugin` field, a required sibling `type` attribute is the `type`
/// field — the exact spelling `design.md`'s P2 story quotes.
/// Generic attribute-instance lowering for declaration sites with no
/// schema-specific field structure of their own — parameters (§3.4.3),
/// variables (§3.2.1), and instances (LRMW2-11, T15): every attr name
/// must be a registered schema, the same loud `UnknownAttrSchema` check
/// `lower_port_attrs`/`lower_net_attrs`/`lower_module_attrs` already run
/// (MD-24's "no silent fallback" rule). Unlike those three, this site has
/// no sibling-field schema of its own (no `device`/`port`/`rfport`
/// special-casing) — each attr becomes its own single-field `Attribute`:
/// `schema` is the attr name, and a valued attr's one data field (also
/// keyed by the attr name) holds its const-evaluated value; a bare marker
/// attr (`(* foo *)`, no `= value`) gets an empty data map.
pub(super) fn lower_generic_attrs(
    attrs: &[crate::ast::Attr],
    known_schemas: &HashSet<String>,
) -> Result<Vec<Attribute>, ElabError> {
    let env = ConstEnv::new();
    attrs
        .iter()
        .map(|a| {
            if !known_schemas.contains(&a.name) {
                return Err(ElabError::from(ElabErrorKind::UnknownAttrSchema(a.name.clone()))
                    .with_span(Some(to_miette_span(a.span))));
            }
            let mut data = HashMap::new();
            if let Some(expr) = &a.value {
                let value = env.eval(expr).map_err(|e| {
                    ElabError::from(ElabErrorKind::ConstEval {
                        context: format!("attribute `{}`", a.name),
                        source: e,
                    })
                    .with_span(Some(to_miette_span(a.span)))
                })?;
                data.insert(a.name.clone(), value);
            }
            Ok(Attribute { schema: a.name.clone(), data })
        })
        .collect()
}

fn lower_module_attrs(
    attrs: &[crate::ast::Attr],
    known_schemas: &HashSet<String>,
) -> Result<Vec<Attribute>, ElabError> {
    let mut out = Vec::new();
    let has_device = if let Some(a) = lower_device_attr(attrs, known_schemas)? {
        out.push(a);
        true
    } else {
        false
    };
    for a in attrs {
        if has_device && matches!(a.name.as_str(), "device" | "type") {
            continue;
        }
        if !known_schemas.contains(&a.name) {
            return Err(ElabError::from(ElabErrorKind::UnknownAttrSchema(a.name.clone()))
                .with_span(Some(to_miette_span(a.span))));
        }
    }
    Ok(out)
}

/// Lower one port declaration's `(* ... *)` attribute instances into the
/// plugin system's `port` schema (VAMS-12/13, T35 — re-anchors PHDL's
/// `@port(name = …, kind = …)` attribute). `kind` is optional, matching
/// PHDL's `port { name: String, kind: String? }` schema. Same
/// sibling-attribute SPEC_DEVIATION as [`lower_module_attrs`].
fn lower_port_attrs(
    attrs: &[crate::ast::Attr],
    known_schemas: &HashSet<String>,
) -> Result<Vec<Attribute>, ElabError> {
    let mut out = Vec::new();
    let has_port = if let Some(a) = lower_port_attr(attrs, known_schemas)? {
        out.push(a);
        true
    } else {
        false
    };
    for a in attrs {
        if has_port && matches!(a.name.as_str(), "port" | "kind") {
            continue;
        }
        if !known_schemas.contains(&a.name) {
            return Err(ElabError::from(ElabErrorKind::UnknownAttrSchema(a.name.clone()))
                .with_span(Some(to_miette_span(a.span))));
        }
    }
    Ok(out)
}

fn lower_device_attr(
    attrs: &[crate::ast::Attr],
    known_schemas: &HashSet<String>,
) -> Result<Option<Attribute>, ElabError> {
    let Some(dev) = attrs.iter().find(|a| a.name == "device") else { return Ok(None) };
    if !known_schemas.contains("device") {
        return Err(ElabError::from(ElabErrorKind::UnknownAttrSchema("device".to_string()))
            .with_span(Some(to_miette_span(dev.span))));
    }
    let env = ConstEnv::new();
    let plugin = attr_string_value(dev, "device", "plugin", &env)?;
    let ty_attr = attrs.iter().find(|a| a.name == "type").ok_or_else(|| {
        ElabError::from(ElabErrorKind::AttrSchemaField {
            schema: "device".into(),
            field: "type".into(),
            reason: "missing — @device needs a `type` string alongside it".into(),
        })
        .with_span(Some(to_miette_span(dev.span)))
    })?;
    let ty = attr_string_value(ty_attr, "device", "type", &env)?;
    let mut data = HashMap::new();
    data.insert("plugin".to_string(), plugin);
    data.insert("type".to_string(), ty);
    Ok(Some(Attribute { schema: "device".to_string(), data }))
}

fn lower_port_attr(
    attrs: &[crate::ast::Attr],
    known_schemas: &HashSet<String>,
) -> Result<Option<Attribute>, ElabError> {
    let Some(p) = attrs.iter().find(|a| a.name == "port") else { return Ok(None) };
    if !known_schemas.contains("port") {
        return Err(ElabError::from(ElabErrorKind::UnknownAttrSchema("port".to_string()))
            .with_span(Some(to_miette_span(p.span))));
    }
    let env = ConstEnv::new();
    let mut data = HashMap::new();
    data.insert("name".to_string(), attr_string_value(p, "port", "name", &env)?);
    if let Some(k) = attrs.iter().find(|a| a.name == "kind") {
        data.insert("kind".to_string(), attr_string_value(k, "port", "kind", &env)?);
    }
    Ok(Some(Attribute { schema: "port".to_string(), data }))
}

fn attr_string_value(
    attr: &crate::ast::Attr,
    schema: &str,
    field: &str,
    env: &ConstEnv,
) -> Result<Value, ElabError> {
    let expr = attr.value.as_ref().ok_or_else(|| {
        ElabError::from(ElabErrorKind::AttrSchemaField {
            schema: schema.to_string(),
            field: field.to_string(),
            reason: "attribute has no value".to_string(),
        })
        .with_span(Some(to_miette_span(attr.span)))
    })?;
    let value = env.eval(expr).map_err(|e| {
        ElabError::from(ElabErrorKind::ConstEval { context: format!("attribute `{schema}.{field}`"), source: e })
            .with_span(Some(to_miette_span(attr.span)))
    })?;
    match &value {
        Value::Str(_) => Ok(value),
        other => Err(ElabError::from(ElabErrorKind::AttrSchemaField {
            schema: schema.to_string(),
            field: field.to_string(),
            reason: format!("expected a String, got {other:?}"),
        })
        .with_span(Some(to_miette_span(attr.span)))),
    }
}

/// §3.5 (LRMW2-13, T17): a declared `genvar` name referenced in an
/// ordinary (non-generate) parameter default is a use outside a valid
/// generate context — a genvar has no runtime storage or value outside a
/// loop-generate construct's own static unrolling. Checked over every
/// `ModuleItem::Param`'s default expressions and `from`/`exclude` range
/// bounds; generate constructs (`for`/`if`/`case` generate, where a
/// genvar's use *is* valid) are not module-level params and so are never
/// visited here — this walk only ever looks at param declarations, which
/// can't nest a generate construct inside them.
fn check_genvar_used_outside_generate(
    items: &[ModuleItem],
    genvars: &[String],
) -> Result<(), ElabError> {
    if genvars.is_empty() {
        return Ok(());
    }
    for item in items {
        let ModuleItem::Param(p) = item else { continue };
        for pa in &p.names {
            expr_check_no_genvar_ref(&pa.value, genvars)?;
        }
        for r in &p.ranges {
            if let Some(lo) = &r.lo {
                expr_check_no_genvar_ref(lo, genvars)?;
            }
            if let Some(hi) = &r.hi {
                expr_check_no_genvar_ref(hi, genvars)?;
            }
        }
    }
    Ok(())
}

/// Recursively checks `expr` for a reference to any name in `genvars`,
/// failing loud citing §3.5 on the first one found.
fn expr_check_no_genvar_ref(expr: &Expr, genvars: &[String]) -> Result<(), ElabError> {
    match expr {
        Expr::Ident { name, .. } => {
            if genvars.iter().any(|g| g == name) {
                // Deliberately unspanned (unlike most `ElabErrorKind::Other`
                // sites in this file): `resolve_elab_error` wraps a spanned
                // error behind an opaque `dyn Diagnostic` that
                // `downcast_ref::<ElabError>()` can't see through (the same
                // `UnknownAttrSchema` limitation T15's tests hit) — this
                // error stays plainly downcastable so a test can assert
                // its typed kind directly.
                return Err(ElabError::from(ElabErrorKind::Other(format!(
                    "genvar `{name}` used outside a valid generate context — a genvar has \
                     no value outside a loop-generate construct's own static unrolling \
                     (LRM §3.5)"
                ))));
            }
            Ok(())
        }
        // A `Hier` path's segments are literal strings, never sub-`Expr`s
        // (unlike `Index`'s subscript), so there is nothing to recurse
        // into that could reference a genvar.
        Expr::Number { .. } | Expr::Str { .. } | Expr::PortAccess { .. } | Expr::Hier { .. } => Ok(()),
        Expr::Unary { operand, .. } => expr_check_no_genvar_ref(operand, genvars),
        Expr::Binary { lhs, rhs, .. } => {
            expr_check_no_genvar_ref(lhs, genvars)?;
            expr_check_no_genvar_ref(rhs, genvars)
        }
        Expr::Ternary { cond, then_branch, else_branch, .. } => {
            expr_check_no_genvar_ref(cond, genvars)?;
            expr_check_no_genvar_ref(then_branch, genvars)?;
            expr_check_no_genvar_ref(else_branch, genvars)
        }
        Expr::Concat { parts, .. } | Expr::Replication { parts, .. } => {
            if let Expr::Replication { count, .. } = expr {
                expr_check_no_genvar_ref(count, genvars)?;
            }
            for p in parts {
                expr_check_no_genvar_ref(p, genvars)?;
            }
            Ok(())
        }
        Expr::Index { base, index, .. } => {
            expr_check_no_genvar_ref(base, genvars)?;
            expr_check_no_genvar_ref(index, genvars)
        }
        Expr::PartSelect { base, msb, lsb, .. } => {
            expr_check_no_genvar_ref(base, genvars)?;
            expr_check_no_genvar_ref(msb, genvars)?;
            expr_check_no_genvar_ref(lsb, genvars)
        }
        Expr::Call { args, .. } | Expr::SystemFunctionCall { args, .. } => {
            for a in args {
                expr_check_no_genvar_ref(a, genvars)?;
            }
            Ok(())
        }
        Expr::AccessFunction { pos, neg, .. } => {
            expr_check_no_genvar_ref(pos, genvars)?;
            if let Some(neg) = neg {
                expr_check_no_genvar_ref(neg, genvars)?;
            }
            Ok(())
        }
        Expr::AssignmentPattern { replicate, parts, .. } => {
            if let Some(r) = replicate {
                expr_check_no_genvar_ref(r, genvars)?;
            }
            for p in parts {
                expr_check_no_genvar_ref(p, genvars)?;
            }
            Ok(())
        }
    }
}

pub(super) fn declarator_to_wire(
    d: &Declarator,
    discipline: &str,
    span: crate::PreprocessedSpan,
    doc: Option<String>,
    env: &ConstEnv,
    attributes: Vec<Attribute>,
    nodeset_expr: Option<&Expr>,
) -> Result<Wire, ElabError> {
    // §3.6.3.2 (LRMW2-10, T14): the nodeset value is an elaboration
    // constant — const-evaluated here, in module scope, the same way
    // `declarator_net_type`'s bus width is, so a value that depends on a
    // module parameter still resolves correctly per monomorphized variant.
    let nodeset = nodeset_expr
        .map(|e| {
            env.eval_f64(e).map_err(|err| {
                ElabError::from(ElabErrorKind::ConstEval {
                    context: format!("net `{}` nodeset value", d.name),
                    source: err,
                })
            })
        })
        .transpose()?;
    Ok(Wire {
        span: Some(to_miette_span(span)),
        attributes,
        doc,
        name: d.name.clone(),
        ty: declarator_net_type(d, discipline, env)?,
        nodeset,
    })
}

/// Net-declaration attribute instances — the `rfport` schema (SP-01):
/// `(* rfport(num = 1, z0 = 50) *) electrical p1;` marks `p1` an S-
/// parameter port. `rfport` is the marker (no value of its own), `num`/
/// `z0` its sibling fields, const-evaluated here. Any other schema name
/// fails loud — nets are not a free attribute site.
fn lower_net_attrs(
    attrs: &[crate::ast::Attr],
    known_schemas: &HashSet<String>,
) -> Result<Vec<Attribute>, ElabError> {
    let Some(marker) = attrs.iter().find(|a| a.name == "rfport") else {
        // A net decl with attributes but no rfport marker: any name is
        // loud, same as the port path.
        for a in attrs {
            if !known_schemas.contains(&a.name) {
                return Err(ElabError::from(ElabErrorKind::UnknownAttrSchema(a.name.clone()))
                    .with_span(Some(to_miette_span(a.span))));
            }
        }
        return Ok(Vec::new());
    };
    if !known_schemas.contains("rfport") {
        return Err(ElabError::from(ElabErrorKind::UnknownAttrSchema("rfport".to_string()))
            .with_span(Some(to_miette_span(marker.span))));
    }
    let env = ConstEnv::new();
    let mut data = HashMap::new();
    let mut num = None;
    let mut z0 = None;
    for a in attrs {
        match a.name.as_str() {
            "rfport" => continue,
            "num" | "z0" => {
                let value = env.eval(a.value.as_ref().ok_or_else(|| {
                    ElabError::from(ElabErrorKind::AttrSchemaField {
                        schema: "rfport".into(),
                        field: a.name.clone(),
                        reason: "attribute has no value".to_string(),
                    })
                    .with_span(Some(to_miette_span(a.span)))
                })?).map_err(|e| {
                    ElabError::from(ElabErrorKind::ConstEval { context: format!("rfport field `{}`", a.name), source: e })
                })?;
                if a.name == "num" {
                    num = Some(value);
                } else {
                    z0 = Some(value);
                }
            }
            other => {
                return Err(ElabError::from(ElabErrorKind::UnknownAttrSchema(other.to_string()))
                    .with_span(Some(to_miette_span(a.span))));
            }
        }
    }
    let num = num.ok_or_else(|| {
        ElabError::from(ElabErrorKind::AttrSchemaField {
            schema: "rfport".into(),
            field: "num".into(),
            reason: "missing — every rfport carries its 1-based port number".to_string(),
        })
    })?;
    let z0 = z0.ok_or_else(|| {
        ElabError::from(ElabErrorKind::AttrSchemaField {
            schema: "rfport".into(),
            field: "z0".into(),
            reason: "missing — every rfport carries its reference impedance".to_string(),
        })
    })?;
    data.insert("num".to_string(), num);
    data.insert("z0".to_string(), z0);
    Ok(vec![Attribute { schema: "rfport".to_string(), data }])
}

/// A `[msb:lsb]` range widens a scalar discipline into `NetType::Array`
/// (LRM §3.7/3.6.3 `range`) — width is `|msb - lsb| + 1`.
/// The width×depth cap on a single vector/memory declaration (T5, design
/// risk row) — a loud, named error is strictly better than an OOM from an
/// unbounded element count (a memory scalarizes to one node per element,
/// MD-52).
const MAX_MEMORY_ELEMENTS: u64 = 65_536;

/// Const-evaluates one `[msb:lsb]` range's width, citing §3.6.3 on a
/// non-constant bound — shared by a declarator's packed `range` and its
/// unpacked `depth_range` (T5), so the two dimensions of a memory report
/// identically-shaped errors.
fn range_width(r: &Range, name: &str, env: &ConstEnv, which: &str) -> Result<u64, ElabError> {
    let msb = env.eval_int(&r.msb).map_err(|e| {
        ElabError::from(ElabErrorKind::ConstEval {
            context: format!("net `{name}` {which} msb (§3.6.3: range bounds must be elaboration constants)"),
            source: e,
        })
    })?;
    let lsb = env.eval_int(&r.lsb).map_err(|e| {
        ElabError::from(ElabErrorKind::ConstEval {
            context: format!("net `{name}` {which} lsb (§3.6.3: range bounds must be elaboration constants)"),
            source: e,
        })
    })?;
    Ok((msb - lsb).unsigned_abs() + 1)
}

fn declarator_net_type(d: &Declarator, discipline: &str, env: &ConstEnv) -> Result<NetType, ElabError> {
    // §3.7 (T52, LRMW2-45): `wire`/`tri`/`reg`/`wreal` are 1364 net-type
    // keywords, not discipline names — before this fix, `wreal in;`
    // silently became `NetType::Discipline("wreal")`, which then failed
    // later in `ResolveDisciplines` with a misleading "unknown discipline
    // `wreal`" error instead of behaving as the LRM's own real-valued net.
    let base = match crate::pom::DigitalNetKind::from_keyword(discipline) {
        Some(kind) => NetType::Digital1364(kind),
        None => NetType::Discipline(discipline.to_string()),
    };
    let packed = match &d.range {
        None => base,
        Some(r) => NetType::Array(Box::new(base), range_width(r, &d.name, env, "range")?),
    };
    // A memory's unpacked depth (T5, Annex A.2.2.1: `reg [3:0] mem
    // [0:7];`) wraps the packed element in one more `NetType::Array`
    // level — `NetType::width()`'s existing product rule already
    // multiplies through both, so a "memory with only a depth" (`reg mem
    // [0:7];`, `packed` still the bare 1-bit `base`) needs no special case.
    let ty = match &d.depth_range {
        None => packed,
        Some(dr) => NetType::Array(Box::new(packed), range_width(dr, &d.name, env, "depth range")?),
    };
    let total = ty.width();
    if total > MAX_MEMORY_ELEMENTS {
        return Err(ElabError::from(ElabErrorKind::Other(format!(
            "net `{}`: declared width x depth is {total} elements, over the {MAX_MEMORY_ELEMENTS}-element cap on a vector/memory declaration",
            d.name
        ))));
    }
    Ok(ty)
}

/// `integer`/`real`/`time`/`realtime` (LRM Annex A.2.1.3 `variable_type`)
/// map onto the same POM `ValueType`s `param_value_type` already uses for
/// the identical keyword set on a `parameter` declaration — `time`/
/// `realtime` have no dedicated POM value type, so both fold into `Real`
/// (seconds), matching `param_value_type`'s existing precedent.
fn var_kind_value_type(kind: VarKind) -> ValueType {
    match kind {
        VarKind::Integer => ValueType::Integer,
        VarKind::Real | VarKind::Time | VarKind::Realtime => ValueType::Real,
    }
}

/// A module-level `var` (GAPS §I.15) — persistent digital register state
/// consumed directly by `piperine-codegen`'s digital kernel
/// (`resolve/pom/body.rs` reads `Module::vars()` to populate
/// `DigitalBody::regs`/`reg_inits`). An optional `= constant_expression`
/// initializer (Annex A.2.2.1's `variable_type`/`real_type` form) is
/// const-evaluated into `init` — evaluated against the same `ConstEnv`
/// parameter defaults use, so it may reference earlier-declared params.
///
/// A `[msb:lsb]` range widens the scalar type into `ValueType::Array`,
/// same width computation as `declarator_net_type` uses for nets/ports.

/// Lower one authored constraint statement onto the POM (recursive at
/// scope groups), appending to `out` in authored order. Duplicate labels
/// fail loud E4008 across kinds and across scope nesting.
fn lower_constraint_stmt(
    stmt: &crate::ast::ConstraintStmt,
    module: &str,
    out: &mut Vec<crate::pom::PomConstraintStmt>,
    seen: &mut Vec<(String, Option<miette::SourceSpan>)>,
) -> Result<(), ElabError> {
    use crate::ast::ConstraintStmt as Cs;
    use crate::pom::PomConstraintStmt as Ps;
    let lowered: Ps = match stmt {
        Cs::Require(req) => Ps::Require(crate::pom::PomRequire {
            span: Some(to_miette_span(req.span)),
            name: req.name.clone(),
            pred: req.pred.clone(),
            tol: req.tol.clone(),
        }),
        Cs::Var { span, name, expr, .. } => Ps::Var {
            span: Some(to_miette_span(*span)),
            name: name.clone(),
            expr: expr.clone(),
        },
        Cs::Target { span, expr, cmp, level, tol, .. } => Ps::Target {
            span: Some(to_miette_span(*span)),
            expr: expr.clone(),
            cmp: *cmp,
            level: level.clone(),
            tol: tol.clone(),
        },
        Cs::Cover { span, name, expr, bins, .. } => Ps::Cover {
            span: Some(to_miette_span(*span)),
            name: name.clone(),
            expr: expr.clone(),
            bins: bins.clone(),
        },
        Cs::CoverCross { span, name, points, .. } => Ps::CoverCross {
            span: Some(to_miette_span(*span)),
            name: name.clone(),
            points: points.clone(),
        },
        Cs::Scope { span, analysis, window, stmts, .. } => {
            // A scope group carries no label of its own; its contained
            // statements run the same duplicate check against every
            // statement seen so far, inside or outside a scope.
            let mut inner = Vec::new();
            for s in stmts {
                lower_constraint_stmt(s, module, &mut inner, seen)?;
            }
            Ps::Scope {
                span: Some(to_miette_span(*span)),
                analysis: match analysis {
                    crate::ast::ScopeAnalysis::Dc => "dc",
                    crate::ast::ScopeAnalysis::Tran => "tran",
                    crate::ast::ScopeAnalysis::Ac => "ac",
                }
                .to_string(),
                after: window.as_ref().map(|w| w.after.clone()),
                dur: window.as_ref().and_then(|w| w.dur.clone()),
                stmts: inner,
            }
        }
    };
    if let Some(name) = lowered.label() {
        if let Some((_, first)) = seen.iter().find(|(n, _)| n == name) {
            return Err(ElabError::from(ElabErrorKind::ConstraintLabelDuplicate {
                name: name.to_string(),
                module: module.to_string(),
                first: *first,
                second: lowered.label_span(),
            }));
        }
    }
    // Register this statement's labels (scope groups register their
    // contained labels too, so later duplicates see into the group).
    lowered.for_each_label(&mut |s| {
        if let Some(name) = s.label() {
            seen.push((name.to_string(), s.label_span()));
        }
    });
    out.push(lowered);
    Ok(())
}

fn var_declarator_to_var(
    d: &Declarator,
    ty: ValueType,
    span: crate::PreprocessedSpan,
    doc: Option<String>,
    env: &ConstEnv,
    attributes: Vec<Attribute>,
) -> Result<Var, ElabError> {
    let ty = match &d.range {
        None => ty,
        Some(r) => {
            let msb = env.eval_int(&r.msb).map_err(|e| {
                ElabError::from(ElabErrorKind::ConstEval { context: format!("var `{}` range msb", d.name), source: e })
            })?;
            let lsb = env.eval_int(&r.lsb).map_err(|e| {
                ElabError::from(ElabErrorKind::ConstEval { context: format!("var `{}` range lsb", d.name), source: e })
            })?;
            let width = (msb - lsb).unsigned_abs() + 1;
            ValueType::Array(Box::new(ty), width)
        }
    };
    // Annex A.2.2.1: `variable_type ::= variable_identifier =
    // constant_expression` — the initializer is an elaboration constant
    // evaluated here, exactly like a parameter default. An initializer on
    // an array form (`st[3:0] = …`) is out of this lowering's scope —
    // loud, never a silently dropped init.
    let init = match (&d.init, &ty) {
        (None, _) => None,
        (Some(_), ValueType::Array(..)) => {
            return Err(ElabError::from(ElabErrorKind::Other(format!(
                "variable `{}`: array initializers (`'{{...}}`) are not supported — \
                 initialize the elements in a behavior block instead",
                d.name
            ))));
        }
        (Some(expr), _) => Some(env.eval(expr).map_err(|e| {
            ElabError::from(ElabErrorKind::ConstEval { context: format!("var `{}` initializer", d.name), source: e })
        })?),
    };
    Ok(Var { span: Some(to_miette_span(span)), attributes, doc, name: d.name.clone(), ty, init })
}

/// Lowers one `analog function`/`function` declaration (§4.7, Annex A.2.6)
/// into a POM [`crate::pom::Function`].
///
/// §4.7's own restrictions, checked here (fail loud, citing the clause):
/// "shall have at least one formal argument declared". Everything else
/// §4.7 restricts (no access functions, no contribution statements, no
/// named blocks, …) is a call-time/body-shape concern the codegen inliner
/// enforces when it actually walks the body — this pass only builds the
/// authored declaration.
fn lower_function_decl(f: &FunctionDecl) -> Result<crate::pom::Function, ElabError> {
    if f.ports.is_empty() {
        return Err(ElabError::from(ElabErrorKind::Other(format!(
            "analog function `{}`: at least one formal argument must be declared (LRM §4.7)",
            f.name
        ))));
    }

    // A `real`/`integer` redeclaration of a port name inside the body
    // (`analog_block_item_declaration`, §4.7.1's own Example 1: `input n1,
    // n2; real n1, n2;`) is where the formal's *value type* actually comes
    // from — the port list only carries direction. Absent a redeclaration,
    // `real` is the LRM default for an unspecified type.
    let mut decl_types: HashMap<&str, ValueType> = HashMap::new();
    for d in &f.decls {
        let (names, ty) = match d {
            BlockItemDecl::Integer { names, .. } => (names, ValueType::Integer),
            BlockItemDecl::Real { names, .. } => (names, ValueType::Real),
            BlockItemDecl::Time { names, .. } => (names, ValueType::Real),
            BlockItemDecl::Realtime { names, .. } => (names, ValueType::Real),
            BlockItemDecl::Reg { names, .. } => (names, ValueType::Real),
        };
        for n in names {
            decl_types.insert(n.as_str(), ty.clone());
        }
    }

    let mut params: Vec<(String, crate::pom::TypeRef)> = Vec::new();
    let mut directions: Vec<Direction> = Vec::new();
    for port in &f.ports {
        // A `PortItemDecl` can carry its own type inline
        // (`input real n1;`) — that takes priority over a separate
        // redeclaration when both are absent it falls back to `real`.
        let inline_ty = port.ty.map(|t| param_value_type(Some(t)));
        for name in &port.names {
            let ty = inline_ty
                .clone()
                .or_else(|| decl_types.get(name.as_str()).cloned())
                .unwrap_or(ValueType::Real);
            params.push((name.clone(), crate::pom::TypeRef::Value(ty)));
            directions.push(Direction::from(port.dir));
        }
    }

    let ret = crate::pom::TypeRef::Value(param_value_type(f.ret));
    let body = match f.body.as_ref() {
        Stmt::Block { stmts, .. } => stmts.clone(),
        other => vec![other.clone()],
    };

    // VAMS Annex A has no default-argument grammar for a function formal
    // (see the field's own doc comment) — every entry is `None`, one per
    // param.
    let defaults = vec![None; params.len()];

    Ok(crate::pom::Function {
        span: Some(to_miette_span(f.span)),
        name: f.name.clone(),
        params,
        directions,
        defaults,
        ret,
        body,
        is_generic: false,
    })
}

/// §6.5.7.1 (LRMW2-53, T48): the declared width of `net_ref` as seen from
/// `caller`'s own port/wire declarations — `None` when it cannot be
/// determined here (an implicit/undeclared interconnect signal, §6.5.7.2's
/// own explicit allowance for exactly this case). An indexed reference
/// (`bus[3]`) is always a single-bit slice, width 1, regardless of the
/// underlying bus's own width. `caller`'s `ground`-declared names and the
/// crate-global ground name list are treated as width 1 (electrical
/// ground is always scalar in every fixture this task's scope covers).
pub(super) fn net_ref_width(caller: &Module, net_ref: &crate::pom::NetRef) -> Option<u64> {
    if net_ref.index().is_some() {
        return Some(1);
    }
    let name = net_ref.net();
    if caller.is_ground(name) {
        return Some(1);
    }
    caller
        .ports()
        .iter()
        .find(|p| p.name() == name)
        .map(|p| p.net_type().width())
        .or_else(|| caller.wires().iter().find(|w| w.name() == name).map(|w| w.net_type().width()))
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

/// Evaluates one `name = expr [ranges]` parameter declarator against
/// `env`, validates its declared `from`/`exclude` ranges, and `.define()`s
/// it into `env` before returning the value — the single place a param
/// becomes visible to elaboration-constant evaluation, shared by
/// `elab_module`'s own `Param` arm and [`module_param_env`] so a module's
/// own params are defined identically everywhere they're needed (no
/// second copy of this eval+range-check+define sequence, MD-13 rule 2/3).
fn eval_and_define_param(
    name: &str,
    expr: &Expr,
    ranges: &[crate::ast::ParamRange],
    env: &mut ConstEnv,
) -> Result<Value, ElabError> {
    let value = eval_param_default(expr, env)?;
    check_param_ranges(name, &value, ranges, env)?;
    env.define(name.to_string(), value.clone());
    Ok(value)
}

/// Evaluates one `ast::ParamAssignment` — scalar (`pa.dims` empty, defers
/// to [`eval_and_define_param`] unchanged) or array (§3.4.4 one dimension /
/// §3.4.8 multidimensional, LRMC-C1-06's FUNCTIONALITY half): each
/// `[msb:lsb]` in `pa.dims` becomes one level of `ValueType::Array`
/// (outermost dimension first, mirroring [`var_declarator_to_var`]'s width
/// computation), and `pa.value` — required to be an assignment pattern
/// `'{...}` at array shape — evaluates through [`ConstEnv::eval_array`]
/// against those widths. `from`/`exclude` ranges are a scalar-only
/// construct (§3.4.2); an array declarator combining the two fails loud
/// rather than silently ignoring the ranges.
fn eval_and_define_param_assignment(
    pa: &crate::ast::ParamAssignment,
    base_ty: &ValueType,
    ranges: &[crate::ast::ParamRange],
    env: &mut ConstEnv,
) -> Result<(ValueType, Value), ElabError> {
    if pa.dims.is_empty() {
        let value = eval_and_define_param(&pa.name, &pa.value, ranges, env)?;
        return Ok((base_ty.clone(), value));
    }
    if !ranges.is_empty() {
        return Err(ElabError::from(ElabErrorKind::Other(format!(
            "parameter `{}`: `from`/`exclude` constraints are not supported on an array parameter (LRM §3.4.2 applies to scalar defaults only)",
            pa.name
        ))));
    }
    let widths = pa
        .dims
        .iter()
        .map(|r| {
            let msb = env.eval_int(&r.msb).map_err(|e| {
                ElabError::from(ElabErrorKind::ConstEval {
                    context: format!("parameter `{}` array dimension msb", pa.name),
                    source: e,
                })
            })?;
            let lsb = env.eval_int(&r.lsb).map_err(|e| {
                ElabError::from(ElabErrorKind::ConstEval {
                    context: format!("parameter `{}` array dimension lsb", pa.name),
                    source: e,
                })
            })?;
            Ok((msb - lsb).unsigned_abs() + 1)
        })
        .collect::<Result<Vec<u64>, ElabError>>()?;
    let value = env.eval_array(&pa.value, &widths).map_err(|e| {
        ElabError::from(ElabErrorKind::ConstEval {
            context: format!("parameter `{}` array initializer", pa.name),
            source: e,
        })
    })?;
    let ty = widths.iter().rev().fold(base_ty.clone(), |acc, &w| ValueType::Array(Box::new(acc), w));
    env.define(pa.name.clone(), value.clone());
    Ok((ty, value))
}

/// Builds a `ConstEnv` populated with every `parameter`/`localparam` a
/// module declares, in source order (the same sequential
/// `eval_and_define_param` order `elab_module`'s own item loop uses so a
/// later param may reference an earlier one). This is the base
/// environment a generate construct's body (T25, LRM ch.6) must evaluate
/// against: a `for`/`if`/`case` generate body is still "this same module"
/// for const-eval purposes and must see its own module's params exactly
/// like an ordinary instantiation does (`elab/lower/generate.rs`'s
/// `UnrollGenerate` pass is the sole caller — it previously built its own
/// permanently-empty `ConstEnv`, the same gap `eval_and_define_param`'s
/// doc comment on the module-body side fixes).
pub(super) fn module_param_env(decl: &ModuleDecl) -> Result<ConstEnv, ElabError> {
    let mut env = ConstEnv::new();
    for item in &decl.items {
        if let ModuleItem::Param(p) = item {
            let base_ty = param_value_type(p.ty);
            for pa in &p.names {
                eval_and_define_param_assignment(pa, &base_ty, &p.ranges, &mut env)?;
            }
        }
    }
    Ok(env)
}

/// A port-connection value resolves to a `NetRef` — a bare identifier, or
/// `name[index]` for a bus tap (`index` resolves against `env`, so a
/// generate-loop's genvar is visible, T25).
fn expr_to_net_ref(expr: &Expr, env: &ConstEnv) -> Result<NetRef, ElabError> {
    match expr {
        Expr::Ident { name, .. } => Ok(NetRef::simple(name.clone())),
        Expr::Index { base, index, .. } => {
            let Expr::Ident { name, .. } = base.as_ref() else {
                return Err(ElabError::from(ElabErrorKind::Other(
                    "port connection: only `name[index]` indexing is supported".to_string(),
                )));
            };
            let idx = env.eval_nat(index).map_err(|e| {
                ElabError::from(ElabErrorKind::ConstEval {
                    context: format!(
                        "port connection index `{name}[...]` (§5.5.2: the index must be a constant expression, though it may include genvar variables)"
                    ),
                    source: e,
                })
            })?;
            Ok(NetRef::indexed(name.clone(), idx))
        }
        _ => Err(ElabError::from(ElabErrorKind::Other(
            "port connection must be a net name or `name[index]`".to_string(),
        ))),
    }
}

/// T6 (LRMW3-01, spec Edge Cases): a `NetRef { index: Some(i), .. }`
/// produced by [`expr_to_net_ref`] (a port connection's `name[index]`
/// form) is checked here, once per module, against the referenced net's
/// *declared* width — `i >= width` or `width == 1` (indexing a scalar)
/// each fail loud naming the net, the index, and the declared bounds,
/// rather than surfacing later as a generic "unresolved net" once
/// flattening/circuit-build looks for a node that was never registered.
/// A referenced name this module doesn't itself declare (e.g. a
/// hierarchical or not-yet-elaborated name) is skipped — out of this
/// check's scope, not a false positive.
fn validate_indexed_net_refs(module: &Module) -> Result<(), ElabError> {
    // `Some(width)` only for a net actually declared `NetType::Array` —
    // a width-1 array (e.g. `tap[0:segs-2]` monomorphized with `segs ==
    // 2`) is still indexable at `[0]`; only a *bare* scalar
    // (`Discipline`/`Digital1364`, no `Array` wrapper at all, `width()
    // == 1` incidentally) has no range to index into. `None` covers a
    // name this module doesn't declare a wire/port for (e.g. a
    // hierarchical name) — out of this check's scope, not a false
    // positive.
    let array_width_of = |name: &str| -> Option<u64> {
        let ty = module
            .wires()
            .iter()
            .find(|w| w.name() == name)
            .map(|w| w.net_type())
            .or_else(|| module.ports().iter().find(|p| p.name() == name).map(|p| p.net_type()))?;
        match ty {
            NetType::Array(_, n) => Some(*n),
            NetType::Discipline(_) | NetType::Digital1364(_) => None,
        }
    };
    let is_declared_scalar = |name: &str| -> bool {
        let ty = module
            .wires()
            .iter()
            .find(|w| w.name() == name)
            .map(|w| w.net_type())
            .or_else(|| module.ports().iter().find(|p| p.name() == name).map(|p| p.net_type()));
        matches!(ty, Some(NetType::Discipline(_) | NetType::Digital1364(_)))
    };
    for inst in module.instances() {
        for port_ref in inst.ports() {
            let Some(idx) = port_ref.index() else { continue };
            if is_declared_scalar(port_ref.net()) {
                return Err(ElabError::from(ElabErrorKind::Other(format!(
                    "instance `{}`: `{port_ref}` indexes scalar net `{}`, which was declared with no range",
                    inst.name(),
                    port_ref.net()
                ))));
            }
            let Some(width) = array_width_of(port_ref.net()) else { continue };
            if idx >= width {
                return Err(ElabError::from(ElabErrorKind::Other(format!(
                    "instance `{}`: `{port_ref}` is out of range — net `{}` is declared `[0:{}]` (0..{})",
                    inst.name(),
                    port_ref.net(),
                    width - 1,
                    width - 1
                ))));
            }
        }
    }
    Ok(())
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
/// `From<crate::ParseError>`'s conversion (`pom/error.rs`).
fn to_miette_span(span: crate::PreprocessedSpan) -> miette::SourceSpan {
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

/// Walks a statement tree looking for an access-function call used as a
/// contribution target (`V(p,n) <+ expr;`, §5.4), resolving each to a
/// branch (a name lookup for `V(br)`, or find-or-create for the
/// two-terminal/ground-referenced implicit form, §5.4.1) and recording the
/// contribution's kind — the T18 `record_contribution` conflict check
/// (§5.4.2's `BranchKindConflict`) fires here, for every discipline, not
/// only electrical.
///
/// `V`/`I`/`potential`/`flow` (parsed as `Expr::AccessFunction` since T16)
/// are always potential/flow respectively, regardless of which discipline
/// is in scope — the LRM's own always-legal generic/example names (§4.4).
/// A custom discipline's own `access` name (§3.6.1.2) has no dedicated AST
/// shape (the grammar has no symbol table, T16) — it parses as a plain
/// `Expr::Call`, promoted here to the same branch resolution via
/// `access_kinds` (LRMC-C1-09, [`build_access_kind_map`]).
/// §5.6.5's switch-branch exception, applied *before*
/// [`Branch::record_contribution`]'s ordinary §5.4.2 conflict check runs:
/// a contribution reached from inside a conditional statement
/// (`conditional`) whose kind differs from the branch's already-recorded
/// source kind promotes the branch to [`BranchKind::Switch`] instead of
/// leaving it to fail the conflict check. A no-op for a probe (nothing
/// recorded yet — no conflict possible), a same-kind contribution
/// (ordinary multiple `V()`/`I()` writes), or an unconditional context
/// (§5.4.2 stays a hard error there, unchanged).
fn promote_to_switch_if_conditional(branch: &mut Branch, incoming: BranchKind, conditional: bool) {
    if conditional && branch.kind().is_source() && branch.kind() != incoming && branch.kind() != BranchKind::Switch {
        branch.mark_switch();
    }
}

pub(super) fn resolve_contribution_branches(
    stmt: &Stmt,
    module: &str,
    branches: &mut Vec<Branch>,
    access_kinds: &HashMap<String, BranchKind>,
    conditional: bool,
) -> Result<(), ElabError> {
    match stmt {
        // A contribution guarded by `@(initial_step)` is a UIC seed, not
        // a live potential/flow source — the branch it names keeps the
        // kind its unconditional contributions give it, and the seed
        // never stamps (the flattener routes it to
        // `FlatAnalog::initial_conditions`). §5.4.2's potential+flow
        // conflict therefore does not see it — the SPICE `.ic` idiom
        // (`@(initial_step) V(p,n) <+ ic;` beside `I(p,n) <+ c*ddt(…)`)
        // must elaborate. (`initial_step` parses as a level event's
        // identifier — LRM §5.6's own spelling.)
        Stmt::EventControl { events, body, .. }
            if events.iter().any(|e| {
                matches!(e, crate::ast::EventExpr::Level(crate::ast::Expr::Ident { name, .. }) if name == "initial_step")
            }) && matches!(body.as_ref(), Stmt::Contribution { .. }) =>
        {
            return Ok(());
        }
        Stmt::Contribution { target, span, .. } => {
            match target {
                Expr::AccessFunction { function, pos, neg, .. } => {
                    let kind = match function.as_str() {
                        "V" | "potential" => BranchKind::Potential,
                        "I" | "flow" => BranchKind::Flow,
                        other => match access_kinds.get(other) {
                            Some(kind) => *kind,
                            None => return Ok(()),
                        },
                    };
                    let branch = resolve_or_create_branch(pos, neg.as_deref(), branches)?;
                    promote_to_switch_if_conditional(branch, kind, conditional);
                    branch.record_contribution(kind, Some(to_miette_span(*span)), module)?;
                }
                Expr::Call { name, args, .. } => {
                    let Some(kind) = access_kinds.get(name.as_str()) else { return Ok(()) };
                    let (pos, neg) = match args.as_slice() {
                        [pos] => (pos, None),
                        [pos, neg] => (pos, Some(neg)),
                        _ => {
                            return Err(ElabError::from(ElabErrorKind::Other(format!(
                                "access function `{name}` (§4.4) takes a branch identifier or one/two net expressions, got {} argument(s)",
                                args.len()
                            ))));
                        }
                    };
                    let branch = resolve_or_create_branch(pos, neg, branches)?;
                    promote_to_switch_if_conditional(branch, *kind, conditional);
                    branch.record_contribution(*kind, Some(to_miette_span(*span)), module)?;
                }
                _ => {}
            }
            Ok(())
        }
        // §5.6.7 (LRMW2-41, T46): `lhs : probe == rhs;` drives `lhs` the
        // same way a direct `lhs <+ …;` would — branch-kind bookkeeping
        // (and the §5.6.5 switch-exception check) applies identically.
        // §5.6.7.2's stricter rule ("once indirectly assigned, a branch
        // cannot ALSO be driven by `<+`") is a separate, not-yet-enforced
        // axis from ordinary potential/flow kind-conflict — a disclosed
        // scope cut (T46), not silently wrong: mixing the two forms on
        // the same branch degrades to whatever the ordinary kind-conflict
        // check does today rather than its own dedicated diagnostic.
        Stmt::IndirectContribution { lhs, span, .. } => {
            if let Expr::AccessFunction { function, pos, neg, .. } = lhs {
                let kind = match function.as_str() {
                    "V" | "potential" => BranchKind::Potential,
                    "I" | "flow" => BranchKind::Flow,
                    other => match access_kinds.get(other) {
                        Some(kind) => *kind,
                        None => return Ok(()),
                    },
                };
                let branch = resolve_or_create_branch(pos, neg.as_deref(), branches)?;
                promote_to_switch_if_conditional(branch, kind, conditional);
                branch.record_contribution(kind, Some(to_miette_span(*span)), module)?;
            }
            Ok(())
        }
        Stmt::Block { stmts, .. } => stmts
            .iter()
            .try_for_each(|s| resolve_contribution_branches(s, module, branches, access_kinds, conditional)),
        // §5.6.5: "contribution statements are allowed within conditional
        // statements" — the switch-branch mechanism itself. `then`/`else`
        // both run with `conditional = true` so a kind conflict between
        // them (`V(out) <+ 0;` vs. `I(out) <+ 0;`, the LRM's own relay
        // example) promotes the branch to `BranchKind::Switch` instead of
        // failing `§5.4.2`'s ordinary conflict check.
        Stmt::If { then_branch, else_branch, .. } => {
            resolve_contribution_branches(then_branch, module, branches, access_kinds, true)?;
            if let Some(e) = else_branch {
                resolve_contribution_branches(e, module, branches, access_kinds, true)?;
            }
            Ok(())
        }
        Stmt::Case { arms, .. } => arms
            .iter()
            .try_for_each(|arm| resolve_contribution_branches(&arm.body, module, branches, access_kinds, true)),
        Stmt::For { body, .. } | Stmt::While { body, .. } | Stmt::Repeat { body, .. } | Stmt::Forever { body, .. } => {
            resolve_contribution_branches(body, module, branches, access_kinds, conditional)
        }
        // §5.6.5's own restriction: "Usage of contribution statements
        // inside event control statements is disallowed" for the switch
        // mechanism specifically — a kind conflict first arising inside an
        // `@(...)` body (even one lexically nested inside an `if`) does
        // not get the switch exception; it still fails §5.4.2's ordinary
        // conflict check.
        Stmt::EventControl { body, .. } | Stmt::DelayControl { body, .. } => {
            resolve_contribution_branches(body, module, branches, access_kinds, false)
        }
        Stmt::Null { .. }
        | Stmt::Assign { .. }
        | Stmt::NonBlockingAssign { .. }
        | Stmt::Disable { .. }
        | Stmt::TaskCall { .. }
        | Stmt::EventTrigger { .. }
        // §8.5.3.2 (T53): none of these four target an access-function
        // call (`ProceduralAssign`/`Force` are only legal on a `reg`/net
        // *name*, never a branch — §9.3 of IEEE 1364-2005 has no
        // `V(...)`/`I(...)` lvalue form) — no implicit branch to resolve.
        | Stmt::ProceduralAssign { .. }
        | Stmt::ProceduralDeassign { .. }
        | Stmt::Force { .. }
        | Stmt::Release { .. } => Ok(()),
    }
}

/// LRMC-C5-04: walks a statement tree looking for `-> event_name;`
/// triggers, failing loud if `event_name` isn't in `declared` (§5.10.4:
/// "An event name shall be declared explicitly before it is used").
fn check_event_triggers(stmt: &Stmt, declared: &[String], module: &str) -> Result<(), ElabError> {
    match stmt {
        Stmt::EventTrigger { name, .. } => {
            if !declared.iter().any(|e| e == name) {
                return Err(ElabError::from(ElabErrorKind::Other(format!(
                    "module `{module}`: `-> {name};` triggers an undeclared event (§5.10.4) — \
                     add `event {name};`"
                ))));
            }
            Ok(())
        }
        Stmt::Block { stmts, .. } => {
            stmts.iter().try_for_each(|s| check_event_triggers(s, declared, module))
        }
        Stmt::If { then_branch, else_branch, .. } => {
            check_event_triggers(then_branch, declared, module)?;
            if let Some(e) = else_branch {
                check_event_triggers(e, declared, module)?;
            }
            Ok(())
        }
        Stmt::Case { arms, .. } => {
            arms.iter().try_for_each(|arm| check_event_triggers(&arm.body, declared, module))
        }
        Stmt::For { body, .. } | Stmt::While { body, .. } | Stmt::Repeat { body, .. } | Stmt::Forever { body, .. }
        | Stmt::EventControl { body, .. } | Stmt::DelayControl { body, .. } => {
            check_event_triggers(body, declared, module)
        }
        Stmt::Null { .. }
        | Stmt::Assign { .. }
        | Stmt::NonBlockingAssign { .. }
        | Stmt::Contribution { .. }
        | Stmt::IndirectContribution { .. }
        | Stmt::Disable { .. }
        | Stmt::TaskCall { .. }
        | Stmt::ProceduralAssign { .. }
        | Stmt::ProceduralDeassign { .. }
        | Stmt::Force { .. }
        | Stmt::Release { .. } => Ok(()),
    }
}

/// LRMC-C1-10: replaces every [`Expr::PortAccess`] reachable from `expr`
/// with the KCL-equivalent signed sum of the module's own branch flows
/// touching that port — §5.4.3: the current flowing into the module
/// through a port equals, by Kirchhoff's current law, the sum of the
/// currents flowing from that port into every module-internal branch
/// connected to it. For a branch whose `pos` terminal is the port, that's
/// `I(port, other)` directly (the branch is already oriented port→other);
/// for a branch whose `neg` terminal is the port, current flowing *out of*
/// the port through that branch is the negated orientation, which is the
/// same `I(port, other)` form (LRM's own `I(a,b) == -I(b,a)` convention) —
/// so both cases produce an identical term shape, only `other` differs.
/// Reuses the port access's own `function` name (not hardcoded to `I`) so
/// a non-electrical discipline's flow accessor round-trips correctly. A
/// port touched by no branch rewrites to a literal `0` — no current path
/// exists to sum.
fn rewrite_port_access_expr(expr: Expr, branches: &[Branch]) -> Expr {
    match expr {
        Expr::PortAccess { span, function, port } => {
            let terms: Vec<Expr> = branches
                .iter()
                .filter_map(|b| {
                    let other: Option<Option<String>> = if b.pos.net() == port {
                        Some(b.neg.as_ref().map(|n| n.net().to_string()))
                    } else if b.neg.as_ref().map(|n| n.net()) == Some(port.as_str()) {
                        Some(Some(b.pos.net().to_string()))
                    } else {
                        None
                    };
                    other.map(|other_net| {
                        let pos = Box::new(Expr::Ident { span, name: port.clone() });
                        let neg = other_net.map(|n| Box::new(Expr::Ident { span, name: n }));
                        Expr::AccessFunction { span, function: function.clone(), pos, neg }
                    })
                })
                .collect();
            terms
                .into_iter()
                .reduce(|acc, t| Expr::Binary { span, op: crate::ast::BinOp::Add, lhs: Box::new(acc), rhs: Box::new(t) })
                .unwrap_or(Expr::Number { span, value: crate::lexer::NumberLit::Integer(0) })
        }
        Expr::Unary { span, op, operand } => {
            Expr::Unary { span, op, operand: Box::new(rewrite_port_access_expr(*operand, branches)) }
        }
        Expr::Binary { span, op, lhs, rhs } => Expr::Binary {
            span,
            op,
            lhs: Box::new(rewrite_port_access_expr(*lhs, branches)),
            rhs: Box::new(rewrite_port_access_expr(*rhs, branches)),
        },
        Expr::Ternary { span, cond, then_branch, else_branch } => Expr::Ternary {
            span,
            cond: Box::new(rewrite_port_access_expr(*cond, branches)),
            then_branch: Box::new(rewrite_port_access_expr(*then_branch, branches)),
            else_branch: Box::new(rewrite_port_access_expr(*else_branch, branches)),
        },
        Expr::Concat { span, parts } => {
            Expr::Concat { span, parts: parts.into_iter().map(|p| rewrite_port_access_expr(p, branches)).collect() }
        }
        Expr::Replication { span, count, parts } => Expr::Replication {
            span,
            count: Box::new(rewrite_port_access_expr(*count, branches)),
            parts: parts.into_iter().map(|p| rewrite_port_access_expr(p, branches)).collect(),
        },
        Expr::Index { span, base, index } => Expr::Index {
            span,
            base: Box::new(rewrite_port_access_expr(*base, branches)),
            index: Box::new(rewrite_port_access_expr(*index, branches)),
        },
        Expr::PartSelect { span, base, msb, lsb } => Expr::PartSelect {
            span,
            base: Box::new(rewrite_port_access_expr(*base, branches)),
            msb: Box::new(rewrite_port_access_expr(*msb, branches)),
            lsb: Box::new(rewrite_port_access_expr(*lsb, branches)),
        },
        Expr::Call { span, name, args } => {
            Expr::Call { span, name, args: args.into_iter().map(|a| rewrite_port_access_expr(a, branches)).collect() }
        }
        Expr::AccessFunction { span, function, pos, neg } => Expr::AccessFunction {
            span,
            function,
            pos: Box::new(rewrite_port_access_expr(*pos, branches)),
            neg: neg.map(|n| Box::new(rewrite_port_access_expr(*n, branches))),
        },
        Expr::SystemFunctionCall { span, name, args } => Expr::SystemFunctionCall {
            span,
            name,
            args: args.into_iter().map(|a| rewrite_port_access_expr(a, branches)).collect(),
        },
        Expr::AssignmentPattern { span, replicate, parts } => Expr::AssignmentPattern {
            span,
            replicate: replicate.map(|c| Box::new(rewrite_port_access_expr(*c, branches))),
            parts: parts.into_iter().map(|p| rewrite_port_access_expr(p, branches)).collect(),
        },
        other @ (Expr::Number { .. } | Expr::Str { .. } | Expr::Ident { .. } | Expr::Hier { .. }) => other,
    }
}

/// Statement-tree counterpart of [`rewrite_port_access_expr`] — recurses
/// into every `Expr`-bearing field of every `Stmt` kind so a `PortAccess`
/// reachable from any statement position (an `if` condition, a case
/// selector, a task-call argument, ...) is rewritten too, not only
/// contribution targets/values.
fn rewrite_port_access_stmt(stmt: Stmt, branches: &[Branch]) -> Stmt {
    let e = |expr: Expr| rewrite_port_access_expr(expr, branches);
    let s = |stmt: Box<Stmt>| Box::new(rewrite_port_access_stmt(*stmt, branches));
    match stmt {
        Stmt::Null { .. } | Stmt::Disable { .. } => stmt,
        Stmt::Assign { span, target, value } => Stmt::Assign { span, target: e(target), value: e(value) },
        Stmt::NonBlockingAssign { span, target, value } => {
            Stmt::NonBlockingAssign { span, target: e(target), value: e(value) }
        }
        Stmt::Contribution { span, target, value } => Stmt::Contribution { span, target: e(target), value: e(value) },
        Stmt::IndirectContribution { span, lhs, probe, rhs } => {
            Stmt::IndirectContribution { span, lhs: e(lhs), probe: e(probe), rhs: e(rhs) }
        }
        Stmt::Block { span, name, decls, stmts } => Stmt::Block {
            span,
            name,
            decls,
            stmts: stmts.into_iter().map(|st| rewrite_port_access_stmt(st, branches)).collect(),
        },
        Stmt::If { span, cond, then_branch, else_branch } => Stmt::If {
            span,
            cond: e(cond),
            then_branch: s(then_branch),
            else_branch: else_branch.map(s),
        },
        Stmt::Case { span, kind, selector, arms, attrs } => Stmt::Case {
            span,
            kind,
            selector: e(selector),
            arms: arms
                .into_iter()
                .map(|arm| CaseArm {
                    span: arm.span,
                    labels: arm.labels.map(|ls| ls.into_iter().map(&e).collect()),
                    body: s(arm.body),
                })
                .collect(),
            attrs,
        },
        Stmt::For { span, init, cond, step, body } => {
            Stmt::For { span, init: s(init), cond: e(cond), step: s(step), body: s(body) }
        }
        Stmt::While { span, cond, body } => Stmt::While { span, cond: e(cond), body: s(body) },
        Stmt::Repeat { span, count, body } => Stmt::Repeat { span, count: e(count), body: s(body) },
        Stmt::Forever { span, body } => Stmt::Forever { span, body: s(body) },
        Stmt::EventControl { span, events, body } => Stmt::EventControl {
            span,
            events: events
                .into_iter()
                .map(|ev| match ev {
                    EventExpr::Level(x) => EventExpr::Level(e(x)),
                    EventExpr::Posedge(x) => EventExpr::Posedge(e(x)),
                    EventExpr::Negedge(x) => EventExpr::Negedge(e(x)),
                    EventExpr::ImplicitAll => EventExpr::ImplicitAll,
                })
                .collect(),
            body: s(body),
        },
        Stmt::DelayControl { span, delay, body } => Stmt::DelayControl { span, delay: e(delay), body: s(body) },
        Stmt::TaskCall { span, name, args } => {
            Stmt::TaskCall { span, name, args: args.into_iter().map(&e).collect() }
        }
        Stmt::ProceduralAssign { span, target, value } => {
            Stmt::ProceduralAssign { span, target: e(target), value: e(value) }
        }
        Stmt::ProceduralDeassign { span, target } => Stmt::ProceduralDeassign { span, target: e(target) },
        Stmt::Force { span, target, value } => Stmt::Force { span, target: e(target), value: e(value) },
        Stmt::Release { span, target } => Stmt::Release { span, target: e(target) },
        Stmt::EventTrigger { .. } => stmt,
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

    let env = ConstEnv::new();
    let pos_ref = expr_to_net_ref(pos, &env)?;
    let neg_ref = neg.map(|e| expr_to_net_ref(e, &env)).transpose()?;
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
/// The declared `from`/`exclude` ranges of a set of parameter
/// declarations, keyed by name (`p.ranges` applies to every name the
/// declaration lists). The lookup half of §3.4.2's second sentence: the
/// declared default is checked by [`eval_and_define_param`], and an
/// instantiation override naming a ranged parameter is checked against
/// these — one lookup shared by `lower_instance` and
/// `lower_paramset_instance`.
fn declared_param_ranges<'a>(
    decls: impl Iterator<Item = &'a crate::ast::ParamDecl>,
) -> HashMap<&'a str, &'a [crate::ast::ParamRange]> {
    let mut map = HashMap::new();
    for p in decls {
        if p.ranges.is_empty() {
            continue;
        }
        for pa in &p.names {
            map.insert(pa.name.as_str(), p.ranges.as_slice());
        }
    }
    map
}

fn check_param_ranges(
    name: &str,
    value: &Value,
    ranges: &[crate::ast::ParamRange],
    env: &ConstEnv,
) -> Result<(), ElabError> {
    if ranges.is_empty() {
        return Ok(());
    }
    // §3.4.6: `from '{ "NPN", "PNP" }`/`exclude '{...}` is an enumerated
    // valid-value list (any `Value` type, most commonly `Str`), not a
    // numeric `[lo:hi]` range — the bare-value parse shorthand
    // (`ParamRange.lo` with no `hi`) also captures an assignment-pattern
    // expression there, so branch on that shape before assuming numeric.
    if ranges.iter().any(|r| r.hi.is_none() && matches!(r.lo, Some(Expr::AssignmentPattern { .. }))) {
        return check_param_value_list(name, value, ranges, env);
    }
    let v = value.coerce_real().map_err(|_| {
        ElabError::from(ElabErrorKind::Other(format!(
            "parameter `{name}`: a `from`/`exclude` range requires a numeric default"
        )))
    })?;

    // `value_range_expression ::= constant_expression | '-inf' | 'inf'`
    // (§3.4.2 Annex A.2.5) — `inf`/`-inf` are grammar-level literals only
    // valid in a `from`/`exclude` bound, not general identifiers, so they
    // never reach the const-eval scope's identifier namespace.
    let eval_bound = |e: &Expr| -> Result<f64, ElabError> {
        match e {
            Expr::Ident { name, .. } if name == "inf" => return Ok(f64::INFINITY),
            Expr::Unary { op: UnOp::Neg, operand, .. } => {
                if let Expr::Ident { name, .. } = operand.as_ref() {
                    if name == "inf" {
                        return Ok(f64::NEG_INFINITY);
                    }
                }
            }
            _ => {}
        }
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

/// The enumerated-valid-value-list branch of [`check_param_ranges`]
/// (§3.4.6) — membership by [`Value`]'s own structural `PartialEq`, not
/// numeric range containment.
fn check_param_value_list(
    name: &str,
    value: &Value,
    ranges: &[crate::ast::ParamRange],
    env: &ConstEnv,
) -> Result<(), ElabError> {
    let eval_list = |e: &Expr| -> Result<Vec<Value>, ElabError> {
        env.eval_value_list(e).map_err(|e| {
            ElabError::from(ElabErrorKind::ConstEval {
                context: format!("parameter `{name}` valid-value list"),
                source: e,
            })
        })
    };

    let froms: Vec<_> = ranges.iter().filter(|r| !r.excluded).collect();
    if !froms.is_empty() {
        let mut in_any = false;
        for r in &froms {
            if let Some(e) = &r.lo {
                if eval_list(e)?.contains(value) {
                    in_any = true;
                    break;
                }
            }
        }
        if !in_any {
            return Err(ElabError::from(ElabErrorKind::ParamRangeViolation {
                param: name.to_string(),
                value: value.to_string(),
                reason: "not within any declared `from` valid-value list".to_string(),
                span: None,
            }));
        }
    }
    for r in ranges.iter().filter(|r| r.excluded) {
        if let Some(e) = &r.lo {
            if eval_list(e)?.contains(value) {
                return Err(ElabError::from(ElabErrorKind::ParamRangeViolation {
                    param: name.to_string(),
                    value: value.to_string(),
                    reason: "falls in a declared `exclude` valid-value list".to_string(),
                    span: None,
                }));
            }
        }
    }
    Ok(())
}

fn in_from_range(
    v: f64,
    r: &crate::ast::ParamRange,
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
